//! Symbolic topology and branch traversal for immutable route preparation.
use super::super::components::lifecycle_generations;
use super::*;
use crate::song::snapshot::{FrozenAudioRoute, FrozenEdit, FrozenPartNode, FrozenPattern};

struct PayloadOwner {
    part: usize,
    track: KwId,
    policy_root: Option<usize>,
    depth: u32,
}
/// First policy is the latest outer declaration. Full scope topology is retained.
type FxPolicy = (KwId, Vec<FrozenSound>, KwId);
impl RouteBuilder<'_> {
    fn charge(&mut self, cost: u32) -> Result<(), Failure> {
        let next = self.work.checked_add(cost).ok_or_else(capacity_overflow)?;
        if next > self.quota {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "song route work exhausted",
            ));
        }
        self.work = next;
        Ok(())
    }
    pub(super) fn walk(
        &mut self,
        index: usize,
        multiplicity: u64,
        path: &mut Vec<SongRoutePlacement>,
        policies: &[FxPolicy],
        excluded: &[(KwId, Option<Vec<FrozenSound>>)],
        depth: u32,
    ) -> Result<(), Failure> {
        if depth >= self.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "song route depth exhausted",
            ));
        }
        self.charge(1)?;
        let part = self
            .inventory
            .parts
            .get(index)
            .ok_or_else(|| route_failure("invalid Part edge"))?;
        match &part.node {
            FrozenPartNode::Capture(entries) => {
                if part.duration > Ratio64::ZERO {
                    self.min_duration = Some(
                        self.min_duration
                            .map_or(part.duration, |d| d.min(part.duration)),
                    );
                }
                for (track, payload) in entries {
                    self.payload(
                        PayloadOwner {
                            part: index,
                            track: *track,
                            policy_root: None,
                            depth,
                        },
                        payload,
                        multiplicity,
                        path,
                        policies,
                        excluded,
                    )?;
                }
            }
            FrozenPartNode::Sequence(children) => {
                for (ordinal, (offset, child)) in children.iter().enumerate() {
                    path.push(SongRoutePlacement::Sequence {
                        part: index,
                        child: u32::try_from(ordinal).map_err(|_| capacity_overflow())?,
                        offset: *offset,
                    });
                    self.walk(*child, multiplicity, path, policies, excluded, depth + 1)?;
                    path.pop();
                }
            }
            FrozenPartNode::Repeat { child, count, .. } => {
                path.push(SongRoutePlacement::Repeat {
                    part: index,
                    count: *count,
                });
                self.walk(
                    *child,
                    mul64(multiplicity, u64::from(*count))?,
                    path,
                    policies,
                    excluded,
                    depth + 1,
                )?;
                path.pop();
            }
            FrozenPartNode::Edit { source, edit } => {
                path.push(SongRoutePlacement::Edit { part: index });
                match edit {
                    FrozenEdit::InstrumentFx {
                        track,
                        family,
                        template,
                    } => {
                        let mut next = policies.to_vec();
                        next.push((*track, family.clone(), *template));
                        self.walk(*source, multiplicity, path, &next, excluded, depth + 1)?;
                    }
                    FrozenEdit::Replace { track, payload }
                    | FrozenEdit::Transform { track, payload, .. } => {
                        let mut skip = excluded.to_vec();
                        let family = if let FrozenEdit::Transform { family, .. } = edit {
                            Some(family.clone())
                        } else {
                            None
                        };
                        skip.push((*track, family));
                        self.walk(*source, multiplicity, path, policies, &skip, depth + 1)?;
                        self.payload(
                            PayloadOwner {
                                part: index,
                                track: *track,
                                policy_root: Some(*source),
                                depth,
                            },
                            payload,
                            multiplicity,
                            path,
                            policies,
                            excluded,
                        )?;
                    }
                    FrozenEdit::Overwrite {
                        track,
                        region,
                        payload,
                    } => {
                        let mut skip = excluded.to_vec();
                        let full = region.begin == Ratio64::ZERO && region.end >= part.duration;
                        if full {
                            skip.push((*track, None));
                        }
                        for duration in [
                            region.begin,
                            region.end.checked_sub(region.begin)?,
                            part.duration.checked_sub(region.end)?,
                        ] {
                            if duration > Ratio64::ZERO {
                                self.min_duration =
                                    Some(self.min_duration.map_or(duration, |d| d.min(duration)));
                            }
                        }
                        let first = self.branches.len();
                        path.push(SongRoutePlacement::Region {
                            part: index,
                            span: *region,
                            inside: false,
                        });
                        self.walk(*source, multiplicity, path, policies, &skip, depth + 1)?;
                        path.pop();
                        if !full && region.begin > Ratio64::ZERO && region.end < part.duration {
                            for branch in &mut self.branches[first..] {
                                if branch.track == *track {
                                    branch.occurrences = mul64(branch.occurrences, 2)?;
                                }
                            }
                        }
                        path.push(SongRoutePlacement::Region {
                            part: index,
                            span: *region,
                            inside: true,
                        });
                        self.payload(
                            PayloadOwner {
                                part: index,
                                track: *track,
                                policy_root: Some(*source),
                                depth,
                            },
                            payload,
                            multiplicity,
                            path,
                            policies,
                            excluded,
                        )?;
                        path.pop();
                    }
                    FrozenEdit::Delete(_) => {
                        self.walk(*source, multiplicity, path, policies, excluded, depth + 1)?
                    }
                }
                path.pop();
            }
        }
        Ok(())
    }
    fn payload(
        &mut self,
        owner: PayloadOwner,
        payload: &FrozenPattern,
        multiplicity: u64,
        path: &[SongRoutePlacement],
        policies: &[FxPolicy],
        excluded: &[(KwId, Option<Vec<FrozenSound>>)],
    ) -> Result<(), Failure> {
        let PayloadOwner {
            part: index,
            track,
            policy_root,
            depth,
        } = owner;
        let fully_excluded = excluded
            .iter()
            .any(|(name, family)| *name == track && family.is_none())
            || payload.families.iter().all(|(member, _)| {
                excluded.iter().any(|(name, family)| {
                    *name == track
                        && family
                            .as_ref()
                            .is_some_and(|members| members.contains(member))
                })
            });
        if fully_excluded {
            return Ok(());
        }
        let source_cover = if payload.sources.is_empty() {
            None
        } else {
            let owner = self
                .inventory
                .parts
                .get(index)
                .ok_or_else(|| route_failure("invalid source owner"))?;
            let window = crate::pattern::TimeSpan::new(Ratio64::ZERO, owner.duration)?;
            if self.work >= self.quota {
                return Err(Failure::new(
                    FailCode::FuelExhausted,
                    "route preparation quota exhausted",
                ));
            }
            let child_depth = depth.checked_add(1).ok_or_else(capacity_overflow)?;
            let mut remaining = self
                .quota
                .checked_sub(self.work)
                .ok_or_else(capacity_overflow)?;
            let certified = crate::song::source_uses::certify_source_uses_metered(
                self.inventory,
                payload,
                window,
                self.limits,
                &mut remaining,
                child_depth,
            );
            self.work = self.quota - remaining;
            let cover = certified?;
            let height = self
                .limits
                .max_depth
                .checked_sub(child_depth)
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    Failure::new(FailCode::DepthExceeded, "route source ancestry exhausted")
                })?;
            let measured =
                density::source_density(self.inventory, payload, window, &mut remaining, height);
            self.work = self.quota - remaining;
            let density = measured?;
            let configurations = density.components_intersecting(owner.duration)?;
            let live = u64::from(lifecycle_generations(density, self.tail_cycles)?);
            let ordinal =
                u32::try_from(self.source_covers.len()).map_err(|_| capacity_overflow())?;
            self.source_covers.push(SongSourceRouteCover {
                scope_part: index,
                track,
                cover,
                live_generation_bound: live,
            });
            Some((ordinal, configurations))
        };
        for name in &payload.named_buses {
            named_bus(self.inventory, *name)?;
            if *name != track {
                return Err(route_failure("explicit bus conflicts with owning track"));
            }
        }
        for (family, route) in &payload.families {
            if excluded
                .iter()
                .any(|(t, f)| *t == track && f.as_ref().is_none_or(|f| f.contains(family)))
            {
                continue;
            }
            let current_outer = policies
                .iter()
                .find_map(|(t, f, k)| (*t == track && f.contains(family)).then_some(*k));
            // A selected source keeps its original policy member even when its
            // callback changes the emitted instrument. Only a uniform override
            // can make every inherited configuration obsolete.
            let outer = current_outer.filter(|name| {
                payload.sources.iter().all(|source| {
                    source.family.iter().all(|member| {
                        policies
                            .iter()
                            .find_map(|(t, f, k)| (*t == track && f.contains(member)).then_some(*k))
                            == Some(*name)
                    })
                })
            });
            let mut effects = if let Some(name) = outer {
                vec![Some(name)]
            } else if let Some(root) = policy_root {
                let owner = &self.inventory.parts[index];
                let span = if let FrozenPartNode::Edit {
                    edit: FrozenEdit::Overwrite { region, .. },
                    ..
                } = &owner.node
                {
                    *region
                } else {
                    crate::pattern::TimeSpan::new(Ratio64::ZERO, owner.duration)?
                };
                self.policy_cover(root, track, family, span, 0)?
            } else if source_cover.is_some() {
                self.source_effects(payload, 0)?
            } else {
                vec![None]
            };
            if source_cover.is_some() && outer.is_none() {
                if let Some(name) = current_outer {
                    if !effects.contains(&Some(name)) {
                        effects.push(Some(name));
                    }
                }
                for source in &payload.sources {
                    for member in &source.family {
                        if let Some(name) = policies
                            .iter()
                            .find_map(|(t, f, k)| (*t == track && f.contains(member)).then_some(*k))
                        {
                            if !effects.contains(&Some(name)) {
                                effects.push(Some(name));
                            }
                        }
                    }
                }
                for effect in self.source_effects(payload, 0)? {
                    if !effects.contains(&effect) {
                        effects.push(effect);
                    }
                }
            }
            for effect in effects {
                if let Some(name) = effect {
                    named_bus(self.inventory, name)?;
                }
                self.branch(index, track, family, route, effect, multiplicity, path)?;
                if let Some((cover, configurations)) = source_cover {
                    let branch = self
                        .branches
                        .last_mut()
                        .ok_or_else(|| route_failure("missing new branch"))?;
                    branch.source_cover = Some(cover);
                    branch.configurations_per_placement = configurations;
                    branch.occurrences = mul64(branch.occurrences, configurations)?;
                }
            }
        }
        Ok(())
    }
    fn source_effects(
        &mut self,
        payload: &FrozenPattern,
        depth: u32,
    ) -> Result<Vec<Option<KwId>>, Failure> {
        if depth >= self.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "source FX depth exhausted",
            ));
        }
        let mut out = vec![None];
        for selected in &payload.sources {
            let duration = self
                .inventory
                .parts
                .get(selected.root_part)
                .ok_or_else(|| route_failure("invalid source FX root"))?
                .duration;
            for member in &selected.family {
                for effect in self.policy_cover(
                    selected.root_part,
                    selected.track,
                    member,
                    crate::pattern::TimeSpan::new(Ratio64::ZERO, duration)?,
                    depth + 1,
                )? {
                    if !out.contains(&effect) {
                        out.push(effect);
                    }
                }
            }
            for effect in self.nested_effects(selected.root_part, selected.track, depth + 1)? {
                if !out.contains(&effect) {
                    out.push(effect);
                }
            }
        }
        Ok(out)
    }
    fn nested_effects(
        &mut self,
        index: usize,
        track: KwId,
        depth: u32,
    ) -> Result<Vec<Option<KwId>>, Failure> {
        if depth >= self.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "nested source FX depth exhausted",
            ));
        }
        self.charge(1)?;
        let part = self
            .inventory
            .parts
            .get(index)
            .ok_or_else(|| route_failure("invalid nested source FX Part"))?;
        let mut out = Vec::new();
        let mut append = |values: Vec<Option<KwId>>| {
            for value in values {
                if !out.contains(&value) {
                    out.push(value);
                }
            }
        };
        match &part.node {
            FrozenPartNode::Capture(entries) => {
                for (name, payload) in entries {
                    if *name == track {
                        append(self.source_effects(payload, depth + 1)?);
                    }
                }
            }
            FrozenPartNode::Sequence(children) => {
                for (_, child) in children {
                    append(self.nested_effects(*child, track, depth + 1)?);
                }
            }
            FrozenPartNode::Repeat { child, .. } => {
                append(self.nested_effects(*child, track, depth + 1)?)
            }
            FrozenPartNode::Edit { source, edit } => {
                append(self.nested_effects(*source, track, depth + 1)?);
                if let FrozenEdit::Transform {
                    track: name,
                    payload,
                    ..
                }
                | FrozenEdit::Replace {
                    track: name,
                    payload,
                }
                | FrozenEdit::Overwrite {
                    track: name,
                    payload,
                    ..
                } = edit
                {
                    if *name == track {
                        append(self.source_effects(payload, depth + 1)?);
                    }
                }
            }
        }
        Ok(out)
    }
    fn policy_cover(
        &mut self,
        index: usize,
        track: KwId,
        family: &FrozenSound,
        span: crate::pattern::TimeSpan,
        depth: u32,
    ) -> Result<Vec<Option<KwId>>, Failure> {
        if depth >= self.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "FX policy depth exhausted",
            ));
        }
        self.charge(1)?;
        let part = self
            .inventory
            .parts
            .get(index)
            .ok_or_else(|| route_failure("invalid policy Part"))?;
        if span.end <= Ratio64::ZERO || span.begin >= part.duration || !part.tracks.contains(&track)
        {
            return Ok(vec![None]);
        }
        match &part.node {
            FrozenPartNode::Capture(_) => Ok(vec![None]),
            FrozenPartNode::Edit { source, edit } => {
                if let FrozenEdit::InstrumentFx {
                    track: selected,
                    family: selection,
                    template,
                } = edit
                {
                    if *selected == track && selection.contains(family) {
                        return Ok(vec![Some(*template)]);
                    }
                }
                self.policy_cover(*source, track, family, span, depth + 1)
            }
            FrozenPartNode::Repeat { child, .. } => {
                // Complete child union is conservative across arbitrary touched
                // repeats; admission never traverses each repetition.
                let duration = self
                    .inventory
                    .parts
                    .get(*child)
                    .ok_or_else(|| route_failure("invalid policy repeat"))?
                    .duration;
                self.policy_cover(
                    *child,
                    track,
                    family,
                    crate::pattern::TimeSpan::new(Ratio64::ZERO, duration)?,
                    depth + 1,
                )
            }
            FrozenPartNode::Sequence(children) => {
                let mut out = Vec::new();
                for (offset, child) in children {
                    let duration = self
                        .inventory
                        .parts
                        .get(*child)
                        .ok_or_else(|| route_failure("invalid policy child"))?
                        .duration;
                    let end = offset.checked_add(duration)?;
                    if span.begin >= end || span.end <= *offset {
                        continue;
                    }
                    let local = crate::pattern::TimeSpan::new(
                        span.begin.max(*offset).checked_sub(*offset)?,
                        span.end.min(end).checked_sub(*offset)?,
                    )?;
                    for template in self.policy_cover(*child, track, family, local, depth + 1)? {
                        if !out.contains(&template) {
                            out.push(template);
                        }
                    }
                }
                Ok(out)
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn branch(
        &mut self,
        index: usize,
        track: KwId,
        family: &FrozenSound,
        route: &FrozenAudioRoute,
        effect: Option<KwId>,
        multiplicity: u64,
        path: &[SongRoutePlacement],
    ) -> Result<(), Failure> {
        if self.branches.len() >= self.available.template_slots as usize {
            return Err(owner_failure(
                capacity_failure("template_slots"),
                self.inventory,
                track,
                family,
                index,
                path,
            ));
        }
        let destination = self
            .tracks
            .iter()
            .find(|stage| stage.track == track)
            .ok_or_else(|| route_failure("payload has no declared track stage"))?
            .destination;
        let id = SongBranchId(u32::try_from(self.branches.len()).map_err(|_| capacity_overflow())?);
        self.branches.push(SongBranchRoute {
            id,
            track,
            instrument: family.clone(),
            resolved_instrument: route.instrument,
            sample: route.sample.clone(),
            effect_template: effect,
            destination,
            scope_part: index,
            placement: path.to_vec(),
            source_cover: None,
            configurations_per_placement: 1,
            occurrences: multiplicity,
            reserved_generations: 0,
        });
        Ok(())
    }
}

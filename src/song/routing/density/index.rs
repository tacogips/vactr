//! Source density traversal and checked Index opening consumption.
use super::*;

impl DensityWalk<'_, '_> {
    fn enter(&mut self, depth: u32) -> Result<(), Failure> {
        if depth >= self.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "configuration density depth exhausted",
            ));
        }
        density_charge(self.remaining, 1)
    }
    fn carries_source(
        &mut self,
        pattern: &crate::song::snapshot::FrozenPattern,
        index: u32,
        depth: u32,
    ) -> Result<bool, Failure> {
        self.enter(depth)?;
        let node = pattern
            .source_uses
            .nodes
            .get(index as usize)
            .ok_or_else(|| Failure::new(FailCode::Type, "ordinary density index"))?;
        if matches!(
            node.mapping,
            crate::song::source_uses::FrozenUseMapping::Source { .. }
        ) {
            return Ok(true);
        }
        for edge in &node.edges {
            if self.carries_source(pattern, edge.child, depth + 1)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(super) fn node(
        &mut self,
        pattern: &crate::song::snapshot::FrozenPattern,
        index: u32,
        owner: crate::pattern::TimeSpan,
        original_members: Option<&[FrozenSound]>,
        depth: u32,
    ) -> Result<ConfigurationBound, Failure> {
        self.node_path(
            pattern,
            index,
            owner,
            original_members,
            depth,
            &mut Vec::new(),
        )
    }
    fn node_path(
        &mut self,
        pattern: &crate::song::snapshot::FrozenPattern,
        index: u32,
        owner: crate::pattern::TimeSpan,
        original_members: Option<&[FrozenSound]>,
        depth: u32,
        prefix: &mut Vec<crate::song::source_uses::FrozenUseTraceTerm>,
    ) -> Result<ConfigurationBound, Failure> {
        use crate::song::source_uses::{FrozenUseMapping as M, FrozenUseTraceTerm as T};
        self.enter(depth)?;
        let node = pattern
            .source_uses
            .nodes
            .get(index as usize)
            .ok_or_else(|| Failure::new(FailCode::Type, "density graph index is invalid"))?;
        if let M::Source { policy } = node.mapping {
            let source = pattern
                .sources
                .get(policy as usize)
                .ok_or_else(|| Failure::new(FailCode::Type, "density source policy is invalid"))?;
            let members = original_members.unwrap_or(&source.family);
            charge_members(self.remaining, members, &source.family)?;
            let selected: Vec<_> = members
                .iter()
                .filter(|m| source.family.contains(m))
                .cloned()
                .collect();
            return self.part(
                source.root_part,
                DensitySelection {
                    track: source.track,
                    original_members: &selected,
                },
                owner,
                depth + 1,
            );
        }
        if matches!(node.mapping, M::Empty) {
            if node.operation == crate::song::source_uses::FrozenUseOperation::Signal {
                return Ok(ConfigurationBound::empty());
            }
            let members = original_members;
            let mut count = 0;
            for (sound, _) in &pattern.families {
                if let Some(members) = members {
                    charge_members(self.remaining, members, std::slice::from_ref(sound))?;
                }
                if members.is_none_or(|m| m.contains(sound)) {
                    count += 1;
                }
            }
            return Ok(ConfigurationBound::constant(count));
        }
        if let M::Uncertifiable(reason) = node.mapping {
            use crate::song::source_uses::FrozenUseReason as R;
            if matches!(reason, R::DynamicRate | R::DynamicCount | R::DynamicTiming)
                && !self.carries_source(pattern, index, depth)?
            {
                return Ok(ConfigurationBound::constant(pattern.families.len() as u64));
            }
            return Err(Failure::new(
                FailCode::BeyondCapability,
                "uncertifiable configuration density",
            ));
        }
        let index_support = if let M::Slices {
            structure: crate::song::source_uses::FrozenSliceStructure::Index { issuer },
            ..
        } = node.mapping
        {
            Some(self.index_support(pattern, issuer, prefix, depth)?)
        } else {
            None
        };
        let subject_owner = if let Some(support) = index_support {
            crate::pattern::TimeSpan::new(owner.begin.checked_sub(support.max_whole)?, owner.end)?
        } else {
            owner
        };
        let mut sum = ConfigurationBound::empty();
        let mut conditional = [ConfigurationBound::empty(); 2];
        for (ordinal, edge) in node.edges.iter().enumerate() {
            if matches!(node.mapping, M::Slices { .. }) && ordinal != 0 {
                continue;
            }
            if let M::SelectContent { content }
            | M::Restructure { content, .. }
            | M::SampleCycles { content }
            | M::SampleGrid { content, .. } = node.mapping
            {
                if ordinal != content as usize {
                    continue;
                }
            }
            let Some(local) = crate::song::source::mapped_edge_support(
                node,
                edge,
                subject_owner,
                None,
                self.remaining,
            )?
            else {
                continue;
            };
            let mut copies = 1u64;
            for term in &edge.trace {
                density_charge(self.remaining, 1)?;
                if let T::Copies { count, .. } = term {
                    copies = copies
                        .checked_mul(u64::from(*count))
                        .ok_or_else(capacity_overflow)?;
                }
            }
            let prefix_len = prefix.len();
            density_charge(
                self.remaining,
                u32::try_from(edge.trace.len()).map_err(|_| capacity_overflow())?,
            )?;
            prefix.extend_from_slice(&edge.trace);
            let child = self.node_path(
                pattern,
                edge.child,
                local,
                original_members,
                depth + 1,
                prefix,
            );
            prefix.truncate(prefix_len);
            let child = child?.scale(copies)?;
            if let Some(slot) = conditional.get_mut(ordinal) {
                *slot = child;
            }
            sum = sum.add(child)?;
        }
        if let Some(support) = index_support {
            return Self::index_opening(sum, support, owner);
        }
        if let M::ConditionalStatic(condition) = node.mapping {
            return conditional_partition(
                conditional[0],
                conditional[1],
                condition,
                owner,
                self.remaining,
            );
        }
        if let M::SampleGrid { sampling, .. } = node.mapping {
            let phases =
                crate::song::source_uses::sampling::enabled_phases(sampling, self.remaining)?;
            let crate::song::source_uses::FrozenStaticSampling::Euclid { divisions, .. } = sampling;
            if phases.is_empty() {
                return Ok(ConfigurationBound::empty());
            }
            density_charge(
                self.remaining,
                u32::try_from(phases.len()).map_err(|_| capacity_overflow())?,
            )?;
            if phases.len() as i64 == divisions {
                // Sampling can move a source boundary by less than one cycle.
                // A constant full-grid configuration has no artificial reentry.
                sum.births.burst = sum
                    .births
                    .burst
                    .checked_add(ceiling(
                        sum.births.slope.checked_mul(Ratio64::from_int(2))?,
                    )?)
                    .ok_or_else(capacity_overflow)?;
                return Ok(sum);
            }
            let mut runs = 0u64;
            for (i, phase) in phases.iter().enumerate() {
                let previous = if i == 0 {
                    *phases.last().ok_or_else(capacity_overflow)?
                } else {
                    phases[i - 1]
                };
                let n = u32::try_from(divisions).map_err(|_| capacity_overflow())?;
                if (previous + 1) % n != *phase {
                    runs = runs.checked_add(1).ok_or_else(capacity_overflow)?;
                }
            }
            let mut bound = sum.fragmented(owner, Ratio64::from_int(3))?;
            bound.births = bound.births.scale(runs.max(1))?;
            bound.finite_total = bound
                .instantaneous
                .checked_add(bound.births.at(density_owner_width(owner)?)?)
                .ok_or_else(capacity_overflow)?;
            return Ok(bound);
        }
        let weighted = matches!(
            node.operation,
            crate::song::source_uses::FrozenUseOperation::Steps
                | crate::song::source_uses::FrozenUseOperation::Pure
        );
        if weighted {
            let identity = node.edges.len() == 1
                && node.edges[0]
                    .layout
                    .iter()
                    .all(|slot| slot.prefix == Ratio64::ZERO && slot.width == Ratio64::ONE);
            return if identity {
                Ok(sum)
            } else {
                sum.fragmented(owner, Ratio64::from_int(3))
            };
        }
        match node.mapping {
            M::Rate { factor } if factor <= Ratio64::ZERO => Ok(ConfigurationBound::empty()),
            M::Rate { factor } => {
                sum.births.slope = sum.births.slope.checked_mul(factor)?;
                Ok(sum)
            }
            M::CycleSelect => sum.fragmented(owner, Ratio64::from_int(2)),
            M::ReflectCycles
            | M::CycleConcat
            | M::SampleCycles { .. }
            | M::Iterate { .. }
            | M::Restructure { .. } => sum.fragmented(owner, Ratio64::from_int(3)),
            M::Subdivide { count } => sum.fragmented(owner, Ratio64::ONE)?.scale(u64::from(count)),
            _ => Ok(sum),
        }
    }
    fn index_support(
        &mut self,
        pattern: &crate::song::snapshot::FrozenPattern,
        issuer: crate::reader::span::NodeId,
        prefix: &[crate::song::source_uses::FrozenUseTraceTerm],
        depth: u32,
    ) -> Result<super::super::index::IndexNodeSupport, Failure> {
        use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
        let mut budget = super::super::source::ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: *self.remaining,
            max_depth: self.max_depth,
            ..Default::default()
        });
        let result = (|| {
            let mut address = None;
            for (index, part) in self.inventory.parts.iter().enumerate() {
                budget.charge(1)?;
                let matching = match &part.node {
                    P::Capture(entries) => {
                        budget.charge(entries.len())?;
                        entries.iter().find_map(|(track, payload)| {
                            std::ptr::eq(payload, pattern).then_some(*track)
                        })
                    }
                    P::Edit {
                        edit:
                            E::Replace { track, payload }
                            | E::Overwrite { track, payload, .. }
                            | E::Transform { track, payload, .. },
                        ..
                    } => std::ptr::eq(payload, pattern).then_some(*track),
                    _ => None,
                };
                if let Some(track) = matching {
                    if address.replace((index, track)).is_some() {
                        return Err(Failure::new(
                            FailCode::Type,
                            "ambiguous original Index output owner",
                        ));
                    }
                }
            }
            let (index, track) = address
                .ok_or_else(|| Failure::new(FailCode::Type, "Index output absent from topology"))?;
            let output = super::super::index::output_operand_owner(
                self.inventory,
                index,
                track,
                &mut budget,
            )?;
            let prepared = super::super::index::prepare_slice_operands(
                output,
                issuer,
                prefix,
                depth,
                &mut budget,
            )?;
            let support = match super::super::index::admit_prepared_index_support(
                &prepared,
                depth,
                &mut budget,
            )? {
                super::super::index::IndexSupportAdmission::Static(support) => support,
                super::super::index::IndexSupportAdmission::RequiresRealization(request) => {
                    return Err(request.failure())
                }
            };
            support
                .nodes
                .get(support.root as usize)
                .copied()
                .ok_or_else(|| Failure::new(FailCode::Type, "Index support root"))
        })();
        *self.remaining = budget.limits().max_nodes;
        result
    }
    fn index_opening(
        subject: ConfigurationBound,
        support: super::super::index::IndexNodeSupport,
        owner: crate::pattern::TimeSpan,
    ) -> Result<ConfigurationBound, Failure> {
        if support.empty || subject.instantaneous == 0 {
            return Ok(ConfigurationBound::empty());
        }
        let instantaneous = support
            .starts(support.max_whole)?
            .checked_mul(subject.instantaneous)
            .ok_or_else(capacity_overflow)?;
        let finite_total = support
            .starts(density_owner_width(owner)?.checked_add(support.max_whole)?)?
            .checked_mul(subject.instantaneous)
            .ok_or_else(capacity_overflow)?;
        u32::try_from(finite_total).map_err(|_| capacity_overflow())?;
        let births = UniformBirthBound {
            slope: support
                .event_slope
                .checked_mul(integer(subject.instantaneous)?)?,
            burst: support
                .event_burst
                .checked_mul(subject.instantaneous)
                .and_then(|burst| burst.checked_add(instantaneous))
                .ok_or_else(capacity_overflow)?,
        };
        Ok(ConfigurationBound {
            instantaneous,
            births,
            finite_total,
        })
    }
    fn payload(
        &mut self,
        payload: &crate::song::snapshot::FrozenPattern,
        selected: DensitySelection<'_>,
        owner: crate::pattern::TimeSpan,
        depth: u32,
    ) -> Result<ConfigurationBound, Failure> {
        self.enter(depth)?;
        for (sound, _) in &payload.families {
            charge_members(
                self.remaining,
                selected.original_members,
                std::slice::from_ref(sound),
            )?;
        }
        if payload.sources.is_empty() {
            return Ok(ConfigurationBound::constant(
                payload
                    .families
                    .iter()
                    .filter(|(sound, _)| selected.original_members.contains(sound))
                    .count() as u64,
            ));
        }
        self.node(
            payload,
            payload.source_uses.root,
            owner,
            Some(selected.original_members),
            depth,
        )
    }
    fn part(
        &mut self,
        index: usize,
        selected: DensitySelection<'_>,
        owner: crate::pattern::TimeSpan,
        depth: u32,
    ) -> Result<ConfigurationBound, Failure> {
        use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
        self.enter(depth)?;
        let part = self
            .inventory
            .parts
            .get(index)
            .ok_or_else(|| Failure::new(FailCode::Type, "density Part index is invalid"))?;
        if !part.tracks.contains(&selected.track) || selected.original_members.is_empty() {
            return Ok(ConfigurationBound::empty());
        }
        let Some(owner) = density_clip(owner, part.duration)? else {
            return Ok(ConfigurationBound::empty());
        };
        match &part.node {
            P::Capture(entries) => {
                let Some((_, payload)) = entries.iter().find(|(name, _)| *name == selected.track)
                else {
                    return Ok(ConfigurationBound::empty());
                };
                self.payload(payload, selected, owner, depth + 1)
            }
            P::Sequence(children) => {
                let mut sum = ConfigurationBound::empty();
                let mut instantaneous = 0;
                for (offset, child) in children {
                    density_charge(self.remaining, 1)?;
                    let local = owner.map(|t| t.checked_sub(*offset))?;
                    let bound = self.part(*child, selected, local, depth + 1)?;
                    instantaneous = instantaneous.max(bound.instantaneous);
                    sum = sum.add(bound)?;
                }
                sum.instantaneous = instantaneous;
                Ok(sum)
            }
            P::Repeat { child, count, .. } => {
                let child_part = self.inventory.parts.get(*child).ok_or_else(|| {
                    Failure::new(FailCode::Type, "density repeat child is invalid")
                })?;
                let span = crate::pattern::TimeSpan::new(Ratio64::ZERO, child_part.duration)?;
                let bound = self.part(*child, selected, span, depth + 1)?;
                finite_repeat(bound, child_part.duration, *count, owner, self.remaining)
            }
            P::Edit { source, edit } => match edit {
                E::Replace { track, payload } if *track == selected.track => {
                    self.payload(payload, selected, owner, depth + 1)
                }
                E::Transform {
                    track,
                    family,
                    payload,
                    ..
                } if *track == selected.track => {
                    charge_members(self.remaining, selected.original_members, family)?;
                    let residual: Vec<_> = selected
                        .original_members
                        .iter()
                        .filter(|m| !family.contains(m))
                        .cloned()
                        .collect();
                    let mut bound = self.part(
                        *source,
                        DensitySelection {
                            track: selected.track,
                            original_members: &residual,
                        },
                        owner,
                        depth + 1,
                    )?;
                    bound = bound.add(self.payload(payload, selected, owner, depth + 1)?)?;
                    Ok(bound)
                }
                E::Overwrite {
                    track,
                    region,
                    payload,
                } if *track == selected.track => {
                    let original = self.part(*source, selected, owner, depth + 1)?;
                    if region.begin == Ratio64::ZERO && region.end >= part.duration {
                        return self.payload(payload, selected, owner, depth + 1);
                    }
                    let replacement = self.payload(payload, selected, owner, depth + 1)?;
                    let mut bound = original.scale(2)?.add(replacement)?;
                    bound.instantaneous = original.instantaneous.max(replacement.instantaneous);
                    Ok(bound)
                }
                _ => self.part(*source, selected, owner, depth + 1),
            },
        }
    }
}

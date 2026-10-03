//! Original source certification engine with original caller work/depth.
use super::*;
pub fn certify_source_uses(
    inventory: &crate::song::snapshot::FrozenRoutingInventory,
    pattern: &FrozenPattern,
    window: TimeSpan,
    limits: crate::song::SongLimits,
) -> Result<FrozenSourceUseCover, crate::vm::fail::Failure> {
    let mut remaining = limits.max_nodes;
    certify_source_uses_metered(inventory, pattern, window, limits, &mut remaining, 0)
}
/// Caller retains all debits even when certification fails.
pub(crate) fn certify_source_uses_metered(
    inventory: &crate::song::snapshot::FrozenRoutingInventory,
    pattern: &FrozenPattern,
    window: TimeSpan,
    limits: crate::song::SongLimits,
    remaining: &mut u32,
    depth: u32,
) -> Result<FrozenSourceUseCover, crate::vm::fail::Failure> {
    use crate::vm::fail::{FailCode, Failure};
    limits.validate()?;
    if window.begin < Ratio64::ZERO || window.end < window.begin {
        return Err(Failure::new(FailCode::Type, "invalid source-use window"));
    }
    if *remaining > limits.max_nodes {
        return Err(Failure::new(
            FailCode::Type,
            "certification work exceeds original policy",
        ));
    }
    let initial = *remaining;
    let mut state = Certification {
        inventory,
        limits,
        remaining,
        visiting: std::collections::BTreeSet::new(),
        cache: std::collections::BTreeMap::new(),
        source_free_cache: std::collections::BTreeMap::new(),
    };
    if depth >= limits.max_depth {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "use-cover depth exhausted",
        ));
    }
    let bound = if pattern.source_uses.nodes.is_empty() {
        if pattern.source_uses.root != 0 {
            return Err(Failure::new(
                FailCode::Type,
                "invalid empty source-use root",
            ));
        }
        0
    } else {
        state.node(pattern, pattern.source_uses.root, window, depth)?
    };
    let mut cost = pattern
        .source_uses
        .nodes
        .len()
        .checked_add(pattern.sources.len())
        .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover copy overflow"))?;
    for node in &pattern.source_uses.nodes {
        if let FrozenUseMapping::Slices { starts, .. } = &node.mapping {
            cost = cost
                .checked_add(starts.len())
                .and_then(|n| n.checked_add(2))
                .ok_or_else(use_overflow)?;
        }
        cost = cost
            .checked_add(node.edges.len())
            .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover edge overflow"))?;
        for edge in &node.edges {
            cost = cost
                .checked_add(edge.trace.len())
                .and_then(|n| n.checked_add(edge.layout.len()))
                .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover trace overflow"))?;
        }
        if let FrozenUseMapping::Slices { starts, .. } = &node.mapping {
            cost = cost
                .checked_add(starts.len())
                .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover cuts overflow"))?;
        }
    }
    state.admit(cost)?;
    for source in &pattern.sources {
        state.admit(source.family.len())?;
    }
    let policies = pattern
        .sources
        .iter()
        .map(|source| {
            let part = inventory
                .parts
                .get(source.root_part)
                .ok_or_else(|| Failure::new(FailCode::Type, "source policy root out of range"))?;
            Ok((part.revision, source.track, source.family.clone()))
        })
        .collect::<Result<_, Failure>>()?;
    let consumed_work = initial
        .checked_sub(*state.remaining)
        .ok_or_else(use_overflow)?;
    Ok(FrozenSourceUseCover {
        graph: pattern.source_uses.clone(),
        window,
        configuration_bound: bound,
        consumed_work,
        policies,
    })
}
impl Certification<'_> {
    pub(super) fn admit(&mut self, n: usize) -> Result<(), crate::vm::fail::Failure> {
        use crate::vm::fail::{FailCode, Failure};
        let n = u32::try_from(n)
            .map_err(|_| Failure::new(FailCode::Overflow, "use-cover admission overflow"))?;
        *self.remaining = self
            .remaining
            .checked_sub(n)
            .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "use-cover work exhausted"))?;
        Ok(())
    }
    pub(super) fn enter(&mut self, depth: u32) -> Result<(), crate::vm::fail::Failure> {
        if depth >= self.limits.max_depth {
            return Err(crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::DepthExceeded,
                "use-cover depth exhausted",
            ));
        }
        self.admit(1)
    }
    pub(super) fn node(
        &mut self,
        pattern: &FrozenPattern,
        index: u32,
        window: TimeSpan,
        depth: u32,
    ) -> Result<u64, crate::vm::fail::Failure> {
        self.enter(depth)?;
        let key = (pattern as *const FrozenPattern as usize, index);
        let cached = (key.0, index, None, window.begin, window.end, depth);
        if let Some(bound) = self.cache.get(&cached) {
            return Ok(*bound);
        }
        if !self.visiting.insert(key) {
            return Err(use_invalid("cyclic source-use recipe"));
        }
        let node = pattern
            .source_uses
            .nodes
            .get(index as usize)
            .ok_or_else(|| use_invalid("source-use node out of range"))?;
        let unary = node.edges.len() == 1
            && matches!(
                node.mapping,
                FrozenUseMapping::Preserve
                    | FrozenUseMapping::Rate { .. }
                    | FrozenUseMapping::ReflectCycles
                    | FrozenUseMapping::Iterate { .. }
                    | FrozenUseMapping::Shift { .. }
                    | FrozenUseMapping::CycleConcat
            );
        let result = if unary {
            self.unary(pattern, node, window, depth)
        } else {
            self.node_inner(pattern, index, window, depth)
        };
        self.visiting.remove(&key);
        if let Ok(bound) = result {
            self.admit(1)?;
            self.cache.insert(cached, bound);
        }
        result
    }
    fn unary(
        &mut self,
        pattern: &FrozenPattern,
        node: &FrozenSourceUseNode,
        window: TimeSpan,
        depth: u32,
    ) -> Result<u64, crate::vm::fail::Failure> {
        let edge = &node.edges[0];
        self.admit(
            edge.trace
                .len()
                .checked_add(edge.layout.len())
                .ok_or_else(use_overflow)?,
        )?;
        layout::validate_layout(edge)?;
        if edge.child as usize >= pattern.source_uses.nodes.len() {
            return Err(use_invalid("source-use edge out of range"));
        }
        let mut copies = 1u64;
        for term in &edge.trace {
            if let FrozenUseTraceTerm::Copies { count, .. } = term {
                if *count > 4096 {
                    return Err(use_invalid("source-use copy bound out of range"));
                }
                copies = copies
                    .checked_mul(u64::from(*count))
                    .ok_or_else(use_overflow)?;
            }
        }
        if matches!(node.mapping, FrozenUseMapping::Rate { factor } if factor <= Ratio64::ZERO) {
            return Ok(0);
        }
        let Some(mapped) = mapped_edge_support(node, edge, window, None, self.remaining)? else {
            return Ok(0);
        };
        self.node(pattern, edge.child, mapped, depth + 1)?
            .checked_mul(copies)
            .ok_or_else(use_overflow)
    }
    #[inline(never)]
    fn node_inner(
        &mut self,
        pattern: &FrozenPattern,
        index: u32,
        window: TimeSpan,
        depth: u32,
    ) -> Result<u64, crate::vm::fail::Failure> {
        let node = pattern
            .source_uses
            .nodes
            .get(index as usize)
            .ok_or_else(|| use_invalid("source-use node out of range"))?;
        for edge in &node.edges {
            self.admit(
                edge.trace
                    .len()
                    .checked_add(edge.layout.len())
                    .ok_or_else(use_overflow)?,
            )?;
            layout::validate_layout(edge)?;
            if edge.child as usize >= pattern.source_uses.nodes.len() {
                return Err(use_invalid("source-use edge out of range"));
            }
            for term in &edge.trace {
                if let FrozenUseTraceTerm::Copies { count, .. } = term {
                    if *count > 4096 {
                        return Err(use_invalid("source-use copy bound out of range"));
                    }
                }
            }
        }
        conditional::validate(node)?;
        sampling::validate(node)?;
        if let FrozenUseMapping::Source { policy } = &node.mapping {
            if !node.edges.is_empty() {
                return Err(use_invalid("source leaf has child edges"));
            }
            let source = pattern
                .sources
                .get(*policy as usize)
                .ok_or_else(|| use_invalid("source policy out of range"))?;
            let root = self
                .inventory
                .parts
                .get(source.root_part)
                .ok_or_else(|| use_invalid("source policy root out of range"))?;
            if !root.tracks.contains(&source.track) {
                return Err(use_invalid("selected source root track is absent"));
            }
            return self.part(source.root_part, source.track, window, depth + 1);
        }
        if let FrozenUseMapping::Uncertifiable(reason) = &node.mapping {
            return Err(crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::BeyondCapability,
                format!(
                    "uncertifiable source use: {:?} {:?}",
                    node.operation, reason
                ),
            ));
        }
        if matches!(node.mapping, FrozenUseMapping::Empty) {
            if !node.edges.is_empty() {
                return Err(use_invalid("empty source-use node has edges"));
            }
            return Ok(0);
        }
        if matches!(node.mapping, FrozenUseMapping::Rate { factor } if factor <= Ratio64::ZERO) {
            return Ok(0);
        }
        let multiplier = match &node.mapping {
            FrozenUseMapping::Subdivide { count } => u64::from(*count)
                .checked_mul(cycles(window)?)
                .ok_or_else(use_overflow)?,
            FrozenUseMapping::Slices { starts, .. } => {
                self.admit(starts.len())?;
                if starts.is_empty()
                    || starts
                        .iter()
                        .any(|p| *p < Ratio64::ZERO || *p > Ratio64::ONE)
                    || starts.windows(2).any(|p| p[0] >= p[1])
                {
                    return Err(use_invalid("invalid certified slice points"));
                }
                1
            }
            _ => 1,
        };
        let mut sum = 0u64;
        for (ordinal, edge) in node.edges.iter().enumerate() {
            if matches!(node.mapping, FrozenUseMapping::Slices { .. }) && ordinal != 0 {
                continue;
            }
            if let FrozenUseMapping::SelectContent { content }
            | FrozenUseMapping::Restructure { content, .. }
            | FrozenUseMapping::SampleGrid { content, .. }
            | FrozenUseMapping::SampleCycles { content } = &node.mapping
            {
                if ordinal != *content as usize {
                    continue;
                }
            }
            let mut copies = 1u64;
            for term in &edge.trace {
                if let FrozenUseTraceTerm::Copies { count, .. } = term {
                    copies = copies
                        .checked_mul(u64::from(*count))
                        .ok_or_else(use_overflow)?;
                }
            }
            let Some(edge_window) = mapped_edge_support(node, edge, window, None, self.remaining)?
            else {
                continue;
            };
            let bound = self
                .node(pattern, edge.child, edge_window, depth + 1)?
                .checked_mul(copies)
                .ok_or_else(use_overflow)?;
            sum = sum.checked_add(bound).ok_or_else(use_overflow)?;
        }
        sum.checked_mul(multiplier).ok_or_else(use_overflow)
    }
    fn part(
        &mut self,
        index: usize,
        track: crate::value::intern::KwId,
        window: TimeSpan,
        depth: u32,
    ) -> Result<u64, crate::vm::fail::Failure> {
        self.enter(depth)?;
        let key = (
            usize::MAX,
            u32::try_from(index).map_err(|_| use_overflow())?,
        );
        let cached = (key.0, key.1, Some(track), window.begin, window.end, depth);
        if let Some(bound) = self.cache.get(&cached) {
            return Ok(*bound);
        }
        if !self.visiting.insert(key) {
            return Err(use_invalid("cyclic source policy topology"));
        }
        let result = self.part_inner(index, track, window, depth);
        self.visiting.remove(&key);
        if let Ok(bound) = result {
            self.admit(1)?;
            self.cache.insert(cached, bound);
        }
        result
    }
    #[inline(never)]
    fn part_inner(
        &mut self,
        index: usize,
        track: crate::value::intern::KwId,
        window: TimeSpan,
        depth: u32,
    ) -> Result<u64, crate::vm::fail::Failure> {
        use crate::song::snapshot::{FrozenEdit, FrozenPartNode};
        let part = self
            .inventory
            .parts
            .get(index)
            .ok_or_else(|| use_invalid("source policy Part out of range"))?;
        if part.duration < Ratio64::ZERO {
            return Err(use_invalid("source policy duration is negative"));
        }
        if part.duration == Ratio64::ZERO || !part.tracks.contains(&track) {
            return Ok(0);
        }
        let Some(window) = supported_window(window, part.duration)? else {
            return Ok(0);
        };
        match &part.node {
            FrozenPartNode::Capture(tracks) => {
                let payload = tracks
                    .iter()
                    .find(|(name, _)| *name == track)
                    .ok_or_else(|| use_invalid("source capture track absent"))?;
                let nested = self.payload_bound(&payload.1, window, depth + 1)?;
                Ok(if payload.1.families.is_empty() {
                    nested
                } else {
                    nested.max(1)
                })
            }
            FrozenPartNode::Sequence(children) => {
                self.admit(children.len())?;
                let mut sum = 0u64;
                for (offset, child) in children {
                    let local = window.map(|t| t.checked_sub(*offset))?;
                    sum = sum
                        .checked_add(self.part(*child, track, local, depth + 1)?)
                        .ok_or_else(use_overflow)?;
                }
                Ok(sum)
            }
            FrozenPartNode::Repeat { child, count, .. } => {
                if *count == 0 {
                    return Ok(0);
                }
                let duration = self
                    .inventory
                    .parts
                    .get(*child)
                    .ok_or_else(|| use_invalid("repeat child out of range"))?
                    .duration;
                if duration <= Ratio64::ZERO {
                    return Ok(0);
                }
                let first = window.begin.checked_div(duration)?.floor();
                let end = window.end.checked_div(duration)?;
                let last = if window.is_point() {
                    first
                } else {
                    end.floor()
                        .checked_sub(i64::from(end.frac() == Ratio64::ZERO))
                        .ok_or_else(use_overflow)?
                };
                let first = first.max(0);
                let last = last.min(i64::from(*count) - 1);
                if last < first {
                    return Ok(0);
                }
                let local = |ordinal| {
                    window.map(|t| t.checked_sub(duration.checked_mul(Ratio64::from_int(ordinal))?))
                };
                let head = self.part(*child, track, local(first)?, depth + 1)?;
                if first == last {
                    return Ok(head);
                }
                let tail = self.part(*child, track, local(last)?, depth + 1)?;
                let interiors = u64::try_from(last - first - 1).map_err(|_| use_overflow())?;
                let middle = if interiors == 0 {
                    0
                } else {
                    self.part(
                        *child,
                        track,
                        TimeSpan::new(Ratio64::ZERO, duration)?,
                        depth + 1,
                    )?
                    .checked_mul(interiors)
                    .ok_or_else(use_overflow)?
                };
                head.checked_add(tail)
                    .and_then(|n| n.checked_add(middle))
                    .ok_or_else(use_overflow)
            }
            FrozenPartNode::Edit { source, edit } => {
                let original = self.part(*source, track, window, depth + 1)?;
                match edit {
                    FrozenEdit::Overwrite {
                        track: t, payload, ..
                    }
                    | FrozenEdit::Replace { track: t, payload }
                    | FrozenEdit::Transform {
                        track: t, payload, ..
                    } if *t == track => {
                        let changed = self.payload_bound(payload, window, depth + 1)?.max(1);
                        original
                            .checked_mul(3)
                            .and_then(|n| n.checked_add(changed))
                            .ok_or_else(use_overflow)
                    }
                    _ => Ok(original),
                }
            }
        }
    }
}

// Exact saved extraction entry; independent historical work witness, test-only.
#[cfg(test)]
pub(crate) fn certify_extraction_reference(
    inventory: &crate::song::snapshot::FrozenRoutingInventory,
    pattern: &FrozenPattern,
    window: TimeSpan,
    limits: crate::song::SongLimits,
) -> Result<FrozenSourceUseCover, crate::vm::fail::Failure> {
    use crate::vm::fail::{FailCode, Failure};
    limits.validate()?;
    if window.begin < Ratio64::ZERO || window.end < window.begin {
        return Err(Failure::new(FailCode::Type, "invalid source-use window"));
    }
    let mut remaining = limits.max_nodes;
    let mut state = Certification {
        inventory,
        limits,
        remaining: &mut remaining,
        visiting: std::collections::BTreeSet::new(),
        cache: std::collections::BTreeMap::new(),
        source_free_cache: std::collections::BTreeMap::new(),
    };
    let bound = if pattern.source_uses.nodes.is_empty() {
        if pattern.source_uses.root != 0 {
            return Err(Failure::new(
                FailCode::Type,
                "invalid empty source-use root",
            ));
        }
        0
    } else {
        state.node(pattern, pattern.source_uses.root, window, 0)?
    };
    let mut cost = pattern
        .source_uses
        .nodes
        .len()
        .checked_add(pattern.sources.len())
        .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover copy overflow"))?;
    for node in &pattern.source_uses.nodes {
        if let FrozenUseMapping::Slices { starts, .. } = &node.mapping {
            cost = cost
                .checked_add(starts.len())
                .and_then(|n| n.checked_add(2))
                .ok_or_else(use_overflow)?;
        }
        cost = cost
            .checked_add(node.edges.len())
            .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover edge overflow"))?;
        for edge in &node.edges {
            cost = cost
                .checked_add(edge.trace.len())
                .and_then(|n| n.checked_add(edge.layout.len()))
                .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover trace overflow"))?;
        }
        if let FrozenUseMapping::Slices { starts, .. } = &node.mapping {
            cost = cost
                .checked_add(starts.len())
                .ok_or_else(|| Failure::new(FailCode::Overflow, "use-cover cuts overflow"))?;
        }
    }
    state.admit(cost)?;
    for source in &pattern.sources {
        state.admit(source.family.len())?;
    }
    let policies = pattern
        .sources
        .iter()
        .map(|source| {
            let part = inventory
                .parts
                .get(source.root_part)
                .ok_or_else(|| Failure::new(FailCode::Type, "source policy root out of range"))?;
            Ok((part.revision, source.track, source.family.clone()))
        })
        .collect::<Result<_, Failure>>()?;
    let consumed_work = limits
        .max_nodes
        .checked_sub(*state.remaining)
        .ok_or_else(use_overflow)?;
    Ok(FrozenSourceUseCover {
        graph: pattern.source_uses.clone(),
        window,
        configuration_bound: bound,
        consumed_work,
        policies,
    })
}

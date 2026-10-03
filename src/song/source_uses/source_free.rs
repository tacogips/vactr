//! Structural admission for ordinary payloads without selected-source policies.
use super::{layout, use_invalid, use_overflow, Certification, FrozenPattern, FrozenUseMapping};
use crate::pattern::TimeSpan;
use crate::vm::fail::{FailCode, Failure};

enum Visit {
    Enter {
        index: u32,
        depth: u32,
        content: bool,
    },
    Finish {
        index: u32,
        depth: u32,
        content: bool,
    },
}

impl Certification<'_> {
    /// Ordinary captured families own zero/one configurations. Selected payloads
    /// retain the full timing certification and its addressed diagnostics.
    pub(super) fn payload_bound(
        &mut self,
        pattern: &FrozenPattern,
        window: TimeSpan,
        depth: u32,
    ) -> Result<u64, Failure> {
        if pattern.sources.is_empty() {
            self.validate_source_free(pattern, depth)?;
            self.admit(pattern.families.len())?;
            Ok(u64::from(!pattern.families.is_empty()))
        } else {
            self.node(pattern, pattern.source_uses.root, window, depth)
        }
    }

    fn validate_source_free(&mut self, pattern: &FrozenPattern, depth: u32) -> Result<(), Failure> {
        let graph = &pattern.source_uses;
        if graph.nodes.is_empty() {
            self.enter(depth)?;
            return if graph.root == 0 {
                Ok(())
            } else {
                Err(use_invalid("invalid empty source-free root"))
            };
        }
        let identity = pattern as *const FrozenPattern as usize;
        self.admit(1)?;
        let mut pending = vec![Visit::Enter {
            index: graph.root,
            depth,
            content: true,
        }];
        let mut active = std::collections::BTreeSet::new();
        while let Some(visit) = pending.pop() {
            match visit {
                Visit::Enter {
                    index,
                    depth: current,
                    content,
                } => {
                    self.enter(current)?;
                    let key = (identity, index, content);
                    if let Some(height) = self.source_free_cache.get(&key) {
                        self.check_source_free_height(current, *height)?;
                        continue;
                    }
                    let node = graph
                        .nodes
                        .get(index as usize)
                        .ok_or_else(|| use_invalid("source-free node out of range"))?;
                    self.admit(1)?;
                    if !active.insert(index) {
                        return Err(use_invalid("cyclic source-free recipe"));
                    }
                    super::conditional::validate(node)?;
                    super::sampling::validate(node)?;
                    match &node.mapping {
                        FrozenUseMapping::Uncertifiable(reason)
                            if !ordinary_reason_allowed(*reason, content) =>
                        {
                            return Err(Failure::new(
                                FailCode::BeyondCapability,
                                format!(
                                    "uncertifiable source use: {:?} {:?}",
                                    node.operation, reason
                                ),
                            ));
                        }
                        FrozenUseMapping::Source { .. } => {
                            return Err(use_invalid("source-free payload has a selected Source"));
                        }
                        FrozenUseMapping::Empty if !node.edges.is_empty() => {
                            return Err(use_invalid("empty source-free node has edges"));
                        }
                        FrozenUseMapping::SelectContent { content }
                        | FrozenUseMapping::SampleGrid { content, .. }
                        | FrozenUseMapping::SampleCycles { content }
                            if *content as usize >= node.edges.len() =>
                        {
                            return Err(use_invalid("source-free content edge out of range"));
                        }
                        FrozenUseMapping::Restructure { timing, content }
                            if *timing as usize >= node.edges.len()
                                || *content as usize >= node.edges.len() =>
                        {
                            return Err(use_invalid("source-free structure edge out of range"));
                        }
                        FrozenUseMapping::Slices { starts, .. } => {
                            self.admit(starts.len())?;
                            if starts.is_empty()
                                || starts.iter().any(|v| {
                                    *v < crate::value::Ratio64::ZERO
                                        || *v > crate::value::Ratio64::ONE
                                })
                                || starts.windows(2).any(|v| v[0] >= v[1])
                            {
                                return Err(use_invalid("invalid source-free slice points"));
                            }
                        }
                        _ => {}
                    }
                    // Admit all pending children before growing the work stack.
                    self.admit(node.edges.len().checked_add(1).ok_or_else(use_overflow)?)?;
                    for edge in &node.edges {
                        self.admit(
                            edge.trace
                                .len()
                                .checked_add(edge.layout.len())
                                .ok_or_else(use_overflow)?,
                        )?;
                        if edge.child as usize >= graph.nodes.len() {
                            return Err(use_invalid("source-free edge out of range"));
                        }
                        layout::validate_layout(edge)?;
                        for term in &edge.trace {
                            if matches!(term, super::FrozenUseTraceTerm::Copies { count, .. } if *count > 4096)
                            {
                                return Err(use_invalid("source-free copy bound out of range"));
                            }
                        }
                    }
                    pending.push(Visit::Finish {
                        index,
                        depth: current,
                        content,
                    });
                    if !node.edges.is_empty() {
                        let child_depth = current.checked_add(1).ok_or_else(use_overflow)?;
                        for (ordinal, edge) in node.edges.iter().enumerate().rev() {
                            pending.push(Visit::Enter {
                                index: edge.child,
                                depth: child_depth,
                                content: child_content(&node.mapping, content, ordinal),
                            });
                        }
                    }
                }
                Visit::Finish {
                    index,
                    depth: current,
                    content,
                } => {
                    let node = &graph.nodes[index as usize];
                    let mut height = 0u32;
                    for (ordinal, edge) in node.edges.iter().enumerate() {
                        let child = self
                            .source_free_cache
                            .get(&(
                                identity,
                                edge.child,
                                child_content(&node.mapping, content, ordinal),
                            ))
                            .ok_or_else(|| use_invalid("missing source-free child height"))?;
                        height = height.max(child.checked_add(1).ok_or_else(use_overflow)?);
                    }
                    self.check_source_free_height(current, height)?;
                    self.admit(1)?;
                    self.source_free_cache
                        .insert((identity, index, content), height);
                    active.remove(&index);
                }
            }
        }
        Ok(())
    }

    fn check_source_free_height(&self, depth: u32, height: u32) -> Result<(), Failure> {
        if depth.checked_add(height).ok_or_else(use_overflow)? >= self.limits.max_depth {
            Err(Failure::new(
                FailCode::DepthExceeded,
                "source-free cached depth exhausted",
            ))
        } else {
            Ok(())
        }
    }
}

/// Structure remains reachable in both roles; only unknown ordinary timing
/// sources may omit selected-source capability certification.
fn child_content(mapping: &FrozenUseMapping, inherited: bool, ordinal: usize) -> bool {
    inherited
        && match mapping {
            FrozenUseMapping::SelectContent { content }
            | FrozenUseMapping::Restructure { content, .. }
            | FrozenUseMapping::SampleGrid { content, .. }
            | FrozenUseMapping::SampleCycles { content } => ordinal == *content as usize,
            FrozenUseMapping::Slices { .. } => ordinal == 0,
            _ => true,
        }
}

fn ordinary_reason_allowed(reason: super::FrozenUseReason, content: bool) -> bool {
    use super::FrozenUseReason as R;
    match reason {
        R::DynamicRate | R::DynamicCount | R::DynamicTiming => true,
        R::DynamicSource => !content,
        R::UncertifiedCallback | R::UnclosedResource | R::UnsupportedLiveInput => false,
    }
}

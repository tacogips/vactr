//! Nested certificate preparation preserving the original policy and ledger.
use super::*;
pub(in crate::song::routing) fn prepare_nested_covers(
    inventory: &FrozenRoutingInventory,
    active: &[SongSourceRouteCover],
    remaining: &mut u32,
) -> Result<Vec<SongNestedSourceRouteCover>, Failure> {
    let limits = crate::song::SongLimits {
        max_nodes: *remaining,
        ..Default::default()
    };
    prepare_nested_covers_metered(inventory, active, limits, remaining, 0)
}
pub(in crate::song::routing) fn prepare_nested_covers_metered(
    inventory: &FrozenRoutingInventory,
    active: &[SongSourceRouteCover],
    limits: crate::song::SongLimits,
    remaining: &mut u32,
    depth: u32,
) -> Result<Vec<SongNestedSourceRouteCover>, Failure> {
    use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
    limits.validate()?;
    if depth >= limits.max_depth {
        return Err(Failure::new(
            crate::vm::fail::FailCode::DepthExceeded,
            "nested preparation entry depth exhausted",
        ));
    }
    if *remaining > limits.max_nodes {
        return Err(invalid("nested certification exceeds original work policy"));
    }
    let mut policy = limits;
    policy.max_nodes = *remaining;
    let mut budget = ResolutionBudget::new(policy);
    let result = (|| {
        let mut roots = std::collections::BTreeSet::new();
        for certificate in active {
            register_source_roots(
                scope_payload(inventory, certificate.scope_part, certificate.track)?,
                &mut roots,
                &mut budget,
            )?;
        }
        let mut covers = Vec::new();
        let mut seen_roots = std::collections::BTreeSet::new();
        while let Some((root, track)) = roots.pop_first() {
            budget.charge(1)?;
            if !seen_roots.insert((root, track)) {
                continue;
            }
            inventory
                .parts
                .get(root)
                .ok_or_else(|| invalid("nested certificate root"))?;
            budget.charge(1)?;
            let mut pending = vec![(root, depth.checked_add(1).ok_or_else(capacity_overflow)?)];
            let mut visited = std::collections::BTreeSet::new();
            while let Some((index, depth)) = pending.pop() {
                budget.enter(depth)?;
                if !visited.insert((index, depth)) {
                    continue;
                }
                let part = inventory
                    .parts
                    .get(index)
                    .ok_or_else(|| invalid("nested certificate scope"))?;
                let mut certify =
                    |payload: &crate::song::snapshot::FrozenPattern| -> Result<(), Failure> {
                        if payload.sources.is_empty() {
                            return Ok(());
                        }
                        let window = crate::pattern::TimeSpan::new(Ratio64::ZERO, part.duration)?;
                        #[cfg(test)]
                        let before = budget.limits().max_nodes;
                        let result = budget.with_remaining(|remaining| {
                            crate::song::source_uses::certify_source_uses_metered(
                                inventory, payload, window, limits, remaining, depth,
                            )
                        });
                        #[cfg(test)]
                        CHILD_TRACE.with(|trace| {
                            trace.borrow_mut().push(ChildTrace {
                                scope: index,
                                track,
                                payload: payload.id,
                                window,
                                depth,
                                before,
                                after: budget.limits().max_nodes,
                                failure: result.as_ref().err().map(|failure| failure.code),
                            })
                        });
                        let cover = result?;
                        register_source_roots(payload, &mut roots, &mut budget)?;
                        budget.charge(1)?;
                        covers.push(SongNestedSourceRouteCover {
                            root_part: root,
                            scope_part: index,
                            track,
                            cover,
                        });
                        Ok(())
                    };
                let child_depth = depth.checked_add(1).ok_or_else(capacity_overflow)?;
                match &part.node {
                    P::Capture(entries) => {
                        for (name, payload) in entries {
                            if *name == track {
                                certify(payload)?;
                            }
                        }
                    }
                    P::Sequence(children) => {
                        budget.charge(children.len())?;
                        for (_, child) in children {
                            pending.push((*child, child_depth));
                        }
                    }
                    P::Repeat { child, .. } => {
                        budget.charge(1)?;
                        pending.push((*child, child_depth));
                    }
                    P::Edit { source, edit } => {
                        match edit {
                            E::Replace {
                                track: name,
                                payload,
                            }
                            | E::Transform {
                                track: name,
                                payload,
                                ..
                            }
                            | E::Overwrite {
                                track: name,
                                payload,
                                ..
                            } if *name == track => certify(payload)?,
                            _ => {}
                        }
                        let replaced = matches!(edit,E::Replace{track:name,..} if *name==track)
                            || matches!(edit,E::Overwrite{track:name,region,..} if *name==track && region.begin==Ratio64::ZERO && region.end>=part.duration);
                        if !replaced {
                            budget.charge(1)?;
                            pending.push((*source, child_depth));
                        }
                    }
                }
            }
        }
        Ok(covers)
    })();
    *remaining = budget.limits().max_nodes;
    result
}

#[cfg(test)]
mod meter_tests;

#[cfg(test)]
struct ChildTrace {
    scope: usize,
    track: crate::value::intern::KwId,
    payload: crate::reader::span::NodeId,
    window: crate::pattern::TimeSpan,
    depth: u32,
    before: u32,
    after: u32,
    failure: Option<crate::vm::fail::FailCode>,
}
#[cfg(test)]
std::thread_local! {
    static CHILD_TRACE: std::cell::RefCell<Vec<ChildTrace>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[cfg(test)]
fn take_child_trace() -> Vec<ChildTrace> {
    CHILD_TRACE.with(|trace| std::mem::take(&mut *trace.borrow_mut()))
}

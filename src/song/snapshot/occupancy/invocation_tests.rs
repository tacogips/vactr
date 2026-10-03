//! Actual q-entry keys and invocation-specific immutable bindings.
use super::{retain_index_occupancy, tests::from_code, CanonicalIndexRequest};
use crate::ns::namespace::VarSlotRef;
use crate::pattern::eval::song_observation::OwnerInvocation;
use crate::pattern::TimeSpan;
use crate::song::{source_uses::FrozenUseOperation, SongLimits, SongSnapshot};
use crate::value::{
    intern::{intern_kw, intern_sym},
    Ratio64,
};
use crate::vm::{
    fail::{FailCode, Failure},
    vm::ReadObserver,
};
use std::{cell::Cell, rc::Rc};

struct Reads {
    count: Rc<Cell<u32>>,
    deny: bool,
}
impl ReadObserver for Reads {
    fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
        if slot.name() == intern_sym("cut") {
            self.count.set(self.count.get() + 1);
            if self.deny {
                return Err(Failure::new(
                    FailCode::HostUnavailable,
                    "original callback must not be reread",
                ));
            }
        }
        Ok(())
    }
}
fn watch(snapshot: &mut SongSnapshot, deny: bool) -> Rc<Cell<u32>> {
    let count = Rc::new(Cell::new(0));
    snapshot
        .evaluator
        .vm_and_ns()
        .0
        .set_read_observer(Some(Box::new(Reads {
            count: count.clone(),
            deny,
        })));
    count
}
fn window(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
}
/// Find an actual site in the latest retained output owner, never forge a record.
fn issue(
    snapshot: &SongSnapshot,
    window: TimeSpan,
    depth: u32,
    remaining: &mut u32,
) -> CanonicalIndexRequest {
    let (scope, payload) = snapshot
        .routing
        .parts
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, part)| match &part.node {
            crate::song::snapshot::FrozenPartNode::Edit {
                edit: crate::song::snapshot::FrozenEdit::Transform { payload, .. },
                ..
            } => Some((i, payload)),
            _ => None,
        })
        .unwrap();
    let recipe = payload.index_timing().unwrap();
    let mut pending = vec![(recipe.root(), Vec::new())];
    while let Some((index, prefix)) = pending.pop() {
        let node = &recipe.nodes()[index as usize];
        if node.operation() == FrozenUseOperation::Slice {
            return snapshot
                .canonical_index_request(
                    scope,
                    intern_kw("drums"),
                    node.issuer(),
                    &prefix,
                    window,
                    depth,
                    SongLimits::default(),
                    remaining,
                )
                .unwrap();
        }
        for edge in node.children() {
            let mut next = prefix.clone();
            next.extend_from_slice(edge.trace());
            pending.push((edge.child(), next));
        }
    }
    panic!("genuine frozen Slice address")
}
fn program(empty: bool, intro: bool) -> String {
    let inner = if empty {
        "grid p [false]"
    } else {
        "slice {beat -> p} 2 [cut nil]"
    };
    let ending = if intro {
        "let intro {part [drums: nil] duration: 1/2}\nsong {sequence [intro selected]} tail-seconds: 0 > play-song"
    } else {
        "song selected tail-seconds: 0 > play-song"
    };
    format!("fn cut beat:\n\tfirst [0]\nfn innered p:\n\t{inner}\nfn indexed p:\n\tstack [{{grid {{beat -> p}} [true true]}} {{slice {{beat -> nil}} 2 [cut nil]}}]\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 1}}\nlet inner {{transform-instrument base :drums :analog innered}}\nlet selected {{transform-instrument inner :drums :analog indexed}}\n{ending}")
}
fn linked(snapshot: &SongSnapshot) -> Vec<&Rc<OwnerInvocation>> {
    // The latest original Transform selects the immediately preceding inner Edit.
    // Authenticate that frozen source revision and payload root, not any source link.
    let source = snapshot
        .routing
        .parts
        .iter()
        .rev()
        .find_map(|part| match &part.node {
            crate::song::snapshot::FrozenPartNode::Edit {
                source,
                edit: crate::song::snapshot::FrozenEdit::Transform { .. },
            } => Some(*source),
            _ => None,
        })
        .unwrap();
    let inner = &snapshot.routing.parts[source];
    let root = match &inner.node {
        crate::song::snapshot::FrozenPartNode::Edit {
            edit: crate::song::snapshot::FrozenEdit::Transform { payload, .. },
            ..
        } => payload.id,
        _ => panic!("original inner Transform source"),
    };
    snapshot
        .occupancy
        .iter()
        .flat_map(|record| &record.invocations)
        .filter(|invocation| {
            invocation.owner().revision == inner.revision
                && invocation.owner().root == root
                && invocation.owner().track == intern_kw("drums")
                && invocation.owner().duration == inner.duration
                && !invocation.clock().source_evidence().is_empty()
        })
        .collect()
}
#[test]
fn two_actual_parent_bindings_share_original_inner_execution_and_keep_empty_membership() {
    let mut prepared = from_code(&program(false, false));
    let snapshot = &mut prepared.snapshot;
    let reads = watch(snapshot, false);
    let mut remaining = SongLimits::default().max_nodes;
    let request = issue(snapshot, TimeSpan::cycle(0).unwrap(), 0, &mut remaining);
    retain_index_occupancy(
        snapshot,
        vec![request],
        SongLimits::default(),
        &mut remaining,
    )
    .unwrap();
    assert!(reads.get() > 0, "actual VM source callback execution");
    let invocations = linked(snapshot);
    let first = invocations
        .iter()
        .find(|inv| inv.clock().source_evidence()[0].0 == TimeSpan::point(Ratio64::ZERO))
        .unwrap();
    let second = invocations
        .iter()
        .find(|inv| {
            inv.owner() == first.owner()
                && inv.clock().source_evidence()[0].0
                    == TimeSpan::point(Ratio64::new(1, 2).unwrap())
        })
        .unwrap();
    assert_eq!(first.owner(), second.owner());
    assert_eq!(first.entry(), second.entry());
    assert_eq!(first.seed(), second.seed());
    assert!(
        Rc::ptr_eq(first.execution(), second.execution()),
        "same genuine original raw q"
    );
    let a = first.clock().source_evidence();
    let b = second.clock().source_evidence();
    assert_eq!(a[0].3.len(), 2);
    assert!(b[0].3.is_empty());
    assert_eq!(a[0].2.sampling_evidence()[0].0, Some(window(0, 1, 2)));
    assert_eq!(b[0].2.sampling_evidence()[0].0, Some(window(1, 2, 2)));
    assert!(!first.observations().is_empty());
    assert!(!second.observations().is_empty());
    let denied = watch(snapshot, true);
    let _ = snapshot
        .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
        .unwrap();
    assert_eq!(denied.get(), 0);
}
#[test]
fn empty_success_without_index_rows_retains_actual_entry_boundary() {
    let mut prepared = from_code(&program(true, false));
    let snapshot = &mut prepared.snapshot;
    let mut remaining = SongLimits::default().max_nodes;
    let request = issue(snapshot, TimeSpan::cycle(0).unwrap(), 0, &mut remaining);
    retain_index_occupancy(
        snapshot,
        vec![request],
        SongLimits::default(),
        &mut remaining,
    )
    .unwrap();
    let invocations = linked(snapshot);
    let first = invocations
        .iter()
        .find(|inv| inv.clock().source_evidence()[0].0 == TimeSpan::point(Ratio64::ZERO))
        .unwrap();
    let second = invocations
        .iter()
        .find(|inv| {
            inv.owner() == first.owner()
                && inv.clock().source_evidence()[0].0
                    == TimeSpan::point(Ratio64::new(1, 2).unwrap())
        })
        .unwrap();
    assert!(first.observations().is_empty());
    assert!(second.observations().is_empty());
    assert_eq!(first.execution().output_len(), 0);
    assert_eq!(second.execution().output_len(), 0);
    assert!(Rc::ptr_eq(first.execution(), second.execution()));
    assert!(first.clock().source_evidence()[0].3.is_empty());
    assert!(second.clock().source_evidence()[0].3.is_empty());
    assert_ne!(
        first.clock().source_evidence()[0].2.sampling_evidence(),
        second.clock().source_evidence()[0].2.sampling_evidence()
    );
}
fn same_source_entry(a: &OwnerInvocation, b: &OwnerInvocation, remaining: &mut u32) -> bool {
    fn compare(
        a: &crate::pattern::eval::song_clock::CanonicalClockProjection,
        b: &crate::pattern::eval::song_clock::CanonicalClockProjection,
        depth: u32,
        remaining: &mut u32,
    ) -> bool {
        assert!(depth < SongLimits::default().max_depth);
        if !a.same_frame_basis(b) {
            return false;
        }
        let a = a.source_evidence();
        let b = b.source_evidence();
        a.len() == b.len()
            && a.iter().zip(&b).all(|(a, b)| {
                a.0 == b.0
                    && a.1 == b.1
                    && compare(&a.2, &b.2, depth + 1, remaining)
                    && a.3.len() == b.3.len()
                    && a.3.iter().zip(&b.3).all(|(a, b)| {
                        let max_depth = SongLimits::default().max_depth - depth;
                        let old =
                            crate::song::source::copy_origin(a, remaining, max_depth).unwrap();
                        let new =
                            crate::song::source::copy_origin(b, remaining, max_depth).unwrap();
                        old == new && a.route == b.route && a.commit_mode == b.commit_mode
                    })
            })
    }
    compare(a.clock(), b.clock(), 0, remaining)
}
fn authentic_fresh_sources(a: &OwnerInvocation, b: &OwnerInvocation, remaining: &mut u32) {
    use crate::pattern::eval::song_clock::{CanonicalClockProjection, ProjectionBudget};
    fn fresh(a: &CanonicalClockProjection, b: &CanonicalClockProjection, depth: u32) {
        assert!(depth < SongLimits::default().max_depth);
        for (a, b) in a.source_evidence().iter().zip(b.source_evidence()) {
            assert!(
                !Rc::ptr_eq(&a.4, &b.4),
                "actual issued boundary is freshly rebound"
            );
            for (a, b) in a.3.iter().zip(&b.3) {
                assert!(
                    !Rc::ptr_eq(a, b),
                    "actual sealed returned member is freshly rebound"
                );
                assert!(
                    !Rc::ptr_eq(
                        a.issued_leaves.as_ref().unwrap(),
                        b.issued_leaves.as_ref().unwrap()
                    ),
                    "member has fresh actual issued leaves"
                );
            }
            fresh(&a.2, &b.2, depth + 1);
        }
    }
    let limits = SongLimits {
        max_nodes: 1_000_000,
        ..SongLimits::default()
    };
    for invocation in [a, b] {
        let mut budget = ProjectionBudget::new(limits, remaining).unwrap();
        let sources = invocation
            .clock()
            .retained_sources(invocation.owner(), 0, &mut budget)
            .unwrap();
        let mut clock = invocation.clock().clone();
        for (depth, source) in sources.iter().enumerate() {
            let evidence = clock.source_evidence();
            assert_eq!(evidence.len(), 1);
            assert_eq!(source.request(), evidence[0].0);
            assert_eq!(*source.producer(), evidence[0].1);
            for member in &evidence[0].3 {
                let actual = source
                    .member(&member.handle, depth as u32, &mut budget)
                    .unwrap()
                    .unwrap();
                assert!(
                    std::ptr::eq(actual, &**member),
                    "exact returned Rc belongs to its authentic sealed source boundary"
                );
            }
            clock = evidence[0].2.clone();
        }
        assert!(clock.source_evidence().is_empty());
    }
    fresh(a.clock(), b.clock(), 0);
}
fn all_linked(record: &super::RetainedCanonicalIndex) -> Vec<&Rc<OwnerInvocation>> {
    record
        .invocations
        .iter()
        .filter(|inv| !inv.clock().source_evidence().is_empty())
        .collect()
}
#[test]
fn cached_outer_hit_copies_original_child_invocations_without_nested_q() {
    let mut prepared = from_code(&program(false, true));
    let snapshot = &mut prepared.snapshot;
    let mut remaining = SongLimits::default().max_nodes;
    let request = issue(snapshot, window(0, 1, 1), 0, &mut remaining);
    retain_index_occupancy(
        snapshot,
        vec![request],
        SongLimits::default(),
        &mut remaining,
    )
    .unwrap();
    let intended = linked(snapshot);
    assert!(!intended.is_empty(), "original inner owner represented");
    let old: Vec<_> = all_linked(&snapshot.occupancy[0])
        .into_iter()
        .cloned()
        .collect();
    assert!(intended
        .iter()
        .all(|inner| old.iter().any(|inv| Rc::ptr_eq(inner, inv))));
    assert!(
        old.len() > intended.len(),
        "genuine source grandchildren also retained"
    );
    let raw = snapshot.replay.clone().unwrap();
    let executions = raw.executions().len();
    let denied = watch(snapshot, true);
    let request = issue(snapshot, window(2, 3, 2), 0, &mut remaining);
    retain_index_occupancy(
        snapshot,
        vec![request],
        SongLimits::default(),
        &mut remaining,
    )
    .unwrap();
    assert_eq!(denied.get(), 0);
    assert_eq!(
        snapshot.replay.as_ref().unwrap().executions().len(),
        executions
    );
    let new = all_linked(snapshot.occupancy.last().unwrap());
    assert_eq!(
        new.len(),
        old.len(),
        "complete source-linked invocation graph"
    );
    let mut consumed = vec![false; new.len()];
    // Comparison has its own single configured allowance; capture keeps its original ledger.
    let mut comparison_remaining = 1_000_000;
    for prior in old {
        let index = new
            .iter()
            .enumerate()
            .find_map(|(index, inv)| {
                (!consumed[index]
                    && inv.owner() == prior.owner()
                    && inv.entry() == prior.entry()
                    && inv.seed() == prior.seed()
                    && inv.execution().entry_depth() == prior.execution().entry_depth()
                    && Rc::ptr_eq(inv.execution(), prior.execution())
                    && same_source_entry(inv, &prior, &mut comparison_remaining))
                .then_some(index)
            })
            .expect("every original source child has one authentic copied invocation");
        consumed[index] = true;
        let copied = new[index];
        assert!(Rc::ptr_eq(copied.execution(), prior.execution()));
        assert_eq!(copied.observations().len(), prior.observations().len());
        authentic_fresh_sources(&prior, copied, &mut comparison_remaining);
    }
    assert!(consumed.iter().all(|consumed| *consumed));
}
#[test]
fn repeat_modes_preserve_actual_seed_and_placement_keys() {
    for mode in ["same", "vary"] {
        let code = format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {{beat -> nil}} 2 [cut nil]\nlet base {{part [drums: {{s :analog}}] duration: 1}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong {{part-repeat selected 2 seed-mode: :{mode}}} tail-seconds: 0 > play-song");
        let mut prepared = from_code(&code);
        let snapshot = &mut prepared.snapshot;
        let mut remaining = SongLimits::default().max_nodes;
        let request = issue(snapshot, window(0, 2, 1), 0, &mut remaining);
        let revision = request.revision;
        retain_index_occupancy(
            snapshot,
            vec![request],
            SongLimits::default(),
            &mut remaining,
        )
        .unwrap();
        let inv: Vec<_> = snapshot.occupancy[0]
            .invocations
            .iter()
            .filter(|inv| inv.owner().revision == revision)
            .collect();
        assert_eq!(inv.len(), 2);
        assert_ne!(inv[0].owner().placement, inv[1].owner().placement);
        if mode == "same" {
            assert_eq!(inv[0].seed(), inv[1].seed());
        } else {
            assert_ne!(inv[0].seed(), inv[1].seed());
        }
        for invocation in inv {
            assert_eq!(invocation.owner(), invocation.execution().owner());
            assert_eq!(invocation.seed(), invocation.execution().seed());
        }
    }
}
fn simple(fault: bool) -> String {
    let body = if fault { "/ 0 {- 1 beat}" } else { "first [0]" };
    format!("fn cut beat:\n\t{body}\nfn indexed p:\n\tslice {{beat -> nil}} 2 [cut nil]\nlet base {{part [drums: {{s :analog}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong selected tail-seconds: 0 > play-song")
}
#[test]
fn complete_invocation_transaction_exact_work_one_less_and_depth_are_real() {
    let limits = SongLimits::default();
    let code = simple(false);
    let mut measured = from_code(&code);
    let mut remaining = limits.max_nodes;
    let request = issue(measured.snapshot(), window(0, 2, 1), 0, &mut remaining);
    retain_index_occupancy(
        &mut measured.snapshot,
        vec![request],
        limits,
        &mut remaining,
    )
    .unwrap();
    let required = limits.max_nodes - remaining;
    assert!(required > 1);
    for (budget, succeeds) in [(required, true), (required - 1, false)] {
        let mut fresh = from_code(&code);
        let mut remaining = budget;
        let request = issue(fresh.snapshot(), window(0, 2, 1), 0, &mut remaining);
        let result =
            retain_index_occupancy(&mut fresh.snapshot, vec![request], limits, &mut remaining);
        if succeeds {
            result.unwrap();
            assert_eq!(remaining, 0);
            assert!(!fresh.snapshot().occupancy[0].invocations.is_empty());
        } else {
            assert_eq!(result.unwrap_err().code, FailCode::FuelExhausted);
            assert!(fresh.snapshot().occupancy.is_empty());
            assert!(fresh.snapshot().replay.is_none());
        }
        assert!(fresh.snapshot.evaluator.vm_and_ns().0.song_work().is_none());
    }
    let mut fresh = from_code(&code);
    let mut remaining = limits.max_nodes;
    let request = issue(
        fresh.snapshot(),
        TimeSpan::cycle(0).unwrap(),
        0,
        &mut remaining,
    );
    retain_index_occupancy(&mut fresh.snapshot, vec![request], limits, &mut remaining).unwrap();
    let previous = fresh.snapshot().replay.clone().unwrap();
    let prior = fresh.snapshot().occupancy[0].invocations[0].clone();
    let request = issue(
        fresh.snapshot(),
        TimeSpan::cycle(1).unwrap(),
        limits.max_depth,
        &mut remaining,
    );
    assert_eq!(
        retain_index_occupancy(&mut fresh.snapshot, vec![request], limits, &mut remaining)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
    assert!(Rc::ptr_eq(
        fresh.snapshot().replay.as_ref().unwrap(),
        &previous
    ));
    assert!(Rc::ptr_eq(
        &fresh.snapshot().occupancy[0].invocations[0],
        &prior
    ));
    assert_eq!(fresh.snapshot().occupancy.len(), 1);
}
#[test]
fn fault_and_foreign_equal_node_ids_cannot_publish_invocations() {
    let limits = SongLimits::default();
    let code = simple(true);
    let mut prepared = from_code(&code);
    let snapshot = &mut prepared.snapshot;
    let mut remaining = limits.max_nodes;
    let request = issue(snapshot, TimeSpan::cycle(0).unwrap(), 0, &mut remaining);
    retain_index_occupancy(snapshot, vec![request], limits, &mut remaining).unwrap();
    let previous = snapshot.replay.clone().unwrap();
    let prior = snapshot.occupancy[0].invocations[0].clone();
    let request = issue(snapshot, TimeSpan::cycle(1).unwrap(), 0, &mut remaining);
    assert_eq!(
        retain_index_occupancy(snapshot, vec![request], limits, &mut remaining)
            .unwrap_err()
            .code,
        FailCode::DivisionByZero
    );
    assert!(Rc::ptr_eq(snapshot.replay.as_ref().unwrap(), &previous));
    assert!(Rc::ptr_eq(&snapshot.occupancy[0].invocations[0], &prior));
    assert_eq!(snapshot.occupancy.len(), 1);
    assert!(snapshot.evaluator.vm_and_ns().0.song_work().is_none());
    let mut foreign = from_code(&code);
    let mut fresh_work = limits.max_nodes;
    let foreign_request = issue(
        foreign.snapshot(),
        TimeSpan::cycle(0).unwrap(),
        0,
        &mut fresh_work,
    );
    assert_eq!(
        foreign_request.root, snapshot.occupancy[0].request.root,
        "same static NodeId does not supply original authority"
    );
    assert_eq!(
        retain_index_occupancy(snapshot, vec![foreign_request], limits, &mut fresh_work)
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert!(foreign
        .snapshot
        .evaluator
        .vm_and_ns()
        .0
        .song_work()
        .is_none());
    assert!(Rc::ptr_eq(snapshot.replay.as_ref().unwrap(), &previous));
    assert!(Rc::ptr_eq(&snapshot.occupancy[0].invocations[0], &prior));
}

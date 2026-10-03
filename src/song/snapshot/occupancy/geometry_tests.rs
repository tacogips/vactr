//! Original evaluator/freeze tests of the actual immutable configuration consumer.
use super::{lookup::*, tests::from_code, *};
use crate::ns::namespace::VarSlotRef;
use crate::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenPattern, FrozenSelectedSource};
use crate::song::source_uses::{FrozenSliceTiming, FrozenUseOperation};
use crate::value::{
    intern::{intern_kw, intern_sym},
    Ratio64,
};
use crate::vm::vm::ReadObserver;
use std::cell::Cell;
struct Denied(Rc<Cell<u32>>);
impl ReadObserver for Denied {
    fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
        if slot.name() == intern_sym("cut") {
            self.0.set(self.0.get() + 1);
            return Err(Failure::new(
                FailCode::HostUnavailable,
                "geometry cannot execute cut",
            ));
        }
        Ok(())
    }
}
fn deny(snapshot: &mut SongSnapshot) -> Rc<Cell<u32>> {
    let reads = Rc::new(Cell::new(0));
    snapshot
        .evaluator
        .vm_and_ns()
        .0
        .set_read_observer(Some(Box::new(Denied(reads.clone()))));
    reads
}
fn span(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
}
fn code(body: &str) -> String {
    format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong selected tail-seconds: 0 > play-song")
}
fn payload(snapshot: &SongSnapshot, scope: usize) -> &FrozenPattern {
    match &snapshot.routing.parts[scope].node {
        FrozenPartNode::Edit {
            edit: FrozenEdit::Transform { payload, .. },
            ..
        } => payload,
        _ => panic!("actual Transform"),
    }
}
fn path(snapshot: &SongSnapshot, scope: usize) -> (NodeId, Vec<FrozenUseTraceTerm>) {
    let recipe = payload(snapshot, scope).index_timing().unwrap();
    let mut todo = vec![(recipe.root(), Vec::new())];
    while let Some((index, path)) = todo.pop() {
        let node = &recipe.nodes()[index as usize];
        if node.operation() == FrozenUseOperation::Slice {
            return (node.issuer(), path);
        }
        for edge in node.children() {
            let mut next = path.clone();
            next.extend_from_slice(edge.trace());
            todo.push((edge.child(), next));
        }
    }
    panic!("actual Slice site")
}
fn request(
    snapshot: &SongSnapshot,
    scope: usize,
    window: TimeSpan,
    limits: SongLimits,
    left: &mut u32,
) -> CanonicalIndexRequest {
    let (issuer, path) = path(snapshot, scope);
    snapshot
        .canonical_index_request(
            scope,
            intern_kw("drums"),
            issuer,
            &path,
            window,
            0,
            limits,
            left,
        )
        .unwrap()
}
fn retain(
    snapshot: &mut SongSnapshot,
    scope: usize,
    window: TimeSpan,
    limits: SongLimits,
    left: &mut u32,
) {
    let request = request(snapshot, scope, window, limits, left);
    retain_index_occupancy(snapshot, vec![request], limits, left).unwrap();
}
type GeometryResult = (Vec<Vec<TimeSpan>>, Option<bool>, Option<TimeSpan>);
#[allow(clippy::too_many_arguments)]
fn consume(
    snapshot: &SongSnapshot,
    scope: usize,
    window: TimeSpan,
    index: &RetainedIndexAddress<'_>,
    selected: Option<&FrozenSelectedSource>,
    timing: Option<&FrozenSliceTiming>,
    source: TimeSpan,
    owner: TimeSpan,
    depth: u32,
    limits: SongLimits,
    left: &mut u32,
) -> Result<GeometryResult, Failure> {
    let (issuer, path) = path(snapshot, scope);
    snapshot.retained_geometry_fixture(
        (scope, intern_kw("drums"), issuer, &path, window),
        index,
        selected,
        timing,
        source,
        owner,
        depth,
        limits,
        left,
    )
}
#[test]
fn prepared_suppressed_and_empty_success_are_distinct_without_subject_handles() {
    for (index_pattern, empty) in [("[cut nil]", false), ("[nil]", true)] {
        let mut prepared = from_code(&code(&format!("slice {{beat -> nil}} 2 {index_pattern}")));
        let snapshot = &mut prepared.snapshot;
        let scope = snapshot.routing.root_part;
        let limits = SongLimits::default();
        let mut left = limits.max_nodes;
        retain(snapshot, scope, span(0, 1, 1), limits, &mut left);
        let reads = deny(snapshot);
        let request = request(snapshot, scope, span(0, 1, 1), limits, &mut left);
        let addresses = owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap();
        assert!(!addresses.is_empty());
        let mut components = 0;
        for address in addresses {
            if address.owner().revision != request.revision {
                continue;
            }
            let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
            let (groups, membership, bound) = consume(
                snapshot,
                scope,
                span(0, 1, 1),
                &index,
                None,
                None,
                span(0, 1, 1),
                span(0, 1, 1),
                0,
                limits,
                &mut left,
            )
            .unwrap();
            assert_eq!(membership, None);
            assert_eq!(bound, None);
            components += groups.iter().map(Vec::len).sum::<usize>();
        }
        assert_eq!(components == 0, empty);
        assert_eq!(reads.get(), 0);
    }
}
#[test]
fn genuine_bound_chord_uses_original_timing_and_owner_clip_after_union() {
    let mut prepared = from_code(&code("slice {beat -> p} 2 [cut]"));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 32768,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 1, 1), limits, &mut left);
    let events = snapshot.query(span(0, 1, 1), &limits).unwrap();
    assert!(events.iter().any(|event| event.note.is_some()));
    let timing = events
        .iter()
        .flat_map(|event| event.source_origin.as_ref())
        .flat_map(|origin| origin.slice_timings())
        .find(|timing| timing.issuer() == path(snapshot, scope).0)
        .unwrap();
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 1, 1), limits, &mut left);
    let address = owner_addresses(snapshot, &request, 0, limits, &mut left)
        .unwrap()
        .into_iter()
        .find(|address| address.owner().revision == request.revision)
        .unwrap();
    let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
    let (groups, membership, bound) = consume(
        snapshot,
        scope,
        span(0, 1, 1),
        &index,
        None,
        Some(timing),
        span(0, 1, 1),
        span(1, 3, 4),
        0,
        limits,
        &mut left,
    )
    .unwrap();
    assert_eq!(membership, None);
    assert_eq!(bound, Some(span(1, 3, 4)));
    assert!(groups.iter().any(|group| group == &vec![span(1, 3, 4)]));
    assert_eq!(reads.get(), 0);
}
#[test]
fn real_geometry_exact_one_less_and_foreign_original_preserve_view() {
    let mut prepared = from_code(&code("slice {beat -> nil} 2 [cut nil]"));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits::default();
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 1, 1), limits, &mut left);
    let view = snapshot.replay.clone().unwrap();
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 1, 1), limits, &mut left);
    let address = owner_addresses(snapshot, &request, 0, limits, &mut left)
        .unwrap()
        .into_iter()
        .find(|address| address.owner().revision == request.revision)
        .unwrap();
    let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
    let run = |left: &mut u32, depth| {
        consume(
            snapshot,
            scope,
            span(0, 1, 1),
            &index,
            None,
            None,
            span(0, 1, 1),
            span(0, 1, 1),
            depth,
            limits,
            left,
        )
    };
    let mut measured = limits.max_nodes;
    let expected = run(&mut measured, 0).unwrap();
    let needed = limits.max_nodes - measured;
    let mut exact = needed;
    assert_eq!(run(&mut exact, 0).unwrap(), expected);
    assert_eq!(exact, 0);
    let mut short = needed - 1;
    assert_eq!(
        run(&mut short, 0).unwrap_err().code,
        FailCode::FuelExhausted
    );
    let mut depth = limits.max_nodes;
    assert_eq!(
        run(&mut depth, limits.max_depth + 1).unwrap_err().code,
        FailCode::DepthExceeded
    );
    let foreign = from_code(&code("slice {beat -> nil} 2 [cut nil]"));
    let mut fresh = limits.max_nodes;
    assert_eq!(
        consume(
            &foreign.snapshot,
            foreign.snapshot.routing.root_part,
            span(0, 1, 1),
            &index,
            None,
            None,
            span(0, 1, 1),
            span(0, 1, 1),
            0,
            limits,
            &mut fresh
        )
        .unwrap_err()
        .code,
        FailCode::Type
    );
    assert!(Rc::ptr_eq(&view, snapshot.replay.as_ref().unwrap()));
    assert_eq!(reads.get(), 0);
}

fn nested(inner: &str, outer: &str, empty_subject: bool) -> String {
    let subject = if empty_subject { "nil" } else { "p" };
    format!("fn cut beat:\n\tfirst [0]\nfn innered p:\n\t{inner}\nfn indexed p:\n\tstack [{{{outer}}} {{slice {{beat -> nil}} 2 [cut nil]}}]\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 1}}\nlet inner {{transform-instrument base :drums :analog innered}}\nlet selected {{transform-instrument inner :drums :analog indexed}}\nsong selected tail-seconds: 0 > play-song").replace("SUBJECT", subject)
}
#[test]
fn real_point_source_requests_keep_domain_rev_and_sample_eligibility_separate() {
    for outer_expression in [
        "fast {beat -> p} 1",
        "rev {beat -> p}",
        "grid {beat -> p} [true true]",
        "slice {beat -> p} 2 [cut nil]",
    ] {
        let mut prepared = from_code(&nested(
            "rev {slice {beat -> SUBJECT} 2 [cut]}",
            outer_expression,
            false,
        ));
        let snapshot = &mut prepared.snapshot;
        let outer = snapshot.routing.root_part;
        let FrozenPartNode::Edit { source: inner, .. } = snapshot.routing.parts[outer].node else {
            panic!("genuine edit ancestry")
        };
        let limits = SongLimits {
            max_nodes: 100_000,
            ..SongLimits::default()
        };
        let mut left = limits.max_nodes;
        retain(snapshot, outer, span(0, 1, 1), limits, &mut left);
        let reads = deny(snapshot);
        let request = request(snapshot, inner, span(0, 1, 1), limits, &mut left);
        let selected = &payload(snapshot, outer).sources[0];
        let mut genuine = 0;
        for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
            if address.owner().revision != request.revision
                || address.owner().root != request.root
                || address.owner().track != request.track
            {
                continue;
            }
            let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
            let boundaries = index.source_boundaries(0, limits, &mut left).unwrap();
            let Some(boundary) = boundaries.first() else {
                continue;
            };
            if boundary.policy_parent().unwrap().revision != snapshot.routing.parts[outer].revision
            {
                continue;
            }
            let query_piece = boundary.request();
            if outer_expression.starts_with("fast") || outer_expression.starts_with("rev") {
                assert_eq!(query_piece, span(0, 1, 1),
                    "direct selected-source traversal keeps its actual finite query: {outer_expression}");
            } else {
                assert!(query_piece.is_point(),
                    "actual structural sampling must issue a point query: {outer_expression}, {query_piece:?}");
            }
            let rows = index.rows(0, limits, &mut left).unwrap();
            let mut endpoints = 0;
            for row in rows
                .iter()
                .filter(|row| row.issuer_start() == Ratio64::ZERO)
            {
                use crate::pattern::eval::song_clock::ClockOrientation::{At, Before};
                let expected = if outer_expression.starts_with("fast") {
                    Before
                } else {
                    At
                };
                assert_eq!(
                    row.footprint().orientation,
                    expected,
                    "actual parent: {outer_expression}"
                );
                for (domain, query, start, orientation, outgoing, next_orientation) in
                    row.source_hops()
                {
                    assert_eq!(domain, span(0, 1, 1));
                    assert_eq!(query, query_piece);
                    assert_eq!((start, orientation), (Ratio64::ONE, Before));
                    assert_eq!(next_orientation, expected);
                    if outer_expression.starts_with("fast") {
                        assert_eq!(outgoing, Ratio64::ONE);
                    }
                    if outer_expression.starts_with("rev") {
                        assert_eq!(outgoing, Ratio64::ZERO);
                    }
                    assert!(row.source_eligible(0, limits, &mut left).unwrap());
                    endpoints += 1;
                }
            }
            assert!(endpoints > 0);
            let (groups, membership, _) = consume(
                snapshot,
                inner,
                span(0, 1, 1),
                &index,
                Some(selected),
                None,
                span(0, 1, 1),
                span(0, 1, 1),
                0,
                limits,
                &mut left,
            )
            .unwrap();
            assert_eq!(
                membership,
                Some(false),
                "positive original inner source: case={outer_expression}, query={query_piece:?}"
            );
            assert!(
                groups.iter().any(|group| !group.is_empty()),
                "point request is not an empty source domain: {outer_expression}"
            );
            let mut bounded = limits.max_nodes;
            assert_eq!(
                consume(
                    snapshot,
                    inner,
                    span(0, 1, 1),
                    &index,
                    Some(selected),
                    None,
                    span(0, 1, 1),
                    span(0, 1, 1),
                    limits.max_depth,
                    limits,
                    &mut bounded
                )
                .unwrap_err()
                .code,
                FailCode::DepthExceeded
            );
            let mut wrong = limits.max_nodes;
            assert_eq!(
                consume(
                    snapshot,
                    inner,
                    span(0, 1, 1),
                    &index,
                    None,
                    None,
                    span(0, 1, 1),
                    span(0, 1, 1),
                    0,
                    limits,
                    &mut wrong
                )
                .unwrap_err()
                .code,
                FailCode::Type
            );
            genuine += 1;
        }
        assert!(genuine > 0);
        assert_eq!(reads.get(), 0);
    }
}
#[test]
fn genuinely_empty_membership_does_not_erase_pre_subject_index_components() {
    let mut prepared = from_code(&nested(
        "slice {beat -> SUBJECT} 2 [cut nil]",
        "fast {beat -> p} 1",
        true,
    ));
    let snapshot = &mut prepared.snapshot;
    let outer = snapshot.routing.root_part;
    let FrozenPartNode::Edit { source: inner, .. } = snapshot.routing.parts[outer].node else {
        panic!("genuine edit source")
    };
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, outer, span(0, 1, 1), limits, &mut left);
    let reads = deny(snapshot);
    let request = request(snapshot, inner, span(0, 1, 1), limits, &mut left);
    let selected = &payload(snapshot, outer).sources[0];
    let mut genuine = 0;
    for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
        if address.owner().revision != request.revision || address.owner().root != request.root {
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        let boundaries = index.source_boundaries(0, limits, &mut left).unwrap();
        let Some(boundary) = boundaries.first() else {
            continue;
        };
        if boundary.policy_parent().unwrap().revision != snapshot.routing.parts[outer].revision {
            continue;
        }
        assert!(boundary.is_empty().unwrap());
        let (groups, membership, _) = consume(
            snapshot,
            inner,
            span(0, 1, 1),
            &index,
            Some(selected),
            None,
            span(0, 1, 1),
            span(0, 1, 1),
            0,
            limits,
            &mut left,
        )
        .unwrap();
        assert_eq!(membership, Some(true));
        assert!(groups.iter().any(|group| !group.is_empty()));
        genuine += 1;
    }
    assert!(genuine > 0);
    assert_eq!(reads.get(), 0);
}
#[test]
fn actual_dynamic_rate_reordered_windows_keep_groups_and_callbacks_immutable() {
    let mut prepared = from_code(&code(
        "slow {slice {beat -> nil} 2 [cut]} {beat -> first [2]}",
    ));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 32768,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 2, 1), limits, &mut left);
    let reads = deny(snapshot);
    for window in [span(1, 2, 1), span(0, 1, 1), span(0, 2, 1), span(1, 3, 2)] {
        let request = request(snapshot, scope, window, limits, &mut left);
        let mut count = 0;
        for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
            if address.owner().revision != request.revision {
                continue;
            }
            let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
            let (groups, membership, _) = consume(
                snapshot,
                scope,
                window,
                &index,
                None,
                None,
                span(0, 1, 1),
                window,
                0,
                limits,
                &mut left,
            )
            .unwrap();
            assert_eq!(membership, None);
            for group in groups {
                for component in group {
                    assert!(component.begin >= window.begin && component.end <= window.end);
                    count += 1;
                }
            }
        }
        assert!(count > 0);
    }
    assert_eq!(reads.get(), 0);
}

#[test]
fn whole_part_configuration_unions_touching_original_cross_cycle_wholes() {
    let mut prepared = from_code(&code("slow {slice {beat -> nil} 1 [cut]} 2"));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let mut useful = 0;
    for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
        if address.owner().revision != request.revision {
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        let (groups, membership, _) = consume(
            snapshot,
            scope,
            span(0, 4, 1),
            &index,
            None,
            None,
            span(0, 2, 1),
            span(0, 4, 1),
            0,
            limits,
            &mut left,
        )
        .unwrap();
        assert_eq!(membership, None);
        assert!(
            groups.iter().any(|group| group == &vec![span(0, 4, 1)]),
            "complete related owner-cycle union: {groups:?}"
        );
        useful += 1;
    }
    assert!(useful > 0);
    assert_eq!(reads.get(), 0);
}

#[test]
fn authentic_partition_union_covers_owner_and_partial_retention_refuses() {
    for windows in [
        vec![span(0, 2, 1), span(2, 4, 1)],
        vec![span(2, 4, 1), span(0, 2, 1)],
        vec![span(0, 1, 1)],
    ] {
        let complete = windows.len() == 2;
        let mut prepared = from_code(&code("slow {slice {beat -> nil} 1 [cut]} 2"));
        let snapshot = &mut prepared.snapshot;
        let scope = snapshot.routing.root_part;
        let limits = SongLimits {
            max_nodes: 100_000,
            ..SongLimits::default()
        };
        let mut left = limits.max_nodes;
        for window in &windows {
            retain(snapshot, scope, *window, limits, &mut left);
        }
        let reads = deny(snapshot);
        let local = windows[0];
        let request = request(snapshot, scope, local, limits, &mut left);
        let address = owner_addresses(snapshot, &request, 0, limits, &mut left)
            .unwrap()
            .into_iter()
            .find(|address| address.owner().revision == request.revision)
            .unwrap();
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        let result = consume(
            snapshot,
            scope,
            span(0, 4, 1),
            &index,
            None,
            None,
            span(0, 2, 1),
            span(0, 4, 1),
            0,
            limits,
            &mut left,
        );
        if complete {
            let (groups, _, _) = result.unwrap();
            assert!(
                groups.iter().any(|group| group == &vec![span(0, 4, 1)]),
                "partitioned genuine coverage: {groups:?}"
            );
        } else {
            assert_eq!(result.unwrap_err().code, FailCode::Type);
        }
        assert_eq!(reads.get(), 0);
    }
}
#[test]
fn addressed_source_start_outside_window_cannot_borrow_other_eligible_component() {
    let mut prepared = from_code(&code("slow {slice {beat -> p} 1 [cut cut]} 2"));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 2, 1), limits, &mut left);
    let events = snapshot.query(span(0, 2, 1), &limits).unwrap();
    let issuer = path(snapshot, scope).0;
    let timing = events
        .iter()
        .flat_map(|event| event.source_origin.as_ref())
        .flat_map(|origin| origin.slice_timings())
        .find(|timing| timing.issuer() == issuer && timing.sample_start() == Ratio64::ZERO)
        .unwrap();
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 2, 1), limits, &mut left);
    let address = owner_addresses(snapshot, &request, 0, limits, &mut left)
        .unwrap()
        .into_iter()
        .find(|address| address.owner().revision == request.revision)
        .unwrap();
    let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
    let (groups, _, bound) = consume(
        snapshot,
        scope,
        span(0, 2, 1),
        &index,
        None,
        Some(timing),
        span(1, 2, 2),
        span(0, 2, 1),
        0,
        limits,
        &mut left,
    )
    .unwrap();
    assert!(
        groups.iter().any(|group| !group.is_empty()),
        "another actual Index START remains eligible"
    );
    assert_eq!(bound, None, "addressed original START0 remains ineligible");
    assert_eq!(reads.get(), 0);
}

#[test]
fn genuine_index_rest_gaps_are_not_replaced_by_a_configuration_hull() {
    let mut prepared = from_code(&code("slice {beat -> nil} 1 [cut nil cut nil]"));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let mut useful = 0;
    for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
        if address.owner().revision != request.revision {
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        let (groups, _, _) = consume(
            snapshot,
            scope,
            span(0, 4, 1),
            &index,
            None,
            None,
            span(0, 4, 1),
            span(0, 4, 1),
            0,
            limits,
            &mut left,
        )
        .unwrap();
        for component in groups.iter().flatten() {
            for cycle in 0..4 {
                for gap in [
                    span(cycle * 4 + 1, cycle * 4 + 2, 4),
                    span(cycle * 4 + 3, cycle * 4 + 4, 4),
                ] {
                    assert!(
                        component.end <= gap.begin || component.begin >= gap.end,
                        "rest gap must not be bridged: component={component:?}, gap={gap:?}"
                    );
                }
            }
            useful += 1;
        }
    }
    assert!(useful > 0);
    assert_eq!(reads.get(), 0);
}

mod domains;

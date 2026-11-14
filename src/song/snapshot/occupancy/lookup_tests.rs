//! Genuine original freeze, invocation and sealed source lookup contracts.
use super::{lookup::*, tests::from_code, *};
use crate::ns::namespace::VarSlotRef;
use crate::song::snapshot::{FrozenEdit, FrozenPartNode};
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
                "lookup cannot execute callbacks",
            ));
        }
        Ok(())
    }
}
fn deny(snapshot: &mut SongSnapshot) -> Rc<Cell<u32>> {
    let calls = Rc::new(Cell::new(0));
    snapshot
        .evaluator
        .vm_and_ns()
        .0
        .set_read_observer(Some(Box::new(Denied(calls.clone()))));
    calls
}
fn span(a: i64, b: i64, denominator: i64) -> TimeSpan {
    TimeSpan::new(
        Ratio64::new(a, denominator).unwrap(),
        Ratio64::new(b, denominator).unwrap(),
    )
    .unwrap()
}
fn scope(snapshot: &SongSnapshot, last: bool) -> usize {
    let scopes: Vec<_> = snapshot
        .routing
        .parts
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            matches!(
                p.node,
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { .. },
                    ..
                }
            )
            .then_some(i)
        })
        .collect();
    if last {
        *scopes.last().unwrap()
    } else {
        scopes[0]
    }
}
fn payload_at(snapshot: &SongSnapshot, scope: usize) -> &crate::song::snapshot::FrozenPattern {
    match &snapshot.routing.parts[scope].node {
        FrozenPartNode::Edit {
            edit: FrozenEdit::Transform { payload, .. },
            ..
        } => payload,
        _ => panic!("original Transform payload"),
    }
}
fn issue(
    snapshot: &SongSnapshot,
    scope: usize,
    window: TimeSpan,
    remaining: &mut u32,
) -> CanonicalIndexRequest {
    issue_with_limits(snapshot, scope, window, remaining, SongLimits::default())
}
fn issue_with_limits(
    snapshot: &SongSnapshot,
    scope: usize,
    window: TimeSpan,
    remaining: &mut u32,
    limits: SongLimits,
) -> CanonicalIndexRequest {
    let recipe = payload_at(snapshot, scope).index_timing().unwrap();
    let mut pending = vec![(recipe.root(), Vec::new())];
    while let Some((index, prefix)) = pending.pop() {
        let node = &recipe.nodes()[index as usize];
        if node.operation() == crate::song::source_uses::FrozenUseOperation::Slice {
            return snapshot
                .canonical_index_request(
                    scope,
                    intern_kw("drums"),
                    node.issuer(),
                    &prefix,
                    window,
                    0,
                    limits,
                    remaining,
                )
                .unwrap();
        }
        for edge in node.children() {
            let mut path = prefix.clone();
            path.extend_from_slice(edge.trace());
            pending.push((edge.child(), path));
        }
    }
    panic!("actual Slice site")
}
fn simple(wrapper: &str) -> String {
    let expression = "slice {beat -> nil} 2 [cut nil]";
    let body = wrapper.replace("SUBJECT", expression);
    format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong selected tail-seconds: 0 > play-song")
}
fn retain(snapshot: &mut SongSnapshot, scope: usize, window: TimeSpan, remaining: &mut u32) {
    retain_with_limits(snapshot, scope, window, remaining, SongLimits::default());
}
fn retain_with_limits(
    snapshot: &mut SongSnapshot,
    scope: usize,
    window: TimeSpan,
    remaining: &mut u32,
    limits: SongLimits,
) {
    let request = issue_with_limits(snapshot, scope, window, remaining, limits);
    retain_index_occupancy(snapshot, vec![request], limits, remaining).unwrap();
}
#[test]
fn suppressed_subject_lookup_keeps_original_start_and_actual_dynamic_projection() {
    let mut prepared = from_code(&simple("fast {SUBJECT} {beat -> first [2]}"));
    let snapshot = &mut prepared.snapshot;
    let owner = scope(snapshot, true);
    let mut remaining = SongLimits::default().max_nodes;
    retain(snapshot, owner, span(0, 1, 1), &mut remaining);
    let calls = deny(snapshot);
    let request = issue(snapshot, owner, span(0, 1, 1), &mut remaining);
    let addresses =
        owner_addresses(snapshot, &request, 0, SongLimits::default(), &mut remaining).unwrap();
    assert!(!addresses.is_empty());
    let mut rows = Vec::new();
    for address in addresses {
        let index = bind_index(
            snapshot,
            &request,
            address,
            0,
            SongLimits::default(),
            &mut remaining,
        )
        .unwrap();
        assert_eq!(index.owner().revision, request.revision);
        let original = snapshot
            .occupancy
            .iter()
            .flat_map(|record| &record.invocations)
            .find(|invocation| {
                invocation.lookup_owner() == index.owner()
                    && invocation.lookup_seed() == index.seed()
            })
            .unwrap();
        assert_eq!(index.entry(), original.lookup_entry());
        rows.extend(
            index
                .rows(0, SongLimits::default(), &mut remaining)
                .unwrap(),
        );
    }
    assert!(
        !rows.is_empty(),
        "pre-subject Index events survive nil subject"
    );
    assert!(rows
        .iter()
        .any(|row| row.issuer_start() != row.footprint().sample_start));
    assert!(rows
        .iter()
        .all(|row| row.issuer() == request.issuer && !row.prefix().steps.is_empty()));
    assert!(rows.iter().all(|row| row.source_count() == 0));
    assert_eq!(calls.get(), 0);
}
fn nested(empty: bool, inner_rev: bool, outer_rev: bool) -> String {
    let inner = if empty {
        "slice {beat -> p} 2 [nil]"
    } else {
        "slice {beat -> p} 2 [cut nil]"
    };
    let inner = if inner_rev {
        format!("rev {{{inner}}}")
    } else {
        inner.into()
    };
    let selected = if outer_rev {
        "rev {beat -> p}"
    } else {
        "fast {beat -> p} 1"
    };
    format!("fn cut beat:\n\tfirst [0]\nfn innered p:\n\t{inner}\nfn indexed p:\n\tstack [{{{selected}}} {{slice {{beat -> nil}} 2 [cut nil]}}]\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 1}}\nlet inner {{transform-instrument base :drums :analog innered}}\nlet selected {{transform-instrument inner :drums :analog indexed}}\nsong selected tail-seconds: 0 > play-song")
}
#[test]
fn nested_original_dependency_policy_and_real_member_are_bound_without_reads() {
    let mut prepared = from_code(&nested(false, false, false));
    let snapshot = &mut prepared.snapshot;
    let outer = scope(snapshot, true);
    let inner = scope(snapshot, false);
    let mut remaining = SongLimits::default().max_nodes;
    retain(snapshot, outer, span(0, 1, 1), &mut remaining);
    let expected = snapshot
        .query(span(0, 1, 1), &SongLimits::default())
        .unwrap();
    assert!(!expected.is_empty());
    let member_handle = expected[0].source_origin.as_ref().unwrap().handle.clone();
    let calls = deny(snapshot);
    let root_request = issue(snapshot, outer, span(0, 1, 1), &mut remaining);
    let inner_request = issue(snapshot, inner, span(0, 1, 1), &mut remaining);
    let addresses = owner_addresses(
        snapshot,
        &root_request,
        0,
        SongLimits::default(),
        &mut remaining,
    )
    .unwrap();
    let selected = &payload_at(snapshot, outer).sources[0];
    let mut bound = 0;
    for address in addresses {
        if address.owner().revision != inner_request.revision {
            continue;
        }
        let index = bind_index(
            snapshot,
            &inner_request,
            address,
            0,
            SongLimits::default(),
            &mut remaining,
        )
        .unwrap();
        let boundaries = index
            .source_boundaries(0, SongLimits::default(), &mut remaining)
            .unwrap();
        if boundaries.is_empty() {
            continue;
        }
        assert!(!index
            .empty_source(selected, 0, SongLimits::default(), &mut remaining)
            .unwrap());
        let member = bind_member(
            &index,
            selected,
            &member_handle,
            0,
            SongLimits::default(),
            &mut remaining,
        )
        .unwrap();
        assert_eq!(member.origin().handle, member_handle);
        assert!(!member.use_edges().is_empty());
        let rows = index
            .rows(0, SongLimits::default(), &mut remaining)
            .unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| row.source_count() > 0));
        bound += 1;
    }
    assert!(bound > 0);
    assert_eq!(calls.get(), 0);
}
#[test]
fn empty_success_policy_cannot_mint_a_member() {
    let mut prepared = from_code(&nested(true, false, false));
    let snapshot = &mut prepared.snapshot;
    let outer = scope(snapshot, true);
    let inner = scope(snapshot, false);
    let mut remaining = SongLimits::default().max_nodes;
    retain(snapshot, outer, span(0, 1, 1), &mut remaining);
    let calls = deny(snapshot);
    let request = issue(snapshot, inner, span(0, 1, 1), &mut remaining);
    let addresses =
        owner_addresses(snapshot, &request, 0, SongLimits::default(), &mut remaining).unwrap();
    let selected = &payload_at(snapshot, outer).sources[0];
    let mut empty = 0;
    for address in addresses {
        let index = bind_index(
            snapshot,
            &request,
            address,
            0,
            SongLimits::default(),
            &mut remaining,
        )
        .unwrap();
        if index
            .source_boundaries(0, SongLimits::default(), &mut remaining)
            .unwrap()
            .is_empty()
        {
            continue;
        }
        assert!(index
            .empty_source(selected, 0, SongLimits::default(), &mut remaining)
            .unwrap());
        assert!(index
            .rows(0, SongLimits::default(), &mut remaining)
            .unwrap()
            .is_empty());
        empty += 1;
    }
    assert!(empty > 0);
    assert_eq!(calls.get(), 0);
}

#[test]
fn exact_lookup_work_one_less_depth_and_foreign_authority_preserve_publication() {
    let mut prepared = from_code(&simple("SUBJECT"));
    let snapshot = &mut prepared.snapshot;
    let owner = scope(snapshot, true);
    let limits = SongLimits::default();
    let mut remaining = limits.max_nodes;
    retain(snapshot, owner, span(0, 1, 1), &mut remaining);
    let calls = deny(snapshot);
    let request = issue(snapshot, owner, span(0, 1, 1), &mut remaining);
    let original_view = snapshot.replay.clone().unwrap();
    let lookup = |remaining: &mut u32, depth| {
        let addresses = owner_addresses(snapshot, &request, depth, limits, remaining)?;
        let mut count = 0;
        for address in addresses {
            let index = bind_index(snapshot, &request, address, depth, limits, remaining)?;
            count += index.rows(depth, limits, remaining)?.len();
        }
        Ok::<_, Failure>(count)
    };
    let mut measured = limits.max_nodes;
    let count = lookup(&mut measured, 0).unwrap();
    assert!(count > 0);
    let needed = limits.max_nodes - measured;
    let mut exact = needed;
    assert_eq!(lookup(&mut exact, 0).unwrap(), count);
    assert_eq!(exact, 0);
    let mut short = needed - 1;
    assert_eq!(
        lookup(&mut short, 0).unwrap_err().code,
        FailCode::FuelExhausted
    );
    let mut ceiling = limits.max_nodes;
    assert_eq!(lookup(&mut ceiling, limits.max_depth).unwrap(), count);
    let mut bounded = limits.max_nodes;
    assert_eq!(
        lookup(&mut bounded, limits.max_depth + 1).unwrap_err().code,
        FailCode::DepthExceeded
    );
    assert!(bounded < limits.max_nodes);
    assert!(Rc::ptr_eq(
        &original_view,
        snapshot.replay.as_ref().unwrap()
    ));
    let foreign = from_code(&simple("SUBJECT"));
    let foreign_request = issue(
        &foreign.snapshot,
        scope(&foreign.snapshot, true),
        span(0, 1, 1),
        &mut bounded,
    );
    assert_eq!(foreign_request.root, request.root);
    assert_eq!(
        owner_addresses(snapshot, &foreign_request, 0, limits, &mut bounded)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    assert_eq!(calls.get(), 0);
}
#[test]
fn missing_complete_site_and_duplicate_coverage_have_truthful_lookup() {
    let mut prepared = from_code(&simple("SUBJECT"));
    let snapshot = &mut prepared.snapshot;
    let owner = scope(snapshot, true);
    let limits = SongLimits::default();
    let mut remaining = limits.max_nodes;
    let unretained = issue(snapshot, owner, span(0, 1, 1), &mut remaining);
    assert_eq!(
        owner_addresses(snapshot, &unretained, 0, limits, &mut remaining)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    retain(snapshot, owner, span(0, 1, 1), &mut remaining);
    let calls = deny(snapshot);
    retain(snapshot, owner, span(1, 2, 2), &mut remaining);
    let request = issue(snapshot, owner, span(1, 2, 2), &mut remaining);
    let addresses = owner_addresses(snapshot, &request, 0, limits, &mut remaining).unwrap();
    assert!(
        addresses.len() >= 2,
        "genuine overlapping retained coverage"
    );
    let mut seed = None;
    let mut owner_basis = None;
    let mut count = 0;
    for address in addresses {
        let index = bind_index(snapshot, &request, address, 0, limits, &mut remaining).unwrap();
        if let Some(seed) = seed {
            assert_eq!(index.seed(), seed);
        } else {
            seed = Some(index.seed());
        }
        if let Some(ref basis) = owner_basis {
            assert_eq!(index.owner(), basis);
        } else {
            owner_basis = Some(index.owner().clone());
        }
        assert!(!index.rows(0, limits, &mut remaining).unwrap().is_empty());
        count += 1;
    }
    assert!(
        count >= 2,
        "explicit original duplicate tokens do not become ambiguity"
    );
    let mut wrong = issue(snapshot, owner, span(1, 2, 2), &mut remaining);
    wrong
        .prefix
        .push(crate::song::source_uses::FrozenUseTraceTerm::Exact(
            crate::pattern::occ::ProducerStep {
                kind: crate::pattern::occ::ProducerKind::Child,
                ordinal: 99,
            },
        ));
    assert_eq!(
        owner_addresses(snapshot, &wrong, 0, limits, &mut remaining)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    assert_eq!(calls.get(), 0);
}
#[test]
fn inherited_rev_orientation_crosses_real_affine_or_second_rev_source_parent() {
    use crate::pattern::eval::song_clock::ClockOrientation;
    for (outer_rev, expected) in [
        (false, ClockOrientation::Before),
        (true, ClockOrientation::At),
    ] {
        let mut prepared = from_code(&nested(false, true, outer_rev));
        let snapshot = &mut prepared.snapshot;
        let outer = scope(snapshot, true);
        let inner = scope(snapshot, false);
        let limits = SongLimits::default();
        let mut remaining = limits.max_nodes;
        retain(snapshot, outer, span(0, 1, 1), &mut remaining);
        let calls = deny(snapshot);
        let request = issue(snapshot, inner, span(0, 1, 1), &mut remaining);
        let addresses = owner_addresses(snapshot, &request, 0, limits, &mut remaining).unwrap();
        let mut projected = 0;
        for address in addresses {
            if address.owner().revision != request.revision
                || address.owner().track != request.track
                || address.owner().root != request.root
            {
                assert_eq!(
                    bind_index(
                        snapshot,
                        &request,
                        address,
                        0,
                        SongLimits::default(),
                        &mut remaining
                    )
                    .err()
                    .unwrap()
                    .code,
                    FailCode::Type
                );
                continue;
            }
            let index = bind_index(snapshot, &request, address, 0, limits, &mut remaining).unwrap();
            if index
                .source_boundaries(0, limits, &mut remaining)
                .unwrap()
                .is_empty()
            {
                continue;
            }
            for row in index.rows(0, limits, &mut remaining).unwrap() {
                assert_eq!(row.issuer_start(), Ratio64::ZERO);
                assert_eq!(row.footprint().orientation, expected);
                assert_eq!(
                    row.footprint().sample_start,
                    if outer_rev {
                        Ratio64::ZERO
                    } else {
                        Ratio64::ONE
                    }
                );
                assert!(row.footprint().applicable);
                let t = row.footprint().sample_start;
                let eligible = match expected {
                    ClockOrientation::At => t >= Ratio64::ZERO && t < Ratio64::ONE,
                    ClockOrientation::Before => t > Ratio64::ZERO && t <= Ratio64::ONE,
                };
                assert!(eligible, "actual reflected endpoint membership");
                projected += 1;
            }
        }
        assert!(projected > 0);
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn genuine_same_policy_sites_preserve_distinct_full_use_paths_and_wrong_parent_refuses() {
    let code = "fn cut beat:\n\tfirst [0]\nfn innered p:\n\tslice {beat -> p} 2 [cut nil]\nfn indexed p:\n\tstack [{slice {beat -> p} 2 [cut nil]} {slice {beat -> p} 2 [cut nil]}]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 1}\nlet inner {transform-instrument base :drums :analog innered}\nlet selected {transform-instrument inner :drums :analog indexed}\nlet sibling {transform-instrument inner :drums :analog indexed}\nsong {sequence [selected sibling]} tail-seconds: 0 > play-song";
    let mut prepared = from_code(code);
    let snapshot = &mut prepared.snapshot;
    let FrozenPartNode::Sequence(children) =
        &snapshot.routing.parts[snapshot.routing.root_part].node
    else {
        panic!("original declared Sequence");
    };
    assert_eq!(children.len(), 2);
    let outer = children[0].1;
    let sibling = children[1].1;
    let FrozenPartNode::Edit {
        source: inner,
        edit: FrozenEdit::Transform { .. },
    } = snapshot.routing.parts[outer].node
    else {
        panic!("original selected Transform source");
    };
    let FrozenPartNode::Edit {
        source: sibling_inner,
        edit: FrozenEdit::Transform { .. },
    } = snapshot.routing.parts[sibling].node
    else {
        panic!("original sibling Transform source");
    };
    assert_eq!(
        snapshot.routing.parts[inner].revision,
        snapshot.routing.parts[sibling_inner].revision
    );
    assert!(matches!(
        snapshot.routing.parts[inner].node,
        FrozenPartNode::Edit {
            edit: FrozenEdit::Transform { .. },
            ..
        }
    ));
    let limits = SongLimits {
        max_nodes: 32_768,
        ..SongLimits::default()
    };
    limits.validate().unwrap();
    let mut remaining = limits.max_nodes;
    let mut stages = vec![("initial", remaining)];
    retain_with_limits(snapshot, outer, span(0, 1, 1), &mut remaining, limits);
    stages.push(("retention", remaining));
    let expected = snapshot.query(span(0, 1, 1), &limits).unwrap();
    assert!(!expected.is_empty());
    let handle = expected[0].source_origin.as_ref().unwrap().handle.clone();
    let calls = deny(snapshot);
    let request = issue_with_limits(snapshot, inner, span(0, 1, 1), &mut remaining, limits);
    stages.push(("request", remaining));
    let addresses = owner_addresses(snapshot, &request, 0, limits, &mut remaining).unwrap();
    stages.push(("enumeration", remaining));
    let correct = &payload_at(snapshot, outer).sources;
    let foreign = &payload_at(snapshot, sibling).sources[0];
    assert!(
        correct
            .iter()
            .any(|source| snapshot.routing.parts[source.root_part].revision
                == snapshot.routing.parts[foreign.root_part].revision
                && snapshot.routing.parts[source.root_part].duration
                    == snapshot.routing.parts[foreign.root_part].duration
                && source.track == foreign.track
                && source.family == foreign.family),
        "two genuine uses select the same original source policy"
    );
    let mut edges = Vec::new();
    let mut traces = Vec::new();
    for address in addresses {
        if address.owner().revision != request.revision
            || address.owner().track != request.track
            || address.owner().root != request.root
        {
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut remaining).unwrap();
        stages.push(("bind", remaining));
        let boundaries = index.source_boundaries(0, limits, &mut remaining).unwrap();
        stages.push(("source-chain", remaining));
        let Some(boundary) = boundaries.first() else {
            continue;
        };
        let parent = boundary.policy_parent().unwrap();
        if parent.revision != snapshot.routing.parts[outer].revision
            || parent.track != request.track
            || parent.root != payload_at(snapshot, outer).id
        {
            continue;
        }
        if traces.contains(boundary.producer()) {
            continue;
        }
        let before_foreign = remaining;
        let foreign_failure = bind_member(&index, foreign, &handle, 0, limits, &mut remaining)
            .err()
            .unwrap();
        stages.push(("wrong-parent", remaining));
        assert_eq!(foreign_failure.code, FailCode::Type,
            "wrong-parent stage before={before_foreign} after={remaining} stages={stages:?} producer={:?} foreign={foreign:?}", boundary.producer());
        let mut actual_member = None;
        for selected in correct.iter().filter(|source| {
            snapshot.routing.parts[source.root_part].revision == request.revision
                && source.track == request.track
        }) {
            let before_member = remaining;
            let result = bind_member(&index, selected, &handle, 0, limits, &mut remaining);
            stages.push(("intended-member", remaining));
            match result {
                Ok(member) => {
                    assert!(
                        actual_member.is_none(),
                        "unique descriptor for actual full use"
                    );
                    actual_member = Some(member);
                }
                Err(failure) => assert_eq!(failure.code, FailCode::Type,
                    "intended-member stage before={before_member} after={remaining} stages={stages:?} producer={:?} selected={selected:?}", boundary.producer()),
            }
        }
        let member = actual_member.expect("actual producer's authenticated source policy");
        assert_eq!(member.origin().handle, handle);
        edges.push(member.use_edges().to_vec());
        traces.push(boundary.producer().clone());
        assert_eq!(boundary.request(), TimeSpan::point(Ratio64::ZERO));
        if traces.len() == 2 {
            break;
        }
    }
    assert_eq!(edges.len(), 2);
    assert!(
        edges.iter().any(|edge| edge != &edges[0]),
        "full incoming use identities stay distinct"
    );
    assert!(
        traces.iter().any(|trace| trace != &traces[0]),
        "real parent producer paths stay distinct"
    );
    assert_eq!(calls.get(), 0);
    eprintln!(
        "distinct-use cumulative work={} remaining={} stages={stages:?}",
        limits.max_nodes - remaining,
        remaining
    );
}
#[test]
fn actual_vary_tokens_preserve_seeds_placements_and_foreign_recipe_binding_refuses() {
    let code = "fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {beat -> nil} 2 [cut nil]\nlet base {part [drums: {s :analog}] duration: 1}\nlet selected {transform-instrument base :drums :analog indexed}\nsong {part-repeat selected 2 seed-mode: :vary} tail-seconds: 0 > play-song";
    let mut prepared = from_code(code);
    let snapshot = &mut prepared.snapshot;
    let owner = scope(snapshot, true);
    let limits = SongLimits::default();
    let mut remaining = limits.max_nodes;
    retain(snapshot, owner, span(0, 2, 1), &mut remaining);
    let calls = deny(snapshot);
    let request = issue(snapshot, owner, span(0, 2, 1), &mut remaining);
    let addresses = owner_addresses(snapshot, &request, 0, limits, &mut remaining).unwrap();
    let mut seeds = Vec::new();
    let mut placements = Vec::new();
    for address in addresses {
        let index = bind_index(snapshot, &request, address, 0, limits, &mut remaining).unwrap();
        seeds.push(index.seed());
        placements.push(index.owner().placement.clone());
        assert!(!index.rows(0, limits, &mut remaining).unwrap().is_empty());
    }
    assert_eq!(seeds.len(), 2);
    assert_ne!(seeds[0], seeds[1]);
    assert_ne!(placements[0], placements[1]);
    let foreign = from_code(code);
    let forged_recipe = issue(
        &foreign.snapshot,
        scope(&foreign.snapshot, true),
        span(0, 2, 1),
        &mut remaining,
    );
    let mut wrong = issue(snapshot, owner, span(0, 2, 1), &mut remaining);
    wrong.recipe = forged_recipe.recipe;
    assert_eq!(
        owner_addresses(snapshot, &wrong, 0, limits, &mut remaining)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    let mut wrong_owner = issue(snapshot, owner, span(0, 2, 1), &mut remaining);
    wrong_owner.scope = 0;
    assert_eq!(
        owner_addresses(snapshot, &wrong_owner, 0, limits, &mut remaining)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn actual_structural_sample_resets_reflected_orientation_without_replacing_issuer_start() {
    use crate::pattern::eval::song_clock::ClockOrientation;
    let code =
        nested(false, true, false).replace("fast {beat -> p} 1", "grid {beat -> p} [true true]");
    let mut prepared = from_code(&code);
    let snapshot = &mut prepared.snapshot;
    let outer = scope(snapshot, true);
    let inner = scope(snapshot, false);
    let limits = SongLimits::default();
    let mut remaining = limits.max_nodes;
    retain(snapshot, outer, span(0, 1, 1), &mut remaining);
    let calls = deny(snapshot);
    let request = issue(snapshot, inner, span(0, 1, 1), &mut remaining);
    let addresses = owner_addresses(snapshot, &request, 0, limits, &mut remaining).unwrap();
    let mut count = 0;
    for address in addresses {
        if address.owner().revision != request.revision
            || address.owner().track != request.track
            || address.owner().root != request.root
        {
            assert_eq!(
                bind_index(
                    snapshot,
                    &request,
                    address,
                    0,
                    SongLimits::default(),
                    &mut remaining
                )
                .err()
                .unwrap()
                .code,
                FailCode::Type
            );
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut remaining).unwrap();
        if index
            .source_boundaries(0, limits, &mut remaining)
            .unwrap()
            .is_empty()
        {
            continue;
        }
        for row in index.rows(0, limits, &mut remaining).unwrap() {
            assert_eq!(row.issuer_start(), Ratio64::ZERO);
            assert_eq!(row.footprint().orientation, ClockOrientation::At);
            assert!(
                matches!(row.footprint().sample_start, Ratio64::ZERO)
                    || row.footprint().sample_start == Ratio64::new(1, 2).unwrap()
            );
            assert_eq!(
                row.footprint().whole.duration().unwrap(),
                Ratio64::new(1, 2).unwrap()
            );
            count += 1;
        }
    }
    assert!(count >= 2);
    assert_eq!(calls.get(), 0);
}
#[test]
fn genuine_inherited_origin_spends_remaining_depth_instead_of_resetting_maximum() {
    use crate::pattern::eval::song_clock::ProjectionBudget;
    let mut prepared = from_code(&nested(false, false, false));
    let snapshot = &mut prepared.snapshot;
    let outer = scope(snapshot, true);
    let inner = scope(snapshot, false);
    let limits = SongLimits::default();
    let mut remaining = limits.max_nodes;
    retain(snapshot, outer, span(0, 1, 1), &mut remaining);
    let events = snapshot.query(span(0, 1, 1), &limits).unwrap();
    let handle = events[0].source_origin.as_ref().unwrap().handle.clone();
    let calls = deny(snapshot);
    let request = issue(snapshot, inner, span(0, 1, 1), &mut remaining);
    let addresses = owner_addresses(snapshot, &request, 0, limits, &mut remaining).unwrap();
    let mut checked = 0;
    for address in addresses {
        if address.owner().revision != request.revision
            || address.owner().track != request.track
            || address.owner().root != request.root
        {
            assert_eq!(
                bind_index(
                    snapshot,
                    &request,
                    address,
                    0,
                    SongLimits::default(),
                    &mut remaining
                )
                .err()
                .unwrap()
                .code,
                FailCode::Type
            );
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut remaining).unwrap();
        let boundaries = index.source_boundaries(0, limits, &mut remaining).unwrap();
        if boundaries.is_empty() {
            continue;
        }
        let mut work = limits.max_nodes;
        let mut budget = ProjectionBudget::new(limits, &mut work).unwrap();
        let origin = boundaries[0]
            .member(&handle, 0, &mut budget)
            .unwrap()
            .unwrap();
        assert!(
            origin.inherited.is_some(),
            "actual nested origin, not synthetic depth"
        );
        let mut work = limits.max_nodes;
        let mut budget = ProjectionBudget::new(limits, &mut work).unwrap();
        assert_eq!(
            boundaries[0]
                .member(&handle, limits.max_depth, &mut budget)
                .err()
                .unwrap()
                .code,
            FailCode::DepthExceeded
        );
        assert!(work < limits.max_nodes);
        checked += 1;
    }
    assert!(checked > 0);
    assert_eq!(calls.get(), 0);
}

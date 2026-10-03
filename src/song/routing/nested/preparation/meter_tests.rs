//! Genuine evaluated source graphs and inherited certification meters.
use super::*;
use crate::{
    pattern::TimeSpan,
    session::song::{evaluate_song_candidate, CandidateBuildCtx},
    song::{
        assets::{DecodedSongAssetFactory, SongAssetLimits},
        prepare_song,
        source_uses::{certify_source_uses, certify_source_uses_metered, FrozenSourceUseCover},
        SnapshotEpoch, SongLimits,
    },
    value::intern::intern_kw,
    vm::fail::FailCode,
};
fn limits() -> SongLimits {
    SongLimits {
        max_nodes: 100_000,
        max_tracks: 7,
        max_cached_events: 1234,
        max_frames: 54321,
        ..Default::default()
    }
}
fn model(code: &str) -> FrozenRoutingInventory {
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let song = prepare_song(
        evaluate_song_candidate(
            code,
            "meter.vact",
            1,
            SnapshotEpoch(9),
            &CandidateBuildCtx {
                assets: &factory,
                asset_limits: SongAssetLimits {
                    max_resources: 256,
                    max_pcm_bytes: 1_000_000,
                    max_source_files: 64,
                    max_source_bytes: 1_000_000,
                    max_banks: 64,
                    max_walk_nodes: 100_000,
                    max_walk_depth: 256,
                },
                lock: None,
                cache: None,
            },
        )
        .unwrap(),
    )
    .unwrap();
    song.snapshot().routing().clone()
}
fn nested() -> FrozenRoutingInventory {
    model("let base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> fast p 2}}\nlet outer {transform-instrument inner :drums :analog {p -> slow p 2}}\nsong outer > play-song")
}
fn payload(inventory: &FrozenRoutingInventory) -> &crate::song::snapshot::FrozenPattern {
    let crate::song::snapshot::FrozenPartNode::Edit {
        edit: crate::song::snapshot::FrozenEdit::Transform { payload, .. },
        ..
    } = &inventory.parts[inventory.root_part].node
    else {
        panic!("genuine selected owner")
    };
    payload
}
fn window() -> TimeSpan {
    TimeSpan::cycle(0).unwrap()
}
fn same(a: &FrozenSourceUseCover, b: &FrozenSourceUseCover) {
    assert_eq!(a.graph(), b.graph());
    assert_eq!(a.window(), b.window());
    assert_eq!(a.configuration_bound(), b.configuration_bound());
    assert_eq!(a.consumed_work(), b.consumed_work());
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}
fn active(inventory: &FrozenRoutingInventory) -> Vec<SongSourceRouteCover> {
    vec![SongSourceRouteCover {
        scope_part: inventory.root_part,
        track: intern_kw("drums"),
        cover: certify_source_uses(inventory, payload(inventory), window(), limits()).unwrap(),
        live_generation_bound: 1,
    }]
}
#[test]
fn direct_original_cover_and_whole_work_exact_one_less_match_legacy() {
    let inventory = nested();
    let pattern = payload(&inventory);
    let policy = limits();
    let legacy = certify_source_uses(&inventory, pattern, window(), policy).unwrap();
    let mut remaining = policy.max_nodes;
    let cover =
        certify_source_uses_metered(&inventory, pattern, window(), policy, &mut remaining, 0)
            .unwrap();
    same(&legacy, &cover);
    let cost = policy.max_nodes - remaining;
    assert_eq!(cost, cover.consumed_work());
    assert!(cost > 1);
    let mut exact = cost;
    let result =
        certify_source_uses_metered(&inventory, pattern, window(), policy, &mut exact, 0).unwrap();
    same(&cover, &result);
    assert_eq!(exact, 0);
    let mut short = cost - 1;
    assert_eq!(
        certify_source_uses_metered(&inventory, pattern, window(), policy, &mut short, 0)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    assert!(short < cost - 1);
}
#[test]
fn nested_original_cover_order_and_exact_one_less_preserve_all_debits() {
    let inventory = nested();
    let active = active(&inventory);
    let policy = limits();
    let mut legacy_remaining = policy.max_nodes;
    let legacy = prepare_nested_covers(&inventory, &active, &mut legacy_remaining).unwrap();
    assert!(!legacy.is_empty());
    let mut remaining = policy.max_nodes;
    let result =
        prepare_nested_covers_metered(&inventory, &active, policy, &mut remaining, 0).unwrap();
    assert_eq!(result.len(), legacy.len());
    for (a, b) in result.iter().zip(&legacy) {
        assert_eq!(
            (a.root_part, a.scope_part, a.track),
            (b.root_part, b.scope_part, b.track)
        );
        same(&a.cover, &b.cover);
    }
    assert_eq!(remaining, legacy_remaining);
    let cost = policy.max_nodes - remaining;
    assert!(cost > 1);
    let mut exact = cost;
    let result2 =
        prepare_nested_covers_metered(&inventory, &active, policy, &mut exact, 0).unwrap();
    assert_eq!(exact, 0);
    assert_eq!(result2.len(), result.len());
    let mut short = cost - 1;
    assert_eq!(
        prepare_nested_covers_metered(&inventory, &active, policy, &mut short, 0)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    assert!(short < cost - 1);
}
#[test]
fn genuine_inherited_hops_enforce_the_original_depth_without_double_subtraction() {
    let inventory = nested();
    let pattern = payload(&inventory);
    let active = active(&inventory);
    let policy = limits();
    for depth in [0, 1, 8] {
        let mut remaining = policy.max_nodes;
        certify_source_uses_metered(&inventory, pattern, window(), policy, &mut remaining, depth)
            .unwrap();
        let mut remaining = policy.max_nodes;
        prepare_nested_covers_metered(&inventory, &active, policy, &mut remaining, depth).unwrap();
    }
    let mut direct = policy.max_nodes;
    assert_eq!(
        certify_source_uses_metered(
            &inventory,
            pattern,
            window(),
            policy,
            &mut direct,
            policy.max_depth - 1
        )
        .unwrap_err()
        .code,
        FailCode::DepthExceeded
    );
    assert!(direct < policy.max_nodes);
    let mut nested = policy.max_nodes;
    assert_eq!(
        prepare_nested_covers_metered(
            &inventory,
            &active,
            policy,
            &mut nested,
            policy.max_depth - 1
        )
        .unwrap_err()
        .code,
        FailCode::DepthExceeded
    );
    assert!(nested < policy.max_nodes);
}
#[test]
fn genuine_unsupported_dynamic_source_spends_work_and_publishes_no_cover() {
    let inventory=model("let base {part [drums: {s :analog}] duration: 4}\nlet selected {transform-instrument base :drums :analog {p -> euclid p {t -> 1} 4}}\nsong selected > play-song");
    let policy = limits();
    let mut remaining = policy.max_nodes;
    assert_eq!(
        certify_source_uses_metered(
            &inventory,
            payload(&inventory),
            window(),
            policy,
            &mut remaining,
            0
        )
        .unwrap_err()
        .code,
        FailCode::BeyondCapability
    );
    assert!(remaining < policy.max_nodes);
}

#[test]
fn saved_extraction_entry_preserves_historical_complete_cover_and_debit() {
    let inventory = nested();
    let policy = limits();
    let historical = crate::song::source_uses::certify_extraction_reference(
        &inventory,
        payload(&inventory),
        window(),
        policy,
    )
    .unwrap();
    let mut remaining = policy.max_nodes;
    let current = certify_source_uses_metered(
        &inventory,
        payload(&inventory),
        window(),
        policy,
        &mut remaining,
        0,
    )
    .unwrap();
    same(&current, &historical);
    assert_eq!(policy.max_nodes - remaining, historical.consumed_work());
}
#[test]
fn actual_minimum_depth_with_nonzero_inheritance_has_exact_and_one_less_boundary() {
    let inventory = nested();
    let active = active(&inventory);
    let inherited = 2;
    for nested_mode in [false, true] {
        let mut smallest = None;
        for max_depth in inherited + 1..=limits().max_depth {
            let policy = SongLimits {
                max_depth,
                ..limits()
            };
            let mut remaining = policy.max_nodes;
            let result = if nested_mode {
                prepare_nested_covers_metered(
                    &inventory,
                    &active,
                    policy,
                    &mut remaining,
                    inherited,
                )
                .map(|_| ())
            } else {
                certify_source_uses_metered(
                    &inventory,
                    payload(&inventory),
                    window(),
                    policy,
                    &mut remaining,
                    inherited,
                )
                .map(|_| ())
            };
            match result {
                Ok(()) => {
                    smallest = Some(max_depth);
                    break;
                }
                Err(failure) => assert_eq!(failure.code, FailCode::DepthExceeded),
            }
        }
        let sufficient = smallest.expect("genuine finite graph depth");
        assert!(sufficient > inherited + 1);
        for (max_depth, passes) in [(sufficient, true), (sufficient - 1, false)] {
            let policy = SongLimits {
                max_depth,
                ..limits()
            };
            let mut remaining = policy.max_nodes;
            let result = if nested_mode {
                prepare_nested_covers_metered(
                    &inventory,
                    &active,
                    policy,
                    &mut remaining,
                    inherited,
                )
                .map(|_| ())
            } else {
                certify_source_uses_metered(
                    &inventory,
                    payload(&inventory),
                    window(),
                    policy,
                    &mut remaining,
                    inherited,
                )
                .map(|_| ())
            };
            if passes {
                result.unwrap();
            } else {
                assert_eq!(result.unwrap_err().code, FailCode::DepthExceeded);
            }
            assert!(remaining < policy.max_nodes);
        }
    }
}
#[test]
fn empty_nested_entry_uses_exclusive_original_depth_without_charging() {
    let inventory = nested();
    let policy = SongLimits {
        max_depth: 3,
        ..limits()
    };
    let mut accepted = policy.max_nodes;
    assert!(
        prepare_nested_covers_metered(&inventory, &[], policy, &mut accepted, 2)
            .unwrap()
            .is_empty()
    );
    assert_eq!(accepted, policy.max_nodes);
    let mut refused = policy.max_nodes;
    assert_eq!(
        prepare_nested_covers_metered(&inventory, &[], policy, &mut refused, 3)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
    assert_eq!(refused, policy.max_nodes);
}
#[test]
fn genuine_nested_dynamic_child_fault_retains_its_actual_certification_debits() {
    let inventory = model("let base {part [drums: {s :analog}] duration: 1}\nlet good {transform-instrument base :drums :analog {p -> fast p 2}}\nlet bad {transform-instrument base :drums :analog {p -> euclid p {t -> 1} 4}}\nlet children {sequence [good bad]}\nlet outer {transform-instrument children :drums :analog {p -> slow p 2}}\nsong outer > play-song");
    let active = active(&inventory); // Genuine original first-cycle cover excludes the later dynamic child.
    let policy = limits();
    take_child_trace(); // Reset only this thread’s read-only observations.
    let mut remaining = policy.max_nodes;
    let failure =
        prepare_nested_covers_metered(&inventory, &active, policy, &mut remaining, 0).unwrap_err();
    assert_eq!(failure.code, FailCode::BeyondCapability);
    assert!(remaining < policy.max_nodes);
    let trace = take_child_trace();
    let child = trace.last().expect("actual child certification seam");
    assert_eq!(child.failure, Some(FailCode::BeyondCapability));
    assert!(
        child.before < policy.max_nodes,
        "original traversal debit before child"
    );
    let bad = scope_payload(&inventory, child.scope, child.track).unwrap();
    assert_eq!(bad.id, child.payload);
    let mut child_remaining = child.before;
    assert_eq!(
        certify_source_uses_metered(
            &inventory,
            bad,
            child.window,
            policy,
            &mut child_remaining,
            child.depth
        )
        .unwrap_err()
        .code,
        FailCode::BeyondCapability
    );
    let child_cost = child.before - child_remaining;
    assert!(child_cost > 0);
    assert_eq!(
        child.before - child.after,
        child_cost,
        "actual seam child debit exactly matches direct certification"
    );
    assert_eq!(child_remaining, child.after);
    assert_eq!(
        remaining, child.after,
        "failed nested writeback retains child debit"
    );
    assert_eq!(
        policy.max_nodes - remaining,
        policy.max_nodes - child.before + child_cost
    );
}

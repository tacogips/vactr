//! Actual certificate admission is retained and charged across caller quotas.
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenRoutingInventory};
use vactr::song::source_uses::{certify_source_uses, FrozenPattern, FrozenSourceUseCover};
use vactr::song::{prepare_song, SnapshotEpoch, SongLimits};
use vactr::vm::fail::FailCode;

fn model(code: &str) -> (FrozenRoutingInventory, FrozenPattern) {
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let cx = CandidateBuildCtx {
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
    };
    let song = prepare_song(
        evaluate_song_candidate(code, "source-budget.vact", 1, SnapshotEpoch(56), &cx).unwrap(),
    )
    .unwrap();
    let inventory = song.snapshot().routing().clone();
    let pattern = match &inventory.parts[inventory.root_part].node {
        FrozenPartNode::Capture(tracks) => tracks[0].1.clone(),
        FrozenPartNode::Edit {
            edit: FrozenEdit::Transform { payload, .. },
            ..
        } => payload.clone(),
        other => panic!("unexpected fixture topology: {other:?}"),
    };
    (inventory, pattern)
}
fn ordinary() -> (FrozenRoutingInventory, FrozenPattern) {
    model("song {part [drums: {s :analog}] duration: 1} > play-song")
}
fn nested() -> (FrozenRoutingInventory, FrozenPattern) {
    model("let base {part [drums: {s :analog > note [60 64]}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> fast p 2}}\nlet outer {transform-instrument inner :drums :analog {p -> slow p 2}}\nsong outer > play-song")
}
fn certify(
    inventory: &FrozenRoutingInventory,
    pattern: &FrozenPattern,
    quota: u32,
) -> Result<FrozenSourceUseCover, vactr::vm::Failure> {
    certify_source_uses(
        inventory,
        pattern,
        TimeSpan::cycle(0).unwrap(),
        SongLimits {
            max_nodes: quota,
            ..Default::default()
        },
    )
}

#[test]
fn actual_ordinary_and_nested_work_is_positive_and_deterministic_at_exact_quota() {
    for (inventory, pattern) in [ordinary(), nested()] {
        let ample = certify(&inventory, &pattern, SongLimits::default().max_nodes).unwrap();
        let work = ample.consumed_work();
        assert!(work > 1 && work <= SongLimits::default().max_nodes);
        let exact = certify(&inventory, &pattern, work).unwrap();
        assert_eq!(exact.consumed_work(), work);
        assert_eq!(exact.configuration_bound(), ample.configuration_bound());
        assert_eq!(exact.graph(), ample.graph());
        assert_eq!(
            certify(&inventory, &pattern, work - 1).unwrap_err().code,
            FailCode::FuelExhausted
        );
    }
}

#[test]
fn callers_charge_two_certificates_without_resetting_remaining_quota() {
    let (ordinary_inventory, ordinary_pattern) = ordinary();
    let (nested_inventory, nested_pattern) = nested();
    let ordinary_cost = certify(&ordinary_inventory, &ordinary_pattern, 16_384)
        .unwrap()
        .consumed_work();
    let nested_cost = certify(&nested_inventory, &nested_pattern, 16_384)
        .unwrap()
        .consumed_work();
    let total = ordinary_cost.checked_add(nested_cost).unwrap();
    for (quota, enough) in [(total, true), (total - 1, false)] {
        let first = certify(&ordinary_inventory, &ordinary_pattern, quota).unwrap();
        let remaining = quota.checked_sub(first.consumed_work()).unwrap();
        let second = certify(&nested_inventory, &nested_pattern, remaining);
        if enough {
            let second = second.unwrap();
            assert_eq!(remaining.checked_sub(second.consumed_work()), Some(0));
        } else {
            assert_eq!(second.unwrap_err().code, FailCode::FuelExhausted);
            assert!(
                certify(&nested_inventory, &nested_pattern, quota).is_ok(),
                "a quota reset would incorrectly accept the second certificate"
            );
        }
    }
}

#[test]
fn cover_cloning_retains_admitted_work_and_window_without_recertification() {
    let (inventory, pattern) = nested();
    let cover = certify(&inventory, &pattern, 16_384).unwrap();
    let clone = cover.clone();
    drop(cover);
    assert!(clone.consumed_work() > 0);
    let fresh = certify(&inventory, &pattern, clone.consumed_work()).unwrap();
    assert_eq!(clone.consumed_work(), fresh.consumed_work());
    assert_eq!(clone.graph(), fresh.graph());
    assert_eq!(clone.window(), fresh.window());
    assert_eq!(clone.configuration_bound(), fresh.configuration_bound());
}

#[test]
fn final_graph_and_policy_copy_admission_is_included_in_reported_work() {
    let (inventory, pattern) = nested();
    let cover = certify(&inventory, &pattern, 16_384).unwrap();
    let copied = pattern.source_uses.nodes.len()
        + pattern.sources.len()
        + pattern
            .source_uses
            .nodes
            .iter()
            .map(|node| {
                node.edges.len()
                    + node
                        .edges
                        .iter()
                        .map(|edge| edge.trace.len() + edge.layout.len())
                        .sum::<usize>()
            })
            .sum::<usize>()
        + pattern
            .sources
            .iter()
            .map(|source| source.family.len())
            .sum::<usize>();
    assert!(
        u32::try_from(copied).unwrap() < cover.consumed_work(),
        "reported work includes graph/family copy plus traversal/cache admission"
    );
    let before_copy = cover
        .consumed_work()
        .checked_sub(u32::try_from(copied).unwrap())
        .unwrap();
    assert_eq!(
        certify(&inventory, &pattern, before_copy).unwrap_err().code,
        FailCode::FuelExhausted
    );
}

#[test]
fn invalid_graph_and_window_or_quota_return_failures_without_accounting_results() {
    let (inventory, mut pattern) = nested();
    pattern.source_uses.root = u32::MAX;
    assert_eq!(
        certify(&inventory, &pattern, 16_384).unwrap_err().code,
        FailCode::Type
    );
    let (inventory, pattern) = nested();
    assert_eq!(
        certify(&inventory, &pattern, 0).unwrap_err().code,
        FailCode::Type
    );
    let negative = TimeSpan::new(
        vactr::value::Ratio64::from_int(-1),
        vactr::value::Ratio64::ZERO,
    )
    .unwrap();
    assert_eq!(
        certify_source_uses(&inventory, &pattern, negative, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
}

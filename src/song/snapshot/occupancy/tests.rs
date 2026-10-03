use super::*;
use crate::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use crate::song::source_uses::FrozenUseOperation;
use crate::song::{prepare_song, SnapshotEpoch};
use crate::value::{
    intern::{intern_kw, intern_sym},
    value::Value,
    Ratio64,
};
use std::collections::BTreeMap;

fn prepared(subject: &str) -> super::super::PreparedSong {
    prepared_with_deletion(subject, false)
}
fn prepared_with_deletion(subject: &str, deleted: bool) -> super::super::PreparedSong {
    let end = if deleted {
        "let events {part-events selected :drums 0 1}\nlet handle {{first events} :handle}\nlet changed {delete-event selected handle}\nsong changed tail-seconds: 0 > play-song"
    } else {
        "song selected tail-seconds: 0 > play-song"
    };
    let code = format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {{beat -> {subject}}} 2 [cut nil]\nlet base {{part [drums: {{s :analog}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog indexed}}\n{end}");
    from_code(&code)
}
pub(super) fn from_code(code: &str) -> super::super::PreparedSong {
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let context = CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 32,
            max_source_bytes: 100_000,
            max_banks: 32,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    prepare_song(
        evaluate_song_candidate(code, "canonical-index.vact", 7, SnapshotEpoch(91), &context)
            .unwrap(),
    )
    .unwrap()
}
fn request(
    snapshot: &SongSnapshot,
    depth: u32,
    limits: SongLimits,
    remaining: &mut u32,
) -> CanonicalIndexRequest {
    request_window(
        snapshot,
        depth,
        TimeSpan::cycle(0).unwrap(),
        limits,
        remaining,
    )
}
pub(super) fn request_window(
    snapshot: &SongSnapshot,
    depth: u32,
    window: TimeSpan,
    limits: SongLimits,
    remaining: &mut u32,
) -> CanonicalIndexRequest {
    let scope = snapshot
        .routing
        .parts
        .iter()
        .position(|part| {
            matches!(
                &part.node,
                super::super::FrozenPartNode::Edit {
                    edit: super::super::FrozenEdit::Transform { .. },
                    ..
                }
            )
        })
        .expect("actual original transformed owner");
    let track = intern_kw("drums");
    let payload = match &snapshot.routing.parts[scope].node {
        super::super::FrozenPartNode::Edit {
            edit: super::super::FrozenEdit::Transform { payload, .. },
            ..
        } => payload,
        node => panic!("actual transformed payload: {node:?}"),
    };
    let recipe = payload.index_timing().unwrap();
    let issuer = recipe
        .nodes()
        .iter()
        .find(|node| node.operation() == FrozenUseOperation::Slice)
        .unwrap()
        .issuer();
    snapshot
        .canonical_index_request(scope, track, issuer, &[], window, depth, limits, remaining)
        .unwrap()
}
fn cuts(record: &RetainedCanonicalIndex) -> Vec<&RetainedQueryCall> {
    record.calls.iter().filter(|call| matches!(&call.callable, Value::Fn(f) if f.proto.name == Some(intern_sym("cut")))).collect()
}
#[test]
fn original_dynamic_index_executes_once_and_replays_fractional_reordered_windows() {
    let limits = SongLimits::default();
    let mut prepared = prepared("p");
    let snapshot = &mut prepared.snapshot;
    let mut remaining = limits.max_nodes;
    let issued = request(snapshot, 0, limits, &mut remaining);
    let expected_owner = issued.revision;
    assert_eq!(
        retain_index_occupancy(snapshot, vec![issued], limits, &mut remaining).unwrap(),
        CanonicalIndexReadiness::RequiresUniformBound
    );
    assert_eq!(snapshot.occupancy.len(), 1);
    let record = &snapshot.occupancy[0];
    assert_eq!(record.observations.len(), 1);
    assert_eq!(cuts(record).len(), 1, "actual query callback journal");
    assert_eq!(cuts(record)[0].arguments.len(), 1);
    assert!(matches!(cuts(record)[0].result, Value::Int(0)));
    assert!(record.vm_instructions > 0);
    let original = &record.observations[0];
    assert_eq!(original.owner.revision, expected_owner);
    assert_eq!(
        original.event.whole,
        Some(TimeSpan::new(Ratio64::ZERO, Ratio64::new(1, 2).unwrap()).unwrap())
    );
    assert!(original
        .event
        .producer
        .as_ref()
        .is_some_and(|p| !p.steps.is_empty()));
    for window in [
        TimeSpan::new(Ratio64::new(3, 4).unwrap(), Ratio64::ONE).unwrap(),
        TimeSpan::new(Ratio64::ZERO, Ratio64::new(1, 4).unwrap()).unwrap(),
    ] {
        let replay = record.replay(window, limits, &mut remaining).unwrap();
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].event.occ, original.event.occ);
        assert_eq!(replay[0].event.producer, original.event.producer);
        assert_eq!(replay[0].event.whole, original.event.whole);
        assert_eq!(replay[0].prefix, original.prefix);
        assert_eq!(replay[0].seed, original.seed);
    }
    let again = request(snapshot, 0, limits, &mut remaining);
    retain_index_occupancy(snapshot, vec![again], limits, &mut remaining).unwrap();
    assert_eq!(snapshot.occupancy.len(), 1);
    assert_eq!(
        cuts(&snapshot.occupancy[0]).len(),
        1,
        "replay never re-enters original callback"
    );
    // Ordinary query is intentionally separate and keeps its old behavior.
    assert_eq!(
        snapshot
            .query(TimeSpan::cycle(0).unwrap(), &limits)
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn pre_subject_observation_survives_genuine_empty_subject() {
    let limits = SongLimits::default();
    let mut prepared = prepared("first []");
    let snapshot = &mut prepared.snapshot;
    assert!(snapshot
        .query(TimeSpan::cycle(0).unwrap(), &limits)
        .unwrap()
        .is_empty());
    let mut remaining = limits.max_nodes;
    let issued = request(snapshot, 0, limits, &mut remaining);
    retain_index_occupancy(snapshot, vec![issued], limits, &mut remaining).unwrap();
    let record = &snapshot.occupancy[0];
    assert_eq!(record.observations.len(), 1);
    assert_eq!(cuts(record).len(), 1);
    assert!(record.observations[0].event.whole.is_some());
}
#[test]
fn pre_subject_index_footprint_survives_actual_handle_deletion() {
    let limits = SongLimits::default();
    let mut prepared = prepared_with_deletion("p", true);
    let snapshot = &mut prepared.snapshot;
    assert!(snapshot
        .query(TimeSpan::cycle(0).unwrap(), &limits)
        .unwrap()
        .is_empty());
    let mut remaining = limits.max_nodes;
    let issued = request(snapshot, 0, limits, &mut remaining);
    retain_index_occupancy(snapshot, vec![issued], limits, &mut remaining).unwrap();
    assert_eq!(snapshot.occupancy[0].observations.len(), 1);
    assert_eq!(cuts(&snapshot.occupancy[0]).len(), 1);
    assert_eq!(
        snapshot.occupancy[0].observations[0]
            .event
            .whole
            .unwrap()
            .begin,
        Ratio64::ZERO
    );
}
#[test]
fn canonical_cumulative_work_exact_one_less_and_inherited_depth_are_real_limits() {
    let limits = SongLimits::default();
    let mut measured = prepared("p");
    let mut left = limits.max_nodes;
    let issued = request(&measured.snapshot, 0, limits, &mut left);
    retain_index_occupancy(&mut measured.snapshot, vec![issued], limits, &mut left).unwrap();
    let used = limits.max_nodes - left;
    for (budget, succeeds) in [(used, true), (used - 1, false)] {
        let mut song = prepared("p");
        let mut remaining = budget;
        let issued = request(&song.snapshot, 0, limits, &mut remaining);
        let outcome =
            retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut remaining);
        if succeeds {
            outcome.unwrap();
            assert_eq!(remaining, 0);
        } else {
            assert_eq!(outcome.unwrap_err().code, FailCode::FuelExhausted);
            assert!(song.snapshot.occupancy.is_empty());
        }
    }
    let mut minimum = None;
    for max_depth in 1..=limits.max_depth {
        let exact_limits = SongLimits {
            max_depth,
            ..limits
        };
        let mut song = prepared("p");
        let mut remaining = limits.max_nodes;
        let issued = request(&song.snapshot, 0, exact_limits, &mut remaining);
        match retain_index_occupancy(
            &mut song.snapshot,
            vec![issued],
            exact_limits,
            &mut remaining,
        ) {
            Ok(_) => {
                minimum = Some(max_depth);
                break;
            }
            Err(error) => {
                assert_eq!(error.code, FailCode::DepthExceeded);
                assert!(song.snapshot.occupancy.is_empty());
            }
        }
    }
    let minimum = minimum.expect("original caller depth admits canonical execution");
    assert!(minimum > 1);
    let one_less = SongLimits {
        max_depth: minimum - 1,
        ..limits
    };
    let mut failed = prepared("p");
    let mut remaining = limits.max_nodes;
    let issued = request(&failed.snapshot, 0, one_less, &mut remaining);
    assert_eq!(
        retain_index_occupancy(&mut failed.snapshot, vec![issued], one_less, &mut remaining)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
    let mut depth = prepared("p");
    let mut remaining = limits.max_nodes;
    let issued = request(&depth.snapshot, limits.max_depth, limits, &mut remaining);
    assert_eq!(
        retain_index_occupancy(&mut depth.snapshot, vec![issued], limits, &mut remaining)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
    assert!(depth.snapshot.occupancy.is_empty());
}
#[test]
fn addressed_owner_preserves_nested_offset_seed_and_excludes_unrelated_callbacks() {
    let code = "fn unrelated beat:\n\t/ 1 {- beat beat}\nfn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {beat -> p} 2 [cut nil]\nlet base {part [drums: {s :analog} hats: {s :analog > note unrelated}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nlet intro {part [drums: {s :analog > note unrelated}] duration: 1}\nsong {sequence [intro selected]} tail-seconds: 0 > play-song";
    let mut song = from_code(code);
    let limits = SongLimits::default();
    let window = TimeSpan::cycle(1).unwrap();
    assert_eq!(
        song.snapshot.query(window, &limits).unwrap_err().code,
        FailCode::DivisionByZero,
        "genuine unrelated callback fails ordinary full query"
    );
    let mut remaining = limits.max_nodes;
    let issued = request_window(&song.snapshot, 0, window, limits, &mut remaining);
    retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut remaining).unwrap();
    let record = &song.snapshot.occupancy[0];
    assert_eq!(cuts(record).len(), 1);
    assert!(record.calls.iter().all(|call| !matches!(&call.callable, Value::Fn(f) if f.proto.name == Some(intern_sym("unrelated")))));
    assert_eq!(record.observations.len(), 1);
    let row = &record.observations[0];
    assert_eq!(row.owner.offset, Ratio64::ONE);
    assert_eq!(row.owner.window, TimeSpan::cycle(0).unwrap());
    assert!(!row.owner.placement.0.is_empty());
    assert_eq!(
        row.event.whole.unwrap().begin,
        Ratio64::ZERO,
        "issuer clock remains local"
    );
    let replay = record
        .replay(
            TimeSpan::new(Ratio64::ONE, Ratio64::new(5, 4).unwrap()).unwrap(),
            limits,
            &mut remaining,
        )
        .unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].event.occ, row.event.occ);
    assert_eq!(replay[0].seed, row.seed);
}
#[test]
fn request_from_independent_original_snapshot_cannot_remint_authority() {
    let limits = SongLimits::default();
    let first = prepared("p");
    let mut second = prepared("p");
    let mut remaining = limits.max_nodes;
    let foreign = request(&first.snapshot, 0, limits, &mut remaining);
    assert_eq!(
        retain_index_occupancy(&mut second.snapshot, vec![foreign], limits, &mut remaining)
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert!(second.snapshot.occupancy.is_empty());
}
fn nested_native() -> super::super::PreparedSong {
    from_code("fn dependent beat:\n\tfirst [61]\nlet captured {part [drums: {s :analog > note dependent}] duration: 1}\nfn cut beat:\n\tlet rows {part-events {captured} :drums 0 1}\n\t- {{first rows} :note} 61\nfn indexed p:\n\tslice {beat -> p} 2 [cut nil]\nlet base {part [drums: {s :analog}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song")
}
fn vm_state(snapshot: &mut SongSnapshot) -> (u64, usize, crate::vm::vm::EffectMode, usize) {
    let (vm, _) = snapshot.evaluator.vm_and_ns();
    assert!(vm.song_work().is_none(), "original VM scope restored");
    (vm.fuel(), vm.depth_limit, vm.effect_mode(), vm.frames.len())
}
#[test]
fn native_part_events_preserves_dependent_values_and_shared_exact_work() {
    let limits = SongLimits::default();
    let mut song = nested_native();
    let mut remaining = limits.max_nodes;
    let issued = request(&song.snapshot, 0, limits, &mut remaining);
    let before = vm_state(&mut song.snapshot);
    retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut remaining).unwrap();
    assert_eq!(vm_state(&mut song.snapshot), before);
    let record = &song.snapshot.occupancy[0];
    assert_eq!(
        record.observations.len(),
        1,
        "dependent query is computational, not output occupancy"
    );
    assert_eq!(cuts(record).len(), 1);
    assert!(
        matches!(cuts(record)[0].result, Value::Int(0)),
        "real dependent note 61 reaches native result despite outer target filter"
    );
    assert!(record.vm_instructions > 0);
    let used = limits.max_nodes - remaining;
    for (budget, succeeds) in [(used, true), (used - 1, false)] {
        let mut song = nested_native();
        let mut left = budget;
        let issued = request(&song.snapshot, 0, limits, &mut left);
        let before = vm_state(&mut song.snapshot);
        let result = retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut left);
        assert_eq!(vm_state(&mut song.snapshot), before);
        if succeeds {
            result.unwrap();
            assert_eq!(left, 0);
            assert!(matches!(
                cuts(&song.snapshot.occupancy[0])[0].result,
                Value::Int(0)
            ));
        } else {
            assert_eq!(result.unwrap_err().code, FailCode::FuelExhausted);
            assert!(song.snapshot.occupancy.is_empty());
            // The same original evaluator remains usable after failure;
            // this is restoration, not a successful partial-cache replay.
            let mut retry = limits.max_nodes;
            let issued = request(&song.snapshot, 0, limits, &mut retry);
            retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut retry).unwrap();
            assert!(matches!(
                cuts(&song.snapshot.occupancy[0])[0].result,
                Value::Int(0)
            ));
            assert_eq!(vm_state(&mut song.snapshot), before);
        }
    }
}
#[test]
fn nested_native_query_inherits_original_depth_without_refund() {
    let defaults = SongLimits::default();
    let mut minimum = None;
    for max_depth in 1..=defaults.max_depth {
        let limits = SongLimits {
            max_depth,
            ..defaults
        };
        let mut song = nested_native();
        let mut remaining = limits.max_nodes;
        let issued = request(&song.snapshot, 0, limits, &mut remaining);
        let before = vm_state(&mut song.snapshot);
        let result =
            retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut remaining);
        assert_eq!(vm_state(&mut song.snapshot), before);
        match result {
            Ok(_) => {
                minimum = Some(max_depth);
                break;
            }
            Err(error) => {
                assert_eq!(error.code, FailCode::DepthExceeded);
                assert!(song.snapshot.occupancy.is_empty());
            }
        }
    }
    let minimum = minimum.expect("genuine transitive native depth admits");
    assert!(minimum > 1);
    let limits = SongLimits {
        max_depth: minimum - 1,
        ..defaults
    };
    let mut song = nested_native();
    let mut remaining = limits.max_nodes;
    let issued = request(&song.snapshot, 0, limits, &mut remaining);
    assert_eq!(
        retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut remaining)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
}
#[test]
fn actual_vm_ticks_debit_once_and_restore_failed_scope() {
    let mut song = nested_native();
    let limits = SongLimits::default();
    let (vm, ns) = song.snapshot.evaluator.vm_and_ns();
    let slot = ns
        .session_slot(intern_sym("dependent"))
        .expect("genuine compiled callback");
    let callable = vm.read_slot(&slot).unwrap();
    let saved = (vm.fuel(), vm.depth_limit, vm.effect_mode(), vm.frames.len());
    let available = limits.max_nodes;
    let work = CanonicalIndexCollector::new(available, limits).unwrap();
    let initial = work.borrow().remaining();
    let value = vm
        .with_song_work(work.clone(), |vm| {
            vm.with_effect_mode(crate::vm::vm::EffectMode::Query, |vm| {
                vm.call_value(ns, &callable, vec![Value::Ratio(Ratio64::ZERO)], Vec::new())
            })
        })
        .unwrap();
    assert!(matches!(value, Value::Int(61)));
    assert_eq!(
        u64::from(initial - work.borrow().remaining()),
        work.borrow().vm_instructions,
        "actual instruction/native ticks debit once"
    );
    assert!(work.borrow().vm_instructions > 0);
    assert!(vm.song_work().is_none());
    assert_eq!(
        (vm.fuel(), vm.depth_limit, vm.effect_mode(), vm.frames.len()),
        saved
    );
    let empty = 1;
    let exhausted = CanonicalIndexCollector::new(empty, limits).unwrap();
    assert_eq!(
        vm.with_song_work(exhausted, |vm| {
            vm.with_effect_mode(crate::vm::vm::EffectMode::Query, |vm| {
                vm.call_value(ns, &callable, vec![Value::Ratio(Ratio64::ZERO)], Vec::new())
            })
        })
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    assert!(vm.song_work().is_none());
    assert_eq!(
        (vm.fuel(), vm.depth_limit, vm.effect_mode(), vm.frames.len()),
        saved
    );
}
#[test]
fn native_structural_thunk_retains_same_meter_and_dependent_result() {
    let mut song = from_code("fn dependent beat:\n\tfirst [61]\nlet voice {s :analog > note dependent}\nfn cut beat:\n\tlet captured {part [drums: {first [voice]}] duration: 1}\n\tlet rows {part-events captured :drums 0 1}\n\t- {{first rows} :note} 61\nfn indexed p:\n\tslice {beat -> p} 2 [cut nil]\nlet base {part [drums: {s :analog}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song");
    let limits = SongLimits::default();
    let mut left = limits.max_nodes;
    let issued = request(&song.snapshot, 0, limits, &mut left);
    let before = vm_state(&mut song.snapshot);
    retain_index_occupancy(&mut song.snapshot, vec![issued], limits, &mut left).unwrap();
    assert_eq!(vm_state(&mut song.snapshot), before);
    let record = &song.snapshot.occupancy[0];
    assert_eq!(record.observations.len(), 1);
    assert_eq!(cuts(record).len(), 1);
    assert!(matches!(cuts(record)[0].result, Value::Int(0)));
    assert!(record.vm_instructions > 0);
}

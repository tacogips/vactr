use super::super::freeze::Freeze;
use super::*;
use crate::host::caps::SampleData;
use crate::ns::{
    evaluator::Evaluator,
    namespace::{FormGen, Prelude, SlotKind},
    stage::RecordingSink,
};
use crate::reader::span::FileId;
use crate::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use crate::song::assets::{
    DecodedSongAssetFactory, SongAssetFactory, SongAssetPreparation, SongAssetSelector,
    SongSourceFile,
};
use crate::song::{prepare_song, SnapshotEpoch};
use crate::value::intern::intern_kw;
use crate::value::{
    intern_sym,
    sample::SampleBuf,
    value::{PathVal, Sound},
};
use std::sync::Arc;
fn factory() -> DecodedSongAssetFactory {
    DecodedSongAssetFactory::new(
        BTreeMap::from([(
            "ir.wav".into(),
            Arc::new(SampleData {
                rate: 48000,
                channels: 2,
                frames: vec![0.25, 0.5].into(),
            }),
        )]),
        BTreeMap::from([(intern_kw("fixed"), vec!["ir.wav".into()])]),
        BTreeMap::new(),
    )
}
fn limits() -> SongAssetLimits {
    SongAssetLimits {
        max_resources: 32,
        max_pcm_bytes: 1_000_000,
        max_source_files: 8,
        max_source_bytes: 100_000,
        max_banks: 8,
        max_walk_nodes: 1_000_000,
        max_walk_depth: 256,
    }
}
fn fresh_evaluator(
    assets: &DecodedSongAssetFactory,
    file: &str,
) -> (SongAssetPreparation, Evaluator) {
    let preparation = assets
        .begin(
            SongSourceFile {
                file: FileId::new(0),
                path: PathVal {
                    text: file.into(),
                    file: None,
                },
            },
            limits(),
        )
        .unwrap();
    let evaluator = Evaluator::new(
        Prelude::core(),
        preparation.source_loader(),
        Box::new(RecordingSink::default()),
    );
    evaluator
        .insts()
        .unwrap()
        .borrow_mut()
        .enable_closed_song_resources();
    (preparation, evaluator)
}
#[test]
fn discovery_calls_nested_callbacks_once_and_closed_copy_never_replays() {
    let assets = factory();
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    let code="inst generated:\n\tsin-osc freq > convolution ir: :fixed\nlet names [:generated :analog]\nfn outer p:\n\tevery p 3 {q -> s {first names}}\nlet base {s :analog}\nlet p {part [drums: {every base 2 outer}] duration: 6}\nsong p > play-song";
    SHAPE_CALLS.with(|calls| calls.set(0));
    let candidate = evaluate_song_candidate(code, "once.vact", 1, SnapshotEpoch(11), &cx).unwrap();
    assert_eq!(SHAPE_CALLS.with(std::cell::Cell::get), 2);
    let song = prepare_song(candidate).unwrap();
    assert_eq!(song.snapshot().routing().resources.entries().len(), 1);
    assert!(song
        .snapshot()
        .routing()
        .instruments
        .iter()
        .any(|inst| inst.name == intern_kw("generated")));
    assert_eq!(SHAPE_CALLS.with(std::cell::Cell::get), 2);
}
#[test]
fn retained_generated_bank_is_an_actual_dependency_root() {
    let assets = factory();
    for (kind, output) in ["s {first [:fixed]}", "s {sample ./ir.wav}", "s captured"]
        .into_iter()
        .enumerate()
    {
        let (mut preparation, mut evaluator) = fresh_evaluator(&assets, "roots.vact");
        let original = SampleBuf::ready(48000, vec![0.25, 0.5]);
        evaluator.ns().define(
            intern_sym("captured"),
            SlotKind::Let,
            Value::Sound(Rc::new(Sound::Buffer(original.clone()))),
            FormGen::new(0),
        );
        let code = format!("fn choose p:\n\t{output}\nsong {{part [drums: {{every {{s :analog}} 2 choose}}] duration: 2}}");
        let forms = evaluator.eval_str(&code, FileId::new(0)).unwrap();
        assert!(forms.iter().all(|form| form.value.is_ok()));
        let Value::Song(song) = forms.last().unwrap().value.as_ref().unwrap() else {
            panic!("genuine evaluated song");
        };
        SHAPE_CALLS.with(|calls| calls.set(0));
        let mut remaining = limits().max_walk_nodes;
        let mut pending = discover_shapes(&mut evaluator, song, limits(), &mut remaining).unwrap();
        assert_eq!(SHAPE_CALLS.with(std::cell::Cell::get), 1);
        let roots = pending.take_dependency_roots();
        let [Value::Pattern(root)] = roots.as_slice() else {
            panic!("one actual generated Pattern root");
        };
        let PendingShapeOutcome::Pattern(output) =
            &pending.entries.values().next().unwrap().outcome
        else {
            panic!("actual callback output");
        };
        assert!(Rc::ptr_eq(root, output));
        let deps = dependencies(&roots, limits(), &mut remaining).unwrap();
        match kind {
            0 => assert_eq!(deps.sound_keywords, vec![intern_kw("fixed")]),
            1 => assert_eq!(deps.sample_paths.len(), 1),
            _ => {
                assert_eq!(deps.buffers.len(), 1);
                assert!(Rc::ptr_eq(&deps.buffers[0], &original));
                let copy = preparation.pin_buffer(&original).unwrap();
                let mut closed = preparation.close().unwrap();
                let mut reduced = limits();
                reduced.max_walk_nodes = remaining
                    .checked_sub(u32::try_from(pending.record_count()).unwrap())
                    .unwrap();
                let mut freeze = Freeze::new(&closed, reduced);
                freeze.keep_copies(&[copy.clone()]);
                for slot in evaluator.candidate_slots() {
                    freeze.slot(&slot).unwrap();
                }
                let pending = pending.freeze_records(&mut freeze, &mut remaining).unwrap();
                let work = freeze.consumed_work();
                drop(freeze);
                remaining = remaining.checked_sub(work).unwrap();
                let shapes = pending
                    .admit_closed(&mut evaluator, &mut closed, limits(), &mut remaining)
                    .unwrap();
                assert!(shapes.rejected.is_empty());
                assert_eq!(shapes.patterns.len(), 1);
                let copied = dependencies(
                    &[Value::Pattern(
                        shapes.patterns.values().next().unwrap().clone(),
                    )],
                    limits(),
                    &mut remaining,
                )
                .unwrap();
                assert!(Rc::ptr_eq(&copied.buffers[0], &copy));
                assert_ne!(copy.id, original.id);
                assert_eq!(closed.resource_count(), 1);
            }
        }
        assert_eq!(SHAPE_CALLS.with(std::cell::Cell::get), 1);
        assert!(remaining < limits().max_walk_nodes);
    }
}
#[test]
fn generated_unknown_event_bank_remains_rejected_after_fixed_pin() {
    let assets = factory();
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    let code="inst generated:\n\tsin-osc freq > convolution ir: :fixed\nlet names [:not-pinned]\nfn choose p:\n\ts {first names}\nlet p {part [drums: {every {s :analog} 2 choose}] duration: 2}\nsong p > play-song";
    // Complete candidate discovery refuses an uncatalogued generated bank.
    let error = evaluate_song_candidate(code, "rejected.vact", 1, SnapshotEpoch(12), &cx)
        .err()
        .expect("generated bank must be closed");
    assert_eq!(error.code, FailCode::HostUnavailable);
    assert!(error.message.contains("complete catalog"));

    // Exercise the retained-output closed validator with actual evaluated
    // callbacks and a fixed-only bank, independently of pre-close pinning.
    let (mut preparation, mut evaluator) = fresh_evaluator(&assets, "rejected.vact");
    let score = code.strip_suffix(" > play-song").unwrap();
    let forms = evaluator.eval_str(score, FileId::new(0)).unwrap();
    assert!(forms.iter().all(|form| form.value.is_ok()));
    let Value::Song(song) = forms.last().unwrap().value.as_ref().unwrap() else {
        panic!("actual evaluated song");
    };
    let song = song.clone();
    let mut remaining = limits().max_walk_nodes;
    SHAPE_CALLS.with(|calls| calls.set(0));
    let pending = discover_shapes(&mut evaluator, &song, limits(), &mut remaining).unwrap();
    assert_eq!(SHAPE_CALLS.with(std::cell::Cell::get), 1);
    preparation
        .pin(&[SongAssetSelector::Bank(intern_kw("fixed"))])
        .unwrap();
    let mut closed = preparation.close().unwrap();
    let mut reduced = limits();
    reduced.max_walk_nodes = remaining
        .checked_sub(u32::try_from(pending.record_count()).unwrap())
        .unwrap();
    let mut freeze = Freeze::new(&closed, reduced);
    for slot in evaluator.candidate_slots() {
        freeze.slot(&slot).unwrap();
    }
    let Value::Song(song) = freeze.value(&Value::Song(song)).unwrap() else {
        panic!("actual frozen song");
    };
    let pending = pending.freeze_records(&mut freeze, &mut remaining).unwrap();
    let work = freeze.consumed_work();
    drop(freeze);
    remaining = remaining.checked_sub(work).unwrap();
    let shapes = pending
        .admit_closed(&mut evaluator, &mut closed, limits(), &mut remaining)
        .unwrap();
    assert_eq!(SHAPE_CALLS.with(std::cell::Cell::get), 1);
    assert_eq!(shapes.rejected.len(), 1);
    assert!(shapes
        .rejected
        .values()
        .all(|reason| *reason == R::UnclosedResource));
    let inventory = super::super::inventory::capture_routing_shared(
        &evaluator,
        &song,
        limits(),
        &shapes,
        &mut remaining,
    )
    .unwrap();
    let crate::song::snapshot::FrozenPartNode::Capture(tracks) =
        &inventory.parts[inventory.root_part].node
    else {
        panic!("actual capture");
    };
    assert!(tracks[0].1.source_uses.nodes.iter().any(|node| matches!(
        node.mapping,
        crate::song::source_uses::FrozenUseMapping::Uncertifiable(R::UnclosedResource)
    )));
    assert_eq!(closed.resource_count(), 1);
}

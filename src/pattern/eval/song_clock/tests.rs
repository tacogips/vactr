use super::*;
use crate::ns::{evaluator::Evaluator, namespace::Prelude, stage::RecordingSink};
use crate::pattern::eval::{
    song_observation::{CanonicalIndexCollector, CanonicalIndexTarget, SharedIndexWork},
    InputCells, QueryCtx,
};
use crate::reader::span::FileId;
use crate::session::song::{evaluate_song_candidate, freeze::Freeze, CandidateBuildCtx};
use crate::song::assets::{
    DecodedSongAssetFactory, PinnedSongAssets, SongAssetFactory, SongAssetLimits, SongSourceFile,
};
use crate::song::{
    PartEdit, PartNode, PreparedSong, SnapshotEpoch, Song, SongCandidate, SongEvent, SongLimits,
    SongQueryCtx,
};
use crate::value::{
    intern::{intern_kw, intern_sym},
    value::{PathVal, Value},
};
use crate::vm::{
    query_vm::{MeteredSongQuery, VmQuery},
    vm::ReadObserver,
};
use std::collections::BTreeMap;

struct Fixture {
    evaluator: Evaluator,
    original: Rc<Song>,
    assets: PinnedSongAssets,
    routing: crate::song::snapshot::FrozenRoutingInventory,
}
impl Fixture {
    fn new(body: &str) -> Self {
        let code = format!("fn cut beat:\n\tfirst [0]\nfn factor beat:\n\tfirst [2]\nfn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong selected tail-seconds: 0");
        Self::code(&code)
    }
    fn code(code: &str) -> Self {
        let limits = SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 32,
            max_source_bytes: 100_000,
            max_banks: 32,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        };
        let factory =
            DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        // Also certify the genuine isolated whole-code entrypoint; the VM
        // below uses its own original issuance, never equates their IDs.
        let context = CandidateBuildCtx {
            assets: &factory,
            asset_limits: limits,
            lock: None,
            cache: None,
        };
        evaluate_song_candidate(
            &format!("{code} > play-song"),
            "clock.vact",
            7,
            SnapshotEpoch(91),
            &context,
        )
        .unwrap();
        let preparation = factory
            .begin(
                SongSourceFile {
                    file: FileId::new(0),
                    path: PathVal {
                        text: "clock.vact".into(),
                        file: None,
                    },
                },
                limits,
            )
            .unwrap();
        let mut evaluator = Evaluator::new(
            Prelude::core(),
            preparation.source_loader(),
            Box::new(RecordingSink::default()),
        );
        let forms = evaluator.eval_str(code, FileId::new(0)).unwrap();
        assert!(
            forms.iter().all(|form| form.value.is_ok()),
            "actual forms: {:?}",
            forms.iter().map(|form| &form.value).collect::<Vec<_>>()
        );
        let value = forms.last().unwrap().value.as_ref().unwrap();
        let assets = preparation.close().unwrap();
        let mut freeze = Freeze::new(&assets, limits);
        let Value::Song(original) = freeze.value(value).unwrap() else {
            panic!("actual frozen Song");
        };
        let routing =
            crate::session::song::capture_original_test_routing(&evaluator, &original, limits)
                .unwrap();
        Self {
            evaluator,
            original,
            assets,
            routing,
        }
    }
    fn payload(&self) -> &Rc<crate::pattern::pat::Pat> {
        let PartNode::Edit {
            edit: PartEdit::TransformInstrument { pattern, .. },
            ..
        } = self.original.part().node()
        else {
            panic!("genuine selected transform");
        };
        pattern
    }
    fn collect(
        &mut self,
        window: TimeSpan,
        limits: SongLimits,
        remaining: u32,
        depth: u32,
    ) -> (SharedIndexWork, Result<(), Failure>) {
        let work = CanonicalIndexCollector::new(remaining, limits).unwrap();
        work.borrow_mut().original = Some(self.original.clone());
        work.borrow_mut().target = Some(CanonicalIndexTarget {
            revision: self.original.part().revision(),
            track: intern_kw("drums"),
            root: self.payload().id,
        });
        if let Err(failure) = super::super::song_replay::prepare_owner_dependencies(
            &self.routing,
            self.routing.root_part,
            intern_kw("drums"),
            depth,
            &work,
        ) {
            return (work, Err(failure));
        }
        let cells = InputCells::default();
        let (vm, ns) = self.evaluator.vm_and_ns();
        let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
        let mut context = SongQueryCtx {
            vm: &mut adapter,
            cells: &cells,
            seed: self.original.settings().seed,
            tempo: self.original.settings().tempo().unwrap(),
            limits: &limits,
        };
        let result = crate::song::query::observe_part(
            self.original.part(),
            window,
            &mut context,
            work.clone(),
            depth,
        );
        (work, result)
    }
    fn ordinary(&mut self, window: TimeSpan) -> Vec<SongEvent> {
        let cells = InputCells::default();
        let limits = SongLimits::default();
        let (vm, ns) = self.evaluator.vm_and_ns();
        let mut adapter = VmQuery::new(vm, ns);
        crate::song::query_part(
            self.original.part(),
            window,
            &mut SongQueryCtx {
                vm: &mut adapter,
                cells: &cells,
                seed: self.original.settings().seed,
                tempo: self.original.settings().tempo().unwrap(),
                limits: &limits,
            },
        )
        .unwrap()
    }
    fn finish(self) -> PreparedSong {
        crate::song::prepare_song(
            SongCandidate::from_isolated_evaluation(
                self.evaluator,
                self.original,
                self.assets,
                Vec::new(),
                "clock.vact".into(),
                7,
                0,
                SnapshotEpoch(91),
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn point(&mut self, piece: TimeSpan, basis: &CanonicalOwnerFrame) -> SharedIndexWork {
        let work =
            CanonicalIndexCollector::new(SongLimits::default().max_nodes, SongLimits::default())
                .unwrap();
        let payload = self.payload().clone();
        let cells = InputCells::default();
        let (vm, ns) = self.evaluator.vm_and_ns();
        let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
        let mut cx = QueryCtx {
            vm: &mut adapter,
            cells: &cells,
            seed: self.original.settings().seed,
            tempo: self.original.settings().tempo().unwrap(),
        };
        let mut state =
            super::super::QState::new_observed(&mut cx, SongLimits::default(), work.clone(), 0)
                .unwrap();
        state.observation_owner(Some(basis.clone()));
        state
            .with_clock_owner(basis, |state| {
                Ok(crate::pattern::query::q(&payload, piece, state))
            })
            .unwrap();
        assert!(
            state.faults.is_empty(),
            "actual point query: {:?}",
            state.faults
        );
        work
    }
}
fn projected(
    row: &super::super::song_observation::CanonicalIndexObservation,
    work: &SharedIndexWork,
) -> (TimeSpan, Ratio64, ClockOrientation, bool) {
    let footprint = row
        .clock
        .project_for(
            &row.owner,
            row.event.whole.unwrap(),
            row.issuer_sample_start,
            work,
        )
        .unwrap()
        .expect("genuine affine issuer context");
    assert_eq!(footprint.owner, &row.owner);
    (
        footprint.whole,
        footprint.sample_start,
        footprint.orientation,
        footprint.applicable,
    )
}
fn ratio(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).unwrap()
}
fn span(a: Ratio64, b: Ratio64) -> TimeSpan {
    TimeSpan::new(a, b).unwrap()
}
fn outer_rows(
    work: &SharedIndexWork,
) -> Vec<super::super::song_observation::CanonicalIndexObservation> {
    let ledger = work.borrow();
    let target = ledger.target.as_ref().unwrap();
    ledger
        .observations
        .iter()
        .filter(|row| {
            row.owner.revision == target.revision
                && row.owner.track == target.track
                && row.owner.root == target.root
        })
        .cloned()
        .collect()
}
struct DenyCut;
impl ReadObserver for DenyCut {
    fn on_read(&mut self, slot: &crate::ns::namespace::VarSlotRef) -> Result<(), Failure> {
        if slot.name() == intern_sym("cut") || slot.name() == intern_sym("dependent") {
            Err(Failure::new(
                FailCode::HostUnavailable,
                "original cut read denied after retained execution",
            ))
        } else {
            Ok(())
        }
    }
}
#[test]
fn actual_dynamic_fast_frames_empty_subject_and_replay_are_owner_bound() {
    for subject in ["p", "nil"] {
        let mut fixture = Fixture::new(&format!(
            "fast {{slice {{beat -> {subject}}} 2 [cut nil]}} factor"
        ));
        let window = TimeSpan::cycle(0).unwrap();
        let expected = fixture.ordinary(window);
        let (work, result) = fixture.collect(
            window,
            SongLimits::default(),
            SongLimits::default().max_nodes,
            0,
        );
        result.unwrap();
        let rows = outer_rows(&work);
        assert_eq!(rows.len(), 2);
        assert!(
            work.borrow().vm_instructions > 0,
            "actual original VM ticks"
        );
        for (i, row) in rows.iter().enumerate() {
            let CanonicalClockProjection::Known(context) = &row.clock else {
                panic!("known fast boundary");
            };
            assert!(context
                .frames
                .iter()
                .any(|frame| frame.boundary == ClockBoundary::Fast(Ratio64::from_int(2))));
            let (whole, start, orientation, applicable) = projected(row, &work);
            assert_eq!(whole, span(ratio(i as i64, 2), ratio(2 * i as i64 + 1, 4)));
            assert_eq!(start, ratio(i as i64, 2));
            assert_eq!(orientation, ClockOrientation::At);
            assert!(applicable);
            assert_eq!(row.issuer_sample_start, Ratio64::from_int(i as i64));
            let mut foreign = row.owner.clone();
            foreign.offset = Ratio64::ONE;
            assert_eq!(
                row.clock
                    .project_for(
                        &foreign,
                        row.event.whole.unwrap(),
                        row.issuer_sample_start,
                        &work
                    )
                    .unwrap_err()
                    .code,
                FailCode::Type
            );
        }
        let view = super::super::song_replay::ReplayView::publish(fixture.original.clone(), &work)
            .unwrap();
        fixture
            .evaluator
            .vm_and_ns()
            .0
            .set_read_observer(Some(Box::new(DenyCut)));
        for window in [
            span(ratio(1, 2), ratio(3, 4)),
            span(Ratio64::ZERO, ratio(1, 4)),
            TimeSpan::cycle(0).unwrap(),
        ] {
            let remaining = CanonicalIndexCollector::new(
                SongLimits::default().max_nodes,
                SongLimits::default(),
            )
            .unwrap();
            let limits = SongLimits::default();
            let cells = InputCells::default();
            let (vm, ns) = fixture.evaluator.vm_and_ns();
            let mut adapter = MeteredSongQuery::new(vm, ns, remaining.clone());
            let actual = crate::song::query::query_part_with_replay(
                fixture.original.part(),
                window,
                &mut SongQueryCtx {
                    vm: &mut adapter,
                    cells: &cells,
                    seed: fixture.original.settings().seed,
                    tempo: fixture.original.settings().tempo().unwrap(),
                    limits: &limits,
                },
                remaining,
                view.clone(),
            )
            .unwrap();
            let expected_handles: Vec<_> = expected
                .iter()
                .filter(|row| sect(row.event.part, window).is_some())
                .map(|row| row.handle.clone())
                .collect();
            assert_eq!(
                actual
                    .iter()
                    .map(|row| row.handle.clone())
                    .collect::<Vec<_>>(),
                expected_handles
            );
        }
        fixture.evaluator.vm_and_ns().0.set_read_observer(None);
        let mut owned = fixture.finish();
        assert_eq!(
            owned.query(window, &SongLimits::default()).unwrap().len(),
            expected.len()
        );
    }
}
#[test]
fn squeezed_actual_returned_pattern_keeps_uncut_whole_beyond_slot() {
    let mut fixture = Fixture::new("[{beat -> slow {slice {t -> p} 2 [0]} 2} nil]");
    for cycle in [0, 1] {
        let (work, result) = fixture.collect(
            TimeSpan::cycle(cycle).unwrap(),
            SongLimits::default(),
            SongLimits::default().max_nodes,
            0,
        );
        result.unwrap();
        let rows = outer_rows(&work);
        assert_eq!(rows.len(), 1);
        let CanonicalClockProjection::Known(context) = &rows[0].clock else {
            panic!("genuine squeezed context");
        };
        assert!(context.frames.iter().any(
            |frame| matches!(frame.boundary,ClockBoundary::Squeeze {width,..} if width==ratio(1,2))
        ));
        assert!(context
            .frames
            .iter()
            .any(|frame| frame.boundary == ClockBoundary::Fast(ratio(1, 2))));
        let (whole, _, _, applicable) = projected(&rows[0], &work);
        assert!(applicable);
        let expected = if cycle == 0 {
            span(Ratio64::ZERO, Ratio64::ONE)
        } else {
            span(ratio(1, 2), ratio(3, 2))
        };
        assert_eq!(whole, expected);
        assert!(context.frames.iter().any(|frame| matches!(
            frame.boundary,
            ClockBoundary::Squeeze { .. }
        ) && whole.duration().unwrap()
            > frame.parent_piece.duration().unwrap()));
        let actual = fixture.ordinary(TimeSpan::cycle(cycle).unwrap());
        assert_eq!(actual.len(), 2);
        assert!(actual.iter().all(|row| row.event.whole == Some(expected)));
    }
}
#[test]
fn iter_rev_point_predicates_and_two_reflections_keep_original_sample_start() {
    let mut fixture = Fixture::new("iter {rev {slice {beat -> p} 2 [cut nil]}} 2");
    let (work, result) = fixture.collect(
        TimeSpan::cycle(1).unwrap(),
        SongLimits::default(),
        SongLimits::default().max_nodes,
        0,
    );
    result.unwrap();
    let rows = outer_rows(&work);
    assert_eq!(rows.len(), 1);
    let (whole, start, orientation, _) = projected(&rows[0], &work);
    assert_eq!(whole, span(Ratio64::ONE, ratio(3, 2)));
    assert_eq!(start, ratio(3, 2));
    assert_eq!(orientation, ClockOrientation::Before);
    assert_eq!(rows[0].issuer_sample_start, Ratio64::ONE);
    for (point, expected) in [(ratio(5, 4), true), (ratio(7, 4), false)] {
        let piece = fixture.point(TimeSpan::point(point), &rows[0].owner);
        let ledger = piece.borrow();
        assert_eq!(ledger.observations.len(), 1);
        let row = ledger.observations[0].clone();
        drop(ledger);
        assert_eq!(
            projected(&row, &piece).3,
            expected,
            "actual Rev full-cycle point applicability"
        );
        let CanonicalClockProjection::Known(context) = &row.clock else {
            panic!("point issuer context");
        };
        assert!(context
            .frames
            .iter()
            .any(|frame| matches!(frame.boundary, ClockBoundary::Rev(_))
                && frame.parent_piece.is_point()
                && !frame.child_piece.is_point()));
    }
    let mut twice = Fixture::new("rev {rev {slice {beat -> p} 2 [cut nil]}}");
    let (work, result) = twice.collect(
        TimeSpan::cycle(0).unwrap(),
        SongLimits::default(),
        SongLimits::default().max_nodes,
        0,
    );
    result.unwrap();
    let rows = outer_rows(&work);
    assert_eq!(rows.len(), 1);
    assert_eq!(projected(&rows[0], &work).2, ClockOrientation::At);
    assert_eq!(projected(&rows[0], &work).1, rows[0].issuer_sample_start);
}
#[test]
fn unsupported_sampling_is_unknown_and_faulted_frame_restores_sibling() {
    let mut unknown = Fixture::new("chunk {slice {beat -> p} 2 [0]} 2 {q -> fast q 2}");
    let (work, result) = unknown.collect(
        TimeSpan::cycle(0).unwrap(),
        SongLimits::default(),
        SongLimits::default().max_nodes,
        0,
    );
    result.unwrap();
    let rows = outer_rows(&work);
    assert!(!rows.is_empty());
    assert!(rows
        .iter()
        .all(|row| row.clock == CanonicalClockProjection::Unknown));
    let mut failure = Fixture::new(
        "stack [{fast {slice {t -> p} 2 [{beat -> / 1 {- beat beat}}]} 2} {slice {t -> p} 2 [0]}]",
    );
    let (work, result) = failure.collect(
        TimeSpan::cycle(0).unwrap(),
        SongLimits::default(),
        SongLimits::default().max_nodes,
        0,
    );
    assert_eq!(result.unwrap_err().code, FailCode::DivisionByZero);
    // Scratch prerequisites may have completed before this later fault.
    // Publication is checked through the actual owning snapshot below.
    let rows = outer_rows(&work);
    assert_eq!(rows.len(), 1);
    let CanonicalClockProjection::Known(context) = &rows[0].clock else {
        panic!("sibling owner basis restored");
    };
    assert!(
        context.frames.is_empty(),
        "failed fast child frame cannot leak into sibling"
    );
    assert!(
        failure.evaluator.vm_and_ns().0.song_work().is_none(),
        "actual VM ledger restored"
    );
    crate::song::snapshot::occupancy::assert_clock_retention_transaction(true);
}
#[test]
fn nested_owner_clock_resets_without_parent_sampling_certificate() {
    let code="fn innered p:\n\tslice {t -> p} 2 [0]\nfn indexed p:\n\tfast {slice {t -> p} 2 [0]} 2\nlet base {part [drums: {s :analog}] duration: 4}\nlet inner {transform-instrument base :drums :analog innered}\nlet selected {transform-instrument inner :drums :analog indexed}\nsong selected tail-seconds: 0";
    let mut fixture = Fixture::code(code);
    let (work, result) = fixture.collect(
        TimeSpan::cycle(0).unwrap(),
        SongLimits::default(),
        SongLimits::default().max_nodes,
        0,
    );
    result.unwrap();
    let outer = outer_rows(&work);
    assert_eq!(outer.len(), 2);
    let inner: Vec<_> = work
        .borrow()
        .observations
        .iter()
        .filter(|row| row.owner.revision != outer[0].owner.revision)
        .cloned()
        .collect();
    assert!(!inner.is_empty());
    for row in &inner {
        let CanonicalClockProjection::Known(context) = &row.clock else {
            panic!("nested genuine local owner");
        };
        assert_eq!(context.owner, row.owner);
        assert!(context.frames.is_empty());
        assert_eq!(
            row.clock
                .project_for(
                    &outer[0].owner,
                    row.event.whole.unwrap(),
                    row.issuer_sample_start,
                    &work
                )
                .unwrap_err()
                .code,
            FailCode::Type
        );
    }
    for row in &outer {
        let CanonicalClockProjection::Known(context) = &row.clock else {
            panic!("parent context restored");
        };
        assert_eq!(context.owner, row.owner);
        assert!(context
            .frames
            .iter()
            .any(|frame| frame.boundary == ClockBoundary::Fast(Ratio64::from_int(2))));
    }
}
#[test]
fn actual_frame_work_and_inherited_depth_exact_one_less_are_not_refunded() {
    let body = "fast {rev {slice {beat -> p} 2 [cut nil]}} factor";
    let limits = SongLimits::default();
    let window = TimeSpan::cycle(0).unwrap();
    let mut measured = Fixture::new(body);
    let (work, result) = measured.collect(window, limits, limits.max_nodes, 0);
    result.unwrap();
    let required = limits.max_nodes - work.borrow().remaining();
    assert!(required > 1);
    for (budget, succeeds) in [(required, true), (required - 1, false)] {
        let mut fixture = Fixture::new(body);
        let (work, result) = fixture.collect(window, limits, budget, 0);
        if succeeds {
            result.unwrap();
            assert_eq!(work.borrow().remaining(), 0);
        } else {
            assert_eq!(result.unwrap_err().code, FailCode::FuelExhausted);
            // Earlier successful q records are unpublished scratch state.
        }
        assert!(fixture.evaluator.vm_and_ns().0.song_work().is_none());
    }
    crate::song::snapshot::occupancy::assert_clock_retention_transaction(false);
    let mut minimum = None;
    for depth in 1..=limits.max_depth {
        let mut fixture = Fixture::new(body);
        let small = SongLimits {
            max_depth: depth,
            ..limits
        };
        let (_, result) = fixture.collect(window, small, small.max_nodes, 1);
        match result {
            Ok(()) => {
                minimum = Some(depth);
                break;
            }
            Err(error) => assert_eq!(error.code, FailCode::DepthExceeded),
        }
    }
    let minimum = minimum.expect("actual inherited frame/VM depth admits");
    assert!(minimum > 1);
    let mut fixture = Fixture::new(body);
    let small = SongLimits {
        max_depth: minimum - 1,
        ..limits
    };
    let (_, result) = fixture.collect(window, small, small.max_nodes, 1);
    assert_eq!(result.unwrap_err().code, FailCode::DepthExceeded);
}

#[test]
fn observed_deep_reversals_keep_normal_stack_and_actual_depth_boundary() {
    // Public reduce constructs the deep Pattern through real VM calls while
    // retaining shallow, valid syntax below the parser's nesting limit.
    let body = "reduce 0..200 {slice {beat -> p} 2 [cut nil]} {acc k -> rev acc}";
    for max_nodes in [SongLimits::default().max_nodes, 1_000_000] {
        let limits = SongLimits {
            max_nodes,
            ..SongLimits::default()
        };
        let mut fixture = Fixture::new(body);
        let (work, outcome) =
            fixture.collect(TimeSpan::cycle(0).unwrap(), limits, limits.max_nodes, 0);
        match outcome {
            Ok(()) => {
                let rows = outer_rows(&work);
                assert_eq!(rows.len(), 1);
                let CanonicalClockProjection::Known(context) = &rows[0].clock else {
                    panic!("genuine observed deep owner");
                };
                assert_eq!(context.frames.len(), 200);
                assert_eq!(projected(&rows[0], &work).2, ClockOrientation::At);
            }
            Err(error) => {
                assert_eq!(max_nodes, SongLimits::default().max_nodes);
                assert_eq!(error.code, FailCode::FuelExhausted);
            }
        }
        assert!(fixture.evaluator.vm_and_ns().0.song_work().is_none());
    }
    let limits = SongLimits {
        max_depth: 32,
        ..SongLimits::default()
    };
    let mut limited = Fixture::new(body);
    let (_, outcome) = limited.collect(TimeSpan::cycle(0).unwrap(), limits, limits.max_nodes, 0);
    assert_eq!(outcome.unwrap_err().code, FailCode::DepthExceeded);
    assert!(limited.evaluator.vm_and_ns().0.song_work().is_none());
}

/// Shared genuine evaluator/freeze fixture; its returned data never mints authority.
pub(crate) fn sampling_with_replay(code: &str, inspect: impl FnOnce(&SharedIndexWork)) {
    let mut fixture = Fixture::code(code);
    let window = TimeSpan::cycle(0).unwrap();
    let expected = fixture.ordinary(window);
    let limits = SongLimits::default();
    let (work, result) = fixture.collect(window, limits, limits.max_nodes, 0);
    result.unwrap();
    inspect(&work);
    let view =
        super::super::song_replay::ReplayView::publish(fixture.original.clone(), &work).unwrap();
    fixture
        .evaluator
        .vm_and_ns()
        .0
        .set_read_observer(Some(Box::new(DenyCut)));
    for window in [
        span(ratio(1, 2), Ratio64::ONE),
        span(Ratio64::ZERO, ratio(1, 2)),
        TimeSpan::cycle(0).unwrap(),
    ] {
        let cells = InputCells::default();
        let work = CanonicalIndexCollector::new(limits.max_nodes, limits).unwrap();
        let (vm, ns) = fixture.evaluator.vm_and_ns();
        let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
        let rows = crate::song::query::query_part_with_replay(
            fixture.original.part(),
            window,
            &mut SongQueryCtx {
                vm: &mut adapter,
                cells: &cells,
                seed: fixture.original.settings().seed,
                tempo: fixture.original.settings().tempo().unwrap(),
                limits: &limits,
            },
            work,
            view.clone(),
        )
        .unwrap();
        let expected: Vec<_> = expected
            .iter()
            .filter(|row| sect(row.event.part, window).is_some())
            .collect();
        assert_eq!(rows.len(), expected.len());
        for (actual, original) in rows.iter().zip(expected) {
            assert_eq!(actual.handle, original.handle);
            assert_eq!(actual.event.whole, original.event.whole);
            assert!(crate::value::eq::deep_eq(&actual.event.value, &original.event.value).unwrap());
            assert_eq!(actual.event.controls.len(), original.event.controls.len());
            for ((a_key, a_value), (b_key, b_value)) in
                actual.event.controls.iter().zip(&original.event.controls)
            {
                assert_eq!(a_key, b_key);
                assert!(crate::value::eq::deep_eq(a_value, b_value).unwrap());
            }
            assert_eq!(actual.event.producer, original.event.producer);
            assert_eq!(
                actual
                    .event
                    .song_source
                    .as_ref()
                    .map(|origin| &origin.issued_handle),
                original
                    .event
                    .song_source
                    .as_ref()
                    .map(|origin| &origin.issued_handle)
            );
        }
    }
}

pub(crate) fn continuous_permit_isolation() {
    let mut fixture =
        Fixture::new("let index {segment {range saw 0 0} 2}\n\tslice {beat -> nil} 2 index");
    let limits = SongLimits::default();
    let (collected, result) =
        fixture.collect(TimeSpan::cycle(0).unwrap(), limits, limits.max_nodes, 0);
    result.unwrap();
    let rows = outer_rows(&collected);
    let crate::pattern::pat::PatNode::Slice {
        pat: subject,
        index,
        ..
    } = &fixture.payload().node
    else {
        panic!("actual Slice")
    };
    assert!(
        !subject.structured && index.structured,
        "genuine Index mode subject={:?} index={:?}",
        subject,
        index
    );
    assert!(
        !rows.is_empty(),
        "genuine Segment Index observations: payload={:?}, executions={}",
        fixture.payload(),
        collected.borrow().executions.len()
    );
    let basis = rows[0].owner.clone();
    let payload = fixture.payload().clone();
    let crate::pattern::pat::PatNode::Slice { pat, index, .. } = &payload.node else {
        panic!("genuine Slice")
    };
    let crate::pattern::pat::PatNode::Segment(signal, _) = &index.node else {
        panic!("genuine Segment")
    };
    let work = CanonicalIndexCollector::new(limits.max_nodes, limits).unwrap();
    let cells = InputCells::default();
    let (vm, ns) = fixture.evaluator.vm_and_ns();
    let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
    let mut cx = QueryCtx {
        vm: &mut adapter,
        cells: &cells,
        seed: fixture.original.settings().seed,
        tempo: fixture.original.settings().tempo().unwrap(),
    };
    let mut state = super::super::QState::new_observed(&mut cx, limits, work.clone(), 0).unwrap();
    state.observation_owner(Some(basis.clone()));
    state
        .with_clock_owner(&basis, |state| {
            let timing = crate::pattern::query::q(signal, TimeSpan::point(Ratio64::ZERO), state);
            assert_eq!(timing.len(), 1);
            assert_eq!(timing[0].whole, None, "actual original continuous signal");
            state.with_structural_sample(
                pat,
                Ratio64::ZERO,
                0,
                timing[0].whole,
                timing[0].part,
                Some(&timing[0]),
                |state| {
                    crate::pattern::combinators::control::sample_child(
                        &payload,
                        Ratio64::ZERO,
                        99,
                        state,
                    )?;
                    assert!(
                        matches!(state.song_clock, CanonicalClockProjection::Permit(_)),
                        "mismatch leaves the actual permit unconsumed"
                    );
                    state.with_producer(crate::pattern::occ::ProducerKind::Child, 0, |state| {
                        state.with_clock_sampling(pat, Ratio64::ZERO, |state| {
                            let projection = state.song_clock.clone();
                            assert_eq!(projection.sampling_evidence()[0].0, None);
                            assert!(projection
                                .project_for(&basis, TimeSpan::cycle(0)?, Ratio64::ZERO, &work)?
                                .is_none());
                            crate::pattern::combinators::control::sample_child(
                                &payload,
                                Ratio64::ZERO,
                                99,
                                state,
                            )?;
                            assert!(matches!(
                                state.song_clock,
                                CanonicalClockProjection::Known(_)
                            ));
                            Ok(crate::pattern::query::q(
                                pat,
                                TimeSpan::point(Ratio64::ZERO),
                                state,
                            ))
                        })
                    })?;
                    Ok(())
                },
            )?;
            let CanonicalClockProjection::Known(context) = &state.song_clock else {
                panic!("owner restored")
            };
            assert!(context.frames.is_empty());
            Ok(())
        })
        .unwrap();
    assert!(
        state.faults.is_empty(),
        "actual scoped queries: {:?}",
        state.faults
    );
    assert!(!work.borrow().observations.is_empty());
    assert!(work
        .borrow()
        .observations
        .iter()
        .all(|row| row.clock == CanonicalClockProjection::Unknown));
}

pub(crate) fn sampling_exact_work(code: &str) {
    let limits = SongLimits::default();
    let mut measured = Fixture::code(code);
    let (work, result) = measured.collect(TimeSpan::cycle(0).unwrap(), limits, limits.max_nodes, 0);
    result.unwrap();
    let required = limits.max_nodes - work.borrow().remaining();
    assert!(required > 1);
    for (budget, succeeds) in [(required, true), (required - 1, false)] {
        let mut fixture = Fixture::code(code);
        let (work, result) = fixture.collect(TimeSpan::cycle(0).unwrap(), limits, budget, 0);
        if succeeds {
            result.unwrap();
            assert_eq!(work.borrow().remaining(), 0);
        } else {
            assert_eq!(result.unwrap_err().code, FailCode::FuelExhausted);
        }
        assert!(fixture.evaluator.vm_and_ns().0.song_work().is_none());
    }
}

pub(crate) fn sampling_source_failure_restoration() {
    let code = "fn innered p:\n\tslice {t -> p} 2 [0 {beat -> / 0 {- 1.5 beat}}]\nfn indexed p:\n\tstack [{fast {slice {t -> p} 2 [0]} 2} {slice {t -> nil} 2 [0]}]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet inner {transform-instrument base :drums :analog innered}\nlet selected {transform-instrument inner :drums :analog indexed}\nsong selected tail-seconds: 0";
    let mut fixture = Fixture::code(code);
    let limits = SongLimits::default();
    let (work, result) = fixture.collect(TimeSpan::cycle(0).unwrap(), limits, limits.max_nodes, 0);
    assert_eq!(result.unwrap_err().code, FailCode::DivisionByZero);
    assert!(fixture.evaluator.vm_and_ns().0.song_work().is_none());
    let rows = outer_rows(&work);
    assert_eq!(
        rows.len(),
        3,
        "two Fast index rows and genuine final sibling"
    );
    let CanonicalClockProjection::Known(sibling) = &rows.last().unwrap().clock else {
        panic!("sibling owner restored after genuine source fault")
    };
    assert!(sibling.frames.is_empty());
    assert!(sibling.sources.is_empty());
    assert!(
        work.borrow().observations.iter().any(|row| {
            matches!(&row.clock, CanonicalClockProjection::Known(context)
            if context.sources.iter().any(|source|
                source.request == TimeSpan::point(Ratio64::ONE)
                && source.returned.get().is_none()))
        }),
        "faulted source boundary cannot publish invented returned membership"
    );
}

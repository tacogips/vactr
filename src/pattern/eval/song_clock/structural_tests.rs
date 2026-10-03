use super::tests::{sampling_exact_work, sampling_with_replay};
use super::{CanonicalClockProjection, ClockOrientation};
use crate::pattern::eval::song_observation::{CanonicalIndexObservation, SharedIndexWork};
use crate::song::{PartEdit, PartNode, Song, SongLimits, SongQueryCtx};

struct DepthFixture {
    evaluator: crate::ns::evaluator::Evaluator,
    original: std::rc::Rc<Song>,
    routing: crate::song::snapshot::FrozenRoutingInventory,
}

impl DepthFixture {
    fn new(body: &str) -> Self {
        use crate::ns::{namespace::Prelude, stage::RecordingSink};
        use crate::reader::span::FileId;
        use crate::session::song::{evaluate_song_candidate, freeze::Freeze, CandidateBuildCtx};
        use crate::song::assets::{
            DecodedSongAssetFactory, SongAssetFactory, SongAssetLimits, SongSourceFile,
        };
        use crate::value::value::{PathVal, Value};
        use std::collections::BTreeMap;

        let code = indexed_song(body);
        let asset_limits = SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 32,
            max_source_bytes: 100_000,
            max_banks: 32,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        };
        let factory = DecodedSongAssetFactory::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let context = CandidateBuildCtx {
            assets: &factory,
            asset_limits,
            lock: None,
            cache: None,
        };
        evaluate_song_candidate(
            &format!("{code} > play-song"),
            "clock.vact",
            7,
            crate::song::SnapshotEpoch(91),
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
                asset_limits,
            )
            .unwrap();
        let mut evaluator = crate::ns::evaluator::Evaluator::new(
            Prelude::core(),
            preparation.source_loader(),
            Box::new(RecordingSink::default()),
        );
        let forms = evaluator.eval_str(&code, FileId::new(0)).unwrap();
        assert!(forms.iter().all(|form| form.value.is_ok()));
        let value = forms.last().unwrap().value.as_ref().unwrap();
        let assets = preparation.close().unwrap();
        let mut freeze = Freeze::new(&assets, asset_limits);
        let Value::Song(original) = freeze.value(value).unwrap() else {
            panic!("actual frozen Song");
        };
        let routing = crate::session::song::capture_original_test_routing(
            &evaluator,
            &original,
            asset_limits,
        )
        .unwrap();
        Self {
            evaluator,
            original,
            routing,
        }
    }

    fn collect(
        &mut self,
        limits: SongLimits,
        depth: u32,
    ) -> (
        SharedIndexWork,
        std::result::Result<(), crate::vm::fail::Failure>,
    ) {
        use crate::pattern::eval::song_observation::{CanonicalIndexCollector, CanonicalIndexTarget};
        use crate::pattern::eval::InputCells;
        use crate::vm::query_vm::MeteredSongQuery;
        use crate::value::intern::intern_kw;

        let work = CanonicalIndexCollector::new(limits.max_nodes, limits).unwrap();
        work.borrow_mut().original = Some(self.original.clone());
        let PartNode::Edit {
            edit: PartEdit::TransformInstrument { pattern, .. },
            ..
        } = self.original.part().node()
        else {
            panic!("genuine selected transform");
        };
        work.borrow_mut().target = Some(CanonicalIndexTarget {
            revision: self.original.part().revision(),
            track: intern_kw("drums"),
            root: pattern.id,
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
            crate::pattern::query::TimeSpan::cycle(0).unwrap(),
            &mut context,
            work.clone(),
            depth,
        );
        (work, result)
    }
}

fn indexed_song(body: &str) -> String {
    let template = "fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tBODY\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0";
    template.replace("BODY", body)
}

fn rows(work: &SharedIndexWork) -> Vec<CanonicalIndexObservation> {
    let ledger = work.borrow();
    let Some(target) = ledger.target.as_ref() else {
        panic!("genuine target was retained");
    };
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

fn assert_unchanged_projection(work: &SharedIndexWork) {
    let observations = rows(work);
    assert!(!observations.is_empty(), "actual Slice observations");
    for row in observations {
        assert!(matches!(row.clock, CanonicalClockProjection::Known(_)));
        let whole = row.event.whole.expect("actual Index whole");
        let footprint = row
            .clock
            .project_for(&row.owner, whole, row.issuer_sample_start, work)
            .unwrap()
            .expect("canonical structural clock");
        assert_eq!(footprint.whole, whole);
        assert_eq!(footprint.sample_start, row.issuer_sample_start);
        assert_eq!(footprint.orientation, ClockOrientation::At);
        assert!(footprint.applicable);
    }
}

fn assert_euclid_sampled_projection(work: &SharedIndexWork) {
    let observations = rows(work);
    assert!(
        !observations.is_empty(),
        "actual Euclid-sampled Slice observations"
    );
    let mut any_applicable = false;
    for row in observations {
        assert!(matches!(row.clock, CanonicalClockProjection::Known(_)));
        let evidence = row.clock.sampling_evidence();
        assert_eq!(evidence.len(), 1, "one Euclid sample relation per row");
        let step = evidence[0].0.expect("Euclid sample has a step whole");
        let whole = row.event.whole.expect("actual Index whole");
        let footprint = row
            .clock
            .project_for(&row.owner, whole, row.issuer_sample_start, work)
            .unwrap()
            .expect("canonical Euclid sample clock");
        assert_eq!(footprint.whole, step);
        assert_eq!(footprint.sample_start, step.begin);
        assert_eq!(footprint.orientation, ClockOrientation::At);
        any_applicable |= footprint.applicable;
    }
    assert!(any_applicable, "at least one Euclid sample row applies");
}

#[test]
fn seven_structural_operators_preserve_slice_clocks() {
    sampling_with_replay(
        &indexed_song("euclid {slice {beat -> p} 2 [cut nil]} 1 2"),
        assert_euclid_sampled_projection,
    );
    for body in [
        "ply {slice {beat -> p} 2 [cut nil]} 2",
        "arp {slice {beat -> p} 2 [cut nil]} :up",
        "chop {slice {beat -> p} 2 [cut nil]} 2",
        "striate {slice {beat -> p} 2 [cut nil]} 2",
        "loop-at {slice {beat -> p} 2 [cut nil]} 2",
        "fit {slice {beat -> p} 2 [cut nil]}",
    ] {
        sampling_with_replay(&indexed_song(body), assert_unchanged_projection);
    }
}

#[test]
fn euclid_empty_subject_has_no_false_index_observation() {
    let code = indexed_song("euclid {slice {beat -> nil} 2 [cut nil]} 1 2");
    sampling_with_replay(&code, |work| {
        // One pulse of two steps is sampled in cycle 0; an empty Slice still records its Index row.
        assert_eq!(rows(work).len(), 1);
        assert_euclid_sampled_projection(work);
    });
}

#[test]
fn nested_fast_rev_weighted_euclid_and_chop_keep_clock_orientation() {
    let body = "euclid {chop {fast [{hold {rev {slice {beat -> p} 2 [cut nil]}} 3} nil] 2} 3} 3 8";
    sampling_with_replay(&indexed_song(body), |work| {
        assert_euclid_sampled_projection(work);
        let observations = rows(work);
        assert!(observations.iter().any(|row| {
            matches!(&row.clock, CanonicalClockProjection::Known(_))
                && !row.clock.sampling_evidence().is_empty()
        }));
    });
}

#[test]
fn split_queries_retain_the_single_query_index_rows() {
    sampling_with_replay(
        &indexed_song("euclid {chop {slice {beat -> p} 2 [cut nil]} 2} 2 4"),
        assert_euclid_sampled_projection,
    );
}

#[test]
fn striate_ranking_reuses_retained_work_without_callback_reads() {
    sampling_with_replay(
        &indexed_song("striate {slice {beat -> p} 2 [cut nil]} 3"),
        assert_unchanged_projection,
    );
}

#[test]
fn chunk_slice_clock_stays_unknown() {
    sampling_with_replay(
        &indexed_song("chunk {slice {beat -> p} 2 [cut nil]} 2 {q -> fast q 2}"),
        |work| {
            let observations = rows(work);
            assert!(!observations.is_empty());
            for row in observations {
                assert_eq!(row.clock, CanonicalClockProjection::Unknown);
                assert!(row
                    .clock
                    .project_for(
                        &row.owner,
                        row.event.whole.expect("actual Index whole"),
                        row.issuer_sample_start,
                        work,
                    )
                    .unwrap()
                    .is_none());
            }
        },
    );
}

#[test]
fn euclid_slice_query_charges_exact_and_one_less_work() {
    sampling_exact_work(&indexed_song(
        "euclid {slice {beat -> p} 2 [cut nil]} 3 8 rotation: 1",
    ));
}

#[test]
fn euclid_over_slice_obeys_inherited_depth_boundary_without_refunding_debit() {
    let body = "euclid {slice {beat -> p} 2 [cut nil]} 3 8 rotation: 1";
    let limits = SongLimits::default();
    let mut boundary = None;
    let mut immediately_above = None;
    for depth in (limits.max_depth - 32..=limits.max_depth - 1).rev() {
        let mut fixture = DepthFixture::new(body);
        let (work, result) = fixture.collect(limits, depth);
        match result {
            Ok(()) => {
                boundary = Some((depth, fixture, work));
                break;
            }
            Err(failure) => {
                assert_eq!(
                    failure.code,
                    crate::vm::fail::FailCode::DepthExceeded,
                    "every refused inherited-depth probe must be DepthExceeded"
                );
                immediately_above = Some((depth, fixture, work, failure));
            }
        }
    }
    let (boundary_depth, mut accepted, accepted_work) =
        boundary.expect("inherited depth boundary found within 32 probes");
    let (refused_depth, mut refused, refused_work, refused_failure) =
        immediately_above.expect("the probe immediately above the boundary is refused");
    assert_eq!(refused_depth, boundary_depth + 1);
    assert_eq!(
        refused_failure.code,
        crate::vm::fail::FailCode::DepthExceeded
    );
    assert!(
        refused_work.borrow().remaining() < limits.max_nodes,
        "failed inherited-depth query keeps its collector debit"
    );
    assert!(refused.evaluator.vm_and_ns().0.song_work().is_none());

    assert!(accepted.evaluator.vm_and_ns().0.song_work().is_none());
    assert_euclid_sampled_projection(&accepted_work);

    let mut max_depth = DepthFixture::new(body);
    let before = limits.max_nodes;
    let (max_depth_work, max_depth_result) = max_depth.collect(limits, limits.max_depth);
    assert_eq!(
        max_depth_result.unwrap_err().code,
        crate::vm::fail::FailCode::DepthExceeded
    );
    assert!(max_depth_work.borrow().observations.is_empty());
    assert!(max_depth_work.borrow().remaining() < before);
    assert!(max_depth.evaluator.vm_and_ns().0.song_work().is_none());
}

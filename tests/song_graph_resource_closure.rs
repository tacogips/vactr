//! Actual closed graph banks and generated family discovery in isolated candidates.
use std::{collections::BTreeMap, sync::Arc};
use vactr::dsp::build::GraphResourceSite;
use vactr::host::caps::{SampleData, SampleSrc};
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::FrozenGraphOwner;
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
use vactr::value::{intern_kw, Ratio64};
use vactr::vm::{FailCode, Failure};
fn limits() -> SongAssetLimits {
    SongAssetLimits {
        max_resources: 128,
        max_pcm_bytes: 1_000_000,
        max_source_files: 16,
        max_source_bytes: 100_000,
        max_banks: 32,
        max_walk_nodes: 1_000_000,
        max_walk_depth: 256,
    }
}
fn factory() -> DecodedSongAssetFactory {
    let samples = BTreeMap::from([
        (
            "a.wav".into(),
            Arc::new(SampleData {
                rate: 48000,
                channels: 2,
                frames: vec![0.25, 0.5].into(),
            }),
        ),
        (
            "b.wav".into(),
            Arc::new(SampleData {
                rate: 48000,
                channels: 2,
                frames: vec![0.75, 1.0].into(),
            }),
        ),
        (
            "c.wav".into(),
            Arc::new(SampleData {
                rate: 48000,
                channels: 2,
                frames: vec![-0.25, -0.5].into(),
            }),
        ),
    ]);
    let banks = BTreeMap::from([
        (intern_kw("fixed-a"), vec!["a.wav".into(), "c.wav".into()]),
        (intern_kw("fixed-b"), vec!["b.wav".into()]),
        (intern_kw("event-bank"), vec!["c.wav".into()]),
    ]);
    DecodedSongAssetFactory::new(samples, banks, BTreeMap::new())
}
fn candidate(code: &str, budget: SongAssetLimits) -> Result<PreparedSong, Failure> {
    let assets = factory();
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: budget,
        lock: None,
        cache: None,
    };
    prepare_song(evaluate_song_candidate(
        code,
        "resources.vact",
        1,
        SnapshotEpoch(501),
        &cx,
    )?)
}
const FIXED:&str="inst closed:\n\tsin-osc freq > convolution ir: :fixed-a\nbus :drums:\n\tconvolution ir: :fixed-b\nmaster:\n\tconvolution ir: :fixed-a\nlet p {part [drums: {s closed}] duration: 2}\nsong p > play-song";
#[test]
fn candidate_fixed_ir_banks_survive_backend_close() {
    let mut song = candidate(FIXED, limits()).unwrap();
    let bindings = song.snapshot().routing().resources.clone();
    assert_eq!(bindings.entries().len(), 3);
    assert!(bindings.consumed_work() > 0);
    assert_eq!(
        bindings.entries().len(),
        song.snapshot().routing().resources.entries().len()
    );
    assert!(bindings
        .entries()
        .iter()
        .any(|r| matches!(r.owner(), FrozenGraphOwner::Instrument(_))));
    assert!(bindings
        .entries()
        .iter()
        .any(|r| matches!(r.owner(), FrozenGraphOwner::Bus(_))));
    assert!(bindings
        .entries()
        .iter()
        .any(|r| matches!(r.owner(), FrozenGraphOwner::Master(_))));
    for binding in bindings.entries() {
        let data = song.sample(binding.source()).unwrap();
        assert_eq!(data.rate, 48000);
        assert!(!data.frames.is_empty());
    }
    // Entire original bank remains closed, including a member never chosen for fixed IR.
    let member = song
        .sample(&SampleSrc::Bank {
            kw: intern_kw("fixed-a"),
            index: 1,
        })
        .unwrap();
    assert_eq!(member.frames.as_ref(), [-0.25, -0.5]);
    assert_eq!(song.snapshot().resource_count(), 3);
}
#[test]
fn equal_placeholders_retain_distinct_issued_bank_and_pcm_authority() {
    let code="inst a:\n\tsin-osc freq > convolution ir: :fixed-a\ninst b:\n\tsin-osc freq > convolution ir: :fixed-b\nlet p {part [a: {s a} b: {s b}] duration: 1}\nsong p > play-song";
    let mut song = candidate(code, limits()).unwrap();
    let records = song.snapshot().routing().resources.entries().to_vec();
    assert_eq!(records.len(), 2);
    assert_ne!(records[0].owner(), records[1].owner());
    assert_ne!(
        song.sample(records[0].source()).unwrap().frames,
        song.sample(records[1].source()).unwrap().frames
    );
}
#[test]
fn header_event_resource_and_fixed_ir_keep_separate_sites() {
    let code="inst closed bank: keyword = :event-bank:\n\tsample-play bank n: 0 rate: 1 > convolution ir: :fixed-a\nlet p {part [drums: {s closed}] duration: 1}\nsong p > play-song";
    let mut song = candidate(code, limits()).unwrap();
    let records = song.snapshot().routing().resources.entries().to_vec();
    assert_eq!(records.len(), 2);
    let header = records
        .iter()
        .find(|r| matches!(r.site(), GraphResourceSite::Header { .. }))
        .unwrap();
    let fixed = records
        .iter()
        .find(|r| matches!(r.site(), GraphResourceSite::EmbeddedEffect { .. }))
        .unwrap();
    assert_eq!(
        song.sample(header.source()).unwrap().frames.as_ref(),
        [-0.25, -0.5]
    );
    assert_eq!(
        song.sample(fixed.source()).unwrap().frames.as_ref(),
        [0.25, 0.5]
    );
}
const GENERATED:&str="inst generated:\n\tsin-osc freq > convolution ir: :fixed-b\nlet words [\"choose\" \"generated\"]\nlet choices [:generated :analog]\nfn choose p:\n\tif {= {first words} \"choose\"} {s {first choices}} p\nlet base {s :analog}\nlet p {part [drums: {every base 2 choose}] duration: 3}\nsong p > play-song";
#[test]
fn actual_computed_generated_family_retains_closed_fixed_ir_and_query_bindings() {
    let mut song = candidate(GENERATED, limits()).unwrap();
    assert!(song
        .snapshot()
        .routing()
        .instruments
        .iter()
        .any(|i| i.name == intern_kw("generated")));
    let records = song.snapshot().routing().resources.entries().to_vec();
    assert_eq!(records.len(), 1);
    assert_eq!(
        song.sample(records[0].source()).unwrap().frames.as_ref(),
        [0.75, 1.0]
    );
    let full = song
        .query(
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    assert!(full.iter().any(|row| matches!(row.instrument,vactr::song::snapshot::FrozenSound::Builtin(k) if k==intern_kw("generated")) || matches!(row.instrument,vactr::song::snapshot::FrozenSound::Instrument(_))));
    for (begin, end) in [(2, 3), (0, 1), (1, 2)] {
        let rows = song
            .query(
                TimeSpan::new(Ratio64::from_int(begin), Ratio64::from_int(end)).unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        for row in rows {
            assert!(full.iter().any(|old| old.handle == row.handle));
        }
    }
    let point = song
        .query(
            TimeSpan::new(Ratio64::ZERO, Ratio64::ZERO).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    assert!(point
        .iter()
        .all(|row| full.iter().any(|old| old.handle == row.handle)));
    assert_eq!(song.snapshot().routing().resources.entries().len(), 1);
}
#[test]
fn closed_candidate_replacement_and_failure_preserve_active_resources() {
    let mut active = candidate(FIXED, limits()).unwrap();
    let original = active
        .sample(&SampleSrc::Bank {
            kw: intern_kw("fixed-a"),
            index: 0,
        })
        .unwrap();
    assert!(candidate(&format!("{FIXED}\nlet failure {{/ 1 0}}"), limits()).is_err());
    assert!(Arc::ptr_eq(
        &original,
        &active
            .sample(&SampleSrc::Bank {
                kw: intern_kw("fixed-a"),
                index: 0
            })
            .unwrap()
    ));
    let replacement="inst closed:\n\tsin-osc freq > convolution ir: :fixed-a\ninst closed:\n\tsin-osc freq > convolution ir: :fixed-b\nlet p {part [drums: {s closed}] duration: 1}\nsong p > play-song";
    let mut new = candidate(replacement, limits()).unwrap();
    let records = new.snapshot().routing().resources.entries().to_vec();
    assert_eq!(records.len(), 1);
    assert_eq!(
        new.sample(records[0].source()).unwrap().frames.as_ref(),
        [0.75, 1.0]
    );
}
#[test]
fn unresolved_missing_empty_and_unused_fixed_resources_have_honest_boundaries() {
    for resource in ["0", "1", ":missing", "{t -> 0}"] {
        let code=format!("inst closed:\n\tsin-osc freq > convolution ir: {resource}\nlet p {{part [drums: {{s closed}}] duration: 1}}\nsong p > play-song");
        assert!(candidate(&code, limits()).is_err(), "{resource}");
    }
    let code="inst unused:\n\tsin-osc freq > convolution ir: :missing\nlet p {part [drums: {s :analog}] duration: 1}\nsong p > play-song";
    let song = candidate(code, limits()).unwrap();
    assert!(song.snapshot().routing().resources.entries().is_empty());
    assert_eq!(song.snapshot().resource_count(), 0);
    let assets = DecodedSongAssetFactory::new(
        BTreeMap::new(),
        BTreeMap::from([(intern_kw("fixed-a"), vec![])]),
        BTreeMap::new(),
    );
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    assert!(evaluate_song_candidate(FIXED, "empty.vact", 1, SnapshotEpoch(1), &cx).is_err());
}
#[test]
fn resource_work_pcm_limits_are_cumulative_and_exact() {
    let mut budget = limits();
    budget.max_resources = 3;
    budget.max_pcm_bytes = 24;
    assert_eq!(candidate(FIXED, budget).unwrap().snapshot().pcm_bytes(), 24);
    budget.max_resources = 2;
    assert!(candidate(FIXED, budget).is_err());
    budget.max_resources = 3;
    budget.max_pcm_bytes = 23;
    assert!(candidate(FIXED, budget).is_err());
    budget = limits();
    let mut low = 1;
    let mut high = budget.max_walk_nodes;
    while low < high {
        let mid = low + (high - low) / 2;
        budget.max_walk_nodes = mid;
        match candidate(GENERATED, budget) {
            Ok(_) => high = mid,
            Err(error) => {
                assert!(
                    matches!(error.code, FailCode::FuelExhausted | FailCode::Overflow),
                    "{error:?}"
                );
                low = mid + 1;
            }
        }
    }
    budget.max_walk_nodes = low;
    assert!(candidate(GENERATED, budget).is_ok());
    budget.max_walk_nodes = low - 1;
    assert!(candidate(GENERATED, budget).is_err());
}
#[test]
fn generated_unpinned_event_resource_does_not_gain_fixed_bank_permission() {
    let code="fn choose p:\n\ts {sample ./not-pinned.wav}\nlet base {s :analog}\nlet p {part [drums: {every base 2 choose}] duration: 2}\nsong p > play-song";
    assert!(candidate(code, limits()).is_err());
}

#[test]
fn computed_generated_event_bank_and_path_close_without_dormant_defaults() {
    let assets = DecodedSongAssetFactory::new(
        BTreeMap::from([(
            "./c.wav".into(),
            Arc::new(SampleData {
                rate: 48000,
                channels: 2,
                frames: vec![-0.25, -0.5].into(),
            }),
        )]),
        BTreeMap::from([(intern_kw("bd"), vec!["./c.wav".into()])]),
        BTreeMap::new(),
    );
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    for (prefix, output, expected_resources) in [
        ("let names [:bd]\n", "s {first names}", 1),
        ("", "s {sample ./c.wav}", 1),
    ] {
        let code = format!("{prefix}fn choose p:\n\t{output}\nlet p {{part [drums: {{every {{s :analog}} 2 choose}}] duration: 2}}\nsong p > play-song");
        let mut song = prepare_song(
            evaluate_song_candidate(&code, "generated-events.vact", 1, SnapshotEpoch(40), &cx)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            song.snapshot().resource_count(),
            expected_resources,
            "{output}"
        );
        assert!(song.snapshot().routing().resources.entries().is_empty());
        let events = song
            .query(
                TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        assert_eq!(events.len(), 1);
        let instrument = &events[0].instrument;
        let sample = match instrument {
            vactr::song::snapshot::FrozenSound::Builtin(kw) if output.contains("first") => {
                SampleSrc::Bank { kw: *kw, index: 0 }
            }
            vactr::song::snapshot::FrozenSound::Sample { file, path }
                if output.contains("sample") =>
            {
                SampleSrc::Path(vactr::value::value::PathVal {
                    text: path.clone(),
                    file: *file,
                })
            }
            other => panic!("actual generated source {other:?}"),
        };
        assert_eq!(song.sample(&sample).unwrap().frames.as_ref(), [-0.25, -0.5]);
        let repeated = song
            .query(
                TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        assert_eq!(repeated, events);
    }
}

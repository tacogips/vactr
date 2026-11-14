//! Whole-code candidate execution and immutable query/asset evidence.
use std::collections::BTreeMap;
use std::sync::Arc;
use vactr::host::caps::SampleData;
use vactr::pattern::TimeSpan;
use vactr::session::song::{cancel_song_candidate, evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenPartNode, FrozenSound};
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits, SongPreparationState};
use vactr::value::intern::intern_kw;
use vactr::value::Ratio64;
fn limits() -> SongAssetLimits {
    SongAssetLimits {
        max_resources: 256,
        max_pcm_bytes: 4_000_000,
        max_source_files: 64,
        max_source_bytes: 1_000_000,
        max_banks: 64,
        max_walk_nodes: 100_000,
        max_walk_depth: 256,
    }
}
fn factory() -> DecodedSongAssetFactory {
    let samples = BTreeMap::from([(
        "drum.wav".into(),
        Arc::new(SampleData {
            rate: 48000,
            channels: 2,
            frames: vec![0.1, 0.1, 0.0, 0.0].into(),
        }),
    )]);
    let banks = [
        "bd", "sd", "hh", "bd-haus", "sn-dub", "bd-tek", "crash", "pluck", "break", "piano", "cp",
        "sawtooth", "vocal",
    ]
    .into_iter()
    .map(|name| (intern_kw(name), vec!["drum.wav".into()]))
    .collect();
    DecodedSongAssetFactory::new(samples, banks, BTreeMap::new())
}
fn candidate(code: &str) -> Result<PreparedSong, vactr::vm::Failure> {
    let f = factory();
    let cx = CandidateBuildCtx {
        assets: &f,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    prepare_song(evaluate_song_candidate(
        code,
        "score.vact",
        7,
        SnapshotEpoch(9),
        &cx,
    )?)
}
fn rows(
    song: &mut PreparedSong,
    begin: i64,
    end: i64,
) -> Vec<vactr::song::snapshot::FrozenSongEvent> {
    song.query(
        TimeSpan::new(Ratio64::from_int(begin), Ratio64::from_int(end)).unwrap(),
        &SongLimits::default(),
    )
    .unwrap()
}
const SIMPLE:&str="var pitch 60.25\nfn source:\n\ts :analog > note pitch\nlet p {part [drums: {{source & []}}] duration: 2}\nsong p > play-song";
#[test]
fn fresh_whole_code_is_preparing_with_stable_fractional_notes_and_identity() {
    let mut song = candidate(SIMPLE).unwrap();
    assert_eq!(song.state(), SongPreparationState::Preparing);
    assert!(song.application().is_none());
    let first = rows(&mut song, 0, 1);
    assert_eq!(first.len(), 1);
    assert_eq!(
        first[0].note,
        Some(vactr::song::ResolvedNote::Float32(60.25))
    );
    let again = rows(&mut song, 0, 1);
    assert_eq!(first[0].handle, again[0].handle);
    assert_eq!(first[0].instrument, again[0].instrument);
    assert_eq!(song.snapshot().revision(), 7);
    assert!(!song.snapshot().routing().instruments.is_empty());
}
#[test]
fn accepted_twenty_four_cycle_selected_transform_stays_lazy() {
    let text="fn drums:\n\tpart [drums: {s [:bd :sd :bd :sd]} hats: {s :hh > euclid 7 8}] duration: 4\nfn developed:\n\tlet base {drums & []}\n\tbase > transform-instrument :drums :bd {p -> lpf p 900}\nlet intro {drums & []}\nlet verse {developed & []}\nlet arrangement sequence [{part-repeat intro 2} {part-repeat verse 4}]\nsong arrangement bpm: 120 cycle-beats: 4 meter: [4 4] seed: 42 tail-seconds: 8 > play-song";
    let mut song = candidate(text).unwrap();
    assert_eq!(song.snapshot().duration(), Ratio64::from_int(24));
    let rows = rows(&mut song, 22, 23);
    assert_eq!(rows.len(), 11);
    assert!(song.snapshot().routing().parts.len() < 20);
    assert_eq!(song.snapshot().resource_count(), 3);
    assert_eq!(song.snapshot().pcm_bytes(), 48);
}
#[test]
fn exactly_one_entrypoint_and_late_errors_are_required() {
    for text in [
        "part [drums: {s :analog}] duration: 1",
        "let p {part [drums: {s :analog}] duration: 1}\nsong p > play-song\nsong p > play-song",
        "song {part [drums: {s :analog}] duration: 1} > play-song\n/ 1 0",
        "song {part [drums: {s :analog}] duration: 1} > play-song\nlet bad missing-name",
    ] {
        assert!(candidate(text).is_err(), "accepted {text}");
    }
}
#[test]
fn scheduled_audio_console_capture_and_live_input_are_rejected() {
    for prefix in [
        "s :analog > d1",
        "s :analog > once",
        "use-bpm 130",
        "print 7",
        "capture :master 1",
        "render 1",
    ] {
        let text =
            format!("{prefix}\nsong {{part [drums: {{s :analog}}] duration: 1}} > play-song");
        assert!(candidate(&text).is_err(), "accepted {prefix}");
    }
    for source in [
        "s :analog > midi-notes",
        "s :analog > gain amp",
        "s :analog > pan {midi-cc 1}",
    ] {
        assert!(candidate(&format!(
            "song {{part [drums: {{{source}}}] duration: 1}} > play-song"
        ))
        .is_err());
    }
}
#[test]
fn huge_repeat_is_symbolic_and_cancel_preserves_owner_state() {
    let mut song = candidate(
        "song {part-repeat {part [drums: {s :analog}] duration: 1} 1000000000} > play-song",
    )
    .unwrap();
    assert_eq!(song.snapshot().routing().parts.len(), 2);
    assert!(matches!(
        song.snapshot().routing().parts.last().unwrap().node,
        FrozenPartNode::Repeat {
            count: 1_000_000_000,
            ..
        }
    ));
    assert!(!rows(&mut song, 999_999_999, 1_000_000_000).is_empty());
    assert!(cancel_song_candidate(&mut song, SnapshotEpoch(8)).is_err());
    assert_eq!(song.state(), SongPreparationState::Preparing);
    cancel_song_candidate(&mut song, SnapshotEpoch(9)).unwrap();
    assert_eq!(song.state(), SongPreparationState::Failed);
    assert!(cancel_song_candidate(&mut song, SnapshotEpoch(9)).is_err());
}
#[test]
fn lazy_keyword_family_is_closed_before_query() {
    let mut song = candidate(
        "fn drum t:\n\tfirst [:bd]\nsong {part [drums: {s drum}] duration: 2} > play-song",
    )
    .unwrap();
    assert_eq!(song.snapshot().resource_count(), 1);
    assert_eq!(
        rows(&mut song, 0, 1)[0].instrument,
        FrozenSound::Builtin(intern_kw("bd"))
    );
    assert!(candidate("song {part [drums: {s ./missing.wav}] duration: 1} > play-song").is_err());
}
#[test]
fn active_globals_functions_and_kits_never_alias_candidate() {
    use vactr::ns::load::NoopHost;
    use vactr::ns::{evaluator::Evaluator, namespace::Prelude, stage::RecordingSink};
    let mut active = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    assert!(active
        .eval_str(
            "var pitch 99\nfn source:\n\ts :sd",
            vactr::reader::span::FileId::new(0)
        )
        .unwrap()
        .iter()
        .all(|f| f.value.is_ok()));
    let mut song = candidate(SIMPLE).unwrap();
    let before = rows(&mut song, 0, 1);
    assert!(active
        .eval_str(
            "upd pitch 5\nfn source:\n\ts :hh",
            vactr::reader::span::FileId::new(0)
        )
        .unwrap()
        .iter()
        .all(|f| f.value.is_ok()));
    active
        .ns()
        .prelude()
        .slot(vactr::value::intern::intern_sym("sound-kit"))
        .unwrap()
        .set(vactr::value::Value::Nil);
    let after = rows(&mut song, 0, 1);
    assert_eq!(before[0].handle, after[0].handle);
    assert_eq!(before[0].note, after[0].note);
    assert!(candidate("let changed 777\n/ 1 0").is_err());
    assert_eq!(active.ns().session_value("pitch").unwrap().to_string(), "5");
}

#[test]
fn callback_global_mutation_rejects_but_fresh_query_local_cells_work() {
    assert!(candidate("var pitch 60\nfn bad t:\n\tupd pitch 99\n\tfirst [:analog]\nsong {part [drums: {s bad}] duration: 2} > play-song").is_err());
    let mut song=candidate("fn good t:\n\tvar local 0\n\tupd local 1\n\tfirst [:analog]\nsong {part [drums: {s good}] duration: 2} > play-song").unwrap();
    assert_eq!(rows(&mut song, 0, 1).len(), 1);
}
#[test]
fn copied_inventory_resolves_used_graphs_and_strict_bus_templates() {
    let song=candidate("bus :room:\n\tplate mix: 0.2\nlet p {part [drums: {s :analog}] duration: 1}\nsong {instrument-fx p :drums :analog :room} > play-song").unwrap();
    let inventory = song.snapshot().routing();
    assert!(inventory.instruments.len() < 5);
    let id = inventory.instruments[0].graph.id;
    assert!(inventory.parts.iter().any(|part|matches!(&part.node,FrozenPartNode::Capture(tracks) if tracks[0].1.families[0].1.instrument==id)));
    assert!(
        candidate("song {part [drums: {s :analog > bus :unknown}] duration: 1} > play-song")
            .is_err()
    );
}
#[test]
fn qualified_verified_package_is_private_and_source_bytes_are_retained() {
    use std::rc::Rc;
    use vactr::ns::pkg::PackageId;
    use vactr::pkg::{
        cache::CacheBackend,
        digest::sources_digest,
        lock::{LockEntry, LockFile},
        mem_cache::MemCache,
        semver::Version,
    };
    let id = PackageId::new("github.com/example/vactr-notes");
    let version = Version::parse_tag("v1.0.0").unwrap();
    let files: Vec<(Rc<str>, Rc<[u8]>)> = vec![(
        Rc::from("mod.vact"),
        Rc::from(b"import github.com/example/vactr-tone as tone\nfn pitch:\n\tfirst [{tone.base & []}]\n".as_slice()),
    )];
    let digest = sources_digest(&files);
    let mut cache = MemCache::new();
    let staging = cache.create_staging().unwrap();
    for (path, bytes) in &files {
        cache.write(staging, path, bytes).unwrap();
    }
    cache.publish(staging, &id, &version, digest).unwrap();
    let dependency = PackageId::new("github.com/example/vactr-tone");
    let dep_files: Vec<(Rc<str>, Rc<[u8]>)> = vec![(
        Rc::from("mod.vact"),
        Rc::from(b"fn base:\n\tfirst [60.25]\n".as_slice()),
    )];
    let dep_digest = sources_digest(&dep_files);
    let staged = cache.create_staging().unwrap();
    for (path, bytes) in &dep_files {
        cache.write(staged, path, bytes).unwrap();
    }
    cache
        .publish(staged, &dependency, &version, dep_digest)
        .unwrap();
    let lock = LockFile::new(vec![
        LockEntry {
            id: id.clone(),
            version: version.clone(),
            sha256: digest,
        },
        LockEntry {
            id: dependency,
            version,
            sha256: dep_digest,
        },
    ]);
    let factory = factory();
    let cx = CandidateBuildCtx {
        assets: &factory,
        asset_limits: limits(),
        lock: Some(&lock),
        cache: Some(&cache),
    };
    let code="import github.com/example/vactr-notes as notes\nsong {part [drums: {s :analog > note {notes.pitch & []}}] duration: 1} > play-song";
    let mut song = prepare_song(
        evaluate_song_candidate(code, "score.vact", 1, SnapshotEpoch(3), &cx).unwrap(),
    )
    .unwrap();
    assert_eq!(song.snapshot().routing().sources.len(), 3);
    assert!(song
        .snapshot()
        .routing()
        .sources
        .iter()
        .any(|s| s.text.contains("fn pitch")));
    assert!(evaluate_song_candidate("song {part [drums: {s :analog > note {notes.pitch & []}}] duration: 1} > play-song\nimport github.com/example/vactr-notes as notes","score.vact",1,SnapshotEpoch(4),&cx).is_err());
    drop(cache);
    assert_eq!(
        rows(&mut song, 0, 1)[0].note,
        Some(vactr::song::ResolvedNote::Float32(60.25))
    );
    assert!(candidate(code).is_err());
}

#[test]
fn recursive_callback_slots_remain_fresh_and_resolvable() {
    let mut song=candidate("fn recurse k:\n\tif {<= k 0} 60 {recurse {- k 1}}\nsong {part [drums: {s :analog > note {t -> recurse 3}}] duration: 1} > play-song").unwrap();
    assert_eq!(
        rows(&mut song, 0, 1)[0].note,
        Some(vactr::song::ResolvedNote::Int(60))
    );
}

#[test]
fn session_keeps_previous_candidate_on_failure_and_never_reports_host_ready() {
    use std::rc::Rc;
    use vactr::session::protocol::{ApplySongBody, DocChangedBody};
    use vactr::session::{ClientMsg, Envelope, ServerMsg, Session, SessionConfig};
    let mut config = SessionConfig::new(vactr::dsp::caps::CapabilitySet::native());
    config.song_assets = Some(Rc::new(factory()));
    let mut session = Session::new(config, vactr::host::caps::Hosts::noop());
    session.set_song_asset_limits(limits()).unwrap();
    let request = ApplySongBody {
        file: "score.vact".into(),
        code: "song {part [drums: {s :analog}] duration: 1} > play-song".into(),
        doc_revision: 7,
        edit_epoch: 0,
    };
    let reply = session.apply(Envelope::new(
        1,
        None,
        ClientMsg::ApplySong(request.clone()),
    ));
    assert!(
        matches!(&reply[0].body,ServerMsg::SongCandidateFailed(b) if b.code=="host-unavailable")
    );
    let epoch = session.pending_song().unwrap().epoch();
    let mut invalid = request;
    invalid.code = "song {part [drums: {s :analog}] duration: 1} > play-song\nunknown-name".into();
    let reply = session.apply(Envelope::new(2, None, ClientMsg::ApplySong(invalid)));
    assert!(
        matches!(&reply[0].body,ServerMsg::SongCandidateFailed(b) if b.code!="host-unavailable")
    );
    assert_eq!(session.pending_song().unwrap().epoch(), epoch);
    session.apply(Envelope::new(
        3,
        None,
        ClientMsg::DocChanged(DocChangedBody {
            file: "score.vact".into(),
            doc_revision: 8,
            base_revision: 7,
            changes: vec![],
            dirty: vec![],
            edit_epoch: 1,
        }),
    ));
    assert_eq!(
        session.pending_song().unwrap().state(),
        SongPreparationState::Failed
    );
    assert!(session.cancel_pending_song(epoch).is_err());
}
#[cfg(feature = "host-native")]
#[test]
fn native_sources_and_pcm_are_closed_before_disk_and_loader_changes() {
    use vactr::host::native::loader::NativeSampleLoader;
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = Directory(std::env::temp_dir().join(format!(
            "vactr-candidate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    std::fs::create_dir_all(&dir.0).unwrap();
    let mut wav = b"RIFF".to_vec();
    wav.extend(40u32.to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(48000u32.to_le_bytes());
    wav.extend(96000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(4u32.to_le_bytes());
    wav.extend(8192i16.to_le_bytes());
    wav.extend(8192i16.to_le_bytes());
    std::fs::write(dir.0.join("drum.wav"), wav).unwrap();
    let source = "s {sample ./drum.wav} > gain 0.5";
    std::fs::write(dir.0.join("voice.vact"), source).unwrap();
    let loader = NativeSampleLoader::new(&[dir.0.clone()], &dir.0);
    let assets = loader.isolated_song_factory();
    let cx = CandidateBuildCtx {
        assets: &*assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    let code = "let voice {load ./voice.vact}\nsong {part [drums: voice] duration: 2} > play-song";
    let mut song = prepare_song(
        evaluate_song_candidate(
            code,
            dir.0.join("score.vact").to_str().unwrap(),
            1,
            SnapshotEpoch(12),
            &cx,
        )
        .unwrap(),
    )
    .unwrap();
    let before = rows(&mut song, 0, 1);
    let sample = match &before[0].instrument {
        FrozenSound::Sample { file, path } => {
            vactr::host::caps::SampleSrc::Path(vactr::value::value::PathVal {
                text: path.clone(),
                file: *file,
            })
        }
        other => panic!("unexpected source {other:?}"),
    };
    assert_eq!(song.sample(&sample).unwrap().frames[0], 0.25);
    std::fs::write(dir.0.join("voice.vact"), "unknown-name").unwrap();
    std::fs::remove_file(dir.0.join("drum.wav")).unwrap();
    loader.register_file(
        vactr::reader::span::FileId::new(0),
        std::path::Path::new("/outside/score.vact"),
    );
    assert_eq!(rows(&mut song, 0, 1), before);
    assert_eq!(song.sample(&sample).unwrap().frames[0], 0.25);
    assert!(song
        .snapshot()
        .routing()
        .sources
        .iter()
        .any(|s| &*s.text == source));
    assert!(evaluate_song_candidate(
        code,
        dir.0.join("score.vact").to_str().unwrap(),
        1,
        SnapshotEpoch(13),
        &cx
    )
    .is_err());
}

#[test]
fn selected_source_descriptors_preserve_track_family_cutoffs_and_symbolic_repetition() {
    use vactr::song::snapshot::FrozenEdit;
    let code="bus :private-room:\n\tplate mix: 1\nlet base {part [drums: {s :analog} hats: {s :sd}] duration: 1}\nlet effected {instrument-fx base :drums :analog :private-room}\nlet selected {transform-instrument effected :drums :analog {p -> fast p 2}}\nsong {sequence [selected {part-repeat selected 3} base selected]} > play-song";
    let mut song = candidate(code).unwrap();
    let inventory = song.snapshot().routing();
    assert_eq!(
        inventory.parts[inventory.root_part].duration,
        Ratio64::from_int(6)
    );
    assert!(inventory
        .parts
        .iter()
        .any(|p| matches!(p.node, FrozenPartNode::Repeat { count: 3, .. })));
    let mut selections = 0;
    for part in &inventory.parts {
        if let FrozenPartNode::Edit {
            source,
            edit:
                FrozenEdit::Transform {
                    track,
                    cutoff,
                    payload,
                    ..
                },
        } = &part.node
        {
            assert_eq!(*cutoff, inventory.parts[*source].revision);
            for selected in &payload.sources {
                selections += 1;
                assert_eq!(selected.track, *track);
                assert_eq!(
                    selected.family,
                    vec![FrozenSound::Builtin(intern_kw("analog"))]
                );
                assert_eq!(inventory.parts[selected.root_part].revision, *cutoff);
            }
        }
    }
    assert_eq!(selections, 1);
    assert!(!rows(&mut song, 0, 1).is_empty());
}

#[test]
fn candidate_carries_closed_default_cells_and_ordered_master_analysis() {
    let code="inst frozenvoice freq: float = 440:\n\tsin-osc freq > * amp\nmaster:\n\tlevel > spectrum\nsong {part [drums: {s :frozenvoice}] duration: 1} > play-song";
    let mut song = candidate(code).unwrap();
    let inventory = song.snapshot().routing();
    assert!(!inventory.cells.values().is_empty());
    assert!(inventory.cells.values().iter().any(|v| v.value == 440.0));
    assert_eq!(inventory.cells.analysis_ranges().len(), 2);
    inventory
        .cells
        .validate_graphs(inventory, &mut 100_000)
        .unwrap();
    let copied = inventory.cells.clone();
    assert_eq!(rows(&mut song, 0, 1).len(), 1);
    assert_eq!(copied.values(), song.snapshot().routing().cells.values());
    assert_eq!(
        copied.analysis_ranges(),
        song.snapshot().routing().cells.analysis_ranges()
    );
}

#[test]
fn accepted_candidate_warnings_survive_preparation_and_query() {
    let mut prepared = candidate(
        "let x 1\nfn f x:\n\tx\nsong {part [drums: {s :analog}] duration: 1} > play-song",
    )
    .unwrap();
    let warnings = prepared.snapshot().warnings().to_vec();
    assert!(warnings.iter().any(|warning| warning.contains("shadow")));
    let before = prepared.revision();
    assert!(!rows(&mut prepared, 0, 1).is_empty());
    assert_eq!(prepared.snapshot().warnings(), warnings);
    assert_eq!(prepared.revision(), before);
}

#[test]
fn isolated_probe_classifies_legacy_and_returns_original_song() {
    let assets = factory();
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    let probe = |code| {
        vactr::session::song::probe_song_candidate(code, "score.vact", 29, SnapshotEpoch(81), &cx)
    };
    assert!(probe("s :analog > d1").unwrap().is_none());
    assert!(
        evaluate_song_candidate("s :analog > d1", "score.vact", 29, SnapshotEpoch(81), &cx)
            .is_err()
    );
    let mut prepared = prepare_song(
        probe("song {part [drums: {s :analog}] duration: 1} tail-seconds: 0 > play-song")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(prepared.epoch(), SnapshotEpoch(81));
    assert_eq!(prepared.revision(), 29);
    assert_eq!(rows(&mut prepared, 0, 1).len(), 1);
}
#[test]
fn isolated_probe_rejects_mixed_effects_multiple_entries_and_real_errors() {
    let assets = factory();
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    let song = "song {part [drums: {s :analog}] duration: 1} > play-song";
    for code in [
        format!("s :analog > d1\n{song}"),
        format!("{song}\n{song}"),
        "/ 1 0".into(),
    ] {
        assert!(vactr::session::song::probe_song_candidate(
            &code,
            "score.vact",
            1,
            SnapshotEpoch(9),
            &cx
        )
        .is_err());
    }
}
#[test]
fn isolated_probe_preserves_unresolved_legacy_graph_declarations() {
    let assets = factory();
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    let code = "inst legacy:\n\tsin-osc freq > convolution ir: 0\ns :legacy > d1";
    assert!(vactr::session::song::probe_song_candidate(
        code,
        "legacy.vact",
        1,
        SnapshotEpoch(9),
        &cx
    )
    .unwrap()
    .is_none());
    assert!(vactr::session::song::probe_song_candidate(
        &format!("{code}\nsong {{part [drums: {{s :legacy}}] duration: 1}} > play-song"),
        "mixed.vact",
        1,
        SnapshotEpoch(10),
        &cx
    )
    .is_err());
}

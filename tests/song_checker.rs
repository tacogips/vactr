//! Finite song checker boundaries; execution is implemented by SONG-05.
#![allow(dead_code)]
mod support;
use vactr::reader::span::FileId;
use vactr::types::natives::{NativeMask, NativeTable};
use vactr::types::ty::{Scheme, Ty};
use vactr::types::unify::Unifier;

fn errors(text: &str) -> Vec<String> {
    let forms = support::eval::clean_forms(text, FileId::new(1));
    assert!(!forms.is_empty(), "empty reader fixture: {text}");
    let result = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    for diagnostic in &result.diags {
        if diagnostic.severity == vactr::types::Severity::Error {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
    }
    support::eval::check_errors(text, &forms)
}
fn valid(text: &str) {
    assert!(errors(text).is_empty(), "{text}: {:?}", errors(text));
}
fn invalid(text: &str) {
    assert!(!errors(text).is_empty(), "expected error: {text}");
}

#[test]
fn nominal_finite_schemes_parse_display_and_do_not_coerce() {
    for (name, ty) in [
        ("part", Ty::Part),
        ("song", Ty::Song),
        ("event-handle", Ty::EventHandle),
    ] {
        assert_eq!(Scheme::parse(name).unwrap().ty, ty);
        assert_eq!(ty.to_string(), name);
        let mut u = Unifier::new();
        assert!(u.unify(&Ty::Any, &ty).is_ok());
        for actual in [
            ty.clone(),
            Ty::List(Box::new(ty.clone())),
            Ty::func(vec![Ty::Ratio], ty.clone()),
            Ty::func(vec![], ty.clone()),
            Ty::func(vec![Ty::Ratio], Ty::List(Box::new(ty.clone()))),
        ] {
            assert!(
                u.unify(&Ty::Pattern(Box::new(Ty::Any)), &actual).is_err(),
                "{actual}"
            );
        }
    }
}

#[test]
fn all_eleven_signatures_have_exact_arity_named_args_and_masks() {
    use NativeMask::{Fn as F, Value as V};
    for (name, count, keywords, mask) in [
        ("part", 1, vec!["duration"], vec![V]),
        ("part-repeat", 2, vec!["seed-mode"], vec![V, V]),
        ("sequence", 1, vec![], vec![V]),
        ("replace-track", 3, vec![], vec![V, V, V]),
        ("transform-instrument", 4, vec![], vec![V, V, V, F]),
        ("part-events", 4, vec![], vec![V, V, V, V]),
        ("delete-event", 2, vec![], vec![V, V]),
        ("overwrite-region", 5, vec![], vec![V, V, V, V, V]),
        ("instrument-fx", 4, vec![], vec![V, V, V, V]),
        (
            "song",
            1,
            vec!["bpm", "cycle-beats", "meter", "seed", "tail-seconds"],
            vec![V],
        ),
        ("play-song", 1, vec![], vec![V]),
    ] {
        let (_, sig) = NativeTable::global().get(name).unwrap();
        assert_eq!((sig.min_args, sig.max_args), (count, Some(count)));
        assert_eq!(sig.keywords, keywords);
        assert_eq!(sig.mask, mask);
        assert!(
            sig.needs.is_empty(),
            "portable playback/construction: {name}"
        );
        assert_eq!(sig.effectful, name == "play-song");
        assert_eq!(sig.schemes().len(), 1);
        assert!(matches!(sig.schemes()[0].ty, Ty::Fn(_, _)));
    }
    assert_eq!(
        NativeTable::global()
            .get("part-events")
            .unwrap()
            .1
            .schemes()[0]
            .ty,
        Scheme::parse("fn part keyword ratio ratio -> [[keyword: any]]")
            .unwrap()
            .ty
    );
}

#[test]
fn accepted_nested_generators_and_subject_first_song_pipeline() {
    valid("fn drums:\n\tpart [drums: {s [:bd :sd]} hats: {s :hh > euclid 7 8}] duration: 4\nfn developed:\n\tlet base {drums & []}\n\tbase > transform-instrument :drums :bd {p -> lpf p 900}\nlet intro {drums & []}\nlet verse {developed & []}\nlet arrangement sequence [{part-repeat intro 2} {part-repeat verse 4 seed-mode: :vary}]\narrangement > song bpm: 126 cycle-beats: 4 meter: [4 4] seed: 42 tail-seconds: 8 > play-song");
}

#[test]
fn every_edit_and_read_only_descriptor_scheme_checks() {
    valid("let p part [drums: {s :bd}] duration: 4\nreplace-track p :drums {s :sd}\ntransform-instrument p :drums :bd {q -> gain q 0.7}\npart-events p :drums 0 4\noverwrite-region p :drums 1 2 {s :hh}\ninstrument-fx p :drums :bd :dark\nfn remove p:part h:event-handle:\n\tdelete-event p h");
    valid("sequence [] > song tail-seconds: 0");
}

#[test]
fn explicit_positive_duration_and_keyword_track_dictionary_required() {
    for source in [
        "part []",
        "part [] duration: 0",
        "part [] duration: -1",
        "part [] duration: 0.5",
        "part [] duration: :four",
        "part [1 2] duration: 4",
        "part [drums: {s :bd} drums: {s :hh}] duration: 4",
        "part [] duration: 4 duration: 5",
        "part [] duration: 4 unexpected: 1",
        "part [drums: {part [] duration: 1}] duration: 4",
    ] {
        invalid(source);
    }
    valid("part [] duration: 3/2");
}

#[test]
fn counts_and_seed_modes_are_checked_without_changing_list_repeat() {
    for count in [
        "-1",
        "1/2",
        "2.0",
        "4294967296",
        "9223372036854775806/9223372036854775807",
    ] {
        invalid(&format!("part-repeat {{part [] duration: 1}} {count}"));
    }
    for mode in [":random", "3", "\"same\""] {
        invalid(&format!(
            "part-repeat {{part [] duration: 1}} 2 seed-mode: {mode}"
        ));
    }
    valid("part-repeat {part [] duration: 1/2} 4294967295 seed-mode: :same");
    valid("part-repeat {part [] duration: 1} 0");
    valid("repeat 4 3\ncat [{s :bd} {s :hh}]");
}

#[test]
fn settings_meter_and_named_argument_types_are_checked() {
    for settings in [
        "bpm: 0",
        "cycle-beats: -1",
        "tail-seconds: -1",
        "seed: -1",
        "meter: [4]",
        "meter: [4 0]",
        "meter: [4 3]",
        "meter: [4 3/1]",
        "meter: [0 4]",
        "meter: [4 4.0]",
        "meter: :four",
        "bpm: :fast",
        "seed: 1/2",
        "meter: [4 4294967296]",
        "wrong: 4",
    ] {
        invalid(&format!("song {{part [] duration: 4}} {settings}"));
    }
    valid("song {part [] duration: 4} bpm: 135/2 cycle-beats: 7/2 meter: [7 8] seed: 9223372036854775807 tail-seconds: 0");
}

#[test]
fn handles_callbacks_and_finite_pattern_misuse_are_rejected() {
    for source in [
        "delete-event {part [] duration: 4} 0",
        "transform-instrument {part [] duration: 4} :drums :bd 7",
        "transform-instrument {part [] duration: 4} :drums :bd {p -> part [] duration: 1}",
        "transform-instrument {part [] duration: 4} :drums :bd {p -> d1 p}",
        "fast {part [] duration: 4} 2",
        "cat [{part [] duration: 4}]",
        "fn lazy t:\n\tpart [] duration: 1\nfast lazy 2",
        "play-song {part [] duration: 4}",
        "sequence [1 2]",
    ] {
        invalid(source);
    }
}

#[test]
fn overwrite_and_query_regions_are_strict_and_track_keys_are_closed_when_known() {
    for region in ["-1 2", "1 1", "2 1", "0 5"] {
        invalid(&format!(
            "overwrite-region {{part [drums: {{s :bd}}] duration: 4}} :drums {region} {{s :hh}}"
        ));
        invalid(&format!(
            "part-events {{part [drums: {{s :bd}}] duration: 4}} :drums {region}"
        ));
    }
    invalid("replace-track {part [drums: {s :bd}] duration: 4} :bass {s :sd}");
    valid("overwrite-region {part [drums: {s :bd}] duration: 4} :drums 1/2 3/2 {s :hh}");
}

#[test]
fn empty_splat_really_invokes_an_existing_zero_argument_generator() {
    let text = "fn generated:\n\t[1 2]\nlet value {generated & []}\nfirst value";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    valid(text);
    let run = support::eval::run(text, &forms);
    assert!(run.fails().is_empty(), "{:?}", run.fails());
    assert_eq!(run.last().unwrap().as_ref().unwrap().to_string(), "1");
}

#[test]
fn eager_grouped_literals_receive_the_same_bound_checks() {
    for source in [
        "part [] duration: {0}",
        "part [] duration: {{-1}}",
        "part-repeat {part [] duration: 1} {{1/2}}",
        "part-repeat {part [] duration: 1} 1 seed-mode: {:random}",
        "song {part [] duration: 1} meter: {[4 {3/1}]}",
        "song {part [] duration: 1} tail-seconds: {-1}",
        "overwrite-region {{part [drums: {s :bd}] duration: 4}} :drums {1} {1} {s :hh}",
    ] {
        invalid(source);
    }
    valid("song {part [] duration: {3/2}} meter: {[4 {4}]} tail-seconds: {0}");
}

#[test]
fn callback_purity_error_is_not_masked_by_track_or_return_type() {
    let pure =
        "transform-instrument {part [drums: {s :bd}] duration: 4} :drums :bd {q -> gain q 0.7}";
    valid(pure);
    let effectful =
        "transform-instrument {part [drums: {s :bd}] duration: 4} :drums :bd {q -> d1 q}";
    let forms = support::eval::clean_forms(effectful, FileId::new(1));
    let result = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    let failures: Vec<_> = result
        .diags
        .iter()
        .filter(|d| d.severity == vactr::types::Severity::Error)
        .collect();
    assert_eq!(failures.len(), 1, "{:?}", failures);
    assert_eq!(failures[0].code, vactr::types::DiagCode::TypeMismatch);
    assert_eq!(failures[0].message, "song transform callback must be pure");
}

#[test]
fn source_wrappers_control_rate_identity_and_named_callbacks_check() {
    for callback in [
        "{p -> s p > note 66.75}",
        "{p -> gain p 0.7}",
        "{p -> fast p 2}",
        "{p -> first [p]}",
    ] {
        valid(&format!("transform-instrument {{part [voice: {{s :analog}}] duration: 1}} :voice :analog {callback}"));
    }
    valid("fn wrap p:\n\ts p > note 66.75\ntransform-instrument {part [voice: {s :analog}] duration: 1} :voice :analog wrap");
    valid("fn identity p:\n\tfirst [p]\ntransform-instrument {part [voice: {s :analog}] duration: 1} :voice :analog identity");
}

#[test]
fn transform_callback_contract_rejects_wrong_forms_without_any_escape() {
    for callback in [
        "{p q -> gain p 0.7}",
        "{-> s :analog}",
        "{p -> + p 1}",
        "{p -> first [7]}",
        "{p -> part [] duration: 1}",
        "{p -> song {part [] duration: 1}}",
        "{p -> d1 p}",
        "7",
    ] {
        invalid(&format!("transform-instrument {{part [voice: {{s :analog}}] duration: 1}} :voice :analog {callback}"));
    }
    let text = "fn unknown p: any:\n\tfirst [p]\ntransform-instrument {part [voice: {s :analog}] duration: 1} :voice :analog unknown";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    let checked = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    assert!(checked
        .diags
        .iter()
        .any(|d| d.code == vactr::types::DiagCode::AnyNotNarrowed));
}

#[test]
fn sound_wrapper_reaches_actual_candidate_with_fractional_note() {
    use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
    use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
    use vactr::song::{prepare_song, ResolvedNote, SnapshotEpoch, SongLimits};
    use vactr::value::Ratio64;
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let cx = CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 16,
            max_pcm_bytes: 1024,
            max_source_files: 16,
            max_source_bytes: 1024,
            max_banks: 16,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    let text = "let p part [voice: {s :analog}] duration: 1\nlet edited transform-instrument p :voice :analog {p -> s p > note 66.75}\nsong edited > play-song";
    valid(text);
    let mut song = prepare_song(
        evaluate_song_candidate(text, "score.vact", 1, SnapshotEpoch(1), &cx).unwrap(),
    )
    .unwrap();
    let rows = song
        .query(
            vactr::pattern::TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].note, Some(ResolvedNote::Float32(66.75)));
    assert!(rows[0].source_origin.is_some());
}

#[test]
fn ordinary_sound_first_and_kit_errors_remain_rejected() {
    invalid("s {n [0 3]} :bd");
    invalid("s :nonexistent-song-wrapper-sound");
    invalid("s 7");
    valid("s :bd > n [0 3]");
}

#[test]
fn effectful_transform_with_valid_pattern_result_keeps_exact_purity_error() {
    let text = "transform-instrument {part [voice: {s :analog}] duration: 1} :voice :analog {p -> gain {d1 p} 0.7}";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    let checked = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    let errors: Vec<_> = checked
        .diags
        .iter()
        .filter(|d| d.severity == vactr::types::Severity::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, vactr::types::DiagCode::TypeMismatch);
    assert_eq!(errors[0].message, "song transform callback must be pure");
}

#[test]
fn ordinary_unannotated_eager_transform_chains_keep_their_source_type() {
    for depth in [20, 100, 200] {
        let mut text = "fn develop p:\n\tlet p0 p\n".to_owned();
        for i in 1..=depth {
            text.push_str(&format!("\tlet p{i} {{fast p{} 1}}\n", i - 1));
        }
        text.push_str(&format!("\tfirst [p{depth}]\nlet base {{part [drums: {{s :analog}}] duration: 1}}\nlet developed {{transform-instrument base :drums :analog {{p -> develop p}}}}\nsong developed > play-song"));
        let forms = support::eval::clean_forms(&text, FileId::new(1));
        let checked = vactr::types::check(
            &forms,
            &vactr::types::ty::CheckEnv::empty(),
            &vactr::types::HostManifest::spec_default(),
        );
        assert_eq!(
            forms.len(),
            4,
            "all candidate forms must be checked: {text}"
        );
        assert!(
            checked
                .diags
                .iter()
                .all(|d| d.severity != vactr::types::Severity::Error),
            "depth {depth}: {:?}",
            checked.diags
        );
    }
}

fn prepared_candidate(text: &str) -> Result<vactr::song::PreparedSong, vactr::vm::Failure> {
    use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
    use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let cx = CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 16,
            max_pcm_bytes: 1024,
            max_source_files: 16,
            max_source_bytes: u64::try_from(text.len()).unwrap().max(1024),
            max_banks: 16,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    vactr::song::prepare_song(evaluate_song_candidate(
        text,
        "score.vact",
        1,
        vactr::song::SnapshotEpoch(1),
        &cx,
    )?)
}

#[test]
fn actual_candidate_retains_unannotated_function_types_across_forms() {
    for depth in [20, 100, 200] {
        let mut text = "fn develop p:\n\tlet p0 p\n".to_owned();
        for i in 1..=depth {
            text.push_str(&format!("\tlet p{i} {{fast p{} 1}}\n", i - 1));
        }
        text.push_str(&format!("\tfirst [p{depth}]\nlet base {{part [drums: {{s :analog}}] duration: 1}}\nlet developed {{transform-instrument base :drums :analog {{p -> develop p}}}}\nsong developed > play-song"));
        let mut song = prepared_candidate(&text).unwrap();
        let rows = song
            .query(
                vactr::pattern::TimeSpan::new(
                    vactr::value::Ratio64::ZERO,
                    vactr::value::Ratio64::ONE,
                )
                .unwrap(),
                &vactr::song::SongLimits::default(),
            )
            .unwrap();
        assert_eq!(rows.len(), 1, "depth {depth}");
        assert!(rows[0].source_origin.is_some());
    }
}

#[test]
fn actual_named_default_keyword_and_alias_callbacks_preserve_types() {
    for callback in [
        "wrap",
        "{p -> wrap p}",
        "{p -> wrap p pitch: 67.25}",
        "alias",
    ] {
        let text = format!("fn wrap p pitch: float = 66.75:\n\ts p > note pitch\nlet alias wrap\nlet base part [voice: {{s :analog}}] duration: 1\nlet edited transform-instrument base :voice :analog {callback}\nsong edited > play-song");
        let mut song = prepared_candidate(&text).unwrap();
        let rows = song
            .query(
                vactr::pattern::TimeSpan::new(
                    vactr::value::Ratio64::ZERO,
                    vactr::value::Ratio64::ONE,
                )
                .unwrap(),
                &vactr::song::SongLimits::default(),
            )
            .unwrap();
        assert_eq!(rows.len(), 1);
        let pitch = if callback.contains("67.25") {
            67.25
        } else {
            66.75
        };
        assert_eq!(
            rows[0].note,
            Some(vactr::song::ResolvedNote::Float32(pitch))
        );
    }
    for callback in ["{p -> wrap p unknown: 4}", "{p -> wrap p pitch: \"wrong\"}"] {
        let text = format!("fn wrap p pitch: float = 66.75:\n\ts p > note pitch\nlet base part [voice: {{s :analog}}] duration: 1\nlet edited transform-instrument base :voice :analog {callback}\nsong edited > play-song");
        assert!(prepared_candidate(&text).is_err());
    }
}

#[test]
fn annotated_continuous_input_is_not_a_selected_sound_source() {
    let text = "fn wrong p: signal:\n\ts :analog\ntransform-instrument {part [voice: {s :analog}] duration: 1} :voice :analog wrong";
    let read = vactr::reader::read(text, FileId::new(1), &vactr::reader::AliasEnv::new());
    assert!(read.diags.is_empty(), "{:?}", read.diags);
    let mut expand = vactr::expand::ExpandCx::new(read.next_node_id());
    let forms: Vec<_> = read
        .nodes
        .iter()
        .map(|n| vactr::expand::expand(n, &mut expand).unwrap())
        .collect();
    assert_eq!(forms.len(), 2);
    let checked = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    let errors: Vec<_> = checked
        .diags
        .iter()
        .filter(|d| d.severity == vactr::types::Severity::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "song transform callback must map a sound/control pattern to a sound/control pattern"
    );
    let candidate =
        prepared_candidate(&format!("{text}\nsong {{part [] duration: 1}} > play-song"));
    let failure = candidate.err().expect("candidate must reject Signal input");
    assert_eq!(failure.code, vactr::vm::FailCode::Type);
    assert!(failure.message.contains("map a sound/control pattern"));
    // Nil's ordinary step/rest lifting is likewise not a callable-input proof.
    let mut env = vactr::types::ty::CheckEnv::empty();
    env.globals.insert(
        "nil-input".into(),
        vactr::types::ty::GlobalInfo {
            kind: vactr::types::ty::BindKind::Fn,
            scheme: Scheme::parse("fn nil -> ctl"),
            mask: None,
            span: None,
        },
    );
    let forms = support::eval::clean_forms(
        "transform-instrument {part [voice: {s :analog}] duration: 1} :voice :analog nil-input",
        FileId::new(1),
    );
    assert_eq!(forms.len(), 1);
    assert!(
        vactr::types::check(&forms, &env, &vactr::types::HostManifest::spec_default())
            .diags
            .iter()
            .any(|d| d.message.contains("map a sound/control pattern"))
    );
}

#[test]
fn persisted_session_qualified_and_open_keyword_views_share_quantifiers() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::rc::Rc;
    use vactr::types::ty::{BindKind, CallableSchema, CheckEnv, GlobalInfo};
    let full = CallableSchema {
        positional: 1,
        keywords: vec![Rc::from("other")],
        signature: Scheme::parse("fn 'a 'a -> 'a").unwrap(),
    };
    let info = GlobalInfo {
        kind: BindKind::Fn,
        scheme: full.positional_scheme(),
        mask: None,
        span: None,
    };
    let mut env = CheckEnv::empty();
    env.globals.insert(Rc::from("pick"), info.clone());
    env.global_callables.insert(Rc::from("pick"), full.clone());
    env.qualified
        .insert(Rc::from("pkg"), BTreeMap::from([(Rc::from("pick"), info)]));
    env.qualified_callables
        .insert(Rc::from("pkg"), BTreeMap::from([(Rc::from("pick"), full)]));
    env.opens
        .push((Rc::from("pkg"), BTreeSet::from([Rc::from("pick")])));
    for head in ["pick", "pkg.pick"] {
        for (args, fails) in [
            ("1 other: 2", false),
            ("1 other: \"wrong\"", true),
            ("1 missing: 2", true),
            ("1 2", true),
        ] {
            let text = format!("{head} {args}");
            let mut aliases = vactr::reader::AliasEnv::new();
            aliases.bind(Rc::from("pkg"), Rc::from("example/package"));
            let read = vactr::reader::read(&text, FileId::new(1), &aliases);
            assert!(read.diags.is_empty(), "{text}: {:?}", read.diags);
            let mut expand = vactr::expand::ExpandCx::new(read.next_node_id());
            let forms: Vec<_> = read
                .nodes
                .iter()
                .map(|n| vactr::expand::expand(n, &mut expand).unwrap())
                .collect();
            assert_eq!(forms.len(), 1, "{text}");
            let checked =
                vactr::types::check(&forms, &env, &vactr::types::HostManifest::spec_default());
            assert_eq!(
                checked
                    .diags
                    .iter()
                    .any(|d| d.severity == vactr::types::Severity::Error),
                fails,
                "{text}: {:?}",
                checked.diags
            );
        }
    }
    env.globals.clear();
    env.global_callables.clear();
    let forms = support::eval::clean_forms("pick 1 other: 2", FileId::new(1));
    assert!(
        vactr::types::check(&forms, &env, &vactr::types::HostManifest::spec_default())
            .diags
            .iter()
            .all(|d| d.severity != vactr::types::Severity::Error)
    );
}

fn metadata_evaluator() -> vactr::ns::evaluator::Evaluator {
    struct NoSources;
    impl vactr::ns::load::SourceLoader for NoSources {
        fn read(
            &mut self,
            _: &vactr::value::value::PathVal,
        ) -> Result<(FileId, std::rc::Rc<str>), vactr::vm::Failure> {
            Err(vactr::vm::Failure::new(
                vactr::vm::FailCode::HostUnavailable,
                "no source",
            ))
        }
    }
    vactr::ns::evaluator::Evaluator::new(
        vactr::ns::namespace::Prelude::core(),
        Box::new(NoSources),
        Box::new(vactr::ns::stage::RecordingSink::default()),
    )
}

#[test]
fn checked_metadata_tracks_binding_and_transitive_dependencies_without_unrelated_invalidation() {
    use vactr::value::intern::intern_sym;
    let mut ev = metadata_evaluator();
    let definitions = ev.eval_str("fn identity p:\n\tfirst [p]\nfn dependent p:\n\tidentity p\nfn transitive p:\n\tdependent p\nfn unrelated p:\n\tgain p 0.5", FileId::new(1)).unwrap();
    assert_eq!(definitions.len(), 4);
    assert!(definitions.iter().all(|o| o.value.is_ok()
        && o.diags
            .iter()
            .all(|d| d.severity != vactr::types::Severity::Error)));
    let certified = |ev: &vactr::ns::evaluator::Evaluator, name: &str| {
        ev.ns().check_env().global(name).unwrap().scheme.is_some()
    };
    for name in ["identity", "dependent", "transitive", "unrelated"] {
        assert!(certified(&ev, name), "{name}");
    }
    let slot = ev.ns().session_slot(intern_sym("identity")).unwrap();
    let old = slot.get();
    let version = slot.version();
    slot.set(vactr::value::Value::Int(7));
    for name in ["identity", "dependent", "transitive"] {
        assert!(!certified(&ev, name), "{name}");
    }
    assert!(certified(&ev, "unrelated"));
    slot.restore(old.clone(), version);
    for name in ["identity", "dependent", "transitive"] {
        assert!(certified(&ev, name), "restored {name}");
    }
    // Raw public storage mutation without a version bump still cannot preserve
    // a function proof: the actual Closure identity is independently checked.
    *slot.0.value.borrow_mut() = vactr::value::Value::Int(8);
    assert!(!certified(&ev, "identity"));
    assert!(!certified(&ev, "dependent"));
    slot.restore(old, version);
    let failed = ev.eval_str("let identity {/ 1 0}", FileId::new(2)).unwrap();
    assert!(failed[0].value.is_err());
    assert!(
        certified(&ev, "identity"),
        "failed replacement must retain prior proof"
    );
    assert!(certified(&ev, "transitive"));
}

#[test]
fn successful_replacement_reissues_only_its_own_checked_schema() {
    let mut ev = metadata_evaluator();
    ev.eval_str(
        "fn original p:\n\tfirst [p]\nfn dependent p:\n\toriginal p",
        FileId::new(1),
    )
    .unwrap();
    let result = ev
        .eval_str("fn original p:\n\tgain p 0.5", FileId::new(2))
        .unwrap();
    assert!(result[0].value.is_ok());
    let env = ev.ns().check_env();
    assert!(env.global("original").unwrap().scheme.is_some());
    assert!(env.global("dependent").unwrap().scheme.is_none());
    assert!(env.global_callables.contains_key("original"));
    assert!(!env.global_callables.contains_key("dependent"));
}

#[test]
fn invalid_and_nonportable_declarations_do_not_become_persisted_proofs() {
    use vactr::types::ty::{CallableSchema, TyVar, TypeId};
    let free = CallableSchema {
        positional: 1,
        keywords: vec![],
        signature: Scheme::mono(Ty::func(vec![Ty::Var(TyVar::new(99))], Ty::Sound)),
    };
    assert!(!free.portable());
    let nominal = CallableSchema {
        positional: 1,
        keywords: vec![],
        signature: Scheme::mono(Ty::func(vec![Ty::Named(TypeId::new(99))], Ty::Sound)),
    };
    assert!(!nominal.portable());
    let forms = support::eval::clean_forms("fn wrong p: float:\n\ts p", FileId::new(1));
    let checked = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    assert_eq!(forms.len(), 1);
    assert!(checked
        .diags
        .iter()
        .any(|d| d.severity == vactr::types::Severity::Error));
    assert!(checked.callables.is_empty());
    let mut ev = metadata_evaluator();
    let outcomes = ev
        .eval_str("fn wrong p: float:\n\ts p", FileId::new(1))
        .unwrap();
    assert!(outcomes[0]
        .diags
        .iter()
        .any(|d| d.severity == vactr::types::Severity::Error));
    assert!(ev
        .ns()
        .check_env()
        .global("wrong")
        .unwrap()
        .scheme
        .is_none());
}

#[test]
fn named_effectful_callback_retains_actual_query_mode_rejection() {
    let text = "fn noisy p:\n\tgain {d1 p} 0.7\nlet base part [voice: {s :analog}] duration: 1\nlet edited transform-instrument base :voice :analog noisy\nsong edited > play-song";
    let failure = prepared_candidate(text)
        .err()
        .expect("effectful callback must fail");
    assert_eq!(failure.code, vactr::vm::FailCode::EffectInQuery);
}

#[test]
fn actual_keyword_default_varref_dependencies_reject_stale_and_restore_after_failure() {
    use vactr::value::intern::intern_sym;
    let mut ev = metadata_evaluator();
    let outcomes = ev
        .eval_str(
            "var base-pitch 66.75\nfn wrap p pitch: float = base-pitch:\n\ts p > note pitch",
            FileId::new(1),
        )
        .unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.value.is_ok()
        && o.diags
            .iter()
            .all(|d| d.severity != vactr::types::Severity::Error)));
    let wrap = ev.ns().session_slot(intern_sym("wrap")).unwrap();
    let vactr::value::Value::Fn(f) = wrap.get() else {
        panic!("function");
    };
    assert!(
        f.captures
            .iter()
            .any(|v| matches!(v, vactr::value::Value::VarRef(_))),
        "real declared default must retain a late VarRef"
    );
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_some());
    let pitch = ev.ns().session_slot(intern_sym("base-pitch")).unwrap();
    let old = pitch.get();
    let version = pitch.version();
    pitch.set(vactr::value::Value::Str("changed".into()));
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_none());
    pitch.restore(old, version);
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_some());
    let failed = ev
        .eval_str("let base-pitch {/ 1 0}", FileId::new(2))
        .unwrap();
    assert!(failed[0].value.is_err());
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_some());
}

#[test]
fn actual_pattern_default_retains_late_cell_dependency() {
    use vactr::value::{intern::intern_sym, Value};
    let mut ev = metadata_evaluator();
    let text = "var base-pitch 66.75\nfn wrap p texture = {s :analog > note base-pitch}:\n\ts p";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    assert_eq!(forms.len(), 2);
    let outcomes = ev.eval_str(text, FileId::new(1)).unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes.iter().all(|o| o.value.is_ok()
            && o.diags
                .iter()
                .all(|d| d.severity != vactr::types::Severity::Error)),
        "{outcomes:?}"
    );
    let wrap = ev.ns().session_slot(intern_sym("wrap")).unwrap();
    let Value::Fn(f) = wrap.get() else {
        panic!("function");
    };
    let pattern = f
        .captures
        .iter()
        .find_map(|v| match v {
            Value::Pattern(p) => Some(p),
            _ => None,
        })
        .expect("real eager default Pattern");
    assert!(
        matches!(&pattern.node,vactr::pattern::pat::PatNode::Control(_,value,_) if matches!(&value.node,vactr::pattern::pat::PatNode::Pure(step) if matches!(&step.value,Value::VarRef(_))))
    );
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_some());
    let pitch = ev.ns().session_slot(intern_sym("base-pitch")).unwrap();
    let old = pitch.get();
    let version = pitch.version();
    pitch.set(Value::Str("changed".into()));
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_none());
    pitch.restore(old, version);
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_some());
    let failed = ev
        .eval_str("let base-pitch {/ 1 0}", FileId::new(2))
        .unwrap();
    assert!(failed[0].value.is_err());
    assert!(ev.ns().check_env().global("wrap").unwrap().scheme.is_some());
}

#[test]
fn lazy_sound_time_callback_checks_one_coherent_function() {
    for text in [
        "fn drum:\n\tfirst [:bd]\ns drum",
        "fn drum:\n\tfirst []\ns drum",
        "fn drum t:\n\tfirst [:bd]\ns drum",
        "fn drum t:\n\tfirst []\ns drum",
    ] {
        let mut ev = metadata_evaluator();
        let outcomes = ev.eval_str(text, FileId::new(1)).unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(
            outcomes.iter().all(|o| o.value.is_ok()
                && o.diags
                    .iter()
                    .all(|d| d.severity != vactr::types::Severity::Error)),
            "source={text}; schema={:?}; outcomes={outcomes:?}",
            ev.ns().check_env().global("bad")
        );
    }
    for text in [
        "fn bad:\n\tfirst [1]\ns bad",
        "fn bad -> any:\n\tfirst [nil]\ns bad",
        "fn bad:\n\tpart [drums: {s :analog}] duration: 1\ns bad",
        "fn identity t:\n\tfirst [t]\ns identity",
        "fn bad t: any:\n\tfirst [:bd]\ns bad",
        "fn bad t: string:\n\tfirst [:bd]\ns bad",
        "fn bad t:\n\tfirst [1]\ns bad",
        "fn bad a b:\n\tfirst [:bd]\ns bad",
    ] {
        let mut ev = metadata_evaluator();
        let outcomes = ev.eval_str(text, FileId::new(1)).unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(
            outcomes[1]
                .diags
                .iter()
                .any(|d| d.message.contains("lazy sound callback")),
            "source={text}; schema={:?}; outcomes={outcomes:?}",
            ev.ns().check_env().global("bad")
        );
    }
}

#[test]
fn selected_slice_time_getters_preserve_sound_control_types() {
    for body in [
        "slice {beat -> p} 2 [0 nil]",
        "slice {beat -> [p nil]} 2 [0 nil]",
        "slice {beat -> fast p 2} 2 [0 nil]",
    ] {
        valid(&format!("fn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong selected tail-seconds: 0 > play-song"));
        valid(&format!("let base {{part [drums: {{s :analog}}] duration: 4}}\ntransform-instrument base :drums :analog {{p -> {body}}}"));
    }
    valid("fn indexed p:\n\tslice {beat -> p} 2 [0 nil]\nlet a {part [drums: {s :analog}] duration: 2}\nlet b {part [drums: {s :analog}] duration: 3}\nsong {sequence [{transform-instrument a :drums :analog indexed} {transform-instrument b :drums :analog indexed}]} tail-seconds: 0 > play-song");
}
#[test]
fn selected_slice_time_getters_cannot_escape_transform_contract() {
    for body in [
        "slice {beat -> beat} 2 [0 nil]",
        "slice {a b -> p} 2 [0 nil]",
        "slice {beat -> part [drums: {s :analog}] duration: 1} 2 [0 nil]",
        "slice {beat -> d1 p} 2 [0 nil]",
    ] {
        invalid(&format!("fn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog}}] duration: 4}}\ntransform-instrument base :drums :analog indexed"));
    }
    for annotation in ["string", "any", "ctl"] {
        invalid(&format!("fn at-time beat: {annotation}:\n\ts :analog\nfn indexed p:\n\tslice at-time 2 [0 nil]\ntransform-instrument {{part [drums: {{s :analog}}] duration: 1}} :drums :analog indexed"));
    }
    invalid("fn unknown beat: ratio -> any:\n\tfirst [nil]\nfn indexed p:\n\tslice unknown 2 [0 nil]\ntransform-instrument {part [drums: {s :analog}] duration: 1} :drums :analog indexed");
}

fn query_effect_error(text: &str) {
    let forms = support::eval::clean_forms(text, FileId::new(1));
    let checked = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    assert!(
        checked
            .diags
            .iter()
            .any(|d| d.code == vactr::types::DiagCode::EffectInPattern
                && d.severity == vactr::types::Severity::Error),
        "missing actual query-effect diagnostic: {:?}",
        checked.diags
    );
}
#[test]
fn value_time_pattern_subjects_reject_actual_named_and_aliased_effects() {
    for name in ["at-time", "alias", "another"] {
        query_effect_error(&format!("fn at-time beat:\n\td1 {{s :analog}}\nlet alias at-time\nlet another alias\nslice {name} 2 [0 nil]"));
    }
    query_effect_error("fn indexed p:\n\tlet timed {beat -> d1 p}\n\tlet alias timed\n\tslice alias 2 [0 nil]\ntransform-instrument {part [drums: {s :analog}] duration: 2} :drums :analog indexed");
    query_effect_error("slice {beat -> d1 {s :analog}} 2 [0 nil]");
}
#[test]
fn value_time_pattern_subjects_keep_pure_alias_metadata_and_eager_values() {
    valid("fn getter beat: ratio pitch: float = 66:\n\ts :analog > note pitch\nlet alias getter\nalias 0 pitch: 67\nslice alias 2 [0 nil]");
    valid("slice {d1 {s :analog}} 2 [0 nil]");
    valid("fn unrelated beat:\n\td1 {s :analog}\nfn getter beat:\n\ts :analog\nslice getter 2 [0 nil]");
}
#[test]
fn value_time_pattern_effects_respect_actual_parameter_shadowing() {
    valid("fn wrapped d1:\n\tslice {beat -> d1 {s :analog}} 2 [0 nil]\nwrapped {p -> first [p]}");
    valid("slice {d1 -> s :analog} 2 [0 nil]");
}

#[test]
fn value_time_pattern_native_aliases_retain_known_effects() {
    for name in ["alias", "another"] {
        query_effect_error(&format!("let alias d1\nlet another alias\nfn getter beat:\n\t{name} {{s :analog}}\nslice getter 2 [0 nil]"));
        query_effect_error(&format!("let alias print\nlet another alias\nfn getter beat:\n\t{name} beat\n\ts :analog\nslice getter 2 [0 nil]"));
    }
    query_effect_error("let alias d1\nslice {beat -> alias {s :analog}} 2 [0 nil]");
}

#[test]
fn value_time_pattern_native_alias_facts_respect_shadowed_bindings() {
    valid("let d1 {p -> first [p]}\nlet alias d1\nfn getter beat:\n\talias {s :analog}\nslice getter 2 [0 nil]");
    valid("fn wrapped print:\n\tlet alias print\n\tslice {beat -> alias {s :analog}} 2 [0 nil]\nwrapped {p -> first [p]}");
}

#[test]
fn declared_time_result_any_is_retained_while_pure_silence_remains_valid() {
    let text = "fn unknown beat: ratio -> any:\n\tfirst [nil]\nunknown 0";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    let checked = vactr::types::check(
        &forms,
        &vactr::types::ty::CheckEnv::empty(),
        &vactr::types::HostManifest::spec_default(),
    );
    assert_eq!(checked.types.get(&forms.last().unwrap().id), Some(&Ty::Any));
    assert!(checked
        .diags
        .iter()
        .all(|d| d.severity != vactr::types::Severity::Error));
    for result in ["nil", "[]", "first [nil]"] {
        valid(&format!(
            "fn silent beat: ratio:\n\t{result}\nslice silent 2 [0 nil]"
        ));
    }
}

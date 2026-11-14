//! Public behavioral evidence for the finite symbolic value foundation.
use std::collections::BTreeMap;
use std::rc::Rc;
use vactr::pattern::pat::{Pat, PatNode};
use vactr::pattern::step::Step;
use vactr::song::*;
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;
use vactr::value::value::{Sound, Value};
use vactr::vm::fail::FailCode;
fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).unwrap()
}
fn pattern() -> Rc<Pat> {
    Rc::new(Pat::new(PatNode::Pure(Step::bare(Value::Nil)), None, false))
}
fn part(duration: Ratio64) -> Rc<Part> {
    Rc::new(capture_part(BTreeMap::from([(intern_kw("drums"), pattern())]), duration).unwrap())
}

#[test]
fn capture_positive_fractional_duration_and_timed_silence() {
    let silent = capture_part(BTreeMap::new(), r(5, 2)).unwrap();
    assert_eq!(silent.duration(), r(5, 2));
    assert!(silent.tracks().is_empty());
    assert!(matches!(silent.node(),PartNode::Capture(tracks) if tracks.is_empty()));
    for duration in [Ratio64::ZERO, Ratio64::from_int(-1)] {
        assert_eq!(
            capture_part(BTreeMap::new(), duration).unwrap_err().code,
            FailCode::Type
        );
    }
}
#[test]
fn empty_sequence_and_zero_repeat_are_empty_not_silent_loops() {
    let empty = sequence(Vec::new()).unwrap();
    assert_eq!(empty.duration(), Ratio64::ZERO);
    let base = part(r(3, 2));
    let original = base.revision();
    let zero = part_repeat(Rc::clone(&base), 0, RepeatSeedMode::Vary).unwrap();
    assert_eq!(zero.duration(), Ratio64::ZERO);
    assert!(zero.tracks().is_empty());
    assert!(matches!(zero.node(),PartNode::Sequence(parts) if parts.is_empty()));
    assert_eq!(base.revision(), original);
    assert_eq!(Rc::strong_count(&base), 1);
}
#[test]
fn repeat_is_symbolic_even_at_u32_max_and_retains_local_source() {
    let base = part(r(1, 2));
    let repeated = part_repeat(Rc::clone(&base), u32::MAX, RepeatSeedMode::Same).unwrap();
    assert_eq!(repeated.duration(), r(i64::from(u32::MAX), 2));
    assert_eq!(repeated.node_count(), 2);
    assert_eq!(repeated.depth(), 2);
    match repeated.node() {
        PartNode::Repeat {
            child,
            count,
            seed_mode,
        } => {
            assert!(Rc::ptr_eq(child, &base));
            assert_eq!(*count, u32::MAX);
            assert_eq!(*seed_mode, RepeatSeedMode::Same);
        }
        _ => panic!("repeat expanded or changed node kind"),
    }
    let song = Song::new(Rc::new(repeated), SongSettings::default()).unwrap();
    assert_eq!(
        song.frame_endpoints(48000, &SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::BeyondCapability
    );
}
#[test]
fn counts_reject_negative_fractional_and_overflow_without_truncation() {
    assert_eq!(checked_repeat_count(Ratio64::ZERO).unwrap(), 0);
    assert_eq!(
        checked_repeat_count(Ratio64::from_int(i64::from(u32::MAX))).unwrap(),
        u32::MAX
    );
    for count in [r(1, 2), Ratio64::from_int(-1)] {
        assert_eq!(
            checked_repeat_count(count).unwrap_err().code,
            FailCode::Type
        );
    }
    assert_eq!(
        checked_repeat_count(Ratio64::from_int(i64::from(u32::MAX) + 1))
            .unwrap_err()
            .code,
        FailCode::Overflow
    );
}
#[test]
fn sequence_preserves_fractional_children_and_checks_exact_overflow() {
    let a = part(r(3, 2));
    let b = part(r(2, 3));
    let joined = sequence(vec![Rc::clone(&a), Rc::clone(&b)]).unwrap();
    assert_eq!(joined.duration(), r(13, 6));
    assert_eq!(a.duration(), r(3, 2));
    assert_eq!(b.duration(), r(2, 3));
    assert!(
        matches!(joined.node(),PartNode::Sequence(v) if Rc::ptr_eq(&v[0],&a) && Rc::ptr_eq(&v[1],&b))
    );
    let largest = part(Ratio64::from_int(i64::MAX));
    assert_eq!(
        sequence(vec![Rc::clone(&largest), part(Ratio64::ONE)])
            .unwrap_err()
            .code,
        FailCode::Overflow
    );
    assert_eq!(
        part_repeat(largest, 2, RepeatSeedMode::Same)
            .unwrap_err()
            .code,
        FailCode::Overflow
    );
}
#[test]
fn duplicate_track_input_and_union_capacity_are_checked() {
    let key = intern_kw("drums");
    assert_eq!(
        Part::capture_entries(
            [(key, pattern()), (key, pattern())],
            Ratio64::ONE,
            &SongLimits::default()
        )
        .unwrap_err()
        .code,
        FailCode::Type
    );
    let a = part(Ratio64::ONE);
    let b = Rc::new(
        capture_part(
            BTreeMap::from([(intern_kw("bass"), pattern())]),
            Ratio64::ONE,
        )
        .unwrap(),
    );
    let limits = SongLimits::for_capacities(1, 10, 1000).unwrap();
    assert_eq!(
        Part::sequence(vec![a, b], &limits).unwrap_err().code,
        FailCode::BeyondCapability
    );
    let no_tracks = SongLimits::for_capacities(0, 0, 0).unwrap();
    assert!(Part::capture(BTreeMap::new(), Ratio64::ONE, &no_tracks).is_ok());
    assert!(no_tracks.check_events(0).is_ok());
    assert!(no_tracks.check_events(1).is_err());
}
#[test]
fn structural_construction_depth_and_node_limits_are_enforced() {
    let base = part(Ratio64::ONE);
    let limits = SongLimits {
        max_nodes: 1,
        ..SongLimits::default()
    };
    assert_eq!(
        Rc::clone(&base)
            .repeat(2, RepeatSeedMode::Same, &limits)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    let limits = SongLimits {
        max_depth: 2,
        ..SongLimits::default()
    };
    let twice = Rc::new(base.repeat(2, RepeatSeedMode::Same, &limits).unwrap());
    assert_eq!(
        twice
            .repeat(2, RepeatSeedMode::Same, &limits)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
    for limits in [
        SongLimits {
            max_nodes: 0,
            ..SongLimits::default()
        },
        SongLimits {
            max_depth: 257,
            ..SongLimits::default()
        },
    ] {
        assert!(limits.validate().is_err());
    }
}
#[test]
fn edits_preserve_seed_original_and_duration_but_change_revision() {
    let base = part(r(3, 2));
    let replacement = pattern();
    let edit = PartEdit::ReplaceTrack {
        track: intern_kw("drums"),
        pattern: Rc::clone(&replacement),
    };
    let changed = Rc::clone(&base).edit(edit).unwrap();
    assert_ne!(changed.revision(), base.revision());
    assert_eq!(changed.seed_identity(), base.seed_identity());
    assert_eq!(changed.duration(), base.duration());
    assert!(matches!(base.node(), PartNode::Capture(_)));
    assert!(
        matches!(changed.node(),PartNode::Edit{source,edit:PartEdit::ReplaceTrack{pattern,..}} if Rc::ptr_eq(source,&base) && Rc::ptr_eq(pattern,&replacement))
    );
    let recaptured = part(r(3, 2));
    assert_ne!(recaptured.revision(), base.revision());
    assert_eq!(recaptured.seed_identity(), base.seed_identity());
}
#[test]
fn edit_unknown_tracks_and_out_of_bounds_regions_are_rejected() {
    let base = part(Ratio64::ONE);
    let unknown = PartEdit::ReplaceTrack {
        track: intern_kw("missing"),
        pattern: pattern(),
    };
    assert_eq!(
        Rc::clone(&base).edit(unknown).unwrap_err().code,
        FailCode::UnknownField
    );
    for region in [
        vactr::pattern::query::TimeSpan {
            begin: Ratio64::ZERO,
            end: Ratio64::ZERO,
        },
        vactr::pattern::query::TimeSpan {
            begin: r(-1, 2),
            end: Ratio64::ONE,
        },
        vactr::pattern::query::TimeSpan {
            begin: Ratio64::ONE,
            end: Ratio64::ZERO,
        },
        vactr::pattern::query::TimeSpan {
            begin: Ratio64::ZERO,
            end: r(3, 2),
        },
    ] {
        let edit = PartEdit::OverwriteRegion {
            track: intern_kw("drums"),
            region,
            pattern: pattern(),
        };
        assert_eq!(
            Rc::clone(&base).edit(edit).unwrap_err().code,
            FailCode::Type
        );
    }
}
#[test]
fn settings_defaults_freeze_tempo_meter_seed_and_tail() {
    let settings = SongSettings::default();
    assert_eq!(settings.bpm, Ratio64::from_int(120));
    assert_eq!(settings.cycle_beats, Ratio64::from_int(4));
    assert_eq!(settings.meter, [4, 4]);
    assert_eq!(settings.tail_seconds, Ratio64::from_int(8));
    let base = part(r(3, 2));
    let mut input = SongSettings {
        meter: [7, 8],
        seed: 42,
        ..settings
    };
    let song = Song::new(Rc::clone(&base), input).unwrap();
    input.bpm = Ratio64::from_int(240);
    assert_eq!(song.settings().bpm, settings.bpm);
    assert_eq!(song.settings().seed, 42);
    assert_eq!(song.arrangement_seconds(), Ratio64::from_int(3));
    assert_eq!(song.deadline_seconds(), Ratio64::from_int(11));
    assert_eq!(song.duration(), base.duration());
    assert!(Rc::ptr_eq(song.part(), &base));
}
#[test]
fn invalid_timing_and_exact_seconds_overflow_fail() {
    let base = part(Ratio64::ONE);
    for invalid in [
        SongSettings {
            bpm: Ratio64::ZERO,
            ..SongSettings::default()
        },
        SongSettings {
            bpm: Ratio64::from_int(-1),
            ..SongSettings::default()
        },
        SongSettings {
            cycle_beats: Ratio64::ZERO,
            ..SongSettings::default()
        },
        SongSettings {
            meter: [0, 4],
            ..SongSettings::default()
        },
        SongSettings {
            meter: [4, 0],
            ..SongSettings::default()
        },
        SongSettings {
            meter: [4, 3],
            ..SongSettings::default()
        },
        SongSettings {
            tail_seconds: Ratio64::from_int(-1),
            ..SongSettings::default()
        },
    ] {
        assert_eq!(
            Song::new(Rc::clone(&base), invalid).unwrap_err().code,
            FailCode::Type
        );
    }
    assert_eq!(
        Song::new(part(Ratio64::from_int(i64::MAX)), SongSettings::default())
            .unwrap_err()
            .code,
        FailCode::Overflow
    );
}
#[test]
fn empty_song_can_have_only_tail_or_zero_frames() {
    let base = Rc::new(sequence(Vec::new()).unwrap());
    let song = Song::new(Rc::clone(&base), SongSettings::default()).unwrap();
    let endpoints = song.frame_endpoints(48000, &SongLimits::default()).unwrap();
    assert_eq!(endpoints.arrangement(), 0);
    assert_eq!(endpoints.deadline(), 384000);
    assert_eq!(endpoints.tail_frames(), 384000);
    let song = Song::new(
        base,
        SongSettings {
            tail_seconds: Ratio64::ZERO,
            ..SongSettings::default()
        },
    )
    .unwrap();
    assert_eq!(
        song.frame_endpoints(48000, &SongLimits::for_capacities(0, 0, 0).unwrap())
            .unwrap()
            .deadline(),
        0
    );
}
#[test]
fn absolute_frame_rounding_is_exact_ties_up_and_not_child_accumulation() {
    let settings = SongSettings {
        bpm: Ratio64::from_int(240),
        cycle_beats: Ratio64::ONE,
        tail_seconds: r(1, 4),
        ..SongSettings::default()
    };
    let combined = Rc::new(sequence(vec![part(Ratio64::ONE), part(Ratio64::ONE)]).unwrap());
    let song = Song::new(combined, settings).unwrap();
    let limits = SongLimits::default();
    let endpoints = song.frame_endpoints(2, &limits).unwrap();
    assert_eq!(endpoints.arrangement(), 1);
    assert_eq!(endpoints.deadline(), 2);
    assert_eq!(endpoints.tail_frames(), 1);
    assert_eq!(limits.frames_at(r(1, 4), 2).unwrap(), 1); // separately rounded children would sum to 2
    assert!(limits.frames_at(r(-1, 2), 48000).is_err());
    assert!(limits.frames_at(Ratio64::ONE, 0).is_err());
    let tiny = SongLimits::for_capacities(1, 100, 10).unwrap();
    assert_eq!(
        song.frame_endpoints(48000, &tiny).unwrap_err().code,
        FailCode::BeyondCapability
    );
    assert!(FrameEndpoints::new(5, 4).is_err());
    assert_eq!(FrameEndpoints::new(5, 5).unwrap().tail_frames(), 0);
    let unlimited = SongLimits::for_capacities(64, 100, u64::MAX).unwrap();
    assert_eq!(
        unlimited
            .frames_at(Ratio64::from_int(i64::MAX), u32::MAX)
            .unwrap_err()
            .code,
        FailCode::Overflow
    );
}
#[test]
fn family_selection_is_frozen_complete_and_audio_only() {
    let bd = Sound::Builtin(intern_kw("bd"));
    let hh = Sound::Builtin(intern_kw("hh"));
    let selector = InstrumentSelector::new(vec![bd.clone(), bd.clone(), hh.clone()]).unwrap();
    assert_eq!(selector.family().len(), 2);
    assert!(selector.contains(&bd));
    assert!(selector.contains(&hh));
    assert!(!selector.contains(&Sound::Builtin(intern_kw("snare"))));
    assert!(InstrumentSelector::new(Vec::new()).is_err());
    assert!(InstrumentSelector::new(vec![Sound::MidiOut(1)]).is_err());
}
#[test]
fn resolved_notes_preserve_fractional_and_float64_values_without_chords() {
    assert_eq!(
        ResolvedNote::try_from(&Value::Ratio(r(121, 2))).unwrap(),
        ResolvedNote::Ratio(r(121, 2))
    );
    let precise = 60.12345678901234;
    assert_eq!(
        ResolvedNote::try_from(&Value::Float64(precise)).unwrap(),
        ResolvedNote::Float64(precise)
    );
    assert!(ResolvedNote::try_from(&Value::Float64(f64::NAN)).is_err());
    assert!(ResolvedNote::try_from(&Value::Float(f32::INFINITY)).is_err());
    assert!(ResolvedNote::try_from(&Value::list(vec![Value::Int(60)])).is_err());
    assert_eq!(NoteCommitMode::Mono, NoteCommitMode::Mono);
}

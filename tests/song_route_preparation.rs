//! Actual frozen route preparation and bounded configuration lifetime admission.
use vactr::song::routing::SongHostCapacities;
use vactr::song::SnapshotEpoch;

fn frozen_routes(code: &str) -> vactr::song::PreparedSong {
    use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
    let factory = DecodedSongAssetFactory::new(
        std::collections::BTreeMap::from([(
            "./drum.wav".into(),
            std::sync::Arc::new(vactr::host::caps::SampleData {
                rate: 48000,
                channels: 2,
                frames: vec![0.1, 0.1, 0.2, 0.2].into(),
            }),
        )]),
        Default::default(),
        Default::default(),
    );
    let cx = vactr::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 16_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    vactr::song::prepare_song(
        vactr::session::song::evaluate_song_candidate(
            code,
            "routes.vact",
            1,
            SnapshotEpoch(100),
            &cx,
        )
        .unwrap(),
    )
    .unwrap()
}
fn route_capacity() -> SongHostCapacities {
    SongHostCapacities {
        sample_rate: 48000,
        cell_slots: 4096,
        voice_slots: 8,
        template_slots: 256,
        bus_slots: 256,
        sample_resources: 256,
        pcm_bytes: 16_000_000,
        voice_frames: 8 * 192000,
        bus_frames: 128_000_000,
        ack_slots: 1024,
    }
}
#[test]
fn neutral_sibling_tracks_admit_distinct_private_room_delay_and_master_stages() {
    use vactr::song::routing::prepare_routes;
    let song = frozen_routes("song {part [drums: {s :analog} bass: {s :analog}] duration: 1} tail-seconds: 0 > play-song");
    for caps in [
        vactr::dsp::caps::CapabilitySet::native(),
        vactr::dsp::caps::CapabilitySet::browser(),
    ] {
        let plan = prepare_routes(song.snapshot(), &caps, &route_capacity()).unwrap();
        assert_eq!(plan.tracks.len(), 2);
        assert_ne!(plan.tracks[0].destination, plan.tracks[1].destination);
        assert!(plan.tracks.iter().all(|t| t.template.is_none()));
        assert_eq!(plan.branches.len(), 2);
        assert!(plan.branches.iter().all(|b| b.effect_template.is_none()));
        assert_eq!(plan.required.bus_slots, 5); // two private, two track, master
        assert_eq!(plan.branch_delay_frames, 2 * 2 * (4 * 48000 + 4));
        let room =
            vactr::dsp::effects::mem_len(vactr::dsp::graph::EffectKind::Room, 48000., &caps) as u64;
        assert_eq!(
            plan.required.bus_frames,
            5 * 4 * room + plan.branch_delay_frames
        );
        assert_eq!(plan.required.voice_slots, 8);
        assert_eq!(plan.required.sample_resources, 0);
    }
}
#[test]
fn exact_route_admission_rejects_each_leased_capacity_and_shared_voice_limit() {
    use vactr::song::routing::prepare_routes;
    let song = frozen_routes("song {part [drums: {s :analog}] duration: 1} > play-song");
    let caps = vactr::dsp::caps::CapabilitySet::native();
    let plan = prepare_routes(song.snapshot(), &caps, &route_capacity()).unwrap();
    assert!(prepare_routes(song.snapshot(), &caps, &plan.required).is_ok());
    for field in 0..4 {
        let mut available = plan.required;
        match field {
            0 => available.template_slots -= 1,
            1 => available.bus_slots -= 1,
            2 => available.bus_frames -= 1,
            _ => available.ack_slots -= 1,
        }
        assert!(prepare_routes(song.snapshot(), &caps, &available).is_err());
    }
    let mut empty = plan.required;
    empty.voice_slots = 0;
    assert!(prepare_routes(song.snapshot(), &caps, &empty).is_err());
}
#[test]
fn fx_declarations_are_scoped_latest_wins_and_explicit_track_bus_is_strict() {
    use vactr::song::routing::prepare_routes;
    let code="bus :plate-fx:\n\tplate mix: 1\nbus :spring-fx:\n\tspring-reverb mix: 1\nbus :drums:\n\tgain 0.5\nlet base {part [drums: {s :analog > bus :drums}] duration: 1}\nlet first {instrument-fx base :drums :analog :plate-fx}\nlet second {instrument-fx first :drums :analog :spring-fx}\nsong {sequence [first base second]} > play-song";
    let song = frozen_routes(code);
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    assert_eq!(
        plan.branches
            .iter()
            .map(|b| b.effect_template)
            .collect::<Vec<_>>(),
        vec![
            Some(vactr::value::intern::intern_kw("plate-fx")),
            None,
            Some(vactr::value::intern::intern_kw("spring-fx"))
        ]
    );
    assert!(plan.tracks[0].template.is_some());
    assert_ne!(plan.branches[0].placement, plan.branches[2].placement);
    let conflicting=frozen_routes("bus :other:\n\tgain 1\nsong {part [drums: {s :analog > bus :other}] duration: 1} > play-song");
    assert!(prepare_routes(
        conflicting.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity()
    )
    .is_err());
}
#[test]
fn billion_direct_repeats_keep_constant_configuration_count_and_bounded_tail_overlap() {
    use vactr::song::routing::prepare_routes;
    let song=frozen_routes("let p {part [drums: {s :analog}] duration: 1}\nsong {part-repeat p 1000000000} tail-seconds: 4 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    assert_eq!(plan.branches.len(), 1);
    assert_eq!(plan.branches[0].occurrences, 1_000_000_000);
    // Half-open 4s tail / 2s placement: two previous generations plus current.
    assert_eq!(plan.max_live_generations, 3);
    assert_eq!(plan.branches[0].reserved_generations, 3);
    assert!(plan.topology.parts.len() < 5);
}
#[test]
fn apply_reservations_reuse_fixed_voice_pool_while_private_arenas_shrink() {
    let total = route_capacity();
    let current = SongHostCapacities {
        sample_rate: 48000,
        voice_slots: 8,
        voice_frames: total.voice_frames,
        template_slots: 4,
        bus_slots: 8,
        cell_slots: 16,
        sample_resources: 2,
        pcm_bytes: 2048,
        bus_frames: 1_000_000,
        ack_slots: 8,
    };
    let available = total.after_reservations(current, current).unwrap();
    assert_eq!(available.voice_slots, total.voice_slots);
    assert_eq!(available.voice_frames, total.voice_frames);
    assert_eq!(available.template_slots, total.template_slots - 8);
    assert_eq!(available.bus_slots, total.bus_slots - 16);
    assert_eq!(available.pcm_bytes, total.pcm_bytes - 4096);
    let song = frozen_routes("song {part [drums: {s :analog}] duration: 1} > play-song");
    assert!(vactr::song::routing::prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &available
    )
    .is_ok());
}

#[test]
fn overwrite_interruptions_reserve_nonadjacent_plate_generations_through_long_tail() {
    use vactr::song::routing::{prepare_routes, SongRoutePlacement};
    let song=frozen_routes("bus :plate-fx:\n\tplate mix: 1\nlet base {part [drums: {s [:analog :analog :analog :analog]}] duration: 1}\nlet plate {instrument-fx base :drums :analog :plate-fx}\nlet cut {overwrite-region plate :drums 1/4 1/2 {s :fm}}\nsong cut tail-seconds: 8 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    assert_eq!(plan.branches.len(), 2);
    assert_eq!(plan.branches[0].reserved_generations, 2);
    assert_eq!(plan.branches[1].reserved_generations, 1);
    assert_eq!(plan.max_live_generations, 3);
    assert!(plan.branches[0]
        .placement
        .iter()
        .any(|p| matches!(p, SongRoutePlacement::Region { inside: false, .. })));
    assert!(plan.branches[1]
        .placement
        .iter()
        .any(|p| matches!(p, SongRoutePlacement::Region { inside: true, .. })));
    let mut short = route_capacity();
    short.bus_slots = 4; // actual requirement: track+master+three private
    assert!(prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &short
    )
    .is_err());
}

#[test]
fn generated_replace_and_overwrite_match_actual_earlier_source_fx_policies() {
    use vactr::song::routing::prepare_routes;
    for edit in [
        "replace-track plate :drums {s :analog}",
        "overwrite-region plate :drums 1/4 1/2 {s :analog > slow 4}",
    ] {
        let code=format!("bus :plate-fx:\n\tplate mix: 1\nlet base {{part [drums: {{s [:analog :analog :analog :analog]}}] duration: 1}}\nlet plate {{instrument-fx base :drums :analog :plate-fx}}\nsong {{{edit}}} tail-seconds: 8 > play-song");
        let mut song = frozen_routes(&code);
        let plan = prepare_routes(
            song.snapshot(),
            &vactr::dsp::caps::CapabilitySet::native(),
            &route_capacity(),
        )
        .unwrap();
        assert!(!plan.branches.is_empty());
        assert!(plan
            .branches
            .iter()
            .all(|b| b.effect_template == Some(vactr::value::intern::intern_kw("plate-fx"))));
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
        assert!(!rows.is_empty());
        assert!(rows
            .iter()
            .all(|e| e.route.as_ref().map(|r| r.1)
                == Some(vactr::value::intern::intern_kw("plate-fx"))));
    }
}
#[test]
fn zero_side_and_full_span_overwrites_have_no_phantom_source_generation() {
    use vactr::song::routing::prepare_routes;
    for (begin, end, source_count) in [("0", "1/2", 1), ("1/2", "1", 1), ("0", "1", 0)] {
        let code=format!("bus :plate-fx:\n\tplate mix: 1\nlet base {{part [drums: {{s [:analog :analog :analog :analog]}}] duration: 1}}\nlet plate {{instrument-fx base :drums :analog :plate-fx}}\nsong {{overwrite-region plate :drums {begin} {end} {{s :fm}}}} tail-seconds: 8 > play-song");
        let song = frozen_routes(&code);
        let plan = prepare_routes(
            song.snapshot(),
            &vactr::dsp::caps::CapabilitySet::native(),
            &route_capacity(),
        )
        .unwrap();
        assert_eq!(
            plan.branches
                .iter()
                .filter(|b| b.effect_template.is_some())
                .map(|b| b.reserved_generations)
                .sum::<u32>(),
            source_count
        );
        assert_eq!(plan.max_live_generations, source_count + 1);
    }
}
#[test]
fn fractional_overwrite_release_continuations_keep_original_note_and_route() {
    use vactr::pattern::TimeSpan;
    use vactr::value::Ratio64;
    let mut song=frozen_routes("bus :plate-fx:\n\tplate mix: 1\nlet base {part [drums: {s :analog > slow 4}] duration: 1}\nlet plate {instrument-fx base :drums :analog :plate-fx}\nsong {overwrite-region plate :drums 1/4 5/16 {s :fm > slow 4}} tail-seconds: 8 > play-song");
    let plan = vactr::song::routing::prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    assert_eq!(plan.max_live_generations, 3);
    let limits = vactr::song::SongLimits::default();
    let all = song
        .query(TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(), &limits)
        .unwrap();
    assert_eq!(all.len(), 2);
    let late = song
        .query(
            TimeSpan::new(Ratio64::new(1, 2).unwrap(), Ratio64::ONE).unwrap(),
            &limits,
        )
        .unwrap();
    assert_eq!(late.len(), 2);
    for continued in late {
        let original = all.iter().find(|e| e.handle == continued.handle).unwrap();
        assert_eq!(continued.whole, original.whole);
        assert_eq!(continued.route, original.route);
        assert!(continued.whole.unwrap().begin < continued.part.begin);
    }
}

#[test]
fn insufficient_private_arena_reports_track_family_and_symbolic_owner() {
    let song = frozen_routes(
        "let p {part [drums: {s :analog}] duration: 1}\nsong {sequence [p p]} > play-song",
    );
    let mut available = route_capacity();
    available.bus_frames = 0;
    let failure = vactr::song::routing::prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &available,
    )
    .unwrap_err();
    assert_eq!(failure.code, vactr::vm::fail::FailCode::BeyondCapability);
    for label in [
        "bus_frames",
        "track=drums",
        "family=:analog",
        "scope_part=",
        "placement=",
        "Sequence",
    ] {
        assert!(failure.message.contains(label), "{}", failure.message);
    }
    assert!(failure.message.len() < 1024);
}

#[test]
fn declared_track_detector_is_distinct_from_audio_destination_and_remapped_logically() {
    let song=frozen_routes("bus :kick:\n\tgain 1\nbus :bass:\n\tcompressor sidechain: :kick threshold: -20 ratio: 4\nsong {part [kick: {s :analog} bass: {s :fm}] duration: 1} > play-song");
    for caps in [
        vactr::dsp::caps::CapabilitySet::native(),
        vactr::dsp::caps::CapabilitySet::browser(),
    ] {
        let plan = vactr::song::routing::prepare_routes(song.snapshot(), &caps, &route_capacity())
            .unwrap();
        assert_eq!(plan.sidechains.len(), 1);
        let binding = &plan.sidechains[0];
        let kick = plan
            .tracks
            .iter()
            .find(|t| t.track == vactr::value::intern::intern_kw("kick"))
            .unwrap();
        assert_eq!(binding.source_destination, kick.destination);
        assert_eq!(binding.source_track, kick.track);
        assert_ne!(
            binding.source_destination,
            kick.template.as_ref().unwrap().id
        );
        assert!(plan.branches.iter().all(|b| plan
            .tracks
            .iter()
            .any(|t| t.track == b.track && t.destination == b.destination)));
    }
    let unsupported=frozen_routes("bus :orphan:\n\tgain 1\nbus :bass:\n\tcompressor sidechain: :orphan threshold: -20 ratio: 4\nsong {part [bass: {s :fm}] duration: 1} > play-song");
    let failure = vactr::song::routing::prepare_routes(
        unsupported.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap_err();
    assert!(failure.message.contains("outside declared song tracks"));
}

#[test]
fn closed_pcm_and_private_default_cells_require_actual_remaining_capacity() {
    let song=frozen_routes("inst tone hz: float = 440:\n\tsin-osc hz\nsong {part [tone: {s :tone} sample: {s {sample ./drum.wav}}] duration: 1} > play-song");
    let caps = vactr::dsp::caps::CapabilitySet::native();
    let plan =
        vactr::song::routing::prepare_routes(song.snapshot(), &caps, &route_capacity()).unwrap();
    assert_eq!(plan.required.sample_resources, 1);
    assert_eq!(plan.required.pcm_bytes, 16);
    assert!(plan.required.cell_slots > 0);
    for field in 0..3 {
        let mut available = route_capacity();
        match field {
            0 => available.sample_resources = 0,
            1 => available.pcm_bytes = 15,
            _ => available.cell_slots = 0,
        }
        let error =
            vactr::song::routing::prepare_routes(song.snapshot(), &caps, &available).unwrap_err();
        assert_eq!(error.code, vactr::vm::fail::FailCode::BeyondCapability);
        assert!(error.message.contains("track="));
    }
}

#[test]
fn weighted_iter_held_source_retains_identity_across_disconnected_query_pieces() {
    use vactr::pattern::TimeSpan;
    use vactr::song::SongLimits;
    use vactr::value::Ratio64;
    let mut song = frozen_routes("let base {part [drums: {slow {s :analog} 64}] duration: 64}\nlet developed {transform-instrument base :drums :analog {p -> iter [{hold p 3} nil] 4}}\nsong developed tail-seconds: 0 > play-song");
    let full = song
        .query(
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    assert!(!full.is_empty());
    let first = full
        .iter()
        .find(|event| event.handle.occurrence().onset == Ratio64::ZERO)
        .unwrap();
    for (begin, end) in [(2, 3), (1, 2), (0, 1)] {
        let rows = song
            .query(
                TimeSpan::new(Ratio64::from_int(begin), Ratio64::from_int(end)).unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        let retained = rows
            .iter()
            .find(|event| event.handle == first.handle)
            .unwrap_or_else(|| panic!("window={begin}..{end} full={full:#?} rows={rows:#?}"));
        assert_eq!(
            retained.source_origin.as_ref().unwrap().handle,
            first.source_origin.as_ref().unwrap().handle
        );
        assert_eq!(
            retained.source_origin.as_ref().unwrap().entry_trace,
            first.source_origin.as_ref().unwrap().entry_trace
        );
        assert_eq!(retained.handle.occurrence().onset, Ratio64::ZERO);
    }
}

#[test]
fn selected_fast_slow_routes_keep_exact_original_and_output_configuration_intervals() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    use vactr::value::Ratio64;
    for (operator, end) in [("fast", 2), ("slow", 4)] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog}}] duration: 4}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator} p 2}}}}\nsong developed tail-seconds: 0 > play-song"));
        let plan = prepare_routes(
            song.snapshot(),
            &vactr::dsp::caps::CapabilitySet::native(),
            &route_capacity(),
        )
        .unwrap();
        let rows = song
            .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
            .unwrap();
        assert!(!rows.is_empty());
        for event in rows {
            let route = resolve_route(&plan, &event, SongLimits::default()).unwrap();
            assert_eq!(
                route.configuration,
                TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(end)).unwrap()
            );
            assert_eq!(
                route.sources[0].original_configuration,
                TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(4)).unwrap()
            );
        }
    }
}

#[test]
fn unchanged_iterated_long_source_keeps_birth_configuration_across_query_holes() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    use vactr::value::Ratio64;
    let mut song=frozen_routes("let base {part [drums: {slow {s :analog} 64}] duration: 64}\nlet developed {transform-instrument base :drums :analog {p -> iter p 2}}\nsong developed tail-seconds: 0 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    let full = song
        .query(
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    let first = full
        .iter()
        .find(|event| event.handle.occurrence().onset == Ratio64::ZERO)
        .unwrap();
    let expected = resolve_route(&plan, first, SongLimits::default()).unwrap();
    assert_eq!(
        expected.configuration,
        TimeSpan::new(Ratio64::ZERO, Ratio64::new(127, 2).unwrap()).unwrap()
    );
    for (begin, end, den) in [(2, 3, 1), (1, 3, 4), (0, 1, 1)] {
        let rows = song
            .query(
                TimeSpan::new(
                    Ratio64::new(begin, den).unwrap(),
                    Ratio64::new(end, den).unwrap(),
                )
                .unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        let event = rows
            .iter()
            .find(|event| event.handle == first.handle)
            .unwrap();
        assert_eq!(
            resolve_route(&plan, event, SongLimits::default()).unwrap(),
            expected
        );
    }
}

#[test]
fn weighted_iterate_cycle_four_birth_is_intrinsic_and_partition_invariant() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    use vactr::value::Ratio64;
    let mut song = frozen_routes("let base {part [drums: {slow {s :analog} 64}] duration: 64}\nlet developed {transform-instrument base :drums :analog {p -> iter [{hold p 3} nil] 4}}\nsong developed tail-seconds: 0 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    let rows = song
        .query(
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(7)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    let first = rows
        .iter()
        .find(|e| e.handle.occurrence().onset == Ratio64::ONE)
        .unwrap();
    let expected = resolve_route(&plan, first, SongLimits::default()).unwrap();
    assert_eq!(
        expected.configuration,
        TimeSpan::new(Ratio64::new(13, 4).unwrap(), Ratio64::new(19, 4).unwrap()).unwrap()
    );
    for (begin, end, den) in [(6, 7, 1), (17, 18, 4), (4, 5, 1)] {
        let rows = song
            .query(
                TimeSpan::new(
                    Ratio64::new(begin, den).unwrap(),
                    Ratio64::new(end, den).unwrap(),
                )
                .unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        let event = rows.iter().find(|e| e.handle == first.handle).unwrap();
        assert_eq!(
            resolve_route(&plan, event, SongLimits::default()).unwrap(),
            expected
        );
    }
}

#[test]
fn weighted_iterate_fractional_source_policy_uses_inverse_coordinates() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    use vactr::value::Ratio64;
    let mut song = frozen_routes("let quiet {part [drums: {s nil}] duration: 9/4}\nlet audible {part [drums: {slow {s :analog} 64}] duration: 64}\nlet base {sequence [quiet audible]}\nlet developed {transform-instrument base :drums :analog {p -> iter [{hold p 3} nil] 4}}\nsong developed tail-seconds: 0 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    let rows = song
        .query(
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    let first = rows
        .iter()
        .find(|e| e.handle.occurrence().onset == Ratio64::new(27, 16).unwrap())
        .unwrap();
    let expected = resolve_route(&plan, first, SongLimits::default()).unwrap();
    assert_eq!(
        expected.configuration,
        TimeSpan::new(Ratio64::new(31, 16).unwrap(), Ratio64::new(9, 4).unwrap()).unwrap()
    );
    assert_eq!(
        expected.sources[0].original_configuration.begin,
        Ratio64::new(9, 4).unwrap()
    );
    for (begin, end, den) in [(2, 3, 1), (20, 21, 10), (41, 42, 20)] {
        let rows = song
            .query(
                TimeSpan::new(
                    Ratio64::new(begin, den).unwrap(),
                    Ratio64::new(end, den).unwrap(),
                )
                .unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        let event = rows.iter().find(|e| e.handle == first.handle).unwrap();
        assert_eq!(
            resolve_route(&plan, event, SongLimits::default()).unwrap(),
            expected
        );
    }
}

#[test]
fn adjacent_iterated_weighted_slices_share_policy_component_across_distinct_handles() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    use vactr::value::Ratio64;
    let mut song = frozen_routes("let base {part [drums: {s :analog}] duration: 64}\nlet developed {transform-instrument base :drums :analog {p -> iter [{hold p 3} nil] 4}}\nsong developed tail-seconds: 0 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    let rows = song
        .query(
            TimeSpan::new(Ratio64::new(23, 4).unwrap(), Ratio64::new(25, 4).unwrap()).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    let selected: Vec<_> = rows
        .iter()
        .filter(|event| {
            event
                .source_origin
                .as_ref()
                .unwrap()
                .handle
                .occurrence()
                .onset
                == Ratio64::from_int(6)
        })
        .collect();
    assert_eq!(selected.len(), 2);
    assert_ne!(selected[0].handle, selected[1].handle);
    let expected =
        TimeSpan::new(Ratio64::new(23, 4).unwrap(), Ratio64::new(25, 4).unwrap()).unwrap();
    for event in &selected {
        assert_eq!(
            resolve_route(&plan, event, SongLimits::default())
                .unwrap()
                .configuration,
            expected
        );
    }
    for (begin, end, den) in [(24, 25, 4), (23, 24, 4), (121, 124, 20), (116, 119, 20)] {
        let rows = song
            .query(
                TimeSpan::new(
                    Ratio64::new(begin, den).unwrap(),
                    Ratio64::new(end, den).unwrap(),
                )
                .unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        let matches: Vec<_> = rows
            .iter()
            .filter(|event| {
                event
                    .source_origin
                    .as_ref()
                    .unwrap()
                    .handle
                    .occurrence()
                    .onset
                    == Ratio64::from_int(6)
            })
            .collect();
        assert!(!matches.is_empty());
        for event in matches {
            assert_eq!(
                resolve_route(&plan, event, SongLimits::default())
                    .unwrap()
                    .configuration,
                expected
            );
        }
    }
}

#[test]
fn direct_and_zero_iterate_weighted_long_sources_use_intrinsic_cycle_six() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    use vactr::value::Ratio64;
    for recipe in ["[{hold p 3} nil]", "iter [{hold p 3} nil] 0"] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{slow {{s :analog}} 64}}] duration: 64}}\nlet developed {{transform-instrument base :drums :analog {{p -> {recipe}}}}}\nsong developed tail-seconds: 0 > play-song"));
        let plan = prepare_routes(
            song.snapshot(),
            &vactr::dsp::caps::CapabilitySet::native(),
            &route_capacity(),
        )
        .unwrap();
        let rows = song
            .query(
                TimeSpan::new(Ratio64::from_int(6), Ratio64::from_int(7)).unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        let first = rows
            .iter()
            .find(|event| event.handle.occurrence().onset == Ratio64::new(3, 2).unwrap())
            .unwrap();
        let expected = resolve_route(&plan, first, SongLimits::default()).unwrap();
        assert_eq!(
            expected.configuration,
            TimeSpan::new(Ratio64::from_int(6), Ratio64::new(27, 4).unwrap()).unwrap()
        );
        let rows = song
            .query(
                TimeSpan::new(Ratio64::new(25, 4).unwrap(), Ratio64::new(26, 4).unwrap()).unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
        let event = rows
            .iter()
            .find(|event| event.handle == first.handle)
            .unwrap();
        assert_eq!(
            resolve_route(&plan, event, SongLimits::default()).unwrap(),
            expected
        );
    }
}

#[test]
fn affine_outer_weighted_recipes_preserve_intrinsic_long_source_geometry() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    use vactr::value::Ratio64;
    for (rate, factor) in [
        ("fast", Ratio64::from_int(2)),
        ("slow", Ratio64::new(1, 2).unwrap()),
    ] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{slow {{s :analog}} 64}}] duration: 64}}\nlet developed {{transform-instrument base :drums :analog {{p -> {rate} [{{hold p 3}} nil] 2}}}}\nsong developed tail-seconds: 0 > play-song"));
        let plan = prepare_routes(
            song.snapshot(),
            &vactr::dsp::caps::CapabilitySet::native(),
            &route_capacity(),
        )
        .unwrap();
        let expected = TimeSpan::new(
            Ratio64::from_int(6).checked_div(factor).unwrap(),
            Ratio64::new(27, 4).unwrap().checked_div(factor).unwrap(),
        )
        .unwrap();
        let rows = song.query(expected, &SongLimits::default()).unwrap();
        let first = rows
            .iter()
            .find(|e| {
                e.handle.occurrence().onset
                    == Ratio64::new(3, 2).unwrap().checked_div(factor).unwrap()
            })
            .unwrap();
        let route = resolve_route(&plan, first, SongLimits::default()).unwrap();
        assert_eq!(route.configuration, expected);
        let sub = TimeSpan::new(
            expected
                .begin
                .checked_add(Ratio64::new(1, 8).unwrap())
                .unwrap(),
            expected
                .end
                .checked_sub(Ratio64::new(1, 8).unwrap())
                .unwrap(),
        )
        .unwrap();
        let rows = song.query(sub, &SongLimits::default()).unwrap();
        let event = rows.iter().find(|e| e.handle == first.handle).unwrap();
        assert_eq!(
            resolve_route(&plan, event, SongLimits::default()).unwrap(),
            route
        );
    }
}

#[test]
fn later_uniform_source_fx_override_admits_only_the_authoritative_template() {
    use vactr::pattern::TimeSpan;
    use vactr::song::routing::{prepare_routes, resolve_route};
    use vactr::song::SongLimits;
    let mut song = frozen_routes("bus :plate-fx:\n\tplate mix: 1\nbus :spring-fx:\n\tspring-reverb mix: 1\nlet base {part [drums: {s :analog}] duration: 4}\nlet plate {instrument-fx base :drums :analog :plate-fx}\nlet moved {transform-instrument plate :drums :analog {p -> fast p 2}}\nlet final {instrument-fx moved :drums :analog :spring-fx}\nsong final tail-seconds: 0 > play-song");
    let caps = vactr::dsp::caps::CapabilitySet::native();
    let plan = prepare_routes(song.snapshot(), &caps, &route_capacity()).unwrap();
    let spring = vactr::value::intern::intern_kw("spring-fx");
    assert_eq!(plan.branches.len(), 1);
    assert!(plan
        .branches
        .iter()
        .all(|b| b.effect_template == Some(spring)));
    assert_eq!(plan.required.template_slots, 1);
    let mut exact = route_capacity();
    exact.template_slots = plan.required.template_slots;
    exact.bus_slots = plan.required.bus_slots;
    exact.bus_frames = plan.required.bus_frames;
    prepare_routes(song.snapshot(), &caps, &exact).unwrap();
    let rows = song
        .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
        .unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        let resolved = resolve_route(&plan, &row, SongLimits::default()).unwrap();
        assert_eq!(
            plan.branches[resolved.branch.0 as usize].effect_template,
            Some(spring)
        );
    }
}

#[test]
fn partial_original_family_override_preserves_other_source_configurations() {
    use vactr::pattern::TimeSpan;
    use vactr::song::routing::{prepare_routes, resolve_route};
    use vactr::song::SongLimits;
    let mut song = frozen_routes("bus :plate-fx:\n\tplate mix: 1\nbus :spring-fx:\n\tspring-reverb mix: 1\nlet sound-kit put default-sound-kit [family: [{default-sound-kit :analog} {default-sound-kit :fm}]]\nlet base {part [drums: {s [:analog :fm]}] duration: 2}\nlet plate {instrument-fx base :drums :family :plate-fx}\nlet moved {transform-instrument plate :drums :family {p -> s p > gain 0.5}}\nlet final {instrument-fx moved :drums :analog :spring-fx}\nsong final tail-seconds: 0 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap();
    let plate = vactr::value::intern::intern_kw("plate-fx");
    let spring = vactr::value::intern::intern_kw("spring-fx");
    let rows = song
        .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
        .unwrap();
    assert!(rows
        .iter()
        .any(|e| e.route.as_ref().is_some_and(|(_, k)| *k == plate)));
    assert!(rows
        .iter()
        .any(|e| e.route.as_ref().is_some_and(|(_, k)| *k == spring)));
    for row in rows {
        let expected = row.route.as_ref().unwrap().1;
        let resolved = resolve_route(&plan, &row, SongLimits::default()).unwrap();
        assert_eq!(
            plan.branches[resolved.branch.0 as usize].effect_template,
            Some(expected)
        );
    }
}

fn measured_placement_capacity(bus_slots: usize) -> SongHostCapacities {
    use vactr::dsp::engine::{Engine, EngineConfig, SongStagingConfig};
    use vactr::dsp::ring::{EngineIo, EventRing, NativeRecord, SpscRing};
    use vactr::host::wire::{CtlMsg, HostMsg};
    use vactr::song::routing::{SongCommand, SongHostAck};
    let mut caps = vactr::dsp::caps::CapabilitySet::native();
    caps.max_voices = 2;
    let mut cfg = EngineConfig::new(&caps, 48000., 16, vactr::dsp::arena::StoreKind::NativeArc);
    cfg.bus_slots = bus_slots;
    let mut engine = Engine::with_config(cfg);
    let (mut acks, mut received) = SpscRing::split(1024);
    engine
        .configure_song_staging_for_transport(
            SongStagingConfig {
                preparations: 1,
                leases: 32,
                branches: 8,
                control_slots: 4096,
                analysis_slots: 64,
                native_pcm_bytes: 16_000_000,
                critical_receipts: 8,
            },
            &acks,
        )
        .unwrap();
    let (mut sender, mut controls) = SpscRing::split(2);
    let (_event_sender, mut events) = EventRing::split(2);
    let mut cells = vactr::dsp::cells::AtomicCells::new(4096);
    assert!(sender
        .push(NativeRecord::Msg(CtlMsg::Song(
            SongCommand::RequestCapacity(SnapshotEpoch(101)),
        )))
        .is_ok());
    engine.process(
        &mut EngineIo {
            events: &mut events,
            controls: &mut controls,
            acks: &mut acks,
            cells: &mut cells,
            garbage: None,
        },
        &mut [0.; 32],
        16,
    );
    let Some(HostMsg::Song(SongHostAck::CapacityReport(report))) = received.pop() else {
        panic!("actual measured Engine capacity receipt");
    };
    assert_eq!(report.epoch, SnapshotEpoch(101));
    report.available
}

#[test]
fn ordinary_repeat_half_open_tail_overlap_preserves_every_family_and_occurrence() {
    use vactr::song::routing::prepare_routes;
    let caps = vactr::dsp::caps::CapabilitySet::native();
    for (tail, overlap) in [(0, 1), (1, 2), (2, 2), (3, 3), (4, 3)] {
        let song = frozen_routes(&format!("inst tone freq: float = 440:\n\tsin-osc freq > * amp\nlet base {{part [tone: {{s :tone}}] duration: 1}}\nsong {{part-repeat base 1000000}} tail-seconds: {tail} > play-song"));
        let plan = prepare_routes(song.snapshot(), &caps, &route_capacity()).unwrap();
        assert_eq!(plan.branches.len(), 2);
        for branch in &plan.branches {
            assert_eq!(branch.occurrences, 1_000_000);
            assert_eq!(branch.reserved_generations, overlap);
        }
        assert_eq!(plan.max_live_generations, 2 * overlap);
        assert_eq!(plan.required.bus_slots, 2 * overlap + 2);
    }
    let empty = frozen_routes("song {sequence []} tail-seconds: 0 > play-song");
    assert_eq!(
        prepare_routes(empty.snapshot(), &caps, &route_capacity())
            .unwrap()
            .max_live_generations,
        0
    );
}

#[test]
fn million_repeat_zero_tail_fits_actual_measured_boundary_and_refuses_one_below() {
    use vactr::song::routing::prepare_routes;
    let mut caps = vactr::dsp::caps::CapabilitySet::native();
    caps.max_voices = 2;
    let song = frozen_routes("inst tone freq: float = 440:\n\tsin-osc freq > * amp\nlet base {part [tone: {s :tone}] duration: 1}\nsong {part-repeat base 1000000} tail-seconds: 0 > play-song");
    let exact = measured_placement_capacity(5);
    assert_eq!(exact.bus_slots, 4); // actual legacy Master consumes one slot
    let plan = prepare_routes(song.snapshot(), &caps, &exact).unwrap();
    assert_eq!(plan.required.bus_slots, exact.bus_slots);
    assert_eq!(plan.branches.len(), 2);
    assert!(plan
        .branches
        .iter()
        .all(|b| b.occurrences == 1_000_000 && b.reserved_generations == 1));
    let short = measured_placement_capacity(4);
    assert_eq!(short.bus_slots, 3);
    assert!(prepare_routes(song.snapshot(), &caps, &short).is_err());
}

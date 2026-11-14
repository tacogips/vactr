use super::*;
use crate::pattern::TimeSpan;
fn prepared(code: &str) -> crate::song::PreparedSong {
    use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
    let assets =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let context = crate::session::song::CandidateBuildCtx {
        assets: &assets,
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
    crate::song::prepare_song(
        crate::session::song::evaluate_song_candidate(
            code,
            "components.vact",
            1,
            crate::song::SnapshotEpoch(100),
            &context,
        )
        .unwrap(),
    )
    .unwrap()
}
fn capacity() -> SongHostCapacities {
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
fn span(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
}
fn selected_bound(
    song: &crate::song::PreparedSong,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<density::ConfigurationBound, Failure> {
    let inventory = song.snapshot().routing();
    let FrozenPartNode::Edit {
        edit: FrozenEdit::Transform { payload, .. },
        ..
    } = &inventory.parts[inventory.root_part].node
    else {
        panic!("fixture root must be a selected transform");
    };
    density::source_density(
        inventory,
        payload,
        TimeSpan::new(Ratio64::ZERO, song.snapshot().duration()).unwrap(),
        remaining,
        max_depth,
    )
}
#[test]
fn generated_current_family_is_admitted_after_historical_other_family_edit() {
    let mut song = prepared("let base {part [drums: {s :analog}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> s :fm}}\nlet outer {transform-instrument inner :drums :fm {p -> first [p]}}\nsong outer tail-seconds: 0 > play-song");
    let bound = selected_bound(&song, &mut 100_000, 256).unwrap();
    assert_eq!(bound.components_intersecting(Ratio64::ZERO).unwrap(), 1);
    let plan = prepare_routes(
        song.snapshot(),
        &crate::dsp::caps::CapabilitySet::native(),
        &capacity(),
    )
    .unwrap();
    assert!(!plan.branches.is_empty());
    assert!(plan.max_live_generations > 0);
    let limits = crate::song::SongLimits::default();
    let expected: Vec<_> = song
        .query(span(0, 4, 1), &limits)
        .unwrap()
        .iter()
        .map(|event| {
            (
                event.handle.clone(),
                source::resolve_route(&plan, event, limits).unwrap(),
            )
        })
        .collect();
    assert!(!expected.is_empty());
    for window in [span(5, 7, 2), span(1, 3, 2), span(0, 1, 2)] {
        for event in song.query(window, &limits).unwrap() {
            assert_eq!(
                source::resolve_route(&plan, &event, limits).unwrap(),
                expected
                    .iter()
                    .find(|(handle, _)| *handle == event.handle)
                    .unwrap()
                    .1
            );
        }
    }
}
#[test]
fn sample_grid_runs_increase_births_without_multiplying_point_occupancy() {
    let mut totals = Vec::new();
    for pulses in [0, 1, 2, 4] {
        let song = prepared(&format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog {{p -> euclid p {pulses} 4}}}}\nsong selected tail-seconds: 0 > play-song"));
        let bound = selected_bound(&song, &mut 100_000, 256).unwrap();
        assert_eq!(
            bound.components_intersecting(Ratio64::ZERO).unwrap(),
            u64::from(pulses != 0)
        );
        totals.push(bound.components_intersecting(Ratio64::from_int(4)).unwrap());
    }
    assert_eq!(totals[0], 0);
    assert!(totals[2] >= totals[1]);
    assert!(totals[2] > totals[3]);
}
#[test]
fn selected_density_shares_exact_work_and_logical_depth_boundaries() {
    let song = prepared("let base {part [drums: {s :analog}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> first [p]}}\nlet outer {transform-instrument inner :drums :analog {p -> first [p]}}\nsong outer tail-seconds: 0 > play-song");
    let mut remaining = 100_000;
    selected_bound(&song, &mut remaining, 256).unwrap();
    let consumed = 100_000 - remaining;
    assert!(consumed > 1);
    let mut exact = consumed;
    selected_bound(&song, &mut exact, 256).unwrap();
    assert_eq!(exact, 0);
    assert_eq!(
        selected_bound(&song, &mut (consumed - 1), 256)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    assert_eq!(
        selected_bound(&song, &mut exact, 256).unwrap_err().code,
        FailCode::FuelExhausted
    );
    let minimum = (0..256)
        .find(|depth| selected_bound(&song, &mut 100_000, *depth).is_ok())
        .unwrap();
    assert!(minimum > 0);
    selected_bound(&song, &mut 100_000, minimum).unwrap();
    assert_eq!(
        selected_bound(&song, &mut 100_000, minimum - 1)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
}
#[test]
fn repeat_tail_exact_duration_counts_half_open_logical_placements() {
    let song = prepared("let base {part [drums: {s :analog}] duration: 1}\nsong {part-repeat base 3} tail-seconds: 0 > play-song");
    let inventory = song.snapshot().routing();
    let path = [SongRoutePlacement::Repeat {
        part: inventory.root_part,
        count: 3,
    }];
    let track = inventory.parts[inventory.root_part].tracks[0];
    for (tail, expected) in [
        (Ratio64::ZERO, 1),
        (Ratio64::ONE, 2),
        (Ratio64::new(3, 2).unwrap(), 3),
    ] {
        assert_eq!(
            outer_placement_overlap(inventory, &path, track, tail, &mut 1).unwrap(),
            expected
        );
    }
    // Half-open intrinsic components ending at the lookback left endpoint
    // are absent; physical unacknowledged leases are charged separately.
}
#[test]
fn repeated_composed_score_fits_real_configured_aggregate_allocations() {
    use crate::dsp::arena::StoreKind;
    use crate::dsp::ring::{EngineConfig, SpscRing};
    let mut caps = crate::dsp::caps::CapabilitySet::native();
    caps.max_voices = 8;
    caps.max_capture_seconds = 1.0;
    let mut cfg = EngineConfig::new(&caps, 8000.0, 64, StoreKind::NativeArc);
    cfg.bus_slots = 32;
    let engine = crate::dsp::engine::Engine::try_with_config(cfg).unwrap();
    let actual = engine.config();
    let cells = crate::dsp::cells::AtomicCells::new(actual.analysis_cells);
    let (ack, _reader) = SpscRing::<crate::host::wire::HostMsg>::split(256);
    let bus_extent = 80_000 + crate::dsp::granular::effect_mem_len(actual.sample_rate, &caps);
    let available = SongHostCapacities {
        sample_rate: 8000,
        cell_slots: u32::try_from(cells.capacity()).unwrap(),
        voice_slots: u32::from(actual.caps.max_voices),
        template_slots: u32::try_from(actual.template_slots).unwrap(),
        bus_slots: u32::try_from(actual.bus_slots).unwrap(),
        sample_resources: 0,
        pcm_bytes: 0,
        voice_frames: 32_000 * u64::from(actual.caps.max_voices),
        bus_frames: u64::try_from(bus_extent * actual.bus_slots).unwrap(),
        ack_slots: u32::try_from(ack.capacity()).unwrap(),
    };
    let song = prepared("let base {part-repeat {sequence [{part [bass: {s :fm}] duration: 3/2} {part [drums: {s :analog > slow 64}] duration: 4}]} 2}\nlet inner {transform-instrument base :drums :analog {p -> fast {every p 2 {x -> first [x]}} 2}}\nlet outer {transform-instrument inner :drums :analog {p -> every p 3 {x -> first [x]}}}\nsong outer tail-seconds: 0 > play-song");
    let plan = prepare_routes(song.snapshot(), &caps, &available).unwrap();
    assert!(plan.required.template_slots <= available.template_slots);
    assert!(plan.required.bus_slots <= available.bus_slots);
    assert!(plan.required.cell_slots <= available.cell_slots);
    assert!(plan.required.voice_frames <= available.voice_frames);
    assert!(plan.required.bus_frames <= available.bus_frames);
    assert!(plan.required.ack_slots <= available.ack_slots);
    assert_eq!(plan.required.pcm_bytes, 0);
    assert_eq!(plan.required.sample_resources, 0);
    let mut insufficient = available;
    insufficient.bus_slots = plan.required.bus_slots - 1;
    assert_eq!(
        prepare_routes(song.snapshot(), &caps, &insufficient)
            .unwrap_err()
            .code,
        FailCode::BeyondCapability
    );
    // These are real allocated aggregate dimensions. Private song state
    // remapping, per-slot extents and lease installation still belong09/10.
}
#[test]
fn fractional_owners_and_joint_full_width_periods_have_finite_birth_bounds() {
    for expression in [
        "every p 9223372036854775807 {x -> first [x]}",
        "every {every p 9223372036854775807 {x -> first [x]}} 9223372036854775806 {x -> first [x]}",
    ] {
        let mut song = prepared(&format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 1/4}}\nlet selected {{transform-instrument base :drums :analog {{p -> {expression}}}}}\nsong selected tail-seconds: 0 > play-song"));
        let bound = selected_bound(&song, &mut 100_000, 256).unwrap();
        assert_eq!(bound.components_intersecting(Ratio64::ZERO).unwrap(), 1);
        assert!(bound.live_generations(Ratio64::new(1, 4).unwrap()).unwrap() < 32);
        let plan = prepare_routes(
            song.snapshot(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacity(),
        )
        .unwrap();
        let limits = crate::song::SongLimits::default();
        let rows = song.query(span(0, 1, 4), &limits).unwrap();
        assert!(!rows.is_empty());
        let expected: Vec<_> = rows
            .iter()
            .map(|event| {
                (
                    event.handle.clone(),
                    source::resolve_route(&plan, event, limits).unwrap(),
                )
            })
            .collect();
        for window in [span(1, 2, 8), span(0, 1, 8)] {
            for event in song.query(window, &limits).unwrap() {
                assert_eq!(
                    source::resolve_route(&plan, &event, limits).unwrap(),
                    expected
                        .iter()
                        .find(|(handle, _)| *handle == event.handle)
                        .unwrap()
                        .1
                );
            }
        }
    }
}
#[test]
fn grid_matcher_reservation_charges_real_mask_and_envelope_work() {
    use crate::song::source_uses::{sampling, FrozenStaticSampling};
    for pulses in [1, 2, 4, -2] {
        let recipe = FrozenStaticSampling::Euclid {
            pulses,
            divisions: 4,
            rotation: 1,
        };
        let mut measured = 100_000;
        let phases = sampling::enabled_phases(recipe, &mut measured).unwrap();
        let expected = (100_000 - measured) * 3 + u32::try_from(phases.len() * 5).unwrap();
        let mut budget = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: expected * 2,
            ..Default::default()
        });
        super::source::reserve_grid_search(recipe, &mut budget).unwrap();
        assert_eq!(budget.limits().max_nodes, expected);
        super::source::reserve_grid_search(recipe, &mut budget).unwrap();
        assert_eq!(budget.limits().max_nodes, 0);
        assert_eq!(
            super::source::reserve_grid_search(recipe, &mut budget)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
    }
    let mut song = prepared("let base {sequence [{part [bass: {s :fm}] duration: 1/4} {part [drums: {s :analog}] duration: 1/4} {part [bass: {s :fm}] duration: 1/2}]}\nlet selected {transform-instrument base :drums :analog {p -> euclid p 4 4}}\nsong selected tail-seconds: 0 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &crate::dsp::caps::CapabilitySet::native(),
        &capacity(),
    )
    .unwrap();
    let limits = crate::song::SongLimits::default();
    let rows = song.query(span(3, 3, 8), &limits).unwrap();
    let event = rows
        .iter()
        .find(|event| event.source_origin.is_some())
        .unwrap();
    let certificate = &plan.source_covers[0];
    let payload =
        configuration::scope_payload(&plan.topology, certificate.scope_part, certificate.track)
            .unwrap();
    let origin = event.source_origin.as_ref().unwrap();
    let mut measured = 100_000;
    crate::song::source_uses::sampling::enabled_phases(
        crate::song::source_uses::FrozenStaticSampling::Euclid {
            pulses: 4,
            divisions: 4,
            rotation: 0,
        },
        &mut measured,
    )
    .unwrap();
    let probe = 100_000 - measured;
    let mut caller = ResolutionBudget::new(limits);
    let allowance = source::reserve_source_search(
        &certificate.cover,
        payload,
        origin.borrowed_view(),
        0,
        &mut caller,
    )
    .unwrap();
    let debit = limits.max_nodes - caller.limits().max_nodes;
    assert_eq!(debit, probe + allowance.max_nodes + 16);
    crate::song::source_uses::resolve_source_use(&certificate.cover, origin, allowance).unwrap();
    let mut exact = ResolutionBudget::new(crate::song::SongLimits {
        max_nodes: debit,
        ..limits
    });
    assert_eq!(
        source::reserve_source_search(
            &certificate.cover,
            payload,
            origin.borrowed_view(),
            0,
            &mut exact
        )
        .unwrap()
        .max_nodes,
        allowance.max_nodes
    );
    assert_eq!(exact.limits().max_nodes, 0);
    let mut short = ResolutionBudget::new(crate::song::SongLimits {
        max_nodes: debit - 1,
        ..limits
    });
    assert_eq!(
        source::reserve_source_search(
            &certificate.cover,
            payload,
            origin.borrowed_view(),
            0,
            &mut short
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    assert_eq!(
        source::resolve_route(&plan, event, limits)
            .unwrap()
            .configuration,
        span(1, 2, 4)
    );
}
#[test]
fn outer_affine_periodic_runs_preserve_real_full_and_split_routes() {
    for wrapper in ["fast q 2", "slow q 2"] {
        let expression = wrapper.replace(
            'q',
            "{every {every p 2 {x -> first [x]}} 3 {x -> first [x]}}",
        );
        let code = format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 4}}\nlet developed {{transform-instrument base :drums :analog {{p -> {expression}}}}}\nsong developed tail-seconds: 0 > play-song");
        let mut song = prepared(&code);
        let plan = prepare_routes(
            song.snapshot(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacity(),
        )
        .unwrap();
        let limits = crate::song::SongLimits::default();
        let rows = song.query(span(0, 4, 1), &limits).unwrap();
        assert!(rows.len() >= 2);
        let expected: Vec<_> = rows
            .iter()
            .map(|e| {
                let route = source::resolve_route(&plan, e, limits).unwrap();
                assert_eq!(route.configuration, e.part);
                (e.handle.clone(), route)
            })
            .collect();
        for window in [span(5, 7, 4), span(1, 3, 4), span(7, 8, 2)] {
            for event in song.query(window, &limits).unwrap() {
                let old = expected
                    .iter()
                    .find(|(handle, _)| *handle == event.handle)
                    .unwrap();
                assert_eq!(source::resolve_route(&plan, &event, limits).unwrap(), old.1);
            }
        }
    }
}
#[test]
fn selected_part_stages_solve_periodic_constraints_in_one_root_clock() {
    let mut song = prepared("let base {part [drums: {s :analog > slow 64}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> every p 2 {x -> first [x]}}}\nlet outer {transform-instrument inner :drums :analog {p -> every p 3 {x -> first [x]}}}\nsong outer tail-seconds: 0 > play-song");
    let plan = prepare_routes(
        song.snapshot(),
        &crate::dsp::caps::CapabilitySet::native(),
        &capacity(),
    )
    .unwrap();
    let limits = crate::song::SongLimits::default();
    let rows = song.query(span(0, 4, 1), &limits).unwrap();
    assert_eq!(rows.len(), 4);
    let expected: Vec<_> = rows
        .iter()
        .map(|event| {
            assert_eq!(event.source_origin.as_ref().unwrap().inherited.len(), 1);
            let route = source::resolve_route(&plan, event, limits).unwrap();
            assert_eq!(route.sources.len(), 2);
            assert_eq!(route.configuration, event.part);
            (event.handle.clone(), route)
        })
        .collect();
    for window in [span(5, 7, 2), span(1, 3, 2), span(0, 1, 2)] {
        for event in song.query(window, &limits).unwrap() {
            let old = expected
                .iter()
                .find(|(handle, _)| *handle == event.handle)
                .unwrap();
            assert_eq!(source::resolve_route(&plan, &event, limits).unwrap(), old.1);
        }
    }
}
#[test]
fn shifted_repeated_source_stages_authenticate_offsets_and_middle_affine_clocks() {
    for base in [
        "part [drums: {s :analog > slow 64}] duration: 4",
        "sequence [{part [bass: {s :fm}] duration: 3/2} {part [drums: {s :analog > slow 64}] duration: 4}]",
        "part-repeat {sequence [{part [bass: {s :fm}] duration: 3/2} {part [drums: {s :analog > slow 64}] duration: 4}]} 2",
    ] {
        let code=format!("let base {{{base}}}\nlet inner {{transform-instrument base :drums :analog {{p -> fast {{every p 2 {{x -> first [x]}}}} 2}}}}\nlet outer {{transform-instrument inner :drums :analog {{p -> every p 3 {{x -> first [x]}}}}}}\nsong outer tail-seconds: 0 > play-song");
        let mut song=prepared(&code);
        let mut available=capacity();
        available.template_slots=1_000_000;
        available.bus_slots=1_000_000;
        available.bus_frames=1_000_000_000_000;
        available.ack_slots=3_000_000;
        let plan=prepare_routes(song.snapshot(), &crate::dsp::caps::CapabilitySet::native(), &available).unwrap();
        let limits=crate::song::SongLimits::default();
        let rows=song.query(span(0,6,1),&limits).unwrap();
        let expected:Vec<_>=rows.iter().map(|event| {
            let route=source::resolve_route(&plan,event,limits).unwrap();
            if event.source_origin.is_some() { assert_eq!(route.sources.len(),2); }
            (event.handle.clone(),route)
        }).collect();
        assert!(expected.iter().any(|(_,route)| !route.sources.is_empty() && route.configuration==span(2,3,2)));
        for window in [span(9,11,2),span(1,5,4),span(5,6,4),span(3,9,4)] {
            for event in song.query(window,&limits).unwrap() {
                let old=expected.iter().find(|(handle,_)| *handle==event.handle).unwrap();
                assert_eq!(source::resolve_route(&plan,&event,limits).unwrap(),old.1);
            }
        }
    }
}

//! Actual frozen route preparation and bounded configuration lifetime admission.
use vactr::song::routing::SongHostCapacities;
use vactr::song::{routing::resolve_route, SnapshotEpoch, SongLimits};

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
fn detector_snapshots_allow_parallel_track_effects_and_order_private_branches() {
    use vactr::song::routing::{prepare_routes, SongDetectorTarget};
    for (code, private) in [
        ("bus :kick:\n\tgain 1\nbus :bass:\n\tgain 1\nbus :kick:\n\tcompressor sidechain: :bass\nbus :bass:\n\tcompressor sidechain: :kick\nsong {part [kick: {s :analog} bass: {s :fm}] duration: 1} > play-song", false),
        ("bus :kick:\n\tgain 1\nbus :duck:\n\tcompressor sidechain: :kick\nlet p {part [kick: {s :analog} bass: {s :fm}] duration: 1}\nsong {instrument-fx p :bass :fm :duck} > play-song", true),
    ] {
        let song = frozen_routes(code);
        let plan = prepare_routes(song.snapshot(), &vactr::dsp::caps::CapabilitySet::native(), &route_capacity()).unwrap();
        assert!(!plan.sidechains.is_empty());
        assert_eq!(plan.sidechains.iter().any(|b| matches!(b.target, SongDetectorTarget::Branch(_))), private);
    }
}
#[test]
fn private_detector_feedback_cycles_fail_before_activation_with_addressed_owner() {
    use vactr::song::routing::prepare_routes;
    for code in [
        "bus :kick:\n\tgain 1\nbus :self-fx:\n\tcompressor sidechain: :kick\nlet p {part [kick: {s :analog}] duration: 1}\nsong {instrument-fx p :kick :analog :self-fx} > play-song",
        "bus :kick:\n\tgain 1\nbus :bass:\n\tgain 1\nbus :duck-kick:\n\tcompressor sidechain: :bass\nbus :duck-bass:\n\tcompressor sidechain: :kick\nlet p {part [kick: {s :analog} bass: {s :fm}] duration: 1}\nlet k {instrument-fx p :kick :analog :duck-kick}\nsong {instrument-fx k :bass :fm :duck-bass} > play-song",
    ] {
        let song = frozen_routes(code);
        let failure = prepare_routes(song.snapshot(), &vactr::dsp::caps::CapabilitySet::native(), &route_capacity()).unwrap_err();
        assert!(failure.message.contains("causal cycle"), "{}", failure.message);
        assert!(failure.message.contains("source_track="));
        assert!(failure.message.contains("Branch"));
    }
}

fn span(begin: i64, end: i64, denominator: i64) -> vactr::pattern::TimeSpan {
    use vactr::value::Ratio64;
    vactr::pattern::TimeSpan::new(
        Ratio64::new(begin, denominator).unwrap(),
        Ratio64::new(end, denominator).unwrap(),
    )
    .unwrap()
}
fn plan(song: &vactr::song::PreparedSong) -> vactr::song::routing::SongRoutePlan {
    vactr::song::routing::prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &route_capacity(),
    )
    .unwrap()
}
fn resolve(
    plan: &vactr::song::routing::SongRoutePlan,
    row: &vactr::song::snapshot::FrozenSongEvent,
) -> vactr::song::routing::SongResolvedRoute {
    vactr::song::routing::resolve_route(plan, row, vactr::song::SongLimits::default())
        .unwrap_or_else(|failure| panic!("{failure:?}\nrow={row:#?}"))
}
#[test]
fn direct_sequence_repeat_and_edit_births_resolve_exact_configuration_intervals() {
    let mut song = frozen_routes("let p {part [drums: {s :analog}] duration: 1}\nlet q {replace-track p :drums {s :fm}}\nsong {sequence [p {part-repeat q 2} p]} > play-song");
    let plan = plan(&song);
    let rows = song.query(span(0, 4, 1), &Default::default()).unwrap();
    assert_eq!(rows.len(), 4);
    for row in &rows {
        let onset = row.handle.occurrence().onset;
        let route = resolve(&plan, row);
        assert_eq!(route.configuration.begin, onset);
        assert_eq!(
            route.configuration.end,
            onset.checked_add(vactr::value::Ratio64::ONE).unwrap()
        );
        assert!(route.sources.is_empty());
    }
    assert_ne!(
        resolve(&plan, &rows[1]).configuration,
        resolve(&plan, &rows[2]).configuration
    );
}
#[test]
fn overwrite_changes_target_configuration_without_splitting_untouched_hats() {
    use vactr::value::intern::intern_kw;
    let mut song = frozen_routes("let p {part [drums: {s :analog} hats: {s :fm}] duration: 3}\nsong {overwrite-region p :drums 1 2 {s :analog}} > play-song");
    let plan = plan(&song);
    let rows = song.query(span(0, 3, 1), &Default::default()).unwrap();
    assert_eq!(rows.len(), 6);
    for row in &rows {
        let route = resolve(&plan, row);
        if row.track == intern_kw("hats") {
            assert_eq!(route.configuration, span(0, 3, 1));
        } else {
            let onset = row.handle.occurrence().onset;
            assert_eq!(route.configuration.begin, onset);
            assert_eq!(
                route.configuration.end,
                onset.checked_add(vactr::value::Ratio64::ONE).unwrap()
            );
        }
    }
    let hats: Vec<_> = rows
        .iter()
        .filter(|row| row.track == intern_kw("hats"))
        .collect();
    assert_eq!(resolve(&plan, hats[0]), resolve(&plan, hats[1]));
    assert_eq!(resolve(&plan, hats[1]), resolve(&plan, hats[2]));
}
#[test]
fn long_release_keeps_onset_owned_effect_and_route_across_fractional_reordered_queries() {
    let mut song = frozen_routes("bus :room:\n\tplate mix: 1\nlet p {part [drums: {slow {s :analog} 4}] duration: 4}\nlet effected {instrument-fx p :drums :analog :room}\nsong {overwrite-region effected :drums 1 2 {s :fm}} > play-song");
    let plan = plan(&song);
    let full = song.query(span(0, 4, 1), &Default::default()).unwrap();
    let original = full
        .iter()
        .find(|row| row.handle.occurrence().onset == vactr::value::Ratio64::ZERO)
        .unwrap();
    let expected = resolve(&plan, original);
    assert!(original.route.is_some());
    assert_eq!(expected.configuration, span(0, 1, 1));
    for window in [span(9, 13, 4), span(1, 2, 1), span(0, 1, 4), span(3, 4, 1)] {
        let rows = song.query(window, &Default::default()).unwrap();
        let retained = rows
            .iter()
            .find(|row| row.handle == original.handle)
            .unwrap_or_else(|| panic!("missing continuation in {window:?}: {rows:#?}"));
        assert_eq!(retained.route, original.route);
        assert_eq!(resolve(&plan, retained), expected);
    }
}
#[test]
fn chord_tones_and_repeated_notes_share_configuration_but_foreign_owners_are_rejected() {
    use vactr::value::intern::intern_kw;
    let code =
        "song {part [drums: {s [:analog :analog] > chord [:c :maj]}] duration: 2} > play-song";
    let mut song = frozen_routes(code);
    let plan = plan(&song);
    let rows = song.query(span(0, 2, 1), &Default::default()).unwrap();
    assert!(rows.len() >= 6, "{rows:#?}");
    let expected = resolve(&plan, &rows[0]);
    for row in &rows {
        assert_eq!(resolve(&plan, row), expected);
    }
    let mut foreign = frozen_routes(code);
    let foreign_rows = foreign.query(span(0, 2, 1), &Default::default()).unwrap();
    assert!(resolve_route(&plan, &foreign_rows[0], SongLimits::default()).is_err());
    let mut altered = rows[0].clone();
    altered.track = intern_kw("foreign");
    assert!(resolve_route(&plan, &altered, SongLimits::default()).is_err());
    let tiny = SongLimits {
        max_nodes: 1,
        ..Default::default()
    };
    assert_eq!(
        resolve_route(&plan, &rows[0], tiny).unwrap_err().code,
        vactr::vm::fail::FailCode::FuelExhausted
    );
}
#[test]
fn direct_nested_placement_obeys_caller_depth_limit() {
    let mut song = frozen_routes("let p {part [drums: {s :analog}] duration: 1}\nsong {sequence [{part-repeat {sequence [p]} 2}]} > play-song");
    let plan = plan(&song);
    let rows = song.query(span(0, 2, 1), &Default::default()).unwrap();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        resolve(&plan, row);
    }
    let shallow = vactr::song::SongLimits {
        max_depth: 1,
        ..Default::default()
    };
    assert_eq!(
        vactr::song::routing::resolve_route(&plan, &rows[0], shallow)
            .unwrap_err()
            .code,
        vactr::vm::fail::FailCode::DepthExceeded
    );
}
#[test]
fn nested_overwrites_charge_logical_part_depth_for_untouched_sibling() {
    use vactr::value::intern::intern_kw;
    let mut code = String::from("let p0 {part [drums: {s :analog} hats: {s :fm}] duration: 3}\n");
    for i in 1..=5 {
        code.push_str(&format!(
            "let p{i} {{overwrite-region p{} :drums 1 2 {{s :analog}}}}\n",
            i - 1
        ));
    }
    code.push_str("song p5 > play-song");
    let mut song = frozen_routes(&code);
    let plan = plan(&song);
    let mut accepted = None;
    for max_depth in 1..=8 {
        let limits = SongLimits {
            max_depth,
            ..Default::default()
        };
        if let Ok(rows) = song.query(span(0, 3, 1), &limits) {
            accepted = Some((limits, rows));
            break;
        }
    }
    let (limits, rows) = accepted.expect("logical Part depth should fit eight");
    assert_eq!(limits.max_depth, 7);
    let hats: Vec<_> = rows
        .iter()
        .filter(|row| row.track == intern_kw("hats"))
        .collect();
    assert_eq!(hats.len(), 3);
    for row in &hats {
        assert_eq!(
            resolve_route(&plan, row, limits).unwrap().configuration,
            span(0, 3, 1)
        );
    }
    let structural = SongLimits {
        max_depth: 6,
        ..limits
    };
    for row in &hats {
        assert_eq!(
            resolve_route(&plan, row, structural).unwrap().configuration,
            span(0, 3, 1)
        );
    }
    let shallower = SongLimits {
        max_depth: 5,
        ..limits
    };
    assert_eq!(
        resolve_route(&plan, hats[0], shallower).unwrap_err().code,
        vactr::vm::fail::FailCode::DepthExceeded
    );
    let tiny = SongLimits {
        max_nodes: 1,
        ..limits
    };
    assert_eq!(
        resolve_route(&plan, hats[0], tiny).unwrap_err().code,
        vactr::vm::fail::FailCode::FuelExhausted
    );
}
#[test]
fn eager_repeat_lists_remain_valid_selected_source_layouts() {
    use vactr::pattern::TimeSpan;
    use vactr::song::{
        routing::{prepare_routes, resolve_route},
        SongLimits,
    };
    let mut expression = "p".to_owned();
    for _ in 0..10 {
        expression = format!("[{{repeat {expression} 1}}]");
    }
    let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog}}] duration: 1}}\nlet developed {{transform-instrument base :drums :analog {{p -> {expression}}}}}\nsong developed tail-seconds: 0 > play-song"));
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
    for row in rows {
        let route = resolve_route(&plan, &row, SongLimits::default()).unwrap();
        assert_eq!(route.sources.len(), 1);
        assert!(route.sources[0].identity.copies.is_empty());
    }
}
#[test]
fn rest_free_nested_slots_share_continuous_configuration_for_long_sources() {
    use vactr::pattern::TimeSpan;
    use vactr::value::Ratio64;
    for (recipe, end) in [
        ("[{hold p 3}]", Ratio64::from_int(64)),
        ("[[p]]", Ratio64::from_int(64)),
        ("iter [{hold p 3}] 0", Ratio64::from_int(64)),
        ("iter [{hold p 3}] 2", Ratio64::new(127, 2).unwrap()),
    ] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{slow {{s :analog}} 64}}] duration: 64}}\nlet developed {{transform-instrument base :drums :analog {{p -> {recipe}}}}}\nsong developed tail-seconds: 0 > play-song"));
        let plan = plan(&song);
        for (begin, stop) in [(6, 7), (2, 3), (0, 1)] {
            let rows = song
                .query(
                    TimeSpan::new(Ratio64::from_int(begin), Ratio64::from_int(stop)).unwrap(),
                    &SongLimits::default(),
                )
                .unwrap();
            let event = rows
                .iter()
                .find(|event| event.handle.occurrence().onset == Ratio64::ZERO)
                .unwrap();
            assert_eq!(
                resolve_route(&plan, event, SongLimits::default())
                    .unwrap()
                    .configuration,
                TimeSpan::new(Ratio64::ZERO, end).unwrap()
            );
        }
    }
}
#[test]
fn replaced_obsolete_dynamic_selected_roots_are_not_recertified() {
    let mut song=frozen_routes("let base {part [drums: {s :analog}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> fast p {t -> t}}}\nlet outer {transform-instrument inner :drums :analog {p -> first [p]}}\nlet replaced {replace-track outer :drums {s :fm}}\nsong replaced > play-song");
    let plan = plan(&song);
    assert!(plan.source_covers.is_empty());
    assert!(plan.nested_source_covers.is_empty());
    let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 4);
    for event in rows {
        assert!(resolve_route(&plan, &event, SongLimits::default())
            .unwrap()
            .sources
            .is_empty());
    }
}
#[test]
fn nested_covers_are_prepared_in_owning_payload_local_clock() {
    let song=frozen_routes("let base {part [drums: {s :analog}] duration: 2}\nlet inner {transform-instrument base :drums :analog {p -> fast p 2}}\nlet quiet {part [drums: {s nil}] duration: 3}\nlet shifted {sequence [quiet inner]}\nlet outer {transform-instrument shifted :drums :analog {p -> slow p 2}}\nsong outer > play-song");
    let plan = plan(&song);
    assert!(!plan.nested_source_covers.is_empty());
    for certificate in &plan.nested_source_covers {
        let owner = &plan.topology.parts[certificate.scope_part];
        assert_eq!(
            certificate.cover.window().begin,
            vactr::value::Ratio64::ZERO
        );
        assert_eq!(certificate.cover.window().end, owner.duration);
        assert!(certificate.cover.consumed_work() > 0);
    }
}
#[test]
fn nested_frames_authenticate_and_compose_geometry_inside_out() {
    use vactr::pattern::TimeSpan;
    use vactr::value::Ratio64;
    for (code,expected) in [
        ("let base {part [drums: {s :analog > note [60 64]}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> fast p 2}}\nlet outer {transform-instrument inner :drums :analog {p -> slow p 2}}\nsong outer > play-song",span(0,4,1)),
        ("let base {part [drums: {s :analog}] duration: 2}\nlet inner {transform-instrument base :drums :analog {p -> fast p 2}}\nlet quiet {part [drums: {s nil}] duration: 3}\nlet shifted {sequence [quiet inner]}\nlet outer {transform-instrument shifted :drums :analog {p -> fast p 2}}\nsong outer > play-song",span(3,4,2))
    ] {
        let mut song=frozen_routes(code);let plan=plan(&song);
        let rows=song.query(expected,&SongLimits::default()).unwrap();
        assert!(!rows.is_empty());
        for event in &rows {
            assert_eq!(event.source_origin.as_ref().unwrap().inherited.len(),1);
            let route=resolve_route(&plan,event,SongLimits::default()).unwrap();
            assert_eq!(route.sources.len(),2);
            assert_eq!(route.configuration,expected);
            assert!(route.sources.iter().all(|s|s.configuration==expected));
        }
        let sub=TimeSpan::new(expected.begin.checked_add(Ratio64::new(1,8).unwrap()).unwrap(),expected.end.checked_sub(Ratio64::new(1,8).unwrap()).unwrap()).unwrap();
        for event in song.query(sub,&SongLimits::default()).unwrap() { assert_eq!(resolve_route(&plan,&event,SongLimits::default()).unwrap().configuration,expected); }
        let mut bad=rows[0].clone();
        let origin=bad.source_origin.as_mut().unwrap();
        origin.inherited[0].handle=origin.handle.clone();
        assert!(resolve_route(&plan,&bad,SongLimits::default()).is_err());
        let mut bad=rows[0].clone();
        bad.source_origin.as_mut().unwrap().inherited[0].original_instrument=vactr::song::snapshot::FrozenSound::Builtin(vactr::value::intern::intern_kw("fm"));
        assert!(resolve_route(&plan,&bad,SongLimits::default()).is_err());
        let low=SongLimits {max_nodes:1,..Default::default()};
        assert_eq!(resolve_route(&plan,&rows[0],low).unwrap_err().code,vactr::vm::fail::FailCode::FuelExhausted);
    }
}
#[test]
fn four_nested_stages_preserve_complete_identity_and_shared_depth() {
    use vactr::pattern::TimeSpan;
    use vactr::value::{intern::intern_kw, Ratio64};
    let mut song=frozen_routes("let base {part [drums: {s :analog} bass: {s :analog}] duration: 2}\nlet a {transform-instrument base :drums :analog {p -> fast p 2}}\nlet b {transform-instrument a :drums :analog {p -> slow p 2}}\nlet c {transform-instrument b :drums :analog {p -> fast p 2}}\nlet d {transform-instrument c :drums :analog {p -> slow p 2}}\nlet bass {transform-instrument base :bass :analog {p -> first [p]}}\nsong {sequence [d bass]} > play-song");
    let plan = plan(&song);
    let limits = SongLimits {
        max_nodes: 1_000_000,
        ..Default::default()
    };
    let rows = song.query(span(0, 4, 1), &limits).unwrap();
    let drums: Vec<_> = rows
        .iter()
        .filter(|e| {
            e.track == intern_kw("drums")
                && e.source_origin
                    .as_ref()
                    .is_some_and(|o| o.inherited.len() == 3)
        })
        .collect();
    assert!(!drums.is_empty());
    let expected: Vec<_> = drums
        .iter()
        .map(|event| {
            (
                event.handle.clone(),
                resolve_route(&plan, event, limits).unwrap(),
            )
        })
        .collect();
    for (_, route) in &expected {
        assert_eq!(route.sources.len(), 4);
        assert_eq!(route.configuration, span(0, 2, 1));
    }
    for (begin, end, den) in [(4, 6, 4), (0, 2, 4), (3, 4, 4), (0, 2, 1)] {
        let selected = song
            .query(
                TimeSpan::new(
                    Ratio64::new(begin, den).unwrap(),
                    Ratio64::new(end, den).unwrap(),
                )
                .unwrap(),
                &limits,
            )
            .unwrap();
        let matching: Vec<_> = selected
            .iter()
            .filter(|e| {
                e.track == intern_kw("drums")
                    && e.source_origin
                        .as_ref()
                        .is_some_and(|o| o.inherited.len() == 3)
            })
            .collect();
        assert!(!matching.is_empty());
        for event in matching {
            let old = expected
                .iter()
                .find(|(handle, _)| *handle == event.handle)
                .unwrap();
            assert_eq!(resolve_route(&plan, event, limits).unwrap(), old.1);
        }
    }
    let mut bad = drums[0].clone();
    let last = bad
        .source_origin
        .as_ref()
        .unwrap()
        .inherited
        .last()
        .unwrap();
    let foreign = rows
        .iter()
        .filter_map(|e| e.source_origin.as_ref())
        .find(|o| {
            o.handle.track() == intern_kw("bass") && o.handle.revision() == last.handle.revision()
        })
        .unwrap();
    assert_ne!(foreign.handle.track(), last.handle.track());
    bad.source_origin
        .as_mut()
        .unwrap()
        .inherited
        .last_mut()
        .unwrap()
        .handle = foreign.handle.clone();
    assert!(resolve_route(&plan, &bad, limits).is_err());
    let mut bad = drums[0].clone();
    let trace = &mut bad.source_origin.as_mut().unwrap().inherited[1].entry_trace;
    assert!(!trace.is_empty());
    trace[0].ordinal = trace[0].ordinal.checked_add(100).unwrap();
    assert!(resolve_route(&plan, &bad, limits).is_err());
    let mut boundary = None;
    for max_depth in 1..=128 {
        let bounded = SongLimits {
            max_depth,
            ..limits
        };
        if resolve_route(&plan, drums[0], bounded).is_ok() {
            boundary = Some(bounded);
            break;
        }
    }
    let boundary = boundary.unwrap();
    assert!(boundary.max_depth > 1);
    assert_eq!(
        resolve_route(
            &plan,
            drums[0],
            SongLimits {
                max_depth: boundary.max_depth - 1,
                ..limits
            }
        )
        .unwrap_err()
        .code,
        vactr::vm::fail::FailCode::DepthExceeded
    );
}
#[test]
fn frozen_fx_policy_rejects_route_tampering_on_ordinary_and_nested_events() {
    use vactr::value::intern::intern_kw;
    for transform in [
        "first [base]",
        "transform-instrument base :drums :analog {p -> fast p 2}",
    ] {
        let code = format!("bus :plate-fx:\n\tplate mix: 1\nbus :spring-fx:\n\tspring-reverb mix: 1\nlet raw {{part [drums: {{s :analog}}] duration: 2}}\nlet plate {{instrument-fx raw :drums :analog :plate-fx}}\nlet spring {{instrument-fx raw :drums :analog :spring-fx}}\nlet base sequence [plate spring raw]\nlet developed {{{transform}}}\nsong developed > play-song");
        let mut song = frozen_routes(&code);
        let plan = plan(&song);
        assert!(plan
            .branches
            .iter()
            .any(|b| b.effect_template == Some(intern_kw("plate-fx"))));
        assert!(plan
            .branches
            .iter()
            .any(|b| b.effect_template == Some(intern_kw("spring-fx"))));
        let rows = song.query(span(0, 6, 1), &SongLimits::default()).unwrap();
        let plate = rows
            .iter()
            .find(|e| {
                e.route
                    .as_ref()
                    .is_some_and(|(_, k)| *k == intern_kw("plate-fx"))
            })
            .unwrap();
        resolve(&plan, plate);
        let mut bad = plate.clone();
        bad.route.as_mut().unwrap().0 =
            vec![vactr::song::snapshot::FrozenSound::Builtin(intern_kw("fm"))];
        assert!(resolve_route(&plan, &bad, SongLimits::default()).is_err());
        let mut bad = plate.clone();
        bad.route.as_mut().unwrap().1 = intern_kw("spring-fx");
        assert!(resolve_route(&plan, &bad, SongLimits::default()).is_err());
        let mut bad = plate.clone();
        bad.route = None;
        assert!(resolve_route(&plan, &bad, SongLimits::default()).is_err());
        let neutral = rows.iter().find(|e| e.route.is_none()).unwrap();
        resolve(&plan, neutral);
        let mut bad = neutral.clone();
        bad.route = plate.route.clone();
        assert!(resolve_route(&plan, &bad, SongLimits::default()).is_err());
        for query in [span(5, 6, 1), span(0, 1, 2), span(1, 5, 2)] {
            for row in song.query(query, &SongLimits::default()).unwrap() {
                let original = rows.iter().find(|e| e.handle == row.handle).unwrap();
                assert_eq!(resolve(&plan, &row), resolve(&plan, original));
            }
        }
    }
}
#[test]
fn static_periodic_components_share_policy_births_and_bound_fragment_density() {
    for operator in [
        "every p 2 {x -> first [x]}",
        "whenmod p 2 1 {x -> first [x]}",
    ] {
        let code = format!("let base {{part [drums: {{s :analog}}] duration: 64}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed tail-seconds: 8 > play-song");
        let mut song = frozen_routes(&code);
        let plan = plan(&song);
        assert!(plan
            .branches
            .iter()
            .all(|b| b.configurations_per_placement >= 64));
        assert!(plan
            .source_covers
            .iter()
            .all(|c| c.live_generation_bound > 2));
        let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
        assert_eq!(rows.len(), 4);
        for row in &rows {
            let birth = row.handle.occurrence().onset.floor();
            assert_eq!(resolve(&plan, row).configuration, span(birth, birth + 1, 1));
        }
        for query in [span(5, 7, 2), span(0, 1, 2), span(3, 5, 2)] {
            for row in song.query(query, &SongLimits::default()).unwrap() {
                let original = rows.iter().find(|e| e.handle == row.handle).unwrap();
                assert_eq!(resolve(&plan, &row), resolve(&plan, original));
            }
        }
    }
}
#[test]
fn static_periodic_long_continuations_keep_the_first_intrinsic_birth_component() {
    for operator in [
        "every p 2 {x -> first [x]}",
        "whenmod p 2 1 {x -> first [x]}",
    ] {
        let code = format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 4}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song");
        let mut song = frozen_routes(&code);
        let plan = plan(&song);
        let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
        assert_eq!(rows.len(), 2); // one merged continuation per static use edge
        let expected: Vec<_> = rows
            .iter()
            .map(|e| (e.handle.clone(), resolve(&plan, e)))
            .collect();
        assert!(expected
            .iter()
            .any(|(_, r)| r.configuration == span(0, 1, 1)));
        assert!(expected
            .iter()
            .any(|(_, r)| r.configuration == span(1, 2, 1)));
        for query in [span(5, 7, 2), span(1, 3, 2), span(0, 1, 2)] {
            for row in song.query(query, &SongLimits::default()).unwrap() {
                let original = expected
                    .iter()
                    .find(|(handle, _)| *handle == row.handle)
                    .unwrap();
                assert_eq!(resolve(&plan, &row), original.1);
            }
        }
    }
}
#[test]
fn chunk_wrap_slices_share_one_component_and_true_gaps_remain_separate() {
    use vactr::value::Ratio64;
    let mut song = frozen_routes("let base {part [drums: {s :analog > fast 2}] duration: 4}\nlet developed {transform-instrument base :drums :analog {p -> chunk p 2 {x -> first [x]}}}\nsong developed > play-song");
    let plan = plan(&song);
    let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    let left = rows
        .iter()
        .find(|e| e.handle.occurrence().onset == Ratio64::new(3, 2).unwrap())
        .unwrap();
    let right = rows
        .iter()
        .find(|e| e.handle.occurrence().onset == Ratio64::from_int(2))
        .unwrap();
    assert_eq!(resolve(&plan, left).configuration, span(3, 5, 2));
    assert_eq!(resolve(&plan, right).configuration, span(3, 5, 2));
    let first = rows
        .iter()
        .find(|e| e.handle.occurrence().onset == Ratio64::ZERO)
        .unwrap();
    assert_eq!(resolve(&plan, first).configuration, span(0, 1, 2));
    assert_ne!(
        resolve(&plan, first).configuration,
        resolve(&plan, left).configuration
    );
    for query in [span(4, 5, 2), span(3, 4, 2), span(0, 1, 4)] {
        for row in song.query(query, &SongLimits::default()).unwrap() {
            let original = rows.iter().find(|e| e.handle == row.handle).unwrap();
            assert_eq!(resolve(&plan, &row), resolve(&plan, original));
        }
    }
}
#[test]
fn chunk_held_continuations_preserve_uncut_configuration_across_reentry() {
    let mut song = frozen_routes("let base {part [drums: {s :analog > slow 64}] duration: 4}\nlet developed {transform-instrument base :drums :analog {p -> chunk p 2 {x -> first [x]}}}\nsong developed > play-song");
    let plan = plan(&song);
    let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 2);
    let expected: Vec<_> = rows
        .iter()
        .map(|e| (e.handle.clone(), resolve(&plan, e)))
        .collect();
    assert!(expected
        .iter()
        .any(|(_, r)| r.configuration == span(0, 1, 2)));
    assert!(expected
        .iter()
        .any(|(_, r)| r.configuration == span(1, 3, 2)));
    for query in [span(5, 7, 2), span(1, 3, 2), span(0, 1, 2)] {
        for row in song.query(query, &SongLimits::default()).unwrap() {
            let original = expected
                .iter()
                .find(|(handle, _)| *handle == row.handle)
                .unwrap();
            assert_eq!(resolve(&plan, &row), original.1);
        }
    }
}
#[test]
fn billion_period_predicates_resolve_intrinsic_runs_with_bounded_work() {
    for (operator, begin, end) in [
        ("every p 1000000000 {x -> first [x]}", 1, 1000000000),
        (
            "whenmod p 1000000000 999999999 {x -> first [x]}",
            0,
            999999999,
        ),
    ] {
        let code = format!("let base {{part [drums: {{s :analog}}] duration: 2000000000}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song");
        let mut song = frozen_routes(&code);
        let plan = plan(&song);
        let limits = SongLimits {
            max_nodes: 1024,
            ..Default::default()
        };
        for cycle in [1, 999999998] {
            let rows = song
                .query(span(cycle, cycle + 1, 1), &SongLimits::default())
                .unwrap();
            assert!(!rows.is_empty());
            for row in &rows {
                let route = resolve_route(&plan, row, limits).unwrap();
                assert_eq!(route.configuration, span(begin, end, 1));
                for query in [
                    span(cycle * 4 + 1, cycle * 4 + 3, 4),
                    span(cycle * 2 + 1, cycle * 2 + 2, 2),
                ] {
                    for split in song.query(query, &SongLimits::default()).unwrap() {
                        assert_eq!(split.handle, row.handle);
                        assert_eq!(resolve_route(&plan, &split, limits).unwrap(), route);
                    }
                }
            }
        }
    }
}
#[test]
fn degenerate_static_predicates_preserve_continuous_intrinsic_policy() {
    for operator in [
        "every p 1 {x -> first [x]}",
        "every p 0 {x -> first [x]}",
        "every p -2 {x -> first [x]}",
        "whenmod p 0 1 {x -> first [x]}",
        "whenmod p -2 1 {x -> first [x]}",
        "whenmod p 2 -1 {x -> first [x]}",
        "whenmod p 2 0 {x -> first [x]}",
        "whenmod p 2 2 {x -> first [x]}",
        "whenmod p 2 3 {x -> first [x]}",
        "chunk p 1 {x -> first [x]}",
        "chunk p 0 {x -> first [x]}",
        "chunk p -2 {x -> first [x]}",
    ] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog}}] duration: 4}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song"));
        let plan = plan(&song);
        let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
        assert_eq!(rows.len(), 4, "{operator}");
        for row in rows {
            assert_eq!(
                resolve(&plan, &row).configuration,
                span(0, 4, 1),
                "{operator}"
            );
        }
    }
}
#[test]
fn nested_periodic_constraints_resolve_joint_held_birth_components() {
    let mut song = frozen_routes("let base {part [drums: {s :analog > slow 64}] duration: 4}\nlet developed {transform-instrument base :drums :analog {p -> every {every p 2 {x -> first [x]}} 3 {x -> first [x]}}}\nsong developed > play-song");
    let plan = plan(&song);
    let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 4);
    let expected: Vec<_> = rows
        .iter()
        .map(|row| {
            let route = resolve(&plan, row);
            assert_eq!(route.configuration, row.part);
            (row.handle.clone(), route)
        })
        .collect();
    for query in [span(5, 7, 2), span(1, 3, 2), span(0, 1, 2)] {
        for row in song.query(query, &SongLimits::default()).unwrap() {
            let original = expected
                .iter()
                .find(|(handle, _)| *handle == row.handle)
                .unwrap();
            assert_eq!(resolve(&plan, &row), original.1);
        }
    }
}
#[test]
fn joint_large_periods_jump_run_boundaries_under_one_shared_quota() {
    let mut song = frozen_routes("let base {part [drums: {s :analog > slow 1000000000}] duration: 1000000000}\nlet developed {transform-instrument base :drums :analog {p -> every {every p 999999999 {x -> first [x]}} 1000000000 {x -> first [x]}}}\nsong developed tail-seconds: 0 > play-song");
    let mut available = route_capacity();
    available.template_slots = 1024;
    available.bus_slots = 1024;
    available.bus_frames = 512_000_000;
    let plan = vactr::song::routing::prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &available,
    )
    .unwrap();
    let limits = SongLimits {
        max_nodes: 1024,
        ..Default::default()
    };
    for (cycle, end) in [(0, 1), (1, 999999999), (999999999, 1000000000)] {
        let rows = song
            .query(span(cycle, cycle + 1, 1), &SongLimits::default())
            .unwrap();
        assert_eq!(rows.len(), 1);
        let route = resolve_route(&plan, &rows[0], limits).unwrap();
        assert_eq!(route.configuration, span(cycle, end, 1));
        for query in [
            span(cycle * 4 + 1, cycle * 4 + 3, 4),
            span(cycle * 2 + 1, cycle * 2 + 2, 2),
        ] {
            let split = song.query(query, &SongLimits::default()).unwrap();
            assert_eq!(split.len(), 1);
            assert_eq!(split[0].handle, rows[0].handle);
            assert_eq!(resolve_route(&plan, &split[0], limits).unwrap(), route);
        }
        assert_eq!(
            resolve_route(
                &plan,
                &rows[0],
                SongLimits {
                    max_nodes: 1,
                    ..limits
                }
            )
            .unwrap_err()
            .code,
            vactr::vm::fail::FailCode::FuelExhausted
        );
    }
}
#[test]
fn early_joint_applicability_does_not_require_a_representable_global_period() {
    let mut song = frozen_routes("let base {part [drums: {s :analog > slow 64}] duration: 4}\nlet developed {transform-instrument base :drums :analog {p -> every {every p 9223372036854775806 {x -> first [x]}} 9223372036854775807 {x -> first [x]}}}\nsong developed tail-seconds: 0 > play-song");
    let plan = plan(&song);
    let rows = song.query(span(0, 2, 1), &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        let route = resolve(&plan, row);
        assert!(route.configuration == span(0, 1, 1) || route.configuration == span(1, 4, 1));
    }
}
#[path = "song_source_routes/grid.rs"]
mod grid;

//! Actual frozen conditional metadata preserves runtime integer/anchor semantics.
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenRoutingInventory};
use vactr::song::source_uses::{
    certify_source_uses, resolve_source_use, FrozenPattern, FrozenStaticSampling as S,
    FrozenUseMapping as M, FrozenUseOperation as O, FrozenUseTraceTerm as T,
};
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
use vactr::value::Ratio64;
use vactr::vm::fail::FailCode;

fn candidate(code: &str) -> PreparedSong {
    try_candidate(code).unwrap_or_else(|e| panic!("candidate {code}: {e:?}"))
}
fn try_candidate(code: &str) -> Result<PreparedSong, vactr::vm::Failure> {
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let cx = CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 1_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    prepare_song(evaluate_song_candidate(
        code,
        "sampling.vact",
        1,
        SnapshotEpoch(71),
        &cx,
    )?)
}

fn selected(operator: &str) -> PreparedSong {
    candidate(&format!("let base {{part [drums: {{s :analog > note [60 64]}}] duration: 4}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song"))
}
fn payload(inventory: &FrozenRoutingInventory) -> FrozenPattern {
    match &inventory.parts[inventory.root_part].node {
        FrozenPartNode::Edit {
            edit: FrozenEdit::Transform { payload, .. },
            ..
        } => payload.clone(),
        FrozenPartNode::Capture(tracks) => tracks[0].1.clone(),
        other => panic!("unexpected fixture: {other:?}"),
    }
}
fn recipe(pattern: &FrozenPattern) -> S {
    pattern
        .source_uses
        .nodes
        .iter()
        .find_map(|node| match node.mapping {
            M::SampleGrid { sampling, .. } => Some(sampling),
            _ => None,
        })
        .expect("actual Euclid grid")
}
fn window() -> TimeSpan {
    TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap()
}
#[test]
fn genuine_sparse_full_complement_and_rotated_masks_are_distinct() {
    for (k, r, count) in [
        (1, 0, 1),
        (4, 0, 4),
        (-1, 0, 3),
        (1, 1, 1),
        (0, 0, 0),
        (-4, 0, 0),
    ] {
        let mut song = selected(&format!("euclid p {k} 4 rotation: {r}"));
        let pattern = payload(song.snapshot().routing());
        assert_eq!(
            recipe(&pattern),
            S::Euclid {
                pulses: k,
                divisions: 4,
                rotation: r
            }
        );
        let cover = certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            window(),
            SongLimits::default(),
        )
        .unwrap();
        let rows = song.query(window(), &SongLimits::default()).unwrap();
        assert_eq!(rows.len(), count, "k={k} r={r}");
        if count == 0 {
            assert_eq!(cover.configuration_bound(), 0);
        }
        for row in rows {
            resolve_source_use(
                &cover,
                row.source_origin.as_ref().unwrap(),
                SongLimits::default(),
            )
            .unwrap();
        }
    }
}
#[test]
fn fractional_points_sample_cell_starts_and_last_enabled_slot_resolves() {
    let mut song = selected("euclid p 4 4");
    let pattern = payload(song.snapshot().routing());
    for t in [
        Ratio64::new(3, 8).unwrap(),
        Ratio64::new(3, 4).unwrap(),
        Ratio64::new(7, 8).unwrap(),
    ] {
        let point = TimeSpan::point(t);
        let cover = certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            point,
            SongLimits::default(),
        )
        .unwrap();
        let rows = song.query(point, &SongLimits::default()).unwrap();
        assert!(!rows.is_empty());
        for row in rows {
            resolve_source_use(
                &cover,
                row.source_origin.as_ref().unwrap(),
                SongLimits::default(),
            )
            .unwrap();
        }
    }
}
#[test]
fn integral_numeric_literals_keep_raw_rotation_and_pulses() {
    for (k, n, r, expected) in [
        (
            "2/1",
            "4/1",
            "-9",
            S::Euclid {
                pulses: 2,
                divisions: 4,
                rotation: -9,
            },
        ),
        (
            "2.0",
            "4.0",
            "8193",
            S::Euclid {
                pulses: 2,
                divisions: 4,
                rotation: 8193,
            },
        ),
    ] {
        let song = selected(&format!("euclid p {k} {n} rotation: {r}"));
        assert_eq!(recipe(&payload(song.snapshot().routing())), expected);
    }
}
#[test]
fn malformed_grid_operation_content_trace_layout_and_divisions_reject() {
    let song = selected("euclid p 1 4");
    for change in 0..7 {
        let mut pattern = payload(song.snapshot().routing());
        let node = pattern
            .source_uses
            .nodes
            .iter_mut()
            .find(|n| matches!(n.mapping, M::SampleGrid { .. }))
            .unwrap();
        match change {
            0 => node.operation = O::Sound,
            1 => {
                node.mapping = M::SampleGrid {
                    content: 1,
                    sampling: S::Euclid {
                        pulses: 1,
                        divisions: 4,
                        rotation: 0,
                    },
                }
            }
            2 => node.edges[0].trace.clear(),
            3 => node.edges[0].child = u32::MAX,
            4 => {
                node.mapping = M::SampleGrid {
                    content: 0,
                    sampling: S::Euclid {
                        pulses: 1,
                        divisions: 0,
                        rotation: 0,
                    },
                }
            }
            5 => node.edges[0]
                .layout
                .push(vactr::song::source_uses::layout::FrozenUseSlotLayout {
                    prefix: Ratio64::ZERO,
                    width: Ratio64::ONE,
                    copy_trace_term: None,
                }),
            _ => node.edges[0].trace.push(T::Copies {
                kind: vactr::pattern::occ::ProducerKind::Child,
                count: 2,
            }),
        }
        assert!(certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            window(),
            SongLimits::default()
        )
        .is_err());
    }
}
#[test]
fn bounded_work_depth_and_expensive_valid_divisions_fail_honestly() {
    let song = selected("euclid p 1 4");
    let pattern = payload(song.snapshot().routing());
    let limits = SongLimits {
        max_nodes: 1,
        ..SongLimits::default()
    };
    assert_eq!(
        certify_source_uses(song.snapshot().routing(), &pattern, window(), limits)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    let limits = SongLimits {
        max_depth: 1,
        ..SongLimits::default()
    };
    assert_eq!(
        certify_source_uses(song.snapshot().routing(), &pattern, window(), limits)
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
}
#[test]
fn ordinary_dynamic_euclid_geometry_keeps_nested_source_free_compatibility() {
    for operator in [
        "euclid {s :analog} {t -> 1} 4",
        "euclid {s :analog} 1 4 rotation: {t -> 1}",
    ] {
        let song = candidate(&format!("let base {{part [drums: {{{operator}}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog {{p -> first [p]}}}}\nsong selected > play-song"));
        let pattern = payload(song.snapshot().routing());
        assert!(song
            .snapshot()
            .routing()
            .parts
            .iter()
            .any(|part| match &part.node {
                FrozenPartNode::Capture(tracks) => tracks.iter().any(|(_, p)| p
                    .source_uses
                    .nodes
                    .iter()
                    .any(|n| matches!(n.mapping, M::Uncertifiable(_)))),
                _ => false,
            }));
        certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            window(),
            SongLimits::default(),
        )
        .unwrap();
    }
    for operator in ["euclid p {t -> 1} 4", "euclid p 1 4 rotation: {t -> 1}"] {
        let song = selected(operator);
        let pattern = payload(song.snapshot().routing());
        assert_eq!(
            certify_source_uses(
                song.snapshot().routing(),
                &pattern,
                window(),
                SongLimits::default()
            )
            .unwrap_err()
            .code,
            FailCode::BeyondCapability
        );
    }
}
#[test]
fn authentic_shared_base_origin_at_disabled_grid_phase_is_rejected() {
    let mut song=candidate("let base {part [drums: {s :analog}] duration: 1}\nlet sparse {transform-instrument base :drums :analog {p -> euclid p 1 4}}\nlet full {transform-instrument base :drums :analog {p -> euclid p 4 4}}\nlet chain {sequence [sparse full]}\nsong chain > play-song");
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } if recipe(payload)
                == S::Euclid {
                    pulses: 1,
                    divisions: 4,
                    rotation: 0,
                } =>
            {
                Some(payload.clone())
            }
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, &pattern, window(), SongLimits::default()).unwrap();
    let rows = song
        .query(
            TimeSpan::new(Ratio64::ONE, Ratio64::from_int(2)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let origin = row.source_origin.as_ref().unwrap();
        assert!(origin.source_part.is_point());
        let result = resolve_source_use(&cover, origin, SongLimits::default());
        if origin.source_part.begin == Ratio64::ZERO {
            result.unwrap();
        } else {
            assert!(result.is_err(), "disabled phase {:?}", origin.source_part);
        }
    }
}

#[test]
fn direct_sampling_support_cannot_be_forged_into_nonpoint_or_foreign_window() {
    let mut song = selected("euclid p 4 4");
    let pattern = payload(song.snapshot().routing());
    let cover = certify_source_uses(
        song.snapshot().routing(),
        &pattern,
        window(),
        SongLimits::default(),
    )
    .unwrap();
    let rows = song.query(window(), &SongLimits::default()).unwrap();
    let origin = rows[0].source_origin.as_ref().unwrap();
    resolve_source_use(&cover, origin, SongLimits::default()).unwrap();
    let mut altered = origin.clone();
    altered.source_part = window();
    assert!(resolve_source_use(&cover, &altered, SongLimits::default()).is_err());
    let narrow = TimeSpan::point(Ratio64::new(3, 8).unwrap());
    let cover = certify_source_uses(
        song.snapshot().routing(),
        &pattern,
        narrow,
        SongLimits::default(),
    )
    .unwrap();
    assert!(resolve_source_use(&cover, origin, SongLimits::default()).is_err());
}
#[test]
fn genuine_source_gap_and_reordered_fractional_queries_use_sample_starts() {
    let mut song=candidate("let bass {part [bass: {s :fm}] duration: 1/4}\nlet drums {part [drums: {s :analog}] duration: 1/4}\nlet tail {part [bass: {s :fm}] duration: 1/2}\nlet base {sequence [bass drums tail]}\nlet selected {transform-instrument base :drums :analog {p -> euclid p 4 4}}\nsong selected > play-song");
    let pattern = payload(song.snapshot().routing());
    for t in [
        Ratio64::new(3, 8).unwrap(),
        Ratio64::new(1, 4).unwrap(),
        Ratio64::new(3, 8).unwrap(),
        Ratio64::new(5, 8).unwrap(),
    ] {
        let point = TimeSpan::point(t);
        let cover = certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            point,
            SongLimits::default(),
        )
        .unwrap();
        let rows = song.query(point, &SongLimits::default()).unwrap();
        let selected: Vec<_> = rows
            .iter()
            .filter_map(|row| row.source_origin.as_ref())
            .collect();
        if t < Ratio64::new(1, 2).unwrap() {
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].source_part.begin, Ratio64::new(1, 4).unwrap());
            resolve_source_use(&cover, selected[0], SongLimits::default()).unwrap();
        } else {
            assert!(selected.is_empty());
            assert_eq!(cover.configuration_bound(), 0);
        }
    }
}
#[test]
fn static_invalid_candidate_numbers_report_runtime_diagnostics() {
    for (value, message) in [
        ("0", "euclid steps must be between 1 and 4096"),
        ("-1", "euclid steps must be between 1 and 4096"),
        ("4097", "euclid steps must be between 1 and 4096"),
        ("3/2", "expected an integer"),
        ("4.5", "expected an integer"),
    ] {
        let code=format!("let base {{part [drums: {{s :analog}}] duration: 1}}\nlet selected {{transform-instrument base :drums :analog {{p -> euclid p 1 {value}}}}}\nsong selected > play-song");
        let error = match try_candidate(&code) {
            Ok(_) => panic!("invalid {value} accepted"),
            Err(e) => e,
        };
        assert_eq!(error.code, FailCode::Type);
        assert!(error.message.contains(message), "{error:?}");
    }
}
#[test]
fn cover_cost_is_reusable_without_resetting_cumulative_work() {
    let song = selected("euclid p 1 4");
    let pattern = payload(song.snapshot().routing());
    let default = SongLimits::default();
    let cover =
        certify_source_uses(song.snapshot().routing(), &pattern, window(), default).unwrap();
    let cost = cover.consumed_work();
    let exact = SongLimits {
        max_nodes: cost,
        ..default
    };
    certify_source_uses(song.snapshot().routing(), &pattern, window(), exact).unwrap();
    let short = SongLimits {
        max_nodes: cost - 1,
        ..default
    };
    assert_eq!(
        certify_source_uses(song.snapshot().routing(), &pattern, window(), short)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    let mut remaining = cost.checked_mul(2).unwrap();
    for _ in 0..2 {
        let cover = certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            window(),
            SongLimits {
                max_nodes: remaining,
                ..default
            },
        )
        .unwrap();
        remaining -= cover.consumed_work();
        assert_eq!(cover.clone().consumed_work(), cost);
    }
    assert_eq!(remaining, 0);
}

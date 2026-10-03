//! Actual candidate weighted source slots; runtime and copied geometry agree.
use std::collections::BTreeMap;
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenEdit, FrozenPartNode};
use vactr::song::source_uses::layout::{map_slot_configuration, slot_interval, validate_layout};
use vactr::song::source_uses::{certify_source_uses, resolve_source_use, FrozenPattern};
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
use vactr::value::Ratio64;
fn ratio(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).unwrap()
}
fn candidate(expression: &str) -> PreparedSong {
    let assets = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let cx = CandidateBuildCtx {
        assets: &assets,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 4_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    let code=format!("let base {{part [drums: {{s :analog}}] duration: 4}}\nlet developed {{transform-instrument base :drums :analog {{p -> {expression}}}}}\nsong developed > play-song");
    prepare_song(evaluate_song_candidate(&code, "score.vact", 1, SnapshotEpoch(1), &cx).unwrap())
        .unwrap()
}
fn payload(song: &PreparedSong) -> &FrozenPattern {
    song.snapshot()
        .routing()
        .parts
        .iter()
        .find_map(|p| match &p.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap()
}
#[test]
fn rests_and_hold_weights_match_actual_output_slots() {
    for (expression, width) in [
        ("[p nil]", ratio(1, 2)),
        ("[p nil nil]", ratio(1, 3)),
        ("[{hold p 3/2} nil]", ratio(3, 5)),
    ] {
        let mut song = candidate(expression);
        let pattern = payload(&song);
        let edge = pattern
            .source_uses
            .nodes
            .iter()
            .flat_map(|n| &n.edges)
            .find(|e| !e.layout.is_empty())
            .unwrap();
        assert_eq!(edge.layout[0].width, width, "{expression}");
        assert_eq!(edge.layout[0].prefix, Ratio64::ZERO);
        let window = TimeSpan::cycle(0).unwrap();
        let cover = certify_source_uses(
            song.snapshot().routing(),
            pattern,
            window,
            SongLimits::default(),
        )
        .unwrap();
        let rows = song.query(window, &SongLimits::default()).unwrap();
        assert!(!rows.is_empty());
        for row in rows {
            let origin = row.source_origin.unwrap();
            resolve_source_use(&cover, &origin, SongLimits::default()).unwrap();
            assert!(row.part.end <= width);
        }
    }
}
#[test]
fn nested_normalized_layout_and_symbolic_repeats_preserve_geometry() {
    let mut song = candidate("[nil [{repeat p 2} nil] nil]");
    let pattern = payload(&song);

    let edge = pattern
        .source_uses
        .nodes
        .iter()
        .flat_map(|n| &n.edges)
        .find(|e| e.layout.len() == 3)
        .unwrap()
        .clone();
    assert_eq!(edge.layout[0].prefix, ratio(1, 3));
    assert_eq!(edge.layout[0].width, ratio(1, 3));
    assert_eq!(edge.layout[1].width, ratio(1, 2));
    assert_eq!(edge.layout[2].width, ratio(1, 2));
    assert!(edge
        .layout
        .iter()
        .all(|slot| slot.copy_trace_term.is_none()));
    let window = TimeSpan::cycle(0).unwrap();
    let cover = certify_source_uses(
        song.snapshot().routing(),
        pattern,
        window,
        SongLimits::default(),
    )
    .unwrap();
    let graph = pattern.source_uses.clone();
    let rows = song.query(window, &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 2);
    let mut copies = Vec::new();
    for row in rows {
        let origin = row.source_origin.unwrap();
        let id = resolve_source_use(&cover, &origin, SongLimits::default()).unwrap();
        let edge = &graph.nodes[graph.root as usize].edges[id.edges[0] as usize];
        copies.push(id.edges);
        let interval = slot_interval(edge, 0, &origin.entry_trace[..edge.trace.len()]).unwrap();
        assert_eq!(row.part, interval);
        assert_eq!(
            interval.end.checked_sub(interval.begin).unwrap(),
            ratio(1, 12)
        );
        assert_eq!(
            map_slot_configuration(
                edge,
                0,
                TimeSpan::cycle(0).unwrap(),
                &origin.entry_trace[..edge.trace.len()]
            )
            .unwrap(),
            interval
        );
    }
    assert_ne!(copies[0], copies[1]);
}
#[test]
fn malformed_geometry_and_small_admission_are_rejected() {
    let song = candidate("[p nil]");
    let original = payload(&song);
    for kind in 0..4 {
        let mut pattern = original.clone();
        let edge = pattern
            .source_uses
            .nodes
            .iter_mut()
            .flat_map(|n| &mut n.edges)
            .find(|e| !e.layout.is_empty())
            .unwrap();
        match kind {
            0 => edge.layout[0].width = Ratio64::ZERO,
            1 => edge.layout[0].prefix = Ratio64::ONE,
            2 => edge.layout[0].copy_trace_term = Some(999),
            _ => edge.layout[0].width = Ratio64::from_int(i64::MAX),
        }
        assert!(validate_layout(edge).is_err());
        assert!(certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            TimeSpan::cycle(0).unwrap(),
            SongLimits::default()
        )
        .is_err());
    }
    assert!(certify_source_uses(
        song.snapshot().routing(),
        original,
        TimeSpan::cycle(0).unwrap(),
        SongLimits {
            max_nodes: 1,
            ..SongLimits::default()
        }
    )
    .is_err());
}
#[test]
fn ordinary_eager_repeat_lists_keep_all_silent_slots() {
    for (expression, expected) in [("[{repeat p 0} nil]", 0), ("[{repeat p 4} nil]", 4)] {
        let song = candidate(expression);
        let pattern = payload(&song);
        let cover = certify_source_uses(
            song.snapshot().routing(),
            pattern,
            TimeSpan::cycle(0).unwrap(),
            SongLimits::default(),
        )
        .unwrap();
        assert_eq!(cover.configuration_bound(), expected);
        assert!(pattern.source_uses.nodes.len() < 10);
    }
}

#[test]
fn actual_points_and_continuations_keep_intrinsic_edge_geometry() {
    let mut song = candidate("[p nil nil]");
    let full = TimeSpan::cycle(0).unwrap();
    let cover = certify_source_uses(
        song.snapshot().routing(),
        payload(&song),
        full,
        SongLimits::default(),
    )
    .unwrap();
    let original = song.query(full, &SongLimits::default()).unwrap();
    let id = resolve_source_use(
        &cover,
        original[0].source_origin.as_ref().unwrap(),
        SongLimits::default(),
    )
    .unwrap();
    for window in [
        TimeSpan::new(ratio(1, 12), ratio(1, 6)).unwrap(),
        TimeSpan::new(ratio(1, 6), ratio(1, 6)).unwrap(),
        TimeSpan::new(ratio(1, 6), ratio(1, 3)).unwrap(),
    ] {
        let narrow = certify_source_uses(
            song.snapshot().routing(),
            payload(&song),
            window,
            SongLimits::default(),
        )
        .unwrap();
        let rows = song.query(window, &SongLimits::default()).unwrap();
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(
                resolve_source_use(
                    &narrow,
                    row.source_origin.as_ref().unwrap(),
                    SongLimits::default()
                )
                .unwrap(),
                id
            );
        }
    }
    let rest = TimeSpan::new(ratio(2, 3), ratio(2, 3)).unwrap();
    assert_eq!(
        certify_source_uses(
            song.snapshot().routing(),
            payload(&song),
            rest,
            SongLimits::default()
        )
        .unwrap()
        .configuration_bound(),
        0
    );
    assert!(song.query(rest, &SongLimits::default()).unwrap().is_empty());
}
#[test]
fn rates_reflection_and_fractional_hold_compose_with_exact_slots() {
    for expression in [
        "fast [{hold p 1/2} nil] 2",
        "slow [p nil] 2",
        "rev [nil p]",
        "[nil [{hold p 1/3} nil] nil]",
    ] {
        let mut song = candidate(expression);
        let window = TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(2)).unwrap();
        let cover = certify_source_uses(
            song.snapshot().routing(),
            payload(&song),
            window,
            SongLimits::default(),
        )
        .unwrap();
        let rows = song.query(window, &SongLimits::default()).unwrap();
        assert!(!rows.is_empty(), "{expression}");
        let full_ids: Vec<_> = rows
            .iter()
            .map(|row| {
                resolve_source_use(
                    &cover,
                    row.source_origin.as_ref().unwrap(),
                    SongLimits::default(),
                )
                .unwrap()
            })
            .collect();
        for window in [
            TimeSpan::new(Ratio64::ZERO, ratio(3, 4)).unwrap(),
            TimeSpan::new(ratio(3, 4), Ratio64::from_int(2)).unwrap(),
        ] {
            let narrow = certify_source_uses(
                song.snapshot().routing(),
                payload(&song),
                window,
                SongLimits::default(),
            )
            .unwrap();
            for row in song.query(window, &SongLimits::default()).unwrap() {
                let id = resolve_source_use(
                    &narrow,
                    row.source_origin.as_ref().unwrap(),
                    SongLimits::default(),
                )
                .unwrap();
                assert!(full_ids.contains(&id), "{expression}");
            }
        }
    }
}

#[test]
fn dynamic_hold_geometry_has_an_addressed_preparation_diagnostic() {
    let song = candidate("[{hold p {t -> 1}} nil]");
    let error = certify_source_uses(
        song.snapshot().routing(),
        payload(&song),
        TimeSpan::cycle(0).unwrap(),
        SongLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, vactr::vm::FailCode::BeyondCapability);
    assert!(error.message.contains("DynamicTiming"));
}
#[test]
fn reordered_windows_and_wrong_edge_traces_cannot_change_layout_identity() {
    let mut song = candidate("[nil p nil]");
    let pattern = payload(&song);
    let edge = pattern
        .source_uses
        .nodes
        .iter()
        .flat_map(|node| &node.edges)
        .find(|edge| !edge.layout.is_empty())
        .unwrap()
        .clone();
    let cover = certify_source_uses(
        song.snapshot().routing(),
        pattern,
        TimeSpan::cycle(0).unwrap(),
        SongLimits::default(),
    )
    .unwrap();
    let mut identities = Vec::new();
    for window in [
        TimeSpan::new(ratio(1, 2), ratio(2, 3)).unwrap(),
        TimeSpan::new(ratio(1, 3), ratio(1, 2)).unwrap(),
        TimeSpan::cycle(0).unwrap(),
    ] {
        for row in song.query(window, &SongLimits::default()).unwrap() {
            let origin = row.source_origin.as_ref().unwrap();
            identities.push(resolve_source_use(&cover, origin, SongLimits::default()).unwrap());
            let mut wrong = origin.entry_trace[..edge.trace.len()].to_vec();
            wrong[0].ordinal = 99;
            assert!(slot_interval(&edge, 0, &wrong).is_err());
        }
    }
    assert!(!identities.is_empty());
    assert!(identities.iter().all(|id| id == &identities[0]));
}

#[test]
fn multi_cycle_envelope_authenticates_exact_and_symbolic_copy_traces() {
    use vactr::pattern::occ::{ProducerKind, ProducerStep};
    use vactr::song::source_uses::layout::{map_slot_support, FrozenUseSlotLayout};
    use vactr::song::source_uses::{FrozenSourceUseEdge, FrozenUseTraceTerm};
    let child = ProducerStep {
        kind: ProducerKind::Child,
        ordinal: 0,
    };
    let copy = ProducerStep {
        kind: ProducerKind::GeneratedBranch,
        ordinal: 1,
    };
    let edge = FrozenSourceUseEdge {
        child: 0,
        trace: vec![
            FrozenUseTraceTerm::Exact(child),
            FrozenUseTraceTerm::Copies {
                kind: ProducerKind::GeneratedBranch,
                count: 2,
            },
        ],
        layout: vec![FrozenUseSlotLayout {
            prefix: Ratio64::ZERO,
            width: ratio(1, 2),
            copy_trace_term: Some(1),
        }],
    };
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap();
    assert_eq!(
        map_slot_support(&edge, window, Some(&[child, copy])).unwrap(),
        Some(window)
    );
    let mut wrong_exact = child;
    wrong_exact.ordinal = 1;
    let mut wrong_kind = copy;
    wrong_kind.kind = ProducerKind::Child;
    let mut wrong_copy = copy;
    wrong_copy.ordinal = 2;
    for trace in [
        vec![child],
        vec![wrong_exact, copy],
        vec![child, wrong_kind],
        vec![child, wrong_copy],
    ] {
        assert!(map_slot_support(&edge, window, Some(&trace)).is_err());
    }
    let mut unweighted = edge;
    unweighted.layout.clear();
    assert!(map_slot_support(&unweighted, window, Some(&[wrong_exact, copy])).is_err());
}

use super::*;

#[test]
fn selected_grid_masks_resolve_maximal_components_across_real_query_partitions() {
    use vactr::value::Ratio64;
    for (pulses, rotation) in [
        (1, 0),
        (4, 0),
        (2, 1),
        (-2, -1),
        (3, 0),
        (3, 1),
        (3, 2),
        (3, 3),
    ] {
        for wrapper in ["q", "fast q 2", "slow q 2"] {
            let expression = wrapper.replace(
                'q',
                &format!("{{euclid p {pulses} 4 rotation: {rotation}}}"),
            );
            let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog {{p -> {expression}}}}}\nsong selected tail-seconds: 0 > play-song"));
            let plan = plan(&song);
            let limits = vactr::song::SongLimits::default();
            let rows = song.query(span(0, 2, 1), &limits).unwrap();
            assert!(!rows.is_empty());
            let mut cells: Vec<_> = rows.iter().map(|event| event.whole.unwrap()).collect();
            cells.sort_unstable_by_key(|cell| cell.begin);
            let mut runs: Vec<vactr::pattern::TimeSpan> = Vec::new();
            for cell in cells {
                if let Some(last) = runs.last_mut().filter(|last| cell.begin <= last.end) {
                    last.end = last.end.max(cell.end);
                } else {
                    runs.push(cell);
                }
            }
            let expected: Vec<_> = rows
                .iter()
                .map(|event| {
                    let route = resolve(&plan, event);
                    assert_eq!(
                        route.configuration,
                        *runs
                            .iter()
                            .find(|run| run.begin <= event.whole.unwrap().begin
                                && event.whole.unwrap().begin < run.end)
                            .unwrap()
                    );
                    assert!(route.configuration.begin <= event.whole.unwrap().begin);
                    assert!(event.whole.unwrap().begin < route.configuration.end);
                    if pulses == 4 {
                        assert_eq!(
                            route.configuration,
                            if wrapper == "fast q 2" {
                                span(0, 1, 1)
                            } else {
                                span(0, 2, 1)
                            }
                        );
                    }
                    (event.handle.clone(), route)
                })
                .collect();
            for window in [span(5, 8, 4), span(1, 3, 4), span(0, 1, 8), span(3, 5, 4)] {
                for event in song.query(window, &limits).unwrap() {
                    assert_eq!(
                        resolve(&plan, &event),
                        expected
                            .iter()
                            .find(|(handle, _)| *handle == event.handle)
                            .unwrap()
                            .1
                    );
                }
            }
            let tiny = vactr::song::SongLimits {
                max_nodes: 1,
                ..Default::default()
            };
            assert_eq!(
                vactr::song::routing::resolve_route(&plan, &rows[0], tiny)
                    .unwrap_err()
                    .code,
                vactr::vm::fail::FailCode::FuelExhausted
            );
            assert!(expected
                .iter()
                .all(|(_, route)| route.configuration.end > Ratio64::ZERO));
        }
    }
}
#[test]
fn grid_interior_point_authenticates_fractional_source_sample_start() {
    let mut song = frozen_routes("let base {sequence [{part [bass: {s :fm}] duration: 1/4} {part [drums: {s :analog}] duration: 1/4} {part [bass: {s :fm}] duration: 1/2}]}\nlet selected {transform-instrument base :drums :analog {p -> euclid p 4 4}}\nsong selected tail-seconds: 0 > play-song");
    let plan = plan(&song);
    let limits = vactr::song::SongLimits::default();
    let full = song.query(span(0, 1, 1), &limits).unwrap();
    let selected = full
        .iter()
        .find(|event| event.source_origin.is_some())
        .unwrap();
    let route = resolve(&plan, selected);
    assert_eq!(route.configuration, span(1, 2, 4));
    for window in [span(3, 3, 8), span(3, 4, 8), span(2, 3, 8)] {
        let rows = song.query(window, &limits).unwrap();
        let event = rows
            .iter()
            .find(|event| event.source_origin.is_some())
            .unwrap();
        assert_eq!(event.handle, selected.handle);
        assert_eq!(resolve(&plan, event), route);
    }
}

#[test]
fn nested_periodic_sources_use_grid_sample_start_as_distinct_configuration_locator() {
    let mut song = frozen_routes("let base {part [drums: {s :analog > slow 64}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> every p 2 {x -> first [x]}}}\nlet grid {transform-instrument inner :drums :analog {p -> euclid p 1 4}}\nsong grid tail-seconds: 0 > play-song");
    let plan = plan(&song);
    let limits = SongLimits::default();
    let rows = song.query(span(0, 4, 1), &limits).unwrap();
    assert_eq!(rows.len(), 4);
    let expected: Vec<_> = rows
        .iter()
        .map(|event| {
            assert_eq!(event.source_origin.as_ref().unwrap().inherited.len(), 1);
            let route = resolve(&plan, event);
            assert_eq!(route.configuration, event.whole.unwrap());
            (event.handle.clone(), route)
        })
        .collect();
    let cycle2 = rows
        .iter()
        .find(|event| event.whole.unwrap().begin == span(2, 2, 1).begin)
        .unwrap();
    assert_eq!(resolve(&plan, cycle2).configuration, span(8, 9, 4));
    for query in [
        span(17, 17, 8),
        span(17, 18, 8),
        span(16, 17, 8),
        span(4, 5, 4),
        span(12, 13, 4),
    ] {
        let split = song.query(query, &limits).unwrap();
        assert_eq!(split.len(), 1);
        let old = expected
            .iter()
            .find(|(handle, _)| *handle == split[0].handle)
            .unwrap();
        assert_eq!(resolve(&plan, &split[0]), old.1);
        if query.begin == span(17, 17, 8).begin {
            assert_eq!(split[0].handle, cycle2.handle);
        }
    }
}

#[test]
fn nested_grid_locator_persists_through_three_frames_affine_and_offset_scopes() {
    for (middle, outer, shifted) in [
        ("every p 3 {x -> first [x]}", "euclid p 1 4", false),
        ("fast p 2", "euclid p 1 4", false),
        ("first [p]", "fast {euclid p 1 4} 2", false),
        ("every p 3 {x -> first [x]}", "euclid p 1 4", true),
    ] {
        let score = if shifted {
            "sequence [{part [bass: {s :fm}] duration: 1} grid]"
        } else {
            "grid"
        };
        let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 4}}\nlet inner {{transform-instrument base :drums :analog {{p -> every p 2 {{x -> first [x]}}}}}}\nlet middle {{transform-instrument inner :drums :analog {{p -> {middle}}}}}\nlet grid {{transform-instrument middle :drums :analog {{p -> {outer}}}}}\nsong {{{score}}} tail-seconds: 0 > play-song"));
        let plan = plan(&song);
        let limits = SongLimits::default();
        let full = song
            .query(span(0, if shifted { 5 } else { 4 }, 1), &limits)
            .unwrap();
        let expected: Vec<_> = full
            .iter()
            .filter(|event| event.source_origin.is_some())
            .map(|event| {
                assert_eq!(event.source_origin.as_ref().unwrap().inherited.len(), 2);
                let route = resolve(&plan, event);
                assert_eq!(route.configuration, event.whole.unwrap());
                (event.handle.clone(), route, event.whole.unwrap())
            })
            .collect();
        assert!(!expected.is_empty());
        for (handle, route, whole) in expected.iter().rev() {
            let midpoint = whole
                .begin
                .checked_add(whole.end)
                .unwrap()
                .checked_div(vactr::value::Ratio64::from_int(2))
                .unwrap();
            for window in [
                vactr::pattern::TimeSpan::point(midpoint),
                vactr::pattern::TimeSpan::new(midpoint, whole.end).unwrap(),
                vactr::pattern::TimeSpan::new(whole.begin, midpoint).unwrap(),
            ] {
                let rows = song.query(window, &limits).unwrap();
                let event = rows
                    .iter()
                    .find(|event| event.source_origin.is_some())
                    .unwrap();
                assert_eq!(&event.handle, handle);
                assert_eq!(&resolve(&plan, event), route);
            }
        }
    }
}

fn composed_grid_song(inner: &str, outer: &str, shifted: bool) -> vactr::song::PreparedSong {
    let source = if shifted {
        "sequence [{part [bass: {s :fm}] duration: 1} inner]"
    } else {
        "inner"
    };
    frozen_routes(&format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 4}}\nlet inner {{transform-instrument base :drums :analog {{p -> {inner}}}}}\nlet source {{{source}}}\nlet grid {{transform-instrument source :drums :analog {{p -> {outer}}}}}\nsong grid tail-seconds: 0 > play-song"))
}
fn assert_sampled_partitions(
    song: &mut vactr::song::PreparedSong,
    plan: &vactr::song::routing::SongRoutePlan,
    rows: &[vactr::song::snapshot::FrozenSongEvent],
) {
    use vactr::{pattern::TimeSpan, value::Ratio64};
    for event in rows.iter().rev() {
        let expected = resolve(plan, event);
        let whole = event.whole.unwrap();
        let middle = whole
            .begin
            .checked_add(whole.end)
            .unwrap()
            .checked_div(Ratio64::from_int(2))
            .unwrap();
        for query in [
            TimeSpan::point(middle),
            TimeSpan::new(middle, whole.end).unwrap(),
            TimeSpan::new(whole.begin, middle).unwrap(),
        ] {
            let split = song.query(query, &SongLimits::default()).unwrap();
            let row = split
                .iter()
                .find(|row| row.source_origin.is_some())
                .unwrap();
            assert_eq!(row.handle, event.handle);
            assert_eq!(resolve(plan, row), expected);
        }
    }
}
fn assert_sampled_cycle_groups(
    plan: &vactr::song::routing::SongRoutePlan,
    rows: &[vactr::song::snapshot::FrozenSongEvent],
    cycles: std::ops::Range<i64>,
    denominator: i64,
    phases: &[i64],
) {
    use std::collections::BTreeMap;
    use vactr::value::Ratio64;
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for row in rows {
        groups
            .entry(row.whole.unwrap().begin.floor())
            .or_default()
            .push(row);
    }
    assert_eq!(
        groups.keys().copied().collect::<Vec<_>>(),
        cycles.collect::<Vec<_>>()
    );
    let mut previous_configuration = None;
    for (cycle, mut group) in groups {
        group.sort_by_key(|row| row.whole.unwrap().begin);
        let expected_starts: Vec<_> = phases
            .iter()
            .map(|phase| {
                Ratio64::from_int(cycle)
                    .checked_add(Ratio64::new(*phase, denominator).unwrap())
                    .unwrap()
            })
            .collect();
        assert_eq!(group.len(), phases.len(), "cycle={cycle}");
        assert_eq!(
            group
                .iter()
                .map(|row| row.whole.unwrap().begin)
                .collect::<Vec<_>>(),
            expected_starts
        );
        for row in &group {
            let whole = row.whole.unwrap();
            assert_eq!(
                whole.end.checked_sub(whole.begin).unwrap(),
                Ratio64::new(1, denominator).unwrap()
            );
        }
        let expected = resolve(plan, group[0]);
        for row in &group {
            assert_eq!(
                resolve(plan, row),
                expected,
                "complete route at cycle={cycle}, whole={:?}",
                row.whole
            );
        }
        if let Some(previous) = previous_configuration {
            assert_ne!(expected.configuration, previous);
        }
        previous_configuration = Some(expected.configuration);
    }
}
#[test]
fn composed_grids_preserve_child_whole_and_maximal_sampled_runs() {
    let mut song = composed_grid_song("euclid p 1 4", "euclid p 8 8", false);
    let plan = plan(&song);
    let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 8);
    for row in &rows {
        let c = row.whole.unwrap().begin.floor();
        assert_eq!(resolve(&plan, row).configuration, span(4 * c, 4 * c + 1, 4));
        let origin = row.source_origin.as_ref().unwrap();
        assert_eq!(
            origin.handle.occurrence().onset,
            vactr::value::Ratio64::from_int(c)
        );
        assert_eq!(
            origin.inherited[0].handle.occurrence().onset,
            vactr::value::Ratio64::ZERO
        );
    }
    // Full8/8 samples the inner quarter at 0 and 1/8, independently of handle order.
    assert_sampled_cycle_groups(&plan, &rows, 0..4, 8, &[0, 1]);
    assert_sampled_partitions(&mut song, &plan, &rows);
}
#[test]
fn sampled_chunk_uses_held_whole_anchor_instead_of_membership_fraction() {
    let mut song = composed_grid_song("chunk p 2 {x -> first [x]}", "euclid p 4 4", false);
    let plan = plan(&song);
    let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 16);
    for row in &rows {
        let c = row.whole.unwrap().begin.floor();
        assert_eq!(resolve(&plan, row).configuration, span(c, c + 1, 1));
        assert_eq!(
            row.source_origin
                .as_ref()
                .unwrap()
                .handle
                .occurrence()
                .onset,
            vactr::value::Ratio64::ZERO
        );
    }
    assert_sampled_cycle_groups(&plan, &rows, 0..4, 4, &[0, 1, 2, 3]);
    assert_sampled_partitions(&mut song, &plan, &rows);
}
#[test]
fn sampled_weighted_slots_preserve_intrinsic_cycle_and_rest_gaps() {
    let mut song = composed_grid_song("[{hold p 3} nil]", "euclid p 4 4", false);
    let plan = plan(&song);
    let rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    assert_eq!(rows.len(), 12);
    for row in &rows {
        let c = row.whole.unwrap().begin.floor();
        assert_eq!(resolve(&plan, row).configuration, span(4 * c, 4 * c + 3, 4));
        assert_eq!(
            row.source_origin
                .as_ref()
                .unwrap()
                .handle
                .occurrence()
                .onset,
            vactr::value::Ratio64::new(c, 4).unwrap()
        );
        assert!(song
            .query(span(8 * c + 6, 8 * c + 7, 8), &SongLimits::default())
            .unwrap()
            .is_empty());
    }
    assert_sampled_partitions(&mut song, &plan, &rows);
}
#[test]
fn composed_grid_sequence_offsets_preserve_global_component_identity() {
    let mut song = composed_grid_song("euclid p 1 4", "euclid p 8 8", true);
    let plan = plan(&song);
    let rows: Vec<_> = song
        .query(span(0, 5, 1), &SongLimits::default())
        .unwrap()
        .into_iter()
        .filter(|row| row.source_origin.is_some())
        .collect();
    assert_eq!(rows.len(), 8);
    for row in &rows {
        let c = row.whole.unwrap().begin.floor();
        assert!(c >= 1);
        assert_eq!(resolve(&plan, row).configuration, span(4 * c, 4 * c + 1, 4));
    }
    assert_sampled_cycle_groups(&plan, &rows, 1..5, 8, &[0, 1]);
    assert_sampled_partitions(&mut song, &plan, &rows);
}

fn cycle_source(inner: &str, outer: &str, shifted: bool) -> vactr::song::PreparedSong {
    let score = if shifted {
        "sequence [{part [bass: {s :fm}] duration: 1} outer]"
    } else {
        "outer"
    };
    frozen_routes(&format!("let base {{part [drums: {{s :analog > slow 64}}] duration: 8}}\nlet inner {{transform-instrument base :drums :analog {{p -> {inner}}}}}\nlet outer {{transform-instrument inner :drums :analog {{p -> {outer}}}}}\nsong {{{score}}} tail-seconds: 0 > play-song"))
}
fn assert_cycle_rows(
    song: &mut vactr::song::PreparedSong,
    window: vactr::pattern::TimeSpan,
    expected: &[(i64, i64, i64)],
    configurations: &[(i64, i64, i64)],
) {
    let plan = plan(song);
    let mut rows: Vec<_> = song
        .query(window, &SongLimits::default())
        .unwrap()
        .into_iter()
        .filter(|row| row.source_origin.is_some())
        .collect();
    rows.sort_by_key(|row| row.whole.unwrap().begin);
    assert_eq!(rows.len(), expected.len());
    assert_eq!(configurations.len(), expected.len());
    for ((row, &(begin, end, denominator)), &(lo, hi, scale)) in
        rows.iter().zip(expected).zip(configurations)
    {
        assert_eq!(row.whole.unwrap(), span(begin, end, denominator));
        assert_eq!(resolve(&plan, row).configuration, span(lo, hi, scale));
        assert!(!resolve(&plan, row).sources.is_empty());
    }
    assert_sampled_partitions(song, &plan, &rows);
}
#[test]
fn sampled_iterate_preserves_intrinsic_policy_and_grid_cell_partitions() {
    // Iter shifts by c mod4 /4. The next integer grid onset is pulled back
    // into c's query cycle; no source-duration boundary occurs in this window.
    let mut song = cycle_source("euclid p 1 4", "iter p 4", false);
    let cells = [(0, 1, 4), (7, 8, 4), (10, 11, 4), (13, 14, 4)];
    assert_eq!(
        song.query(span(0, 4, 1), &SongLimits::default())
            .unwrap()
            .len(),
        4,
        "held grid -> Iterate: exact shifted grid cells"
    );
    assert_cycle_rows(&mut song, span(0, 4, 1), &cells, &cells);
    // An inner finite Part filters whole onsets before the outer sampler:
    // the held [0,64) note shifts to a negative onset at residues1/2/3.
    let mut held_inner = frozen_routes("let base {part [drums: {s :analog > slow 64}] duration: 8}\nlet inner {transform-instrument base :drums :analog {p -> iter p 4}}\nsong inner tail-seconds: 0 > play-song");
    let first = held_inner
        .query(span(0, 1, 1), &SongLimits::default())
        .unwrap();
    assert_eq!(first.len(), 1, "held inner Iterate residue0");
    assert_eq!(first[0].whole.unwrap(), span(0, 64, 1));
    for cycle in 1..4 {
        assert!(
            held_inner
                .query(span(cycle, cycle + 1, 1), &SongLimits::default())
                .unwrap()
                .is_empty(),
            "held inner Iterate residue{cycle}: negative whole onset excluded"
        );
    }
    let mut held_reverse = cycle_source("iter p 4", "euclid p 1 4", false);
    assert_cycle_rows(&mut held_reverse, span(0, 4, 1), &[(0, 1, 4)], &[(0, 1, 4)]);
    for cycle in 1..4 {
        assert!(held_reverse
            .query(
                span(8 * cycle + 1, 8 * cycle + 1, 8),
                &SongLimits::default()
            )
            .unwrap()
            .is_empty());
    }
    // Natural one-cycle notes have nonnegative retimed onsets in every
    // queried cycle, genuinely exercising all four reverse-order grid cells.
    let mut reverse = frozen_routes("let base {part [drums: {s :analog}] duration: 8}\nlet inner {transform-instrument base :drums :analog {p -> iter p 4}}\nlet outer {transform-instrument inner :drums :analog {p -> euclid p 1 4}}\nsong outer tail-seconds: 0 > play-song");
    let cells = [(0, 1, 4), (4, 5, 4), (8, 9, 4), (12, 13, 4)];
    assert_eq!(
        reverse
            .query(span(0, 4, 1), &SongLimits::default())
            .unwrap()
            .len(),
        4,
        "one-cycle Iterate -> grid: four valid source quarters"
    );
    assert_cycle_rows(&mut reverse, span(0, 4, 1), &cells, &cells);
}
#[test]
fn sampled_cat_and_fastcat_preserve_rest_gaps_and_arity_one_continuity() {
    for (outer, cells) in [
        ("cat [p {s nil}]", vec![(0, 1, 4), (8, 9, 4)]),
        (
            "fastcat [p {s nil}]",
            vec![(0, 1, 8), (8, 9, 8), (16, 17, 8), (24, 25, 8)],
        ),
        (
            "cat [p]",
            vec![(0, 1, 4), (4, 5, 4), (8, 9, 4), (12, 13, 4)],
        ),
        (
            "fastcat [p]",
            vec![(0, 1, 4), (4, 5, 4), (8, 9, 4), (12, 13, 4)],
        ),
    ] {
        let mut song = cycle_source("euclid p 1 4", outer, false);
        assert_cycle_rows(&mut song, span(0, 4, 1), &cells, &cells);
    }
    for inner in ["cat [p {s nil}]", "fastcat [p {s nil}]"] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog}}] duration: 8}}\nlet inner {{transform-instrument base :drums :analog {{p -> {inner}}}}}\nlet outer {{transform-instrument inner :drums :analog {{p -> euclid p 4 4}}}}\nsong outer tail-seconds: 0 > play-song"));
        let (cells, runs) = if inner.starts_with("cat") {
            (
                vec![
                    (0, 1, 4),
                    (1, 2, 4),
                    (2, 3, 4),
                    (3, 4, 4),
                    (8, 9, 4),
                    (9, 10, 4),
                    (10, 11, 4),
                    (11, 12, 4),
                ],
                vec![(0, 1, 1); 4]
                    .into_iter()
                    .chain(vec![(2, 3, 1); 4])
                    .collect::<Vec<_>>(),
            )
        } else {
            let cells = (0..4)
                .flat_map(|c| [(4 * c, 4 * c + 1, 4), (4 * c + 1, 4 * c + 2, 4)])
                .collect::<Vec<_>>();
            let runs = (0..4)
                .flat_map(|c| [(2 * c, 2 * c + 1, 2); 2])
                .collect::<Vec<_>>();
            (cells, runs)
        };
        assert_cycle_rows(&mut song, span(0, 4, 1), &cells, &runs);
    }
}
#[test]
fn inherited_cycle_maps_preserve_offset_and_complete_partition_identity() {
    for (outer, cells) in [
        (
            "iter p 4",
            vec![(4, 5, 4), (11, 12, 4), (14, 15, 4), (17, 18, 4)],
        ),
        ("cat [p {s nil}]", vec![(4, 5, 4), (12, 13, 4)]),
        (
            "fastcat [p {s nil}]",
            vec![(8, 9, 8), (16, 17, 8), (24, 25, 8), (32, 33, 8)],
        ),
    ] {
        let mut song = cycle_source("euclid p 1 4", outer, true);
        assert_cycle_rows(&mut song, span(1, 5, 1), &cells, &cells);
    }
}
#[test]
fn ordinary_cat_births_preserve_intrinsic_rest_gap_components() {
    for (expression, denominator) in [("cat [p {s nil}]", 1), ("fastcat [p {s nil}]", 2)] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog {{p -> {expression}}}}}\nsong selected tail-seconds: 0 > play-song"));
        let cells = if denominator == 1 {
            vec![(0, 1, 1), (2, 3, 1)]
        } else {
            vec![(0, 1, 2), (2, 3, 2), (4, 5, 2), (6, 7, 2)]
        };
        assert_cycle_rows(&mut song, span(0, 4, 1), &cells, &cells);
        for c in 0..if denominator == 1 { 2 } else { 4 } {
            let rest = span(4 * c + 3, 4 * c + 3, if denominator == 1 { 2 } else { 4 });
            assert!(song.query(rest, &SongLimits::default()).unwrap().is_empty());
        }
    }
    for expression in ["cat [p]", "fastcat [p]"] {
        let mut song = frozen_routes(&format!("let base {{part [drums: {{s :analog}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog {{p -> {expression}}}}}\nsong selected tail-seconds: 0 > play-song"));
        let cells = [(0, 1, 1), (1, 2, 1), (2, 3, 1), (3, 4, 1)];
        assert_cycle_rows(&mut song, span(0, 4, 1), &cells, &[(0, 4, 1); 4]);
    }
}
#[test]
fn cycle_map_signed_arithmetic_and_cumulative_quota_are_checked() {
    use vactr::value::Ratio64;
    // Signed helper equations, not a claim that the root accepts negative onset.
    for cycle in [-5_i64, -1, 0, 1, 5] {
        let child = cycle.div_euclid(2);
        let ordinal = cycle.rem_euclid(2);
        assert_eq!(2 * child + ordinal, cycle);
        let phase = Ratio64::new(cycle.rem_euclid(4), 4).unwrap();
        let member = Ratio64::from_int(cycle).checked_add(phase).unwrap();
        assert_eq!(member.checked_sub(phase).unwrap(), Ratio64::from_int(cycle));
    }
    let mut song = cycle_source("euclid p 1 4", "cat [p {s nil}]", true);
    let plan = plan(&song);
    let rows = song.query(span(1, 5, 1), &SongLimits::default()).unwrap();
    let event = rows.iter().find(|row| row.source_origin.is_some()).unwrap();
    let expected = resolve(&plan, event);
    // A quota applies cumulatively across both authentic origin stages and
    // matcher, sampling mask, inverse clock and maximal-component work.
    let mut sufficient = None;
    for nodes in 1..=4096 {
        let limits = SongLimits {
            max_nodes: nodes,
            ..Default::default()
        };
        match resolve_route(&plan, event, limits) {
            Ok(route) => {
                assert_eq!(route, expected);
                sufficient = Some(nodes);
                break;
            }
            Err(failure) => assert_eq!(failure.code, vactr::vm::fail::FailCode::FuelExhausted),
        }
    }
    let sufficient = sufficient.expect("bounded two-stage route admission");
    assert!(sufficient > 1);
    assert_eq!(
        resolve_route(
            &plan,
            event,
            SongLimits {
                max_nodes: sufficient - 1,
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        vactr::vm::fail::FailCode::FuelExhausted
    );
    assert_sampled_partitions(
        &mut song,
        &plan,
        &rows
            .into_iter()
            .filter(|row| row.source_origin.is_some())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn ordinary_rev_preserves_full_policy_and_fractional_rest_components() {
    let mut song = frozen_routes("let base {part [drums: {fast {s :analog} 4}] duration: 4}\nlet selected {transform-instrument base :drums :analog {p -> rev p}}\nsong selected tail-seconds: 0 > play-song");
    let cells: Vec<_> = (0..16).map(|i| (i, i + 1, 4)).collect();
    assert_cycle_rows(&mut song, span(0, 4, 1), &cells, &[(0, 4, 1); 16]);
    let mut partial = frozen_routes("let base {sequence [{part [bass: {s :fm}] duration: 1/4} {part [drums: {fast {s :analog} 4}] duration: 1/4} {part [bass: {s :fm}] duration: 1/2}]}\nlet selected {transform-instrument base :drums :analog {p -> rev p}}\nsong selected tail-seconds: 0 > play-song");
    assert_cycle_rows(&mut partial, span(0, 1, 1), &[(2, 3, 4)], &[(2, 3, 4)]);
    for point in [span(1, 1, 4), span(3, 3, 4)] {
        assert!(partial
            .query(point, &SongLimits::default())
            .unwrap()
            .iter()
            .all(|row| row.source_origin.is_none()));
    }
}
#[test]
fn inherited_sampled_rev_boundary_preserves_authentic_whole_and_membership() {
    let mut song = frozen_routes("let base {part [drums: {fast {s :analog} 4}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> rev p}}\nlet grid {transform-instrument inner :drums :analog {p -> euclid p 4 4}}\nsong grid tail-seconds: 0 > play-song");
    let plan = plan(&song);
    let mut rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    rows.sort_by_key(|row| row.whole.unwrap().begin);
    assert_eq!(
        rows.len(),
        16,
        "actual outer grid -> inherited Rev boundaries"
    );
    for (i, row) in rows.iter().enumerate() {
        let cycle = i64::try_from(i / 4).unwrap();
        let phase = i64::try_from(i % 4).unwrap();
        assert_eq!(
            row.whole.unwrap(),
            span(4 * cycle + phase, 4 * cycle + phase + 1, 4)
        );
        let origin = row.source_origin.as_ref().unwrap();
        assert_eq!(origin.inherited.len(), 1);
        assert_eq!(origin.handle.occurrence().onset, row.whole.unwrap().begin);
        assert_eq!(
            origin.inherited[0].handle.occurrence().onset,
            span(4 * cycle + 3 - phase, 4 * cycle + 3 - phase, 4).begin
        );
        // Positive expected behavior remains explicit; an unresolved sampled
        // context must fail visibly with real row provenance, not become an exemption.
        let expected = resolve(&plan, row);
        assert_eq!(expected.configuration, span(0, 4, 1));
        let boundary = vactr::pattern::TimeSpan::point(row.whole.unwrap().begin);
        let point = song.query(boundary, &SongLimits::default()).unwrap();
        assert_eq!(point.len(), 1);
        assert_eq!(point[0].handle, row.handle);
        assert_eq!(resolve(&plan, &point[0]), expected);
    }
    assert_sampled_partitions(&mut song, &plan, &rows);
}

#[test]
fn nonaligned_sampled_rev_uses_authentic_pre_grid_whole() {
    let mut song = frozen_routes("let base {part [drums: {fast {s :analog} 3}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> rev p}}\nlet grid {transform-instrument inner :drums :analog {p -> euclid p 4 4}}\nsong grid tail-seconds: 0 > play-song");
    let plan = plan(&song);
    let mut rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    rows.sort_by_key(|row| row.whole.unwrap().begin);
    assert_eq!(rows.len(), 16);
    let first = rows[0].source_origin.as_ref().unwrap();
    assert_eq!(first.source_whole(), Some(span(0, 1, 3)));
    assert_eq!(first.inherited[0].source_whole(), Some(span(2, 3, 3)));
    assert_ne!(first.source_whole(), rows[0].whole);
    for (i, row) in rows.iter().enumerate() {
        let i = i64::try_from(i).unwrap();
        assert_eq!(row.whole, Some(span(i, i + 1, 4)));
        assert_eq!(resolve(&plan, row).configuration, span(0, 4, 1));
        let point = song
            .query(
                vactr::pattern::TimeSpan::point(row.whole.unwrap().begin),
                &SongLimits::default(),
            )
            .unwrap();
        assert_eq!(point.len(), 1);
        assert_eq!(point[0].handle, row.handle);
        assert_eq!(resolve(&plan, &point[0]), resolve(&plan, row));
    }
    assert_sampled_partitions(&mut song, &plan, &rows);
}
#[test]
fn sampled_rev_long_release_preserves_halfopen_membership_and_ownership() {
    let mut song = frozen_routes("let base {part [drums: {slow {s :analog} 2}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> rev p}}\nlet grid {transform-instrument inner :drums :analog {p -> euclid p 4 4}}\nsong grid tail-seconds: 0 > play-song");
    let plan = plan(&song);
    let mut rows = song.query(span(0, 4, 1), &SongLimits::default()).unwrap();
    rows.sort_by_key(|row| row.whole.unwrap().begin);
    assert_eq!(
        rows.len(),
        12,
        "negative-onset reflected cycle zero is excluded"
    );
    assert!(song
        .query(span(0, 1, 1), &SongLimits::default())
        .unwrap()
        .is_empty());
    for (i, row) in rows.iter().enumerate() {
        let phase = i64::try_from(i).unwrap() + 4;
        assert_eq!(row.whole, Some(span(phase, phase + 1, 4)));
        let origin = row.source_origin.as_ref().unwrap();
        assert_eq!(origin.inherited.len(), 1);
        let original = origin.inherited[0].source_whole().unwrap();
        assert_eq!(
            original.end.checked_sub(original.begin).unwrap(),
            vactr::value::Ratio64::from_int(2)
        );
        assert_eq!(resolve(&plan, row).configuration, span(0, 4, 1));
        let point = song
            .query(
                vactr::pattern::TimeSpan::point(row.whole.unwrap().begin),
                &SongLimits::default(),
            )
            .unwrap();
        assert_eq!(point[0].handle, row.handle);
        assert_eq!(resolve(&plan, &point[0]), resolve(&plan, row));
    }
    assert_sampled_partitions(&mut song, &plan, &rows);
}
#[test]
fn sampled_rev_sequence_offsets_affine_and_weighted_slots_preserve_full_routes() {
    let base = "let base {part [drums: {fast {s :analog} 3}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> rev p}}\n";
    for (callback, cells, configurations) in [
        (
            "euclid p 4 4",
            (0..16).map(|i| (i + 4, i + 5, 4)).collect::<Vec<_>>(),
            vec![(1, 5, 1); 16],
        ),
        (
            "fast {euclid p 4 4} 2",
            (0..16).map(|i| (i + 8, i + 9, 8)).collect(),
            vec![(1, 3, 1); 16],
        ),
        (
            "[{euclid p 4 4} nil]",
            (0..4)
                .flat_map(|c| (0..4).map(move |j| (8 * c + j + 8, 8 * c + j + 9, 8)))
                .collect(),
            (0..4)
                .flat_map(|c| std::iter::repeat_n((2 * c + 2, 2 * c + 3, 2), 4))
                .collect(),
        ),
    ] {
        let code = format!("{base}let grid {{transform-instrument inner :drums :analog {{p -> {callback}}}}}\nlet full {{sequence [{{part [bass: {{s :fm}}] duration: 1}} grid]}}\nsong full tail-seconds: 0 > play-song");
        let mut song = frozen_routes(&code);
        assert_cycle_rows(&mut song, span(1, 5, 1), &cells, &configurations);
    }
    let mut partial = frozen_routes("let base {sequence [{part [bass: {s :fm}] duration: 1/4} {part [drums: {fast {s :analog} 4}] duration: 1/4} {part [bass: {s :fm}] duration: 1/2}]}\nlet inner {transform-instrument base :drums :analog {p -> rev p}}\nlet grid {transform-instrument inner :drums :analog {p -> euclid p 4 4}}\nlet full {sequence [{part [bass: {s :fm}] duration: 1} grid]}\nsong full tail-seconds: 0 > play-song");
    assert_cycle_rows(&mut partial, span(1, 2, 1), &[(6, 7, 4)], &[(6, 7, 4)]);
}

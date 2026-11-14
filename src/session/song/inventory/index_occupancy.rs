//! Genuine internal Index issuance; public transform sources remain structured.
use super::*;
use crate::dsp::caps::CapabilitySet;
use crate::pattern::pat::{PParam, SliceCuts};
use crate::pattern::TimeSpan;
use crate::song::assets::{DecodedSongAssetFactory, SongAssetFactory, SongSourceFile};
use crate::song::routing::{prepare_routes, resolve_route, SongHostCapacities};
use crate::song::snapshot::SongCandidate;
use crate::song::{
    capture_part, InstrumentSelector, PreparedSong, SnapshotEpoch, SongSettings, SongSource,
};
use crate::value::intern::intern_kw;
use crate::value::value::PathVal;
use crate::value::Ratio64;

fn span(begin: i64, end: i64, denominator: i64) -> TimeSpan {
    TimeSpan::new(
        Ratio64::new(begin, denominator).unwrap(),
        Ratio64::new(end, denominator).unwrap(),
    )
    .unwrap()
}
fn capacities() -> SongHostCapacities {
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
fn prepared_index(index: Rc<Pat>, rate: Ratio64) -> PreparedSong {
    prepared_at(index, rate, Ratio64::ZERO)
}
fn prepared_at(index: Rc<Pat>, rate: Ratio64, offset: Ratio64) -> PreparedSong {
    prepared_context(index, rate, offset, None, Ratio64::from_int(4))
}
fn prepared_context(
    index: Rc<Pat>,
    rate: Ratio64,
    offset: Ratio64,
    shift: Option<Ratio64>,
    duration: Ratio64,
) -> PreparedSong {
    let limits = super::source_inventory_test_support::limits();
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let mut assets = factory
        .begin(
            SongSourceFile {
                file: crate::reader::span::FileId::new(0),
                path: PathVal {
                    text: Rc::from("index-runtime.vact"),
                    file: None,
                },
            },
            limits,
        )
        .unwrap()
        .close()
        .unwrap();
    let mut evaluator = super::source_inventory_test_support::evaluator();
    let track = intern_kw("drums");
    let sound = Sound::Builtin(intern_kw("analog"));
    let base = Rc::new(
        capture_part(
            BTreeMap::from([(
                track,
                Rc::new(crate::pattern::build::pure(
                    Value::Sound(Rc::new(sound.clone())),
                    None,
                )),
            )]),
            Ratio64::from_int(4),
        )
        .unwrap(),
    );
    let source =
        SongSource::new(base, track, InstrumentSelector::new(vec![sound]).unwrap()).unwrap();
    let selected = Rc::new(Pat::new(PatNode::SongSource(Rc::new(source)), None, false));
    let sliced = Rc::new(crate::pattern::combinators::region::slice(
        selected,
        SliceCuts::Equal(PParam::Const(Value::Int(2))),
        index,
        None,
    ));
    let sliced = if rate == Ratio64::ONE {
        sliced
    } else {
        Rc::new(crate::pattern::combinators::time::fast(
            sliced,
            PParam::Const(Value::Ratio(rate)),
            None,
        ))
    };
    let sliced = if let Some(amount) = shift {
        let forms = evaluator
            .eval_str(
                "fn unchanged p:\n\tfirst [p]",
                crate::reader::span::FileId::new(0),
            )
            .unwrap();
        assert!(forms.iter().all(|form| form.value.is_ok()));
        let callback = evaluator
            .ns()
            .lookup(crate::value::intern::intern_sym("unchanged"))
            .unwrap()
            .slot()
            .get();
        Rc::new(crate::pattern::combinators::structure::off(
            sliced,
            PParam::Const(Value::Ratio(amount)),
            callback,
            None,
        ))
    } else {
        sliced
    };
    let part = Rc::new(capture_part(BTreeMap::from([(track, sliced)]), duration).unwrap());
    let part = if offset == Ratio64::ZERO {
        part
    } else {
        Rc::new(
            crate::song::part::sequence(vec![
                Rc::new(capture_part(BTreeMap::new(), offset).unwrap()),
                part,
            ])
            .unwrap(),
        )
    };
    let song = Rc::new(
        Song::new(
            part,
            SongSettings {
                tail_seconds: Ratio64::ZERO,
                ..SongSettings::default()
            },
        )
        .unwrap(),
    );
    let mut remaining = limits.max_walk_nodes;
    let pending = shift.map(|_| {
        super::super::shape_preparation::discover_shapes(
            &mut evaluator,
            &song,
            limits,
            &mut remaining,
        )
        .unwrap()
    });
    let mut reduced = limits;
    reduced.max_walk_nodes = remaining
        .checked_sub(
            pending
                .as_ref()
                .map_or(0, |p| u32::try_from(p.record_count()).unwrap()),
        )
        .unwrap();
    let mut freeze = crate::session::song::freeze::Freeze::new(&assets, reduced);
    if shift.is_some() {
        for slot in evaluator.candidate_slots() {
            freeze.slot(&slot).unwrap();
        }
    }
    let Value::Song(song) = freeze.value(&Value::Song(song)).unwrap() else {
        panic!("actual immutable copied Song");
    };
    let pending = pending.map(|p| p.freeze_records(&mut freeze, &mut remaining).unwrap());
    let work = freeze.consumed_work();
    assert!(work > 0);
    drop(freeze);
    remaining = remaining.checked_sub(work).unwrap();
    let shapes = pending
        .map(|p| {
            p.admit_closed(&mut evaluator, &mut assets, limits, &mut remaining)
                .unwrap()
        })
        .unwrap_or_default();
    if shift.is_some() {
        assert_eq!(
            shapes.patterns.len(),
            1,
            "actual retained identity callback"
        );
        assert!(shapes.rejected.is_empty());
    }
    let mut capture_limits = limits;
    capture_limits.max_walk_nodes = remaining;
    let inventory = capture_routing_prepared(&evaluator, &song, capture_limits, &shapes).unwrap();
    let mut candidate = SongCandidate::from_isolated_evaluation(
        evaluator,
        song,
        assets,
        Vec::new(),
        "index-runtime.vact".into(),
        1,
        0,
        SnapshotEpoch(1),
    )
    .unwrap();
    candidate.set_routing(inventory);
    crate::song::prepare_song(candidate).unwrap()
}

fn slow_index(value: i32, slow: Ratio64) -> Rc<Pat> {
    Rc::new(crate::pattern::combinators::time::slow(
        Rc::new(crate::pattern::build::pure(Value::Int(value), None)),
        PParam::Const(Value::Ratio(slow)),
        None,
    ))
}
fn prepared() -> PreparedSong {
    prepared_index(
        Rc::new(crate::pattern::build::value_steps(&[
            Value::Pattern(slow_index(0, Ratio64::from_int(2))),
            Value::Nil,
        ])),
        Ratio64::ONE,
    )
}

#[test]
fn weighted_slow_index_wholes_preserve_connected_configuration_across_part_gap() {
    let mut song = prepared();
    let limits = crate::song::SongLimits::default();
    let rows = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        let origin = row.source_origin.as_ref().expect("actual selected source");
        assert_eq!(origin.slice_timings().len(), 1, "{row:#?}");
        let timing = &origin.slice_timings()[0];
        assert_eq!(Some(timing.index_whole()), row.whole);
        assert_eq!(timing.sample_start(), row.whole.unwrap().begin);
        assert_eq!(timing.subject_handle(), &origin.handle);
    }
    assert_eq!(rows[0].whole, Some(span(0, 1, 1)));
    assert_eq!(rows[1].whole, Some(span(1, 3, 2)));
    assert_eq!(rows[0].part, span(0, 1, 2));
    assert_eq!(rows[1].part, span(2, 3, 2));
    assert!(song.query(span(5, 7, 8), &limits).unwrap().is_empty());
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let resolved = resolve_route(&plan, &rows[0], limits).unwrap();
    assert_eq!(resolved.configuration, span(0, 3, 2));
    for row in rows {
        assert_eq!(resolve_route(&plan, &row, limits).unwrap(), resolved);
    }
}

#[test]
fn reordered_queries_reuse_actual_index_configuration_identity() {
    let mut song = prepared();
    let limits = crate::song::SongLimits::default();
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let full = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(full.len(), 2);
    let resolved = resolve_route(&plan, &full[0], limits).unwrap();
    assert_eq!(resolved.configuration, span(0, 3, 2));
    assert_eq!(resolve_route(&plan, &full[1], limits).unwrap(), resolved);
    for query in [span(9, 11, 8), span(1, 3, 8), span(8, 12, 8)] {
        let partial = song.query(query, &limits).unwrap();
        assert_eq!(partial.len(), 1, "query {query:?}");
        for row in partial {
            let origin = row.source_origin.as_ref().unwrap();
            assert_eq!(origin.slice_timings().len(), 1);
            assert!(full
                .iter()
                .any(|whole| whole.handle == row.handle && whole.whole == row.whole));
            assert_eq!(row.part, query);
            assert_eq!(resolve_route(&plan, &row, limits).unwrap(), resolved);
        }
    }
}

fn family_rows(index: Rc<Pat>, rate: Ratio64, expected: &[TimeSpan]) {
    let mut song = prepared_index(index, rate);
    let limits = crate::song::SongLimits::default();
    let window = TimeSpan::new(Ratio64::ZERO, expected.last().unwrap().end).unwrap();
    let rows = song.query(window, &limits).unwrap();
    assert_eq!(rows.len(), expected.len() * 4, "{rows:#?}");
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let mut routes = Vec::new();
    for component in expected {
        let members: Vec<_> = rows
            .iter()
            .filter(|r| {
                component.begin <= r.whole.unwrap().begin && r.whole.unwrap().begin < component.end
            })
            .collect();
        assert_eq!(members.len(), 4, "{component:?}: {rows:#?}");
        let resolved = resolve_route(&plan, members[0], limits).unwrap();
        assert_eq!(resolved.configuration, *component);
        let branch = plan
            .branches
            .iter()
            .find(|branch| branch.id == resolved.branch)
            .unwrap();
        assert!(branch.configurations_per_placement >= expected.len() as u64);
        // This genuine fixture has one finite placement: occurrence pricing
        // applies its configuration bound exactly once, not once per note.
        assert_eq!(branch.occurrences, branch.configurations_per_placement);
        assert!(branch.reserved_generations >= 1);
        let cover = &plan.source_covers[branch.source_cover.unwrap() as usize];
        assert!(cover.live_generation_bound >= 1);
        for row in members {
            let origin = row.source_origin.as_ref().unwrap();
            assert_eq!(origin.slice_timings().len(), 1);
            let timing = &origin.slice_timings()[0];
            assert_eq!(timing.subject_handle(), &origin.handle);
            assert_eq!(timing.sample_start(), timing.index_whole().begin);
            assert_eq!(
                timing.index_whole().map(|t| t.checked_div(rate)).unwrap(),
                row.whole.unwrap()
            );
            assert_eq!(resolve_route(&plan, row, limits).unwrap(), resolved);
            // A strictly interior query must emit this real row, not pass vacuously.
            let midpoint = row
                .part
                .begin
                .checked_add(row.part.end)
                .unwrap()
                .checked_div(Ratio64::from_int(2))
                .unwrap();
            let query = TimeSpan::new(midpoint, row.part.end).unwrap();
            let partial = song.query(query, &limits).unwrap();
            assert_eq!(partial.len(), 1, "{query:?}: {partial:#?}");
            assert_eq!(partial[0].handle, row.handle);
            assert_eq!(partial[0].whole, row.whole);
            assert_eq!(resolve_route(&plan, &partial[0], limits).unwrap(), resolved);
        }
        routes.push(resolved);
    }
    for pair in expected.windows(2) {
        assert!(pair[0].end < pair[1].begin);
        assert!(song
            .query(TimeSpan::new(pair[0].end, pair[1].begin).unwrap(), &limits)
            .unwrap()
            .is_empty());
    }
    if routes.len() > 1 {
        assert_ne!(routes[0], routes[1]);
    }
}
fn multiple_index() -> Rc<Pat> {
    Rc::new(crate::pattern::build::value_steps(&[
        Value::Pattern(slow_index(0, Ratio64::from_int(2))),
        Value::Nil,
        Value::Pattern(slow_index(1, Ratio64::from_int(2))),
        Value::Nil,
    ]))
}
#[test]
fn multiple_leaves_form_connected_whole_families_without_bridging_period_gap() {
    family_rows(
        multiple_index(),
        Ratio64::ONE,
        &[span(0, 7, 4), span(8, 15, 4)],
    );
}
#[test]
fn symbolic_repeat_copies_share_the_complete_configuration_not_note_identity() {
    let repeated = crate::pattern::combinators::structure::repeat(
        slow_index(0, Ratio64::from_int(2)),
        PParam::Const(Value::Int(2)),
        None,
    );
    let index = Rc::new(crate::pattern::build::value_steps(&[
        Value::Pattern(Rc::new(repeated)),
        Value::Nil,
    ]));
    family_rows(index, Ratio64::ONE, &[span(0, 5, 3), span(6, 11, 3)]);
}
#[test]
fn positive_outer_fast_keeps_local_issued_timing_and_projects_components() {
    family_rows(
        multiple_index(),
        Ratio64::from_int(2),
        &[span(0, 7, 8), span(8, 15, 8)],
    );
}
#[test]
fn positive_outer_slow_keeps_selected_source_clock_separate() {
    family_rows(
        multiple_index(),
        Ratio64::new(1, 2).unwrap(),
        &[span(0, 7, 2)],
    );
}

#[test]
fn fractional_slow_copies_connect_full_parent_footprints_but_keep_seams() {
    let repeated = crate::pattern::combinators::structure::repeat(
        slow_index(0, Ratio64::new(1, 2).unwrap()),
        PParam::Const(Value::Int(2)),
        None,
    );
    family_rows(
        Rc::new(crate::pattern::build::value_steps(&[
            Value::Pattern(Rc::new(repeated)),
            Value::Nil,
        ])),
        Ratio64::ONE,
        &[span(0, 2, 3), span(3, 5, 3)],
    );
}
#[test]
fn actual_sequence_placement_projects_family_without_rewriting_issued_clock() {
    let mut song = prepared_at(multiple_index(), Ratio64::ONE, Ratio64::ONE);
    let limits = crate::song::SongLimits::default();
    let rows = song.query(span(1, 5, 1), &limits).unwrap();
    assert_eq!(rows.len(), 8);
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    for component in [span(4, 11, 4), span(12, 19, 4)] {
        let members: Vec<_> = rows
            .iter()
            .filter(|row| {
                component.begin <= row.whole.unwrap().begin
                    && row.whole.unwrap().begin < component.end
            })
            .collect();
        assert_eq!(members.len(), 4);
        let route = resolve_route(&plan, members[0], limits).unwrap();
        assert_eq!(route.configuration, component);
        for row in members {
            let origin = row.source_origin.as_ref().unwrap();
            assert_eq!(origin.slice_timings().len(), 1);
            assert_eq!(
                origin.slice_timings()[0]
                    .index_whole()
                    .map(|t| t.checked_add(Ratio64::ONE))
                    .unwrap(),
                row.whole.unwrap()
            );
            assert_eq!(resolve_route(&plan, row, limits).unwrap(), route);
        }
    }
}

#[test]
fn zero_repeat_body_emits_nothing_and_preserves_shifted_family_authority() {
    let silent = crate::pattern::combinators::structure::repeat(
        slow_index(1, Ratio64::from_int(2)),
        PParam::Const(Value::Int(0)),
        None,
    );
    let index = Rc::new(crate::pattern::build::value_steps(&[
        Value::Pattern(Rc::new(silent)),
        Value::Pattern(slow_index(0, Ratio64::from_int(2))),
        Value::Nil,
        Value::Pattern(slow_index(1, Ratio64::from_int(2))),
        Value::Nil,
    ]));
    // The genuine zero-copy producer occupies ordinal0 but creates no row.
    // The shared strict helper requires exactly four occupied rows per period,
    // genuine issued timing and equal complete routes across all actual rows.
    family_rows(index, Ratio64::ONE, &[span(0, 7, 4), span(8, 15, 4)]);
}

#[test]
fn actual_off_shift_retains_local_wholes_and_distinct_complete_use_identity() {
    let shift = Ratio64::new(1, 4).unwrap();
    let mut song = prepared_context(
        multiple_index(),
        Ratio64::ONE,
        Ratio64::ZERO,
        Some(shift),
        Ratio64::from_int(4),
    );
    let limits = crate::song::SongLimits::default();
    let rows = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 8, "{rows:#?}");
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let mut groups = [Vec::new(), Vec::new()];
    for row in &rows {
        let origin = row.source_origin.as_ref().unwrap();
        assert_eq!(origin.slice_timings().len(), 1);
        let timing = &origin.slice_timings()[0];
        assert_eq!(timing.subject_handle(), &origin.handle);
        assert_eq!(timing.sample_start(), timing.index_whole().begin);
        let translation = row
            .whole
            .unwrap()
            .begin
            .checked_sub(timing.index_whole().begin)
            .unwrap();
        let group = if translation == Ratio64::ZERO {
            0
        } else {
            assert_eq!(translation, shift);
            1
        };
        assert_eq!(
            timing
                .index_whole()
                .map(|t| t.checked_add(translation))
                .unwrap(),
            row.whole.unwrap()
        );
        groups[group].push(row);
    }
    assert_eq!(groups[0].len(), 4);
    assert_eq!(groups[1].len(), 4);
    let direct = resolve_route(&plan, groups[0][0], limits).unwrap();
    let shifted = resolve_route(&plan, groups[1][0], limits).unwrap();
    assert_eq!(direct.configuration, span(0, 7, 4));
    assert_eq!(shifted.configuration, span(1, 8, 4));
    assert_ne!(direct.sources[0].identity, shifted.sources[0].identity);
    for (group, expected) in [(&groups[0], direct), (&groups[1], shifted)] {
        for row in group {
            assert_eq!(resolve_route(&plan, row, limits).unwrap(), expected);
            let midpoint = row
                .part
                .begin
                .checked_add(row.part.end)
                .unwrap()
                .checked_div(Ratio64::from_int(2))
                .unwrap();
            let partial = song
                .query(TimeSpan::new(midpoint, row.part.end).unwrap(), &limits)
                .unwrap();
            let matching: Vec<_> = partial
                .iter()
                .filter(|part| part.handle == row.handle)
                .collect();
            assert_eq!(matching.len(), 1, "actual partial issuer row");
            assert_eq!(matching[0].whole, row.whole);
            assert_eq!(resolve_route(&plan, matching[0], limits).unwrap(), expected);
        }
    }
}
#[test]
fn fractional_canonical_capture_filters_onsets_and_preserves_uncut_index_whole() {
    let duration = Ratio64::new(5, 4).unwrap();
    let limits = crate::song::SongLimits::default();
    let mut song = prepared_context(
        multiple_index(),
        Ratio64::ONE,
        Ratio64::ZERO,
        None,
        duration,
    );
    let rows = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 3, "{rows:#?}");
    let mut starts: Vec<_> = rows.iter().map(|r| r.whole.unwrap().begin).collect();
    starts.sort();
    assert_eq!(
        starts,
        vec![
            Ratio64::ZERO,
            Ratio64::new(1, 2).unwrap(),
            Ratio64::new(3, 4).unwrap()
        ]
    );
    assert!(rows.iter().all(|r| r.whole.unwrap().begin < duration));
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let route = resolve_route(&plan, &rows[0], limits).unwrap();
    assert_eq!(route.configuration, span(0, 5, 4));
    for row in &rows {
        assert_eq!(resolve_route(&plan, row, limits).unwrap(), route);
    }
    let index = Rc::new(crate::pattern::build::value_steps(&[
        Value::Pattern(slow_index(0, Ratio64::from_int(2))),
        Value::Nil,
    ]));
    let mut song = prepared_context(index, Ratio64::ONE, Ratio64::ZERO, None, duration);
    let rows = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].whole, Some(span(1, 3, 2)));
    assert_eq!(rows[1].part, span(4, 5, 4));
    assert_eq!(
        rows[1].source_origin.as_ref().unwrap().slice_timings()[0].index_whole(),
        span(1, 3, 2)
    );
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let route = resolve_route(&plan, &rows[0], limits).unwrap();
    assert_eq!(route.configuration, span(0, 5, 4));
    assert_eq!(resolve_route(&plan, &rows[1], limits).unwrap(), route);
}
#[test]
fn genuine_route_resolution_has_smallest_exact_shared_quota_and_depth() {
    let mut song = prepared_index(multiple_index(), Ratio64::from_int(2));
    let limits = crate::song::SongLimits::default();
    let rows = song.query(span(0, 1, 1), &limits).unwrap();
    assert_eq!(rows.len(), 4);
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let expected = resolve_route(&plan, &rows[0], limits).unwrap();
    for depth in [false, true] {
        let mut lo = 1;
        let mut hi = if depth {
            limits.max_depth
        } else {
            limits.max_nodes
        };
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let candidate = if depth {
                crate::song::SongLimits {
                    max_depth: mid,
                    ..limits
                }
            } else {
                crate::song::SongLimits {
                    max_nodes: mid,
                    ..limits
                }
            };
            match resolve_route(&plan, &rows[0], candidate) {
                Ok(route) => {
                    assert_eq!(route, expected);
                    hi = mid;
                }
                Err(failure) => {
                    assert_eq!(
                        failure.code,
                        if depth {
                            crate::vm::fail::FailCode::DepthExceeded
                        } else {
                            crate::vm::fail::FailCode::FuelExhausted
                        },
                        "{failure:?}"
                    );
                    lo = mid + 1;
                }
            }
        }
        assert!(lo > 1);
        let exact = if depth {
            crate::song::SongLimits {
                max_depth: lo,
                ..limits
            }
        } else {
            crate::song::SongLimits {
                max_nodes: lo,
                ..limits
            }
        };
        assert_eq!(resolve_route(&plan, &rows[0], exact).unwrap(), expected);
        let short = if depth {
            crate::song::SongLimits {
                max_depth: lo - 1,
                ..limits
            }
        } else {
            crate::song::SongLimits {
                max_nodes: lo - 1,
                ..limits
            }
        };
        assert_eq!(
            resolve_route(&plan, &rows[0], short).unwrap_err().code,
            if depth {
                crate::vm::fail::FailCode::DepthExceeded
            } else {
                crate::vm::fail::FailCode::FuelExhausted
            }
        );
    }
    for row in rows.iter().rev() {
        assert_eq!(resolve_route(&plan, row, limits).unwrap(), expected);
    }
}

mod static_joint;

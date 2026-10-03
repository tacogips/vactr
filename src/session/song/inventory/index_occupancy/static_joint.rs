//! TASK-001 recognition/replay; general heterogeneous gap solving remains open.
use super::*;
use crate::pattern::combinators::{structure, time};
use crate::vm::fail::FailCode;

fn nested(value: i32) -> Rc<Pat> {
    let hurry = Rc::new(time::hurry(
        Rc::new(crate::pattern::build::pure(Value::Int(value), None)),
        PParam::Const(Value::Ratio(Ratio64::new(3, 2).unwrap())),
        None,
    ));
    let fast = Rc::new(time::fast(hurry, PParam::Const(Value::Int(2)), None));
    Rc::new(time::slow(fast, PParam::Const(Value::Int(6)), None))
}
fn steps(values: &[Value]) -> Rc<Pat> {
    Rc::new(crate::pattern::build::value_steps(values))
}
fn program(
    song: &PreparedSong,
    limits: crate::song::SongLimits,
) -> (crate::song::routing::StaticFamilyRows, u32) {
    let inventory = song.snapshot().routing();
    crate::song::routing::inspect_static_family_program(
        inventory,
        inventory.root_part,
        intern_kw("drums"),
        0,
        limits,
    )
    .unwrap()
}
fn nested_index() -> Rc<Pat> {
    steps(&[Value::Pattern(nested(0)), Value::Nil])
}

#[test]
fn composed_positive_rates_replay_original_wholes_and_partition_identity() {
    let limits = crate::song::SongLimits::default();
    let mut nested = prepared_index(nested_index(), Ratio64::ONE);
    let mut direct = prepared();
    let rows = nested.query(span(0, 2, 1), &limits).unwrap();
    let direct_rows = direct.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(direct_rows.len(), 2);
    assert_eq!(
        program(&nested, limits).0,
        vec![(
            Ratio64::ZERO,
            Ratio64::new(1, 2).unwrap(),
            1,
            Ratio64::from_int(2)
        )]
    );
    for (a, b) in rows.iter().zip(&direct_rows) {
        assert_eq!(a.whole, b.whole);
        assert_eq!(a.part, b.part);
        assert_eq!(a.note, b.note);
        assert_eq!(a.instrument, b.instrument);
        assert_eq!(a.controls, b.controls);
        let origin = a.source_origin.as_ref().unwrap();
        let timing = &origin.slice_timings()[0];
        assert_eq!(Some(timing.index_whole()), a.whole);
        assert_eq!(timing.sample_start(), a.whole.unwrap().begin);
        assert_eq!(timing.subject_handle(), &origin.handle);
    }
    let plan = prepare_routes(nested.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let expected = resolve_route(&plan, &rows[0], limits).unwrap();
    assert_eq!(expected.configuration, span(0, 3, 2));
    assert_eq!(resolve_route(&plan, &rows[1], limits).unwrap(), expected);
    for window in [span(9, 11, 8), span(1, 3, 8), span(8, 12, 8)] {
        let partial = nested.query(window, &limits).unwrap();
        assert_eq!(partial.len(), 1);
        for mut row in partial {
            let original = rows.iter().find(|a| a.handle == row.handle).unwrap();
            assert_eq!(resolve_route(&plan, &row, limits).unwrap(), expected);
            row.part = original.part;
            assert_eq!(&row, original, "all original provenance preserved");
        }
    }
}

#[test]
fn heterogeneous_family_recognition_preserves_temporary_joint_requirement() {
    let held = Rc::new(structure::hold(
        slow_index(1, Ratio64::from_int(3)),
        PParam::Const(Value::Int(2)),
        None,
    ));
    let mut song = prepared_index(
        steps(&[Value::Pattern(nested(0)), Value::Pattern(held)]),
        Ratio64::ONE,
    );
    let limits = crate::song::SongLimits::default();
    assert_eq!(
        program(&song, limits).0,
        vec![
            (
                Ratio64::ZERO,
                Ratio64::new(1, 3).unwrap(),
                1,
                Ratio64::from_int(2)
            ),
            (
                Ratio64::new(1, 3).unwrap(),
                Ratio64::new(2, 3).unwrap(),
                1,
                Ratio64::from_int(3)
            ),
        ]
    );
    let rows = song.query(span(0, 1, 1), &limits).unwrap();
    assert!(!rows.is_empty());
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    for row in rows {
        assert!(!row
            .source_origin
            .as_ref()
            .unwrap()
            .slice_timings()
            .is_empty());
        let failure = resolve_route(&plan, &row, limits).unwrap_err();
        assert_eq!(failure.code, FailCode::BeyondCapability);
        assert!(failure.message.contains("shared canonical realization"));
        assert!(failure.message.contains("issuance=Some"));
    }
    // This is explicit unfinished joint geometry, not a dynamic operand claim.
}

#[test]
fn rest_and_zero_repeat_keep_occupied_weights_and_authentic_producer_ordinals() {
    let silent = Rc::new(structure::repeat(
        nested(1),
        PParam::Const(Value::Int(0)),
        None,
    ));
    let mut song = prepared_index(
        steps(&[
            Value::Pattern(silent),
            Value::Pattern(nested(0)),
            Value::Nil,
        ]),
        Ratio64::ONE,
    );
    let limits = crate::song::SongLimits::default();
    assert_eq!(
        program(&song, limits).0,
        vec![(
            Ratio64::ZERO,
            Ratio64::new(1, 2).unwrap(),
            1,
            Ratio64::from_int(2)
        )]
    );
    let rows = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].whole, Some(span(0, 1, 1)));
    assert_eq!(rows[1].whole, Some(span(1, 3, 2)));
    assert!(song.query(span(5, 7, 8), &limits).unwrap().is_empty());
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let route = resolve_route(&plan, &rows[0], limits).unwrap();
    assert_eq!(route.configuration, span(0, 3, 2));
    assert_eq!(resolve_route(&plan, &rows[1], limits).unwrap(), route);
}

#[test]
fn composed_route_has_exact_cumulative_work_and_depth_boundary() {
    let limits = crate::song::SongLimits::default();
    let mut song = prepared_index(nested_index(), Ratio64::ONE);
    let rows = song.query(span(0, 1, 1), &limits).unwrap();
    assert_eq!(rows.len(), 1);
    let plan = prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities()).unwrap();
    let expected = resolve_route(&plan, &rows[0], limits).unwrap();
    for depth in [false, true] {
        let mut low = 1;
        let mut high = if depth {
            limits.max_depth
        } else {
            limits.max_nodes
        };
        let with = |value| {
            if depth {
                crate::song::SongLimits {
                    max_depth: value,
                    ..limits
                }
            } else {
                crate::song::SongLimits {
                    max_nodes: value,
                    ..limits
                }
            }
        };
        while low < high {
            let mid = low + (high - low) / 2;
            match resolve_route(&plan, &rows[0], with(mid)) {
                Ok(route) => {
                    assert_eq!(route, expected);
                    high = mid;
                }
                Err(failure) => {
                    assert_eq!(
                        failure.code,
                        if depth {
                            FailCode::DepthExceeded
                        } else {
                            FailCode::FuelExhausted
                        }
                    );
                    low = mid + 1;
                }
            }
        }
        assert!(low > 1);
        assert_eq!(resolve_route(&plan, &rows[0], with(low)).unwrap(), expected);
        assert_eq!(
            resolve_route(&plan, &rows[0], with(low - 1))
                .unwrap_err()
                .code,
            if depth {
                FailCode::DepthExceeded
            } else {
                FailCode::FuelExhausted
            }
        );
    }
}

#[test]
fn large_requested_pattern_repeat_preserves_existing_limit_without_expansion() {
    let limits = crate::song::SongLimits::default();
    let make = |copies| {
        prepared_index(
            steps(&[
                Value::Pattern(Rc::new(structure::repeat(
                    nested(0),
                    PParam::Const(Value::Int(copies)),
                    None,
                ))),
                Value::Nil,
            ]),
            Ratio64::ONE,
        )
    };
    let small = make(10);
    let large = make(1_000_000);
    let (families, work) = program(&large, limits);
    assert_eq!(families.len(), 1);
    assert_eq!(families[0].2, 4096);
    assert_eq!(families[0].1, Ratio64::new(1, 4097).unwrap());
    assert_eq!(families[0].3, Ratio64::from_int(2));
    assert_eq!(program(&small, limits).1, work);
    assert_eq!(
        program(
            &large,
            crate::song::SongLimits {
                max_nodes: work,
                ..limits
            }
        )
        .0,
        families
    );
    let inventory = large.snapshot().routing();
    let short = crate::song::SongLimits {
        max_nodes: work - 1,
        ..limits
    };
    assert_eq!(
        crate::song::routing::inspect_static_family_program(
            inventory,
            inventory.root_part,
            intern_kw("drums"),
            0,
            short,
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    // Original Steps Repeat clamps to 4096 before normalized slot weighting.
    // Admission honors real physical limits; no expanded event query.
    assert_eq!(
        prepare_routes(large.snapshot(), &CapabilitySet::native(), &capacities())
            .unwrap_err()
            .code,
        FailCode::BeyondCapability
    );
}

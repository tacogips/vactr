//! Canonical finite query contracts exercised through public APIs.
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use vactr::clock::tempo::Tempo;
use vactr::ns::namespace::VarSlotRef;
use vactr::pattern::combinators::{control::control, random::choose, time::slow};
use vactr::pattern::eval::{InputCells, QueryVm};
use vactr::pattern::pat::{PParam, Pat, PatNode};
use vactr::pattern::query::TimeSpan;
use vactr::pattern::step::{steps, Step};
use vactr::reader::span::NodeId;
use vactr::song::*;
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;
use vactr::value::value::{NativeId, Sound, Value};
use vactr::vm::fail::{FailCode, Failure, Origin};
#[derive(Default)]
struct TestVm {
    output: Vec<(Origin, Rc<str>)>,
    calls: usize,
}
impl QueryVm for TestVm {
    fn call(&mut self, callable: &Value, args: &[Value]) -> Result<Value, Failure> {
        self.calls += 1;
        if matches!(callable,Value::Native(id) if id.get()==3) {
            let Value::Pattern(selected) = &args[0] else {
                panic!()
            };
            return Ok(Value::Pattern(Rc::new(Pat::new(
                PatNode::Fast(selected.clone(), PParam::Const(Value::Int(2))),
                None,
                true,
            ))));
        }
        if matches!(callable,Value::Native(id) if id.get()==2) {
            return Ok(args[0].clone());
        }
        let time = match args.first() {
            Some(Value::Ratio(t)) => *t,
            _ => Ratio64::ZERO,
        };
        Ok(audio(if time < Ratio64::ONE {
            "before"
        } else {
            "after"
        }))
    }
    fn deref(&mut self, slot: &VarSlotRef) -> Result<Value, Failure> {
        Ok(slot.get())
    }
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        std::mem::take(&mut self.output)
    }
    fn put_output(&mut self, output: Vec<(Origin, Rc<str>)>) {
        self.output = output;
    }
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        Ok(Value::dict(Default::default()))
    }
}
fn audio(name: &str) -> Value {
    Value::Sound(Rc::new(Sound::Builtin(intern_kw(name))))
}
fn scalar(value: Value) -> Pat {
    vactr::pattern::build::pure(value, None)
}
fn span(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
}
fn capture(pattern: Pat, duration: Ratio64) -> Part {
    capture_part(
        BTreeMap::from([(intern_kw("drums"), Rc::new(pattern))]),
        duration,
    )
    .unwrap()
}
fn run(part: &Part, span: TimeSpan) -> Vec<SongEvent> {
    run_limits(part, span, &SongLimits::default()).unwrap()
}
fn run_limits(part: &Part, span: TimeSpan, limits: &SongLimits) -> Result<Vec<SongEvent>, Failure> {
    let mut vm = TestVm::default();
    let cells = InputCells::new();
    query_part(
        part,
        span,
        &mut SongQueryCtx {
            vm: &mut vm,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits,
        },
    )
}
fn equal_payload(a: &SongEvent, b: &SongEvent) {
    assert_eq!(a.handle, b.handle);
    assert_eq!(a.event.whole, b.event.whole);
    assert_eq!(a.instrument, b.instrument);
    assert_eq!(a.tone, b.tone);
    assert_eq!(a.route, b.route);
    assert_eq!(
        format!("{:?}", a.event.controls),
        format!("{:?}", b.event.controls)
    );
}
#[test]
fn complete_handles_and_payloads_are_invariant_under_reordered_fractional_partitions() {
    let p = capture(
        steps(
            vec![audio("bd"), Value::Nil, audio("sd"), audio("bd")]
                .into_iter()
                .map(Step::bare)
                .collect(),
            None,
        ),
        Ratio64::new(11, 4).unwrap(),
    );
    let whole = run(&p, span(0, 11, 4));
    assert_eq!(whole.len(), 8);
    let mut seen = BTreeMap::new();
    for i in (0..33).rev() {
        for row in run(&p, span(i, i + 1, 12)) {
            let original = whole.iter().find(|e| e.handle == row.handle).unwrap();
            equal_payload(original, &row);
            seen.insert(row.handle.clone(), row);
        }
    }
    assert_eq!(seen.len(), whole.len());
    assert!(run(&p, span(11, 12, 4)).is_empty());
}
#[test]
fn simultaneous_twins_ignore_compact_hash_collisions_and_filtered_ranks() {
    let mut first = scalar(audio("bd"));
    let mut second = scalar(audio("bd"));
    first.id = NodeId::new(7);
    second.id = NodeId::new(7);
    let p = capture(
        Pat::new(
            PatNode::Stack(vec![first, second].into_boxed_slice()),
            None,
            true,
        ),
        Ratio64::ONE,
    );
    let all = run(&p, span(0, 1, 1));
    assert_eq!(all.len(), 2);
    assert_ne!(all[0].handle, all[1].handle);
    assert_ne!(
        all[0].handle.occurrence().producer_ordinals,
        all[1].handle.occurrence().producer_ordinals
    );
    let clipped = run(&p, span(1, 2, 4));
    assert_eq!(clipped.len(), 2);
    for row in clipped {
        equal_payload(all.iter().find(|e| e.handle == row.handle).unwrap(), &row);
    }
}
#[test]
fn slow_dynamic_whole_payload_survives_every_canonical_continuation() {
    let p = capture(
        slow(
            Rc::new(scalar(Value::Native(NativeId::new(1)))),
            PParam::Const(Value::Int(4)),
            None,
        ),
        Ratio64::from_int(4),
    );
    let all = run(&p, span(0, 4, 1));
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].instrument, Sound::Builtin(intern_kw("before")));
    assert_eq!(all[0].event.whole, Some(span(0, 4, 1)));
    for i in 0..16 {
        let one = run(&p, span(i, i + 1, 4));
        assert_eq!(one.len(), 1);
        equal_payload(&all[0], &one[0]);
    }
}
#[test]
fn fractional_sequence_offsets_and_boundary_onsets_remain_exact() {
    let a = Rc::new(capture(scalar(audio("bd")), Ratio64::new(2, 3).unwrap()));
    let b = Rc::new(capture(scalar(audio("sd")), Ratio64::new(3, 5).unwrap()));
    let p = sequence(vec![a, b]).unwrap();
    assert_eq!(p.duration(), Ratio64::new(19, 15).unwrap());
    let rows = run(&p, span(0, 19, 15));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].event.anchor(), Ratio64::ZERO);
    assert_eq!(rows[1].event.anchor(), Ratio64::new(2, 3).unwrap());
    assert_ne!(rows[0].placement, rows[1].placement);
    let next = run(&p, span(2, 3, 3));
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].instrument, Sound::Builtin(intern_kw("sd")));
}
fn random_capture() -> Rc<Part> {
    Rc::new(capture(
        choose(
            (0..16)
                .map(|i| scalar(audio(&format!("voice-{i}"))))
                .collect(),
            None,
        ),
        Ratio64::ONE,
    ))
}
#[test]
fn same_and_vary_nested_seed_paths_normalize_only_their_own_ordinals() {
    let base = random_capture();
    let inner_vary = Rc::new(part_repeat(base.clone(), 8, RepeatSeedMode::Vary).unwrap());
    let outer_same = part_repeat(inner_vary, 2, RepeatSeedMode::Same).unwrap();
    let rows = run(&outer_same, span(0, 16, 1));
    assert_eq!(rows.len(), 16);
    for i in 0..8 {
        assert_eq!(rows[i].instrument, rows[i + 8].instrument);
        assert_ne!(rows[i].handle, rows[i + 8].handle);
    }
    assert!(
        rows[..8]
            .iter()
            .map(|e| format!("{:?}", e.instrument))
            .collect::<BTreeSet<_>>()
            .len()
            > 1
    );
    let inner_same = Rc::new(part_repeat(base, 4, RepeatSeedMode::Same).unwrap());
    let outer_vary = part_repeat(inner_same, 8, RepeatSeedMode::Vary).unwrap();
    let rows = run(&outer_vary, span(0, 32, 1));
    assert_eq!(rows.len(), 32);
    for group in rows.chunks(4) {
        assert!(group.iter().all(|e| e.instrument == group[0].instrument));
    }
    assert!(
        rows.chunks(4)
            .map(|g| format!("{:?}", g[0].instrument))
            .collect::<BTreeSet<_>>()
            .len()
            > 1
    );
    let sequence = sequence(vec![Rc::new(outer_vary.clone()), Rc::new(outer_vary)]).unwrap();
    let rows = run(&sequence, span(0, 64, 1));
    assert_eq!(rows.len(), 64);
    for group in rows.chunks(4) {
        assert!(group.iter().all(|e| e.instrument == group[0].instrument));
    }
    for (i, row) in rows.iter().enumerate() {
        equal_payload(row, &run(&sequence, span(i as i64, i as i64 + 1, 1))[0]);
    }
}
#[test]
fn billion_repeats_query_only_touched_placements_and_keep_last_index() {
    let p = part_repeat(
        Rc::new(capture(scalar(audio("bd")), Ratio64::ONE)),
        u32::MAX,
        RepeatSeedMode::Same,
    )
    .unwrap();
    let end = i64::from(u32::MAX);
    let last = run(&p, span(end - 1, end, 1));
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].placement.0, vec![2, u32::MAX - 1]);
    assert_eq!(last[0].event.anchor(), Ratio64::from_int(end - 1));
    assert!(run(&p, span(end, end + 1, 1)).is_empty());
}
#[test]
fn canonical_capacity_admission_precedes_caller_clipping() {
    let p = capture(
        steps(
            vec![Step::bare(audio("bd")), Step::bare(audio("sd"))].into_boxed_slice(),
            None,
        ),
        Ratio64::ONE,
    );
    let limits = SongLimits::for_capacities(64, 1, u64::MAX).unwrap();
    for query in [span(0, 1, 1), span(0, 1, 4), span(3, 4, 4)] {
        assert_eq!(
            run_limits(&p, query, &limits).unwrap_err().code,
            FailCode::BeyondCapability
        );
    }
    let empty = sequence(vec![]).unwrap();
    assert!(run(&empty, span(0, 1, 1)).is_empty());
    let zero = part_repeat(Rc::new(p), 0, RepeatSeedMode::Same).unwrap();
    assert!(run(&zero, span(0, 1, 1)).is_empty());
}
#[test]
fn numeric_widths_and_same_pitch_tones_expand_once_with_mono_commit() {
    let pitches = vec![
        Value::Int64(60),
        Value::Ratio(Ratio64::new(241, 4).unwrap()),
        Value::Float(60.25),
        Value::Float64(60.25),
        Value::Int64(60),
    ];
    let p = capture(
        control(
            intern_kw("note"),
            Rc::new(scalar(Value::list(pitches.clone()))),
            Rc::new(scalar(audio("piano"))),
            None,
        ),
        Ratio64::ONE,
    );
    let rows = run(&p, span(0, 1, 1));
    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows.iter().map(|e| e.handle.tone()).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(
        rows.iter()
            .map(|e| e.handle.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        5
    );
    for (row, pitch) in rows.iter().zip(&pitches) {
        assert_eq!(row.tone, Some(ResolvedNote::try_from(pitch).unwrap()));
        assert_eq!(row.commit_mode, NoteCommitMode::Mono);
        assert!(!matches!(
            row.event.controls.get(&intern_kw("note")),
            Some(Value::List(_))
        ));
    }
}
#[test]
fn negative_onset_continuations_are_not_admitted_and_whole_release_is_preserved() {
    let delayed = Pat::new(
        PatNode::Off(
            Rc::new(slow(
                Rc::new(scalar(audio("bd"))),
                PParam::Const(Value::Int(4)),
                None,
            )),
            PParam::Const(Value::Int(-1)),
            Value::Native(NativeId::new(2)),
        ),
        None,
        true,
    );
    let p = capture(delayed, Ratio64::from_int(2));
    let admitted = run(&p, span(0, 2, 1));
    assert_eq!(admitted.len(), 1);
    assert_eq!(admitted[0].event.anchor(), Ratio64::ZERO);
    let sustained = capture(
        slow(
            Rc::new(scalar(audio("bd"))),
            PParam::Const(Value::Int(4)),
            None,
        ),
        Ratio64::new(3, 2).unwrap(),
    );
    let rows = run(&sustained, span(0, 3, 2));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event.whole, Some(span(0, 4, 1)));
    assert_eq!(rows[0].event.part, span(0, 3, 2));
}
#[test]
fn any_source_fault_rejects_the_complete_song_query() {
    let p = capture(
        Pat::new(
            PatNode::Stack(
                vec![
                    scalar(audio("bd")),
                    scalar(Value::Part(Rc::new(sequence(vec![]).unwrap()))),
                ]
                .into_boxed_slice(),
            ),
            None,
            true,
        ),
        Ratio64::ONE,
    );
    assert_eq!(
        run_limits(&p, span(0, 1, 1), &SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
    let bad = capture(
        control(
            intern_kw("note"),
            Rc::new(scalar(Value::Float64(f64::NAN))),
            Rc::new(scalar(audio("piano"))),
            None,
        ),
        Ratio64::ONE,
    );
    assert_eq!(
        run_limits(&bad, span(0, 1, 1), &SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
}

#[test]
fn n_fallback_in_unsupported_query_context_faults_instead_of_guessing_sound_tag() {
    let p = capture(
        control(
            intern_kw("n"),
            Rc::new(scalar(Value::Int(60))),
            Rc::new(scalar(audio("analog"))),
            None,
        ),
        Ratio64::ONE,
    );
    assert_eq!(
        run_limits(&p, span(0, 1, 1), &SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::HostUnavailable
    );
    let p = capture(
        control(
            intern_kw("note"),
            Rc::new(scalar(Value::Float64(60.25))),
            Rc::new(control(
                intern_kw("n"),
                Rc::new(scalar(Value::Int(60))),
                Rc::new(scalar(audio("analog"))),
                None,
            )),
            None,
        ),
        Ratio64::ONE,
    );
    assert_eq!(
        run(&p, span(0, 1, 1))[0].tone,
        Some(ResolvedNote::Float64(60.25))
    );
}

#[test]
fn explicit_frequency_suppresses_note_and_n_chords_before_first_certification() {
    for key in ["note", "n"] {
        let controls = control(
            intern_kw(key),
            Rc::new(scalar(Value::list(vec![
                Value::Float64(60.25),
                Value::Float64(60.25),
            ]))),
            Rc::new(scalar(audio("analog"))),
            None,
        );
        let p = capture(
            control(
                intern_kw("freq"),
                Rc::new(scalar(Value::Float64(440.0))),
                Rc::new(controls),
                None,
            ),
            Ratio64::ONE,
        );
        let result = run(&p, span(0, 1, 1));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tone, None);
        assert_eq!(result[0].handle.tone(), 0);
        assert_eq!(result[0].commit_mode, NoteCommitMode::Mono);
    }
}

#[test]
fn full_track_replacement_skips_obsolete_faults_but_retains_unrelated_track_failures() {
    let base = capture(scalar(Value::Int(9)), Ratio64::ONE);
    assert_eq!(
        run_limits(&base, span(0, 1, 1), &SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
    let replacement =
        replace_track(&base, intern_kw("drums"), Rc::new(scalar(audio("bd")))).unwrap();
    assert_eq!(run(&replacement, span(0, 1, 1)).len(), 1);
    let repeated = part_repeat(Rc::new(base), 4, RepeatSeedMode::Same).unwrap();
    let replacement =
        replace_track(&repeated, intern_kw("drums"), Rc::new(scalar(audio("sd")))).unwrap();
    assert_eq!(run(&replacement, span(0, 4, 1)).len(), 4);
    let other = capture_part(
        BTreeMap::from([
            (intern_kw("drums"), Rc::new(scalar(audio("bd")))),
            (intern_kw("faulty"), Rc::new(scalar(Value::Int(7)))),
        ]),
        Ratio64::ONE,
    )
    .unwrap();
    let replacement =
        replace_track(&other, intern_kw("drums"), Rc::new(scalar(audio("sd")))).unwrap();
    assert_eq!(
        run_limits(&replacement, span(0, 1, 1), &SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
}

#[test]
fn selected_source_does_not_resurrect_obsolete_sibling_fault_after_later_replace() {
    let base = capture_part(
        BTreeMap::from([
            (intern_kw("drums"), Rc::new(scalar(audio("bd")))),
            (intern_kw("bass"), Rc::new(scalar(Value::Int(7)))),
        ]),
        Ratio64::ONE,
    )
    .unwrap();
    let mut vm = TestVm::default();
    let edited = transform_instrument(
        &base,
        intern_kw("drums"),
        InstrumentSelector::new(vec![Sound::Builtin(intern_kw("bd"))]).unwrap(),
        Value::Native(NativeId::new(2)),
        &mut SongBuildCtx {
            vm: &mut vm,
            limits: &SongLimits::default(),
        },
    )
    .unwrap();
    let changed =
        replace_track(&edited, intern_kw("bass"), Rc::new(scalar(audio("bass")))).unwrap();
    let rows = run(&changed, span(0, 1, 1));
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .find(|e| e.track == intern_kw("drums"))
        .unwrap()
        .event
        .song_source
        .is_some());
}

#[test]
fn nested_selected_identity_growth_uses_shared_budget_before_large_encoding_allocations() {
    let mut part = capture(scalar(audio("bd")), Ratio64::ONE);
    let mut vm = TestVm::default();
    for _ in 0..16 {
        part = transform_instrument(
            &part,
            intern_kw("drums"),
            InstrumentSelector::new(vec![Sound::Builtin(intern_kw("bd"))]).unwrap(),
            Value::Native(NativeId::new(2)),
            &mut SongBuildCtx {
                vm: &mut vm,
                limits: &SongLimits::default(),
            },
        )
        .unwrap();
    }
    let failure = run_limits(&part, span(0, 1, 1), &SongLimits::default()).unwrap_err();
    assert_eq!(failure.code, FailCode::FuelExhausted);
    assert!(failure.message.contains("shared query budget"));
    let small = capture(scalar(audio("bd")), Ratio64::ONE);
    let small = transform_instrument(
        &small,
        intern_kw("drums"),
        InstrumentSelector::new(vec![Sound::Builtin(intern_kw("bd"))]).unwrap(),
        Value::Native(NativeId::new(2)),
        &mut SongBuildCtx {
            vm: &mut vm,
            limits: &SongLimits::default(),
        },
    )
    .unwrap();
    assert_eq!(run(&small, span(0, 1, 1)).len(), 1);
}

#[test]
fn moved_selected_events_keep_original_scoped_routes_and_later_fx_overrides_them() {
    let family = InstrumentSelector::new(vec![Sound::Builtin(intern_kw("bd"))]).unwrap();
    let first = instrument_fx(
        &capture(scalar(audio("bd")), Ratio64::ONE),
        intern_kw("drums"),
        family.clone(),
        intern_kw("plate"),
    )
    .unwrap();
    let second = instrument_fx(
        &capture(scalar(audio("bd")), Ratio64::ONE),
        intern_kw("drums"),
        family.clone(),
        intern_kw("spring"),
    )
    .unwrap();
    let base = sequence(vec![Rc::new(first), Rc::new(second)]).unwrap();
    let mut vm = TestVm::default();
    let changed = transform_instrument(
        &base,
        intern_kw("drums"),
        family.clone(),
        Value::Native(NativeId::new(3)),
        &mut SongBuildCtx {
            vm: &mut vm,
            limits: &SongLimits::default(),
        },
    )
    .unwrap();
    let result = run(&changed, span(0, 2, 1));
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].event.anchor(), Ratio64::ZERO);
    assert_eq!(result[1].event.anchor(), Ratio64::new(1, 2).unwrap());
    assert_eq!(
        result[0].route.as_ref().unwrap().template,
        intern_kw("plate")
    );
    assert_eq!(
        result[1].route.as_ref().unwrap().template,
        intern_kw("spring")
    );
    for row in &result {
        assert_eq!(row.route, row.event.song_source.as_ref().unwrap().route);
    }
    let repeated = part_repeat(Rc::new(changed.clone()), 2, RepeatSeedMode::Same).unwrap();
    let repeated = run(&repeated, span(0, 4, 1));
    assert_eq!(repeated.len(), 4);
    for pair in repeated.chunks(2) {
        assert_eq!(pair[0].route.as_ref().unwrap().template, intern_kw("plate"));
        assert_eq!(
            pair[1].route.as_ref().unwrap().template,
            intern_kw("spring")
        );
    }
    let silence = Rc::new(capture_part(BTreeMap::new(), Ratio64::new(7, 3).unwrap()).unwrap());
    let moved = sequence(vec![silence, Rc::new(changed.clone())]).unwrap();
    let moved = run(&moved, span(0, 13, 3));
    assert_eq!(moved.len(), 2);
    assert_eq!(
        moved[1].route.as_ref().unwrap().template,
        intern_kw("spring")
    );
    let overridden =
        instrument_fx(&changed, intern_kw("drums"), family, intern_kw("space")).unwrap();
    let result = run(&overridden, span(0, 2, 1));
    assert_eq!(result.len(), 2);
    assert!(result
        .iter()
        .all(|row| row.route.as_ref().unwrap().template == intern_kw("space")));
}

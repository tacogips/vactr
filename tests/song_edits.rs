//! Immutable edits, selected origins and scoped private FX policies.
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use vactr::clock::tempo::Tempo;
use vactr::ns::namespace::VarSlotRef;
use vactr::pattern::combinators::{control::control, sound::sound, time::slow};
use vactr::pattern::eval::{InputCells, QueryVm};
use vactr::pattern::pat::{PParam, Pat, PatNode};
use vactr::pattern::query::TimeSpan;
use vactr::pattern::step::{steps, Step};
use vactr::song::*;
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;
use vactr::value::value::{NativeId, Sound, Value};
use vactr::vm::fail::{FailCode, Failure, Origin};
#[derive(Default)]
struct TestVm {
    calls: usize,
    output: Vec<(Origin, Rc<str>)>,
    selected: Option<Value>,
}
impl QueryVm for TestVm {
    fn call(&mut self, f: &Value, args: &[Value]) -> Result<Value, Failure> {
        self.calls += 1;
        let Value::Native(id) = f else {
            return Err(Failure::new(FailCode::Type, "test callback expected"));
        };
        match id.get() {
            1 => {
                let Value::Pattern(p) = &args[0] else {
                    panic!()
                };
                Ok(Value::Pattern(Rc::new(control(
                    intern_kw("gain"),
                    Rc::new(scalar(Value::Float64(0.25))),
                    p.clone(),
                    None,
                ))))
            }
            2 => {
                self.selected = Some(args[0].clone());
                Ok(Value::Pattern(Rc::new(sound(
                    PParam::Fn(Value::Native(NativeId::new(3))),
                    None,
                    None,
                ))))
            }
            3 => Ok(self.selected.clone().unwrap()),
            4 => {
                self.output.push((Origin::none(), Rc::from("forbidden")));
                Ok(args[0].clone())
            }
            5 => Err(Failure::new(FailCode::DivisionByZero, "callback failed")),
            6 => Ok(args[0].clone()),
            7 => {
                let Value::Pattern(p) = &args[0] else {
                    panic!()
                };
                Ok(Value::Pattern(Rc::new(Pat::new(
                    PatNode::Stack(vec![(**p).clone(), (**p).clone()].into_boxed_slice()),
                    None,
                    true,
                ))))
            }
            8 => {
                let Value::Pattern(p) = &args[0] else {
                    panic!()
                };
                Ok(Value::Pattern(Rc::new(control(
                    intern_kw("freq"),
                    Rc::new(scalar(Value::Float64(440.0))),
                    p.clone(),
                    None,
                ))))
            }
            _ => Err(Failure::new(FailCode::Type, "unknown callback")),
        }
    }
    fn deref(&mut self, s: &VarSlotRef) -> Result<Value, Failure> {
        Ok(s.get())
    }
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        std::mem::take(&mut self.output)
    }
    fn put_output(&mut self, v: Vec<(Origin, Rc<str>)>) {
        self.output = v;
    }
    fn song_sample_backed(&mut self, _: &Sound) -> Result<bool, Failure> {
        Ok(true)
    }
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        Ok(Value::dict(Default::default()))
    }
}
fn audio(s: &str) -> Value {
    Value::Sound(Rc::new(Sound::Builtin(intern_kw(s))))
}
fn scalar(v: Value) -> Pat {
    vactr::pattern::build::pure(v, None)
}
fn span(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
}
fn capture(p: Pat, duration: Ratio64) -> Part {
    capture_part(BTreeMap::from([(intern_kw("drums"), Rc::new(p))]), duration).unwrap()
}
fn selector(s: &str) -> InstrumentSelector {
    InstrumentSelector::new(vec![Sound::Builtin(intern_kw(s))]).unwrap()
}
fn rows(part: &Part, span: TimeSpan, vm: &mut TestVm) -> Vec<SongEvent> {
    let cells = InputCells::new();
    query_part(
        part,
        span,
        &mut SongQueryCtx {
            vm,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits: &SongLimits::default(),
        },
    )
    .unwrap()
}
fn build(part: &Part, callback: u32, vm: &mut TestVm) -> Part {
    transform_instrument(
        part,
        intern_kw("drums"),
        selector("bd"),
        Value::Native(NativeId::new(callback)),
        &mut SongBuildCtx {
            vm,
            limits: &SongLimits::default(),
        },
    )
    .unwrap()
}
fn drums() -> Part {
    capture(
        steps(
            vec![audio("bd"), audio("sd"), audio("bd"), audio("sd")]
                .into_iter()
                .map(Step::bare)
                .collect(),
            None,
        ),
        Ratio64::from_int(2),
    )
}
#[test]
fn selective_transform_executes_once_and_leaves_unmatched_source_immutable() {
    let base = drums();
    let mut vm = TestVm::default();
    let old = rows(&base, span(0, 2, 1), &mut vm);
    let changed = build(&base, 1, &mut vm);
    assert_eq!(vm.calls, 1);
    let result = rows(&changed, span(0, 2, 1), &mut vm);
    assert_eq!(result.len(), 8);
    assert_eq!(vm.calls, 1);
    for row in &result {
        let gain = row.event.controls.get(&intern_kw("gain"));
        if row.instrument == Sound::Builtin(intern_kw("bd")) {
            assert!(matches!(gain, Some(Value::Float64(0.25))));
            assert!(row.event.song_source.is_some());
        } else {
            assert!(gain.is_none());
        }
    }
    for i in 0..16 {
        rows(&changed, span(i, i + 1, 8), &mut vm);
    }
    assert_eq!(vm.calls, 1);
    let after = rows(&base, span(0, 2, 1), &mut vm);
    assert_eq!(
        old.iter().map(|e| e.handle.clone()).collect::<Vec<_>>(),
        after.iter().map(|e| e.handle.clone()).collect::<Vec<_>>()
    );
    assert!(after.iter().all(|e| e.event.controls.is_empty()));
    assert_eq!(base.seed_identity(), changed.seed_identity());
}
#[test]
fn callback_output_and_failure_restore_preexisting_output_without_publishing_edit() {
    let part = drums();
    let mut vm = TestVm::default();
    vm.output.push((Origin::none(), Rc::from("prior")));
    for id in [4, 5] {
        let result = transform_instrument(
            &part,
            intern_kw("drums"),
            selector("bd"),
            Value::Native(NativeId::new(id)),
            &mut SongBuildCtx {
                vm: &mut vm,
                limits: &SongLimits::default(),
            },
        );
        assert_eq!(
            result.unwrap_err().code,
            if id == 4 {
                FailCode::EffectInQuery
            } else {
                FailCode::DivisionByZero
            }
        );
        assert_eq!(vm.output.len(), 1);
        assert_eq!(&*vm.output[0].1, "prior");
    }
    assert_eq!(vm.calls, 2);
    assert_eq!(rows(&part, span(0, 1, 1), &mut vm).len(), 4);
    let unknown = transform_instrument(
        &part,
        intern_kw("missing"),
        selector("bd"),
        Value::Native(NativeId::new(6)),
        &mut SongBuildCtx {
            vm: &mut vm,
            limits: &SongLimits::default(),
        },
    );
    assert_eq!(unknown.unwrap_err().code, FailCode::UnknownField);
    assert_eq!(vm.calls, 2);
}
#[test]
fn one_equal_fractional_chord_tone_is_deleted_and_stale_handles_rejected() {
    let chord = control(
        intern_kw("note"),
        Rc::new(scalar(Value::list(vec![
            Value::Float64(60.25),
            Value::Float64(60.25),
            Value::Ratio(Ratio64::new(241, 4).unwrap()),
        ]))),
        Rc::new(scalar(audio("bd"))),
        None,
    );
    let base = capture(chord, Ratio64::ONE);
    let mut vm = TestVm::default();
    let original = rows(&base, span(0, 1, 1), &mut vm);
    assert_eq!(original.len(), 3);
    let edited = delete_event(&base, &original[1].handle).unwrap();
    let kept = rows(&edited, span(0, 1, 1), &mut vm);
    assert_eq!(kept.len(), 2);
    assert_eq!(
        kept.iter().map(|e| e.handle.tone()).collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert_eq!(
        delete_event(&edited, &original[0].handle).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(rows(&base, span(0, 1, 1), &mut vm).len(), 3);
}
#[test]
fn child_deletion_survives_integer_fractional_sequence_and_repeat_offsets() {
    let base = drums();
    let mut vm = TestVm::default();
    let handle = rows(&base, span(0, 2, 1), &mut vm)[0].handle.clone();
    let edited = Rc::new(delete_event(&base, &handle).unwrap());
    let silence = Rc::new(capture_part(BTreeMap::new(), Ratio64::new(7, 3).unwrap()).unwrap());
    let sequence = sequence(vec![silence, edited.clone()]).unwrap();
    let result = rows(&sequence, span(0, 13, 3), &mut vm);
    assert_eq!(result.len(), 7);
    assert!(result
        .iter()
        .all(|e| e.event.anchor() != Ratio64::new(7, 3).unwrap()));
    let repeated = part_repeat(edited, 3, RepeatSeedMode::Same).unwrap();
    let result = rows(&repeated, span(0, 6, 1), &mut vm);
    assert_eq!(result.len(), 21);
    for offset in [0, 2, 4] {
        assert!(result
            .iter()
            .all(|e| e.event.anchor() != Ratio64::from_int(offset)));
    }
}
#[test]
fn overwrite_is_onset_based_retains_prior_release_and_inserts_local_zero() {
    let slow = slow(
        Rc::new(scalar(audio("bd"))),
        PParam::Const(Value::Int(4)),
        None,
    );
    let base = capture(slow, Ratio64::from_int(4));
    let mut vm = TestVm::default();
    let changed = overwrite_region(
        &base,
        intern_kw("drums"),
        span(1, 2, 1),
        Rc::new(scalar(audio("sd"))),
    )
    .unwrap();
    let result = rows(&changed, span(0, 4, 1), &mut vm);
    assert_eq!(result.len(), 2);
    let old = result
        .iter()
        .find(|e| e.instrument == Sound::Builtin(intern_kw("bd")))
        .unwrap();
    assert_eq!(old.event.whole, Some(span(0, 4, 1)));
    assert_eq!(old.event.part, span(0, 4, 1));
    let inserted = result
        .iter()
        .find(|e| e.instrument == Sound::Builtin(intern_kw("sd")))
        .unwrap();
    assert_eq!(inserted.event.anchor(), Ratio64::ONE);
    let normal = drums();
    let edited = overwrite_region(
        &normal,
        intern_kw("drums"),
        span(1, 2, 4),
        Rc::new(scalar(audio("clap"))),
    )
    .unwrap();
    let events = rows(&edited, span(0, 1, 1), &mut vm);
    assert_eq!(events.len(), 4);
    assert!(events
        .iter()
        .any(|e| e.event.anchor() == Ratio64::new(1, 2).unwrap()));
    assert_eq!(
        events
            .iter()
            .find(|e| e.event.anchor() == Ratio64::new(1, 4).unwrap())
            .unwrap()
            .instrument,
        Sound::Builtin(intern_kw("clap"))
    );
}
#[test]
fn scoped_fx_survives_global_replace_overwrite_chains_and_unmatched_families() {
    let base = drums();
    let fx = instrument_fx(
        &base,
        intern_kw("drums"),
        selector("bd"),
        intern_kw("plate"),
    )
    .unwrap();
    let replaced = replace_track(&fx, intern_kw("drums"), Rc::new(scalar(audio("bd")))).unwrap();
    let mut vm = TestVm::default();
    assert!(rows(&replaced, span(0, 2, 1), &mut vm).iter().all(|e| e
        .route
        .as_ref()
        .unwrap()
        .template
        == intern_kw("plate")));
    let overwritten = overwrite_region(
        &replaced,
        intern_kw("drums"),
        span(1, 2, 1),
        Rc::new(scalar(audio("bd"))),
    )
    .unwrap();
    assert!(rows(&overwritten, span(0, 2, 1), &mut vm).iter().all(|e| e
        .route
        .as_ref()
        .unwrap()
        .template
        == intern_kw("plate")));
    let override_fx = instrument_fx(
        &overwritten,
        intern_kw("drums"),
        selector("bd"),
        intern_kw("spring"),
    )
    .unwrap();
    assert!(rows(&override_fx, span(0, 2, 1), &mut vm).iter().all(|e| e
        .route
        .as_ref()
        .unwrap()
        .template
        == intern_kw("spring")));
    let unmatched = replace_track(&fx, intern_kw("drums"), Rc::new(scalar(audio("sd")))).unwrap();
    assert!(rows(&unmatched, span(0, 2, 1), &mut vm)
        .iter()
        .all(|e| e.route.is_none()));
    let routed = rows(&fx, span(0, 2, 1), &mut vm);
    assert!(routed
        .iter()
        .filter(|e| e.instrument == Sound::Builtin(intern_kw("sd")))
        .all(|e| e.route.is_none()));
    for source in [&replaced, &overwritten] {
        let handle = rows(source, span(0, 2, 1), &mut vm)[0].handle.clone();
        let deleted = delete_event(source, &handle).unwrap();
        assert_eq!(rows(&deleted, span(0, 2, 1), &mut vm).len(), 1);
    }
}
#[test]
fn scoped_child_fx_applies_to_new_replacements_at_original_onset_not_current_window() {
    let sustained = capture(
        slow(
            Rc::new(scalar(audio("bd"))),
            PParam::Const(Value::Int(4)),
            None,
        ),
        Ratio64::from_int(4),
    );
    let first = instrument_fx(
        &sustained,
        intern_kw("drums"),
        selector("bd"),
        intern_kw("plate"),
    )
    .unwrap();
    let second = instrument_fx(
        &sustained,
        intern_kw("drums"),
        selector("bd"),
        intern_kw("spring"),
    )
    .unwrap();
    let sequence = sequence(vec![Rc::new(first), Rc::new(second)]).unwrap();
    let replaced = replace_track(
        &sequence,
        intern_kw("drums"),
        Rc::new(slow(
            Rc::new(scalar(audio("bd"))),
            PParam::Const(Value::Int(4)),
            None,
        )),
    )
    .unwrap();
    let mut vm = TestVm::default();
    let first = rows(&replaced, span(3, 4, 1), &mut vm);
    assert_eq!(first.len(), 1);
    assert_eq!(
        first[0].route.as_ref().unwrap().template,
        intern_kw("plate")
    );
    let second = rows(&replaced, span(7, 8, 1), &mut vm);
    assert_eq!(second.len(), 1);
    assert_eq!(
        second[0].route.as_ref().unwrap().template,
        intern_kw("spring")
    );
}
#[test]
fn public_issued_origins_survive_callable_sound_with_fractional_notes_and_routes() {
    let base = capture(
        control(
            intern_kw("note"),
            Rc::new(scalar(Value::list(vec![
                Value::Float64(60.25),
                Value::Ratio(Ratio64::new(241, 4).unwrap()),
            ]))),
            Rc::new(scalar(audio("bd"))),
            None,
        ),
        Ratio64::ONE,
    );
    let fx = instrument_fx(
        &base,
        intern_kw("drums"),
        selector("bd"),
        intern_kw("plate"),
    )
    .unwrap();
    let mut vm = TestVm::default();
    let changed = build(&fx, 2, &mut vm);
    assert_eq!(vm.calls, 1);
    let result = rows(&changed, span(0, 1, 1), &mut vm);
    // Existing callable-sound semantics sample the first mono source at its onset.
    assert_eq!(result.len(), 1);
    let origin = result[0].event.song_source.as_ref().unwrap();
    assert_eq!(origin.handle.revision(), fx.revision());
    assert_eq!(origin.original_instrument, Sound::Builtin(intern_kw("bd")));
    assert_eq!(origin.route.as_ref().unwrap().template, intern_kw("plate"));
    assert_eq!(origin.handle.tone(), 0);
    assert_eq!(result[0].tone, Some(ResolvedNote::Float64(60.25)));
    assert_eq!(result[0].commit_mode, NoteCommitMode::Mono);
    assert_eq!(
        result[0].route.as_ref().unwrap().template,
        intern_kw("plate")
    );
    let clipped = rows(&changed, span(1, 2, 4), &mut vm);
    assert_eq!(clipped[0].handle, result[0].handle);
    assert_eq!(clipped[0].tone, result[0].tone);
}
#[test]
fn generated_duplicates_keep_unique_handles_and_unchanged_certified_source_origin() {
    let base = capture(scalar(audio("bd")), Ratio64::ONE);
    let mut vm = TestVm::default();
    let changed = build(&base, 7, &mut vm);
    let events = rows(&changed, span(0, 1, 1), &mut vm);
    assert_eq!(events.len(), 2);
    assert_eq!(
        events
            .iter()
            .map(|e| e.handle.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    assert_eq!(
        events[0].event.song_source.as_ref().unwrap().handle,
        events[1].event.song_source.as_ref().unwrap().handle
    );
    let edited = delete_event(&changed, &events[0].handle).unwrap();
    assert_eq!(rows(&edited, span(0, 1, 1), &mut vm).len(), 1);
}
#[test]
fn nested_selected_sources_inherit_exact_limits_and_cannot_reset_depth() {
    let base = capture(scalar(audio("bd")), Ratio64::ONE);
    let mut vm = TestVm::default();
    let changed = build(&base, 6, &mut vm);
    let limits = SongLimits {
        max_depth: 3,
        ..SongLimits::default()
    };
    let cells = InputCells::new();
    let result = query_part(
        &changed,
        span(0, 1, 1),
        &mut SongQueryCtx {
            vm: &mut vm,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits: &limits,
        },
    );
    assert_eq!(result.unwrap_err().code, FailCode::DepthExceeded);
    assert_eq!(rows(&changed, span(0, 1, 1), &mut vm).len(), 1);
}

#[test]
fn actual_vmquery_callback_preserves_public_origin_and_rejects_print_and_global_write() {
    use vactr::ns::load::NoopHost;
    use vactr::ns::{evaluator::Evaluator, namespace::Prelude, stage::RecordingSink};
    use vactr::reader::span::FileId;
    use vactr::value::intern::intern_sym;
    use vactr::vm::query_vm::VmQuery;
    let mut evaluator = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    let code="var live 1\nfn selected p:\n\ts {t -> p}\nfn noisy p:\n\tprint \"forbidden\"\n\tfirst [p]\nfn writing p:\n\tupd live 2\n\tfirst [p]\ns :analog";
    let outcomes = evaluator.eval_str(code, FileId::new(7)).unwrap();
    for outcome in &outcomes {
        assert!(outcome.value.is_ok(), "{:?}", outcome.value);
    }
    let Value::Pattern(source) = outcomes.last().unwrap().value.as_ref().unwrap() else {
        panic!()
    };
    let captured = capture(
        control(
            intern_kw("note"),
            Rc::new(scalar(Value::Float64(60.25))),
            source.clone(),
            None,
        ),
        Ratio64::ONE,
    );
    let callback = evaluator
        .ns()
        .session_slot(intern_sym("selected"))
        .unwrap()
        .get();
    let noisy = evaluator
        .ns()
        .session_slot(intern_sym("noisy"))
        .unwrap()
        .get();
    let writing = evaluator
        .ns()
        .session_slot(intern_sym("writing"))
        .unwrap()
        .get();
    let cells = InputCells::new();
    let limits = SongLimits::default();
    let (vm, ns) = evaluator.vm_and_ns();
    let mut query = VmQuery::new(vm, ns);
    let initial = query_part(
        &captured,
        span(0, 1, 1),
        &mut SongQueryCtx {
            vm: &mut query,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits: &limits,
        },
    )
    .unwrap();
    assert_eq!(initial.len(), 1);
    let family = InstrumentSelector::new(vec![initial[0].instrument.clone()]).unwrap();
    let changed = transform_instrument(
        &captured,
        intern_kw("drums"),
        family.clone(),
        callback,
        &mut SongBuildCtx {
            vm: &mut query,
            limits: &limits,
        },
    )
    .unwrap();
    let result = query_part(
        &changed,
        span(0, 1, 1),
        &mut SongQueryCtx {
            vm: &mut query,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits: &limits,
        },
    )
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].tone, Some(ResolvedNote::Float64(60.25)));
    assert_eq!(
        result[0].event.song_source.as_ref().unwrap().handle,
        initial[0].handle
    );
    query.put_output(vec![(Origin::none(), Rc::from("prior"))]);
    for (label, callback) in [("noisy", noisy), ("writing", writing)] {
        assert!(
            matches!(&callback, Value::Fn(_) | Value::Native(_)),
            "{label}: {callback:?}"
        );
        let failed = transform_instrument(
            &captured,
            intern_kw("drums"),
            family.clone(),
            callback,
            &mut SongBuildCtx {
                vm: &mut query,
                limits: &limits,
            },
        );
        assert_eq!(failed.unwrap_err().code, FailCode::EffectInQuery, "{label}");
        let output = query.take_output();
        assert_eq!(output.len(), 1);
        assert_eq!(&*output[0].1, "prior");
        query.put_output(output);
    }
    assert!(matches!(
        ns.session_slot(intern_sym("live")).unwrap().get(),
        Value::Int(1)
    ));
}

#[test]
fn complete_bank_family_selection_is_frozen_before_member_index_and_fx_excludes_other_tracks() {
    let bank = Value::list(vec![audio("bd-a"), audio("bd-b")]);
    let pattern = sound(PParam::Const(bank), None, None);
    let pattern = control(
        intern_kw("n"),
        Rc::new(steps(
            vec![Step::bare(Value::Int(0)), Step::bare(Value::Int(1))].into_boxed_slice(),
            None,
        )),
        Rc::new(pattern),
        None,
    );
    let base = capture_part(
        BTreeMap::from([
            (intern_kw("drums"), Rc::new(pattern)),
            (intern_kw("other"), Rc::new(scalar(audio("bd-a")))),
        ]),
        Ratio64::ONE,
    )
    .unwrap();
    let family = InstrumentSelector::new(vec![
        Sound::Builtin(intern_kw("bd-a")),
        Sound::Builtin(intern_kw("bd-b")),
    ])
    .unwrap();
    let fx = instrument_fx(
        &base,
        intern_kw("drums"),
        family.clone(),
        intern_kw("plate"),
    )
    .unwrap();
    let mut vm = TestVm::default();
    let edited = transform_instrument(
        &fx,
        intern_kw("drums"),
        family,
        Value::Native(NativeId::new(1)),
        &mut SongBuildCtx {
            vm: &mut vm,
            limits: &SongLimits::default(),
        },
    )
    .unwrap();
    let result = rows(&edited, span(0, 1, 1), &mut vm);
    assert_eq!(result.len(), 3);
    assert_eq!(vm.calls, 1);
    for row in result {
        if row.track == intern_kw("drums") {
            assert!(matches!(
                row.event.controls.get(&intern_kw("gain")),
                Some(Value::Float64(0.25))
            ));
            assert_eq!(row.route.unwrap().template, intern_kw("plate"));
        } else {
            assert!(row.route.is_none());
            assert!(row.event.song_source.is_none());
        }
    }
}

#[test]
fn inserted_overwrite_release_crosses_region_end_without_new_onset_or_route_change() {
    let base = drums();
    let fx = instrument_fx(
        &base,
        intern_kw("drums"),
        selector("bd"),
        intern_kw("plate"),
    )
    .unwrap();
    let inserted = Rc::new(slow(
        Rc::new(scalar(audio("bd"))),
        PParam::Const(Value::Int(4)),
        None,
    ));
    let edited = overwrite_region(&fx, intern_kw("drums"), span(1, 2, 4), inserted).unwrap();
    let mut vm = TestVm::default();
    let all = rows(&edited, span(0, 2, 1), &mut vm);
    let note = all
        .iter()
        .find(|e| e.placement.0.first() == Some(&4))
        .unwrap();
    assert_eq!(note.event.whole, Some(span(1, 17, 4)));
    assert_eq!(note.event.part, span(1, 8, 4));
    assert_eq!(note.route.as_ref().unwrap().template, intern_kw("plate"));
    let later = rows(&edited, span(1, 2, 1), &mut vm);
    let continued = later.iter().find(|e| e.handle == note.handle).unwrap();
    assert!(!continued.event.is_onset());
    assert_eq!(continued.event.whole, note.event.whole);
    assert_eq!(continued.route, note.route);
}

#[test]
fn selected_identity_transform_does_not_reseed_random_source_or_unmatched_tracks() {
    use vactr::pattern::combinators::random::choose;
    let pattern = choose(vec![scalar(audio("bd")), scalar(audio("sd"))], None);
    let child = Rc::new(capture(pattern, Ratio64::ONE));
    let repeated = part_repeat(child, 32, RepeatSeedMode::Vary).unwrap();
    let mut vm = TestVm::default();
    let before = rows(&repeated, span(0, 32, 1), &mut vm);
    assert_eq!(before.len(), 32);
    let edited = build(&repeated, 6, &mut vm);
    let after = rows(&edited, span(0, 32, 1), &mut vm);
    assert_eq!(after.len(), 32);
    let a = before
        .iter()
        .map(|e| (e.event.anchor(), e.instrument.clone()))
        .collect::<Vec<_>>();
    let mut b = after
        .iter()
        .map(|e| (e.event.anchor(), e.instrument.clone()))
        .collect::<Vec<_>>();
    let mut a = a;
    a.sort_by_key(|e| e.0);
    b.sort_by_key(|e| e.0);
    assert_eq!(a, b);
    for row in after
        .iter()
        .filter(|e| e.instrument == Sound::Builtin(intern_kw("sd")))
    {
        let old = before
            .iter()
            .find(|e| e.event.anchor() == row.event.anchor())
            .unwrap();
        assert_eq!(old.placement, row.placement);
        assert_eq!(old.handle.occurrence(), row.handle.occurrence());
    }
}

#[test]
fn actual_route_n_chords_expand_non_sample_builtin_and_keep_resource_indices_atomic() {
    use vactr::ns::{
        evaluator::Evaluator, load::NoopHost, namespace::Prelude, stage::RecordingSink,
    };
    use vactr::vm::query_vm::VmQuery;
    let mut evaluator = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    let cells = InputCells::new();
    let limits = SongLimits::default();
    let resource = evaluator
        .insts()
        .unwrap()
        .borrow()
        .id_of(intern_kw("wavetable"))
        .unwrap();
    let (vm, ns) = evaluator.vm_and_ns();
    let mut query = VmQuery::new(vm, ns);
    let pitches = Value::list(vec![
        Value::Float64(60.25),
        Value::Ratio(Ratio64::new(241, 4).unwrap()),
        Value::Float64(60.25),
    ]);
    let builtin = capture(
        control(
            intern_kw("n"),
            Rc::new(scalar(pitches.clone())),
            Rc::new(scalar(audio("analog"))),
            None,
        ),
        Ratio64::ONE,
    );
    let result = query_part(
        &builtin,
        span(0, 1, 1),
        &mut SongQueryCtx {
            vm: &mut query,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits: &limits,
        },
    )
    .unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(
        result[1].tone,
        Some(ResolvedNote::Ratio(Ratio64::new(241, 4).unwrap()))
    );
    assert_eq!(result[2].handle.tone(), 2);
    let bank = capture(
        control(
            intern_kw("n"),
            Rc::new(scalar(Value::Int(3))),
            Rc::new(scalar(Value::Sound(Rc::new(Sound::Inst(resource))))),
            None,
        ),
        Ratio64::ONE,
    );
    let result = query_part(
        &bank,
        span(0, 1, 1),
        &mut SongQueryCtx {
            vm: &mut query,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits: &limits,
        },
    )
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].tone, None);
    assert!(matches!(
        result[0].event.controls.get(&intern_kw("n")),
        Some(Value::Int(3))
    ));
    let explicit = capture(
        control(
            intern_kw("note"),
            Rc::new(scalar(pitches)),
            Rc::new(control(
                intern_kw("n"),
                Rc::new(scalar(Value::Int(3))),
                Rc::new(scalar(Value::Sound(Rc::new(Sound::Inst(resource))))),
                None,
            )),
            None,
        ),
        Ratio64::ONE,
    );
    let result = query_part(
        &explicit,
        span(0, 1, 1),
        &mut SongQueryCtx {
            vm: &mut query,
            cells: &cells,
            seed: 42,
            tempo: Tempo::default(),
            limits: &limits,
        },
    )
    .unwrap();
    assert_eq!(result.len(), 3);
    assert!(result.iter().all(|e| e.tone.is_some()));
    assert!(result
        .iter()
        .all(|e| matches!(e.event.controls.get(&intern_kw("n")), Some(Value::Int(3)))));
}

#[test]
fn adding_explicit_frequency_to_selected_mono_chord_keeps_independently_issued_inputs() {
    let base = capture(
        control(
            intern_kw("note"),
            Rc::new(scalar(Value::list(vec![
                Value::Float64(60.25),
                Value::Float64(60.25),
            ]))),
            Rc::new(scalar(audio("bd"))),
            None,
        ),
        Ratio64::ONE,
    );
    let mut vm = TestVm::default();
    let original = rows(&base, span(0, 1, 1), &mut vm);
    assert_eq!(original.len(), 2);
    let edited = build(&base, 8, &mut vm);
    let result = rows(&edited, span(0, 1, 1), &mut vm);
    assert_eq!(result.len(), 2);
    assert_eq!(
        result
            .iter()
            .map(|e| e.handle.tone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1])
    );
    for row in &result {
        assert!(matches!(
            row.event.controls.get(&intern_kw("freq")),
            Some(Value::Float64(440.0))
        ));
        assert_eq!(row.tone, None);
        let origin = &row.event.song_source.as_ref().unwrap().handle;
        assert!(original.iter().any(|e| &e.handle == origin));
    }
    let deleted = delete_event(&edited, &result[0].handle).unwrap();
    assert_eq!(rows(&deleted, span(0, 1, 1), &mut vm).len(), 1);
}

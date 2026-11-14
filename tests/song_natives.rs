//! Actual language/VM execution of immutable song construction and staging.
#![allow(dead_code)]
mod support;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use vactr::dsp::caps::CapabilitySet;
use vactr::host::caps::{Hosts, InstResolver};
use vactr::ns::evaluator::Evaluator;
use vactr::ns::load::NoopHost;
use vactr::ns::namespace::Prelude;
use vactr::ns::stage::{EffectSink, StagedEffect};
use vactr::pattern::{InputCells, TimeSpan};
use vactr::reader::span::FileId;
use vactr::sched::runtime::{Runtime, RuntimeConfig};
use vactr::song::{query_part, Part, PartNode, SongLimits, SongQueryCtx};
use vactr::value::intern::{intern_kw, intern_sym};
use vactr::value::value::Sound;
use vactr::value::{Key, Ratio64, Value};
use vactr::vm::{FailCode, Failure, VmQuery};

#[derive(Clone, Default)]
struct Sink(Rc<RefCell<Vec<StagedEffect>>>);
impl EffectSink for Sink {
    fn apply(&mut self, effect: StagedEffect) {
        self.0.borrow_mut().push(effect);
    }
}
fn evaluator() -> (Evaluator, Sink) {
    let sink = Sink::default();
    let ev = Evaluator::new(Prelude::core(), Box::new(NoopHost), Box::new(sink.clone()));
    sink.0.borrow_mut().clear();
    (ev, sink)
}
fn evaluate(ev: &mut Evaluator, text: &str) -> Result<Value, Failure> {
    let forms = support::eval::clean_forms(text, FileId::new(1));
    assert!(!forms.is_empty(), "empty fixture: {text}");
    let mut last = Value::Nil;
    for form in forms {
        last = ev.eval_form(&form).value?;
    }
    Ok(last)
}
fn invoke(
    ev: &mut Evaluator,
    name: &str,
    args: Vec<Value>,
    named: Vec<(&str, Value)>,
) -> Result<Value, Failure> {
    let function = ev.ns().prelude().slot(intern_sym(name)).unwrap().get();
    let (vm, ns) = ev.vm_and_ns();
    vm.call_value(
        ns,
        &function,
        args,
        named.into_iter().map(|(k, v)| (intern_kw(k), v)).collect(),
    )
}
fn captured(ev: &mut Evaluator) -> Value {
    evaluate(ev, "part [drums: {s [:bd :sd] > note 60.25}] duration: 2").unwrap()
}
fn part(value: &Value) -> &Rc<Part> {
    let Value::Part(p) = value else {
        panic!("expected Part, got {value}");
    };
    p
}
fn rows(ev: &mut Evaluator, value: &Value) -> Vec<vactr::song::SongEvent> {
    let p = part(value);
    let cells = InputCells::new();
    let limits = SongLimits::default();
    let (vm, ns) = ev.vm_and_ns();
    let mut vm = VmQuery::new(vm, ns);
    query_part(
        p,
        TimeSpan::new(Ratio64::ZERO, p.duration()).unwrap(),
        &mut SongQueryCtx {
            vm: &mut vm,
            cells: &cells,
            seed: 0,
            tempo: Default::default(),
            limits: &limits,
        },
    )
    .unwrap()
}
fn field(value: &Value, name: &str) -> Value {
    let Value::Dict(d) = value else {
        panic!("expected descriptor dictionary");
    };
    d[&Key::Kw(intern_kw(name))].clone()
}

#[test]
fn accepted_nested_generators_stage_exactly_twenty_four_cycles() {
    let text = "fn drums:\n\tpart [drums: {s [:bd :sd :bd :sd]} hats: {s :hh > euclid 7 8}] duration: 4\nfn developed:\n\tlet base {drums & []}\n\tbase > transform-instrument :drums :bd {p -> lpf p 900}\nlet intro {drums & []}\nlet verse {developed & []}\nlet arrangement sequence [{part-repeat intro 2} {part-repeat verse 4}]\nsong arrangement bpm: 120 cycle-beats: 4 meter: [4 4] seed: 42 tail-seconds: 8 > play-song";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    assert!(support::eval::check_errors(text, &forms).is_empty());
    let (mut ev, sink) = evaluator();
    let Value::Song(song) = evaluate(&mut ev, text).unwrap() else {
        panic!()
    };
    assert_eq!(song.duration(), Ratio64::from_int(24));
    assert_eq!(song.arrangement_seconds(), Ratio64::from_int(48));
    assert_eq!(song.deadline_seconds(), Ratio64::from_int(56));
    assert_eq!(song.settings().seed, 42);
    let effects = sink.0.borrow();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, StagedEffect::PlaySong(_)))
            .count(),
        1
    );
    assert!(!effects
        .iter()
        .any(|e| matches!(e, StagedEffect::SlotBind { .. } | StagedEffect::Install(_))));
}

#[test]
fn nested_function_enumerates_deletes_overwrites_and_returns_copy() {
    let text = "fn inner p:\n\tlet events {part-events p :drums 0 2}\n\tlet handle {{first events} :handle}\n\tlet changed {delete-event p handle}\n\toverwrite-region changed :drums 1 2 {s :hh}\nfn outer p:\n\tinner p\nlet original {part [drums: {s [:bd :sd] > note 60.25}] duration: 2}\nouter original";
    let (mut ev, sink) = evaluator();
    let changed = evaluate(&mut ev, text).unwrap();
    let original = ev.ns().session_slot(intern_sym("original")).unwrap().get();
    assert_eq!(rows(&mut ev, &original).len(), 4);
    let changed_rows = rows(&mut ev, &changed);
    assert_eq!(changed_rows.len(), 2);
    assert_eq!(
        changed_rows[0].handle.occurrence().onset,
        Ratio64::new(1, 2).unwrap()
    );
    assert_eq!(changed_rows[0].tone.unwrap().to_f64(), 60.25);
    assert_eq!(changed_rows[1].instrument, Sound::Builtin(intern_kw("hh")));
    assert!(!sink
        .0
        .borrow()
        .iter()
        .any(|e| matches!(e, StagedEffect::PlaySong(_) | StagedEffect::SlotBind { .. })));
}

#[test]
fn immutable_descriptors_preserve_fractional_notes_and_certified_handle_revision() {
    let (mut ev, _) = evaluator();
    let original = captured(&mut ev);
    let result = invoke(
        &mut ev,
        "part-events",
        vec![
            original.clone(),
            Value::kw("drums"),
            Value::Int(0),
            Value::Int(2),
        ],
        vec![],
    )
    .unwrap();
    let Value::List(list) = result else { panic!() };
    assert_eq!(list.items.len(), 4);
    assert_eq!(field(&list.items[0], "note").to_string(), "60.25");
    assert!(matches!(field(&list.items[0], "whole"), Value::List(_)));
    assert!(matches!(
        field(&list.items[0], "instrument"),
        Value::Sound(_)
    ));
    let handle = field(&list.items[0], "handle");
    assert!(matches!(&handle,Value::EventHandle(h) if h.revision()==part(&original).revision()));
    let changed = invoke(
        &mut ev,
        "delete-event",
        vec![original.clone(), handle.clone()],
        vec![],
    )
    .unwrap();
    assert_eq!(rows(&mut ev, &original).len(), 4);
    assert_eq!(rows(&mut ev, &changed).len(), 3);
    let error = invoke(&mut ev, "delete-event", vec![changed, handle], vec![]).unwrap_err();
    assert_eq!(error.code, FailCode::Type);
    assert!(error.message.contains("stale"));
}

#[test]
fn exact_sequences_zero_and_large_repeats_stay_symbolic() {
    let (mut ev, _) = evaluator();
    let original = captured(&mut ev);
    let zero = invoke(
        &mut ev,
        "part-repeat",
        vec![original.clone(), Value::Int(0)],
        vec![],
    )
    .unwrap();
    assert_eq!(part(&zero).duration(), Ratio64::ZERO);
    let empty = invoke(&mut ev, "sequence", vec![Value::list(vec![])], vec![]).unwrap();
    assert_eq!(part(&empty).duration(), Ratio64::ZERO);
    let large = invoke(
        &mut ev,
        "part-repeat",
        vec![original, Value::Int(1_000_000_000)],
        vec![("seed-mode", Value::kw("vary"))],
    )
    .unwrap();
    assert_eq!(part(&large).duration(), Ratio64::from_int(2_000_000_000));
    assert!(matches!(
        part(&large).node(),
        PartNode::Repeat {
            count: 1_000_000_000,
            ..
        }
    ));
    let joined = invoke(
        &mut ev,
        "sequence",
        vec![Value::list(vec![large, zero, empty])],
        vec![],
    )
    .unwrap();
    assert_eq!(part(&joined).duration(), Ratio64::from_int(2_000_000_000));
}

#[test]
fn dynamic_invalid_arguments_are_rejected_without_effects() {
    let (mut ev, sink) = evaluator();
    let p = captured(&mut ev);
    sink.0.borrow_mut().clear();
    for (name, args, named) in [
        ("part", vec![Value::dict(BTreeMap::new())], vec![]),
        (
            "part",
            vec![Value::dict(BTreeMap::new())],
            vec![("duration", Value::Int(0))],
        ),
        (
            "part-repeat",
            vec![p.clone(), Value::Ratio(Ratio64::new(1, 2).unwrap())],
            vec![],
        ),
        ("part-repeat", vec![p.clone(), Value::Int(-1)], vec![]),
        (
            "part-repeat",
            vec![p.clone(), Value::Int64(i64::from(u32::MAX) + 1)],
            vec![],
        ),
        (
            "part-repeat",
            vec![p.clone(), Value::Int(1)],
            vec![("seed-mode", Value::kw("random"))],
        ),
        ("sequence", vec![Value::list(vec![Value::Int(1)])], vec![]),
        (
            "replace-track",
            vec![p.clone(), Value::kw("missing"), Value::Int(1)],
            vec![],
        ),
        ("delete-event", vec![p.clone(), Value::Int(0)], vec![]),
        (
            "overwrite-region",
            vec![
                p.clone(),
                Value::kw("drums"),
                Value::Int(1),
                Value::Int(1),
                Value::Int(2),
            ],
            vec![],
        ),
        (
            "part-events",
            vec![p.clone(), Value::kw("drums"), Value::Int(-1), Value::Int(1)],
            vec![],
        ),
        (
            "part-events",
            vec![p.clone(), Value::kw("drums"), Value::Int(1), Value::Int(1)],
            vec![],
        ),
        (
            "song",
            vec![p.clone()],
            vec![("meter", Value::list(vec![Value::Int(4), Value::Int(3)]))],
        ),
        (
            "song",
            vec![p.clone()],
            vec![("tail-seconds", Value::Int(-1))],
        ),
        (
            "song",
            vec![p.clone()],
            vec![("bpm", Value::Float64(120.0))],
        ),
        ("play-song", vec![p], vec![]),
    ] {
        assert!(
            invoke(&mut ev, name, args, named).is_err(),
            "accepted {name}"
        );
    }
    assert!(sink.0.borrow().is_empty());
}

#[test]
fn structural_thunks_and_references_are_forced_once_in_nested_literals() {
    let (mut ev, _) = evaluator();
    let result=evaluate(&mut ev,"var instrument :bd\nfn track:\n\ts instrument\nlet p {part [drums: {{track & []}}] duration: {2}}\nsequence [{part-repeat p {2}} {part [drums: {s :sd}] duration: {1/2}}]").unwrap();
    assert_eq!(part(&result).duration(), Ratio64::new(9, 2).unwrap());
    assert_eq!(rows(&mut ev, &result).len(), 5);
}

#[test]
fn deeply_nested_structures_fail_explicitly_without_clone_fallback() {
    let (mut ev, _) = evaluator();
    let p = captured(&mut ev);
    let mut value = p;
    for _ in 0..258 {
        value = Value::list(vec![value]);
    }
    assert_eq!(
        invoke(&mut ev, "sequence", vec![value], vec![])
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
    let oversized = Value::list(vec![Value::Nil; 16_385]);
    assert_eq!(
        invoke(&mut ev, "sequence", vec![oversized], vec![])
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
}

#[test]
fn selected_callback_runs_once_is_pure_and_preserves_existing_output() {
    let (mut ev, _) = evaluator();
    let p = captured(&mut ev);
    let callback = evaluate(&mut ev, "{p -> lpf p 900}").unwrap();
    let changed = invoke(
        &mut ev,
        "transform-instrument",
        vec![p.clone(), Value::kw("drums"), Value::kw("bd"), callback],
        vec![],
    )
    .unwrap();
    let result = rows(&mut ev, &changed);
    for row in result {
        assert_eq!(
            row.event.controls.contains_key(&intern_kw("lpf")),
            row.instrument == Sound::Builtin(intern_kw("bd"))
        );
    }
    for text in [
        "fn callback p:\n\tprint :noise\n\tfirst [p]",
        "fn callback p:\n\tuse-bpm 90\n\tfirst [p]",
    ] {
        evaluate(&mut ev, text).unwrap();
        let callback = ev.ns().session_slot(intern_sym("callback")).unwrap().get();
        assert_eq!(
            invoke(
                &mut ev,
                "transform-instrument",
                vec![p.clone(), Value::kw("drums"), Value::kw("bd"), callback],
                vec![]
            )
            .unwrap_err()
            .code,
            FailCode::EffectInQuery
        );
    }
    assert!(ev.vm_mut().take_output().is_empty());
    let error = invoke(
        &mut ev,
        "transform-instrument",
        vec![p, Value::kw("drums"), Value::kw("bd"), Value::Int(7)],
        vec![],
    )
    .unwrap_err();
    assert_eq!(error.code, FailCode::Type);
}

#[test]
fn selector_freezes_complete_kit_bank_and_registry_fallback() {
    let (mut ev, _) = evaluator();
    let p=evaluate(&mut ev,"let sound-kit [family: [{default-sound-kit :bd} {default-sound-kit :sd}]]\npart [drums: {s :family > n [0 1]}] duration: 1").unwrap();
    let callback = evaluate(&mut ev, "{p -> gain p 0.25}").unwrap();
    let changed = invoke(
        &mut ev,
        "transform-instrument",
        vec![p.clone(), Value::kw("drums"), Value::kw("family"), callback],
        vec![],
    )
    .unwrap();
    let PartNode::Edit {
        edit: vactr::song::PartEdit::TransformInstrument { selector, .. },
        ..
    } = part(&changed).node()
    else {
        panic!()
    };
    assert_eq!(selector.family().len(), 2);
    let result = rows(&mut ev, &changed);
    assert_eq!(result.len(), 2);
    assert!(result
        .iter()
        .all(|row| row.event.controls.contains_key(&intern_kw("gain"))));
    // Frozen selector remains complete after a later kit replacement.
    evaluate(&mut ev, "let sound-kit [family: {default-sound-kit :hh}]").unwrap();
    assert_eq!(selector.family().len(), 2);
    let fallback=evaluate(&mut ev,"part [drums: {s :analog}] duration: 1 > transform-instrument :drums :analog {p -> gain p 0.5}").unwrap();
    assert_eq!(rows(&mut ev, &fallback).len(), 1);
}

#[test]
fn declared_fx_template_is_validated_and_stored_without_installing_branches() {
    let (mut ev, sink) = evaluator();
    let p = evaluate(
        &mut ev,
        "bus :selected:\n\troom mix: 0.5\npart [drums: {s :bd}] duration: 1",
    )
    .unwrap();
    sink.0.borrow_mut().clear();
    let changed = invoke(
        &mut ev,
        "instrument-fx",
        vec![
            p.clone(),
            Value::kw("drums"),
            Value::kw("bd"),
            Value::kw("selected"),
        ],
        vec![],
    )
    .unwrap();
    assert!(sink.0.borrow().is_empty());
    assert!(rows(&mut ev, &changed)
        .iter()
        .all(|row| row.route.as_ref().unwrap().template == intern_kw("selected")));
    assert_eq!(
        invoke(
            &mut ev,
            "instrument-fx",
            vec![p, Value::kw("drums"), Value::kw("bd"), Value::kw("missing")],
            vec![]
        )
        .unwrap_err()
        .code,
        FailCode::Type
    );
}

#[test]
fn failed_form_discards_playback_request_and_runtime_reports_unsupported() {
    let (mut ev, sink) = evaluator();
    assert!(evaluate(&mut ev,"let p {part [drums: {s :bd}] duration: 1}\nfn fail p:\n\tplay-song {song p}\n\t/ 1 0\nfail p").is_err());
    assert!(!sink
        .0
        .borrow()
        .iter()
        .any(|e| matches!(e, StagedEffect::PlaySong(_))));
    let value = evaluate(&mut ev, "song p > play-song").unwrap();
    let Value::Song(song) = value else { panic!() };
    let registry = ev.insts().unwrap();
    let resolver: Rc<dyn InstResolver> = Rc::new(registry);
    let (mut runtime, mut runtime_sink) = Runtime::new(
        Hosts::noop(),
        resolver,
        CapabilitySet::native(),
        RuntimeConfig::default(),
    );
    runtime_sink.apply(StagedEffect::PlaySong(Rc::clone(&song)));
    let report = runtime.drain(&mut ev);
    assert_eq!(report.faults.len(), 1);
    assert_eq!(report.faults[0].code, FailCode::BeyondCapability);
    assert!(Rc::ptr_eq(runtime.pending_song().unwrap(), &song));
    assert!(runtime.slots().iter().next().is_none());
}

#[test]
fn reactive_rebuild_never_replays_play_song() {
    let (mut ev, sink) = evaluator();
    evaluate(
        &mut ev,
        "var duration 1\nlet playing {song {part [drums: {s :bd}] duration: duration} > play-song}",
    )
    .unwrap();
    assert_eq!(
        sink.0
            .borrow()
            .iter()
            .filter(|e| matches!(e, StagedEffect::PlaySong(_)))
            .count(),
        1
    );
    assert!(ev.edges("playing").contains(&"duration".to_string()));
    let form = ev.form_of("playing").unwrap();
    assert!(ev.graph().get(form).unwrap().non_replayable);
    let runs = ev.runs("playing");
    sink.0.borrow_mut().clear();
    ev.queue_upd("duration", Value::Int(2)).unwrap();
    ev.run_pass();
    assert_eq!(ev.runs("playing"), runs);
    assert_eq!(
        ev.ns()
            .session_slot(intern_sym("duration"))
            .unwrap()
            .get()
            .to_string(),
        "2"
    );
    assert!(!sink
        .0
        .borrow()
        .iter()
        .any(|e| matches!(e, StagedEffect::PlaySong(_))));
    assert!(matches!(
        ev.form_state("playing"),
        Some(vactr::ns::depgraph::FormState::Clean)
    ));
}

#[test]
fn legacy_repeat_cat_and_sound_first_patterns_still_work() {
    let (mut ev, _) = evaluator();
    assert_eq!(
        evaluate(&mut ev, "repeat 4 3").unwrap().to_string(),
        "[4 4 4]"
    );
    let p = evaluate(&mut ev, "cat [{s :bd} {s :sd}]").unwrap();
    assert!(matches!(p, Value::Pattern(_)));
    assert_eq!(
        support::eval::query_cycle(&mut ev, &p, 0)
            .unwrap()
            .events
            .len(),
        1
    );
}

thread_local! {
    static CONSTRUCTION_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
fn counted_transform(
    _: &mut vactr::vm::call::NativeCx<'_>,
    a: &[Value],
    _: &[(vactr::value::intern::KwId, Value)],
) -> Result<Value, Failure> {
    CONSTRUCTION_CALLS.with(|n| n.set(n.get() + 1));
    Ok(a[0].clone())
}
fn counted_track(
    _: &mut vactr::vm::call::NativeCx<'_>,
    _: &[Value],
    _: &[(vactr::value::intern::KwId, Value)],
) -> Result<Value, Failure> {
    CONSTRUCTION_CALLS.with(|n| n.set(n.get() + 1));
    Ok(Value::Pattern(vactr::pattern::build::pattern_of(
        &Value::Sound(Rc::new(Sound::Builtin(intern_kw("bd")))),
        None,
    )?))
}
#[test]
fn real_vm_structural_thunk_and_transform_each_execute_once() {
    use vactr::types::natives::{NativeMask, NativeSig};
    let mut prelude = Prelude::core();
    prelude.register_custom(
        NativeSig::func(
            "counted-transform",
            1,
            1,
            &["fn any -> any"],
            &[NativeMask::Value],
        ),
        counted_transform,
    );
    prelude.register_custom(
        NativeSig::func("counted-track", 0, 0, &["fn -> (pattern any)"], &[]),
        counted_track,
    );
    let mut ev = Evaluator::new(prelude, Box::new(NoopHost), Box::new(Sink::default()));
    CONSTRUCTION_CALLS.with(|n| n.set(0));
    evaluate(&mut ev, "fn track:\n\tcounted-track & []").unwrap();
    let Value::Fn(track) = ev.ns().session_slot(intern_sym("track")).unwrap().get() else {
        panic!()
    };
    let tracks = Value::dict(BTreeMap::from([(
        Key::Kw(intern_kw("drums")),
        Value::Thunk(track),
    )]));
    let p = invoke(
        &mut ev,
        "part",
        vec![tracks],
        vec![("duration", Value::Int(2))],
    )
    .unwrap();
    assert_eq!(CONSTRUCTION_CALLS.with(|n| n.get()), 1);
    CONSTRUCTION_CALLS.with(|n| n.set(0));
    let callback = ev
        .ns()
        .prelude()
        .slot(intern_sym("counted-transform"))
        .unwrap()
        .get();
    let changed = invoke(
        &mut ev,
        "transform-instrument",
        vec![p, Value::kw("drums"), Value::kw("bd"), callback],
        vec![],
    )
    .unwrap();
    assert_eq!(CONSTRUCTION_CALLS.with(|n| n.get()), 1);
    for _ in 0..3 {
        assert_eq!(rows(&mut ev, &changed).len(), 2);
    }
    assert_eq!(CONSTRUCTION_CALLS.with(|n| n.get()), 1);
}

#[test]
fn nested_structural_thunks_reject_output_and_writes_restore_prior_output() {
    use vactr::vm::fail::Origin;
    let (mut ev, sink) = evaluator();
    evaluate(
        &mut ev,
        "var live 1\nfn write:\n\tupd live 2\n\ts :bd\nfn noise:\n\tprint :forbidden\n\ts :bd",
    )
    .unwrap();
    for function in ["write", "noise"] {
        sink.0.borrow_mut().clear();
        let Value::Fn(body) = ev.ns().session_slot(intern_sym(function)).unwrap().get() else {
            panic!()
        };
        let tracks = Value::dict(BTreeMap::from([(
            Key::Kw(intern_kw("drums")),
            Value::Thunk(body),
        )]));
        let error = invoke(
            &mut ev,
            "part",
            vec![tracks],
            vec![("duration", Value::Int(1))],
        )
        .unwrap_err();
        assert_eq!(error.code, FailCode::EffectInQuery);
        assert!(sink.0.borrow().is_empty());
        assert_eq!(
            ev.ns()
                .session_slot(intern_sym("live"))
                .unwrap()
                .get()
                .to_string(),
            "1"
        );
    }
    let p = captured(&mut ev);
    evaluate(&mut ev, "fn callback p:\n\tprint :forbidden\n\tfirst [p]").unwrap();
    let callback = ev.ns().session_slot(intern_sym("callback")).unwrap().get();
    ev.vm_mut()
        .put_output(vec![(Origin::none(), Rc::from("prior"))]);
    let error = invoke(
        &mut ev,
        "transform-instrument",
        vec![p, Value::kw("drums"), Value::kw("bd"), callback],
        vec![],
    )
    .unwrap_err();
    assert_eq!(error.code, FailCode::EffectInQuery);
    let output = ev.vm_mut().take_output();
    assert_eq!(output.len(), 1);
    assert_eq!(&*output[0].1, "prior");
}

#[test]
fn unrelated_kit_thunks_are_not_evaluated_by_selection() {
    let (mut ev, _) = evaluator();
    let p = captured(&mut ev);
    evaluate(
        &mut ev,
        "var sound-kit default-sound-kit\nfn broken:\n\t/ 1 0",
    )
    .unwrap();
    let Value::Fn(broken) = ev.ns().session_slot(intern_sym("broken")).unwrap().get() else {
        panic!()
    };
    ev.ns()
        .session_slot(intern_sym("sound-kit"))
        .unwrap()
        .set(Value::dict(BTreeMap::from([
            (
                Key::Kw(intern_kw("bd")),
                Value::Sound(Rc::new(Sound::Builtin(intern_kw("bd")))),
            ),
            (Key::Kw(intern_kw("broken")), Value::Thunk(broken)),
        ])));
    let callback = evaluate(&mut ev, "{p -> first [p]}").unwrap();
    let changed = invoke(
        &mut ev,
        "transform-instrument",
        vec![p, Value::kw("drums"), Value::kw("bd"), callback],
        vec![],
    )
    .unwrap();
    let PartNode::Edit {
        edit: vactr::song::PartEdit::TransformInstrument { selector, .. },
        ..
    } = part(&changed).node()
    else {
        panic!()
    };
    assert_eq!(selector.family(), &[Sound::Builtin(intern_kw("bd"))]);
}

#[test]
fn scheduled_argument_budget_bounds_nested_wide_pending_allocations() {
    let (mut ev, _) = evaluator();
    let mut value = Value::Nil;
    for _ in 0..4 {
        let mut items = vec![Value::Nil; 8_000];
        items[0] = value;
        value = Value::list(items);
    }
    assert_eq!(
        invoke(&mut ev, "sequence", vec![value], vec![])
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
}

#[test]
fn every_native_rejects_missing_arguments_and_unknown_named_arguments() {
    let (mut ev, _) = evaluator();
    for name in [
        "part",
        "part-repeat",
        "sequence",
        "replace-track",
        "transform-instrument",
        "part-events",
        "delete-event",
        "overwrite-region",
        "instrument-fx",
        "song",
        "play-song",
    ] {
        assert_eq!(
            invoke(&mut ev, name, vec![], vec![]).unwrap_err().code,
            FailCode::Arity,
            "{name}"
        );
    }
    let p = captured(&mut ev);
    assert_eq!(
        invoke(
            &mut ev,
            "song",
            vec![p],
            vec![("unknown-setting", Value::Int(1))]
        )
        .unwrap_err()
        .code,
        FailCode::Arity
    );
    assert_eq!(
        invoke(
            &mut ev,
            "part",
            vec![Value::dict(BTreeMap::new())],
            vec![("duraton", Value::Int(1))]
        )
        .unwrap_err()
        .code,
        FailCode::Arity
    );
}

#[test]
fn timed_silence_and_empty_arrangements_remain_valid_finite_songs() {
    let (mut ev, sink) = evaluator();
    let silent = invoke(
        &mut ev,
        "part",
        vec![Value::dict(BTreeMap::new())],
        vec![("duration", Value::Ratio(Ratio64::new(3, 2).unwrap()))],
    )
    .unwrap();
    assert!(rows(&mut ev, &silent).is_empty());
    let descriptor = invoke(
        &mut ev,
        "song",
        vec![silent],
        vec![
            ("bpm", Value::Int(126)),
            ("tail-seconds", Value::Int(0)),
            ("meter", Value::list(vec![Value::Int(7), Value::Int(8)])),
        ],
    )
    .unwrap();
    let Value::Song(song) = descriptor else {
        panic!()
    };
    assert_eq!(song.arrangement_seconds(), Ratio64::new(20, 7).unwrap());
    assert_eq!(song.deadline_seconds(), song.arrangement_seconds());
    let empty = invoke(&mut ev, "sequence", vec![Value::list(vec![])], vec![]).unwrap();
    let Value::Song(song) = invoke(&mut ev, "song", vec![empty], vec![]).unwrap() else {
        panic!()
    };
    assert_eq!(song.duration(), Ratio64::ZERO);
    assert_eq!(song.deadline_seconds(), Ratio64::from_int(8));
    assert!(!sink
        .0
        .borrow()
        .iter()
        .any(|e| matches!(e, StagedEffect::PlaySong(_))));
}

//! Route parity for mono song-note expansion; no enum-tag approximation.
use std::cell::RefCell;
use std::rc::Rc;
use vactr::dsp::graph::InstId;
use vactr::host::caps::{InstResolver, Route, SampleSrc};
use vactr::ns::evaluator::Evaluator;
use vactr::ns::insts::InstRegistry;
use vactr::ns::load::NoopHost;
use vactr::ns::namespace::{Namespace, Prelude, VarSlotRef};
use vactr::ns::stage::RecordingSink;
use vactr::pattern::eval::QueryVm;
use vactr::value::intern::intern_kw;
use vactr::value::sample::SampleBuf;
use vactr::value::value::{PathVal, Sound, Value};
use vactr::vm::fail::{FailCode, Failure, Origin};
use vactr::vm::{EffectMode, Vm, VmQuery};

fn registry() -> Rc<RefCell<InstRegistry>> {
    let evaluator = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    let registry = evaluator.insts().unwrap();
    assert!(registry.borrow().template_errors().is_empty());
    registry
}
fn classify(registry: Rc<RefCell<InstRegistry>>, sound: &Sound) -> Result<bool, Failure> {
    let mut vm = Vm::new();
    vm.dsp.registry = Some(registry);
    let namespace = Namespace::new(Prelude::empty());
    VmQuery::new(&mut vm, &namespace).song_sample_backed(sound)
}
#[test]
fn registered_builtin_synth_and_its_inst_are_not_sample_backed() {
    let registry = registry();
    let name = intern_kw("analog");
    let id = registry.borrow().id_of(name).unwrap();
    for sound in [Sound::Builtin(name), Sound::Inst(id)] {
        assert!(matches!(
            registry.borrow().route(&sound).unwrap(),
            Route::Audio { sample: None, .. }
        ));
        assert!(!classify(Rc::clone(&registry), &sound).unwrap());
    }
}
#[test]
fn resource_backed_inst_and_registered_builtin_use_the_actual_bank_route() {
    let registry = registry();
    let name = intern_kw("wavetable");
    let id = registry.borrow().id_of(name).unwrap();
    for sound in [Sound::Builtin(name), Sound::Inst(id)] {
        assert!(matches!(
            registry.borrow().route(&sound).unwrap(),
            Route::Audio {
                sample: Some(SampleSrc::Bank { .. }),
                ..
            }
        ));
        assert!(classify(Rc::clone(&registry), &sound).unwrap());
    }
}
#[test]
fn unregistered_builtin_bank_is_sample_backed() {
    let registry = registry();
    let sound = Sound::Builtin(intern_kw("external-break-bank"));
    assert!(registry
        .borrow()
        .id_of(intern_kw("external-break-bank"))
        .is_none());
    assert!(matches!(
        registry.borrow().route(&sound).unwrap(),
        Route::Audio {
            sample: Some(SampleSrc::Bank { .. }),
            ..
        }
    ));
    assert!(classify(registry, &sound).unwrap());
}
#[test]
fn path_resource_is_sample_backed_without_reading_disk() {
    let registry = registry();
    let sound = Sound::Sample(PathVal {
        text: Rc::from("./unread-break.wav"),
        file: None,
    });
    assert!(matches!(
        registry.borrow().route(&sound).unwrap(),
        Route::Audio {
            sample: Some(SampleSrc::Path(_)),
            ..
        }
    ));
    assert!(classify(registry, &sound).unwrap());
}
#[test]
fn buffer_resource_classification_does_not_read_pending_pcm() {
    let registry = registry();
    let buffer = SampleBuf::pending(0);
    let sound = Sound::Buffer(Rc::clone(&buffer));
    assert!(matches!(
        registry.borrow().route(&sound).unwrap(),
        Route::Audio {
            sample: Some(SampleSrc::Buffer { .. }),
            ..
        }
    ));
    assert!(classify(registry, &sound).unwrap());
    assert_eq!(
        buffer.ready_frames().unwrap_err().code,
        FailCode::CapturePending
    );
}
#[test]
fn unknown_inst_and_external_routes_fail_explicitly() {
    let registry = registry();
    assert_eq!(
        classify(Rc::clone(&registry), &Sound::Inst(InstId::new(u32::MAX)))
            .unwrap_err()
            .code,
        FailCode::UnknownSound
    );
    for sound in [Sound::MidiOut(0), Sound::Osc(Rc::from("/notes"))] {
        assert_eq!(
            classify(Rc::clone(&registry), &sound).unwrap_err().code,
            FailCode::BeyondCapability
        );
    }
}
#[test]
fn missing_concrete_resolver_does_not_guess_from_sound_tags() {
    let mut vm = Vm::new();
    let namespace = Namespace::new(Prelude::empty());
    let mut query = VmQuery::new(&mut vm, &namespace);
    for sound in [
        Sound::Builtin(intern_kw("analog")),
        Sound::Inst(InstId::new(0)),
        Sound::Sample(PathVal {
            text: Rc::from("./break.wav"),
            file: None,
        }),
    ] {
        assert_eq!(
            query.song_sample_backed(&sound).unwrap_err().code,
            FailCode::HostUnavailable
        );
    }
}
struct LegacyVm;
impl QueryVm for LegacyVm {
    fn call(&mut self, _: &Value, _: &[Value]) -> Result<Value, Failure> {
        Ok(Value::Nil)
    }
    fn deref(&mut self, slot: &VarSlotRef) -> Result<Value, Failure> {
        Ok(slot.get())
    }
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        vec![]
    }
    fn put_output(&mut self, _: Vec<(Origin, Rc<str>)>) {}
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        Ok(Value::dict(Default::default()))
    }
}
#[test]
fn default_legacy_capability_is_explicitly_unavailable() {
    let mut vm = LegacyVm;
    for sound in [
        Sound::Builtin(intern_kw("analog")),
        Sound::Inst(InstId::new(0)),
        Sound::MidiOut(0),
    ] {
        let error = vm.song_sample_backed(&sound).unwrap_err();
        assert_eq!(error.code, FailCode::HostUnavailable);
        assert!(error.message.contains("classification is unavailable"));
    }
    assert!(matches!(vm.call(&Value::Nil, &[]).unwrap(), Value::Nil));
}
#[test]
fn classification_leaves_vm_mode_fuel_effects_and_registry_unchanged() {
    let registry = registry();
    let initial_entries = registry.borrow().entries().count();
    let initial_graphs = registry.borrow().graphs().len();
    let mut vm = Vm::new();
    vm.dsp.registry = Some(Rc::clone(&registry));
    vm.set_fuel(1234);
    let namespace = Namespace::new(Prelude::empty());
    {
        let mut query = VmQuery::new(&mut vm, &namespace);
        assert!(!query
            .song_sample_backed(&Sound::Builtin(intern_kw("analog")))
            .unwrap());
        assert_eq!(
            query
                .song_sample_backed(&Sound::MidiOut(0))
                .unwrap_err()
                .code,
            FailCode::BeyondCapability
        );
    }
    assert_eq!(vm.fuel(), 1234);
    assert_eq!(vm.effect_mode(), EffectMode::Normal);
    assert!(vm.effects().is_empty());
    assert!(vm.take_output().is_empty());
    assert_eq!(registry.borrow().entries().count(), initial_entries);
    assert_eq!(registry.borrow().graphs().len(), initial_graphs);
    assert!(registry.try_borrow_mut().is_ok());
}

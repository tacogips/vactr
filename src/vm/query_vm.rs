//! The Query VM handle (design 7.1.3, 10.3, 10.4): `QueryVm` for the VM
//! against a session namespace.
//!
//! Every call runs in Query effect mode behind the scope guard of
//! `Vm::with_effect_mode` (restored also on failure), with a fresh fuel
//! budget of `fuel_limit` (1,000,000 by default, 7.1.5) that is given back
//! to the caller's budget afterwards. A `call` is one Rust-level re-entry
//! (`Vm::call_value`), so the re-entry limit applies across nested queries.

use std::rc::Rc;

use crate::host::caps::{InstResolver, Route};
use crate::ns::namespace::{Namespace, VarSlotRef};
use crate::pattern::eval::QueryVm;
use crate::value::intern::{intern_sym, KwId};
use crate::value::value::{Sound, Value};
use crate::vm::fail::{FailCode, Failure, Origin};
use crate::vm::vm::{EffectMode, Vm};

/// The VM and the session it queries against.
pub struct VmQuery<'a> {
    pub vm: &'a mut Vm,
    pub ns: &'a Namespace,
}

impl<'a> VmQuery<'a> {
    /// A handle over `vm` and the session namespace `ns`.
    pub fn new(vm: &'a mut Vm, ns: &'a Namespace) -> Self {
        Self { vm, ns }
    }

    /// Runs `f` in Query mode with a fresh fuel budget.
    fn guarded<T>(
        &mut self,
        f: impl FnOnce(&mut Vm, &Namespace) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if self.vm.song_work().is_some() {
            let ns = self.ns;
            return self.vm.with_effect_mode(EffectMode::Query, |vm| f(vm, ns));
        }
        let saved = self.vm.fuel();
        let budget = self.vm.fuel_limit;
        self.vm.set_fuel(budget);
        let ns = self.ns;
        let r = self.vm.with_effect_mode(EffectMode::Query, |vm| f(vm, ns));
        self.vm.set_fuel(saved);
        r
    }

    /// The value of a slot (an eager read, seen by the read observer).
    fn read(&mut self, slot: &VarSlotRef) -> Result<Value, Failure> {
        self.guarded(|vm, _| vm.read_slot(slot))
    }
}

impl QueryVm for VmQuery<'_> {
    fn call(&mut self, f: &Value, args: &[Value]) -> Result<Value, Failure> {
        if let Some(work) = self.vm.song_work() {
            work.borrow_mut().charge(args.len() as u64 + 1)?;
        }
        let args = args.to_vec();
        self.guarded(|vm, ns| vm.call_value(ns, f, args, Vec::new()))
    }

    fn deref(&mut self, r: &VarSlotRef) -> Result<Value, Failure> {
        self.read(r)
    }

    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        self.vm.take_output()
    }

    fn put_output(&mut self, out: Vec<(Origin, Rc<str>)>) {
        self.vm.put_output(out);
    }

    /// The SESSION-level `sound-kit` (a `fn`-local binding never reaches
    /// here), else the prelude's.
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        let name = intern_sym("sound-kit");
        let slot = self
            .ns
            .session_slot(name)
            .filter(VarSlotRef::is_bound)
            .or_else(|| self.ns.prelude().slot(name))
            .ok_or_else(|| Failure::new(FailCode::UndefinedName, "`sound-kit` is not bound"))?;
        self.read(&slot)
    }

    fn inst_sound(&mut self, k: KwId) -> Option<Value> {
        self.vm.dsp.registry.as_ref()?.borrow().sound(k)
    }

    fn song_sample_backed(&mut self, sound: &Sound) -> Result<bool, Failure> {
        if let Some(work) = self.vm.song_work() {
            work.borrow_mut().charge(1)?;
        }
        self.classify_song_resource(sound)
    }
}

impl VmQuery<'_> {
    fn classify_song_resource(&self, sound: &Sound) -> Result<bool, Failure> {
        let registry = self.vm.dsp.registry.as_ref().ok_or_else(|| {
            Failure::new(
                FailCode::HostUnavailable,
                "song instrument resolver is unavailable",
            )
        })?;
        match registry.borrow().route(sound)? {
            Route::Audio { sample, .. } => Ok(sample.is_some()),
            Route::Midi { .. } | Route::Osc { .. } => Err(Failure::new(
                FailCode::BeyondCapability,
                "song sources require an audio route",
            )),
        }
    }
}

/// Private original-snapshot adapter; VmQuery's public layout stays unchanged.
pub(crate) struct MeteredSongQuery<'a> {
    inner: VmQuery<'a>,
    work: crate::pattern::eval::song_observation::SharedIndexWork,
}
impl<'a> MeteredSongQuery<'a> {
    pub(crate) fn new(
        vm: &'a mut Vm,
        ns: &'a Namespace,
        work: crate::pattern::eval::song_observation::SharedIndexWork,
    ) -> Self {
        Self {
            inner: VmQuery::new(vm, ns),
            work,
        }
    }
    fn metered<T>(
        &mut self,
        f: impl FnOnce(&mut Vm, &Namespace) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        self.work.borrow_mut().charge(1)?;
        let ns = self.inner.ns;
        self.inner.vm.with_song_work(self.work.clone(), |vm| {
            vm.with_effect_mode(EffectMode::Query, |vm| f(vm, ns))
        })
    }
}
impl QueryVm for MeteredSongQuery<'_> {
    fn call(&mut self, callable: &Value, args: &[Value]) -> Result<Value, Failure> {
        self.work.borrow_mut().charge(args.len() as u64 + 1)?;
        let arguments = args.to_vec();
        let result = self.metered(|vm, ns| vm.call_value(ns, callable, arguments, Vec::new()))?;
        self.work
            .borrow_mut()
            .retain_call(callable, args, &result)?;
        Ok(result)
    }
    fn deref(&mut self, slot: &VarSlotRef) -> Result<Value, Failure> {
        let result = self.metered(|vm, _| vm.read_slot(slot))?;
        self.work
            .borrow_mut()
            .retain_call(&Value::VarRef(slot.clone()), &[], &result)?;
        Ok(result)
    }
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        self.inner.take_output()
    }
    fn put_output(&mut self, output: Vec<(Origin, Rc<str>)>) {
        self.inner.put_output(output);
    }
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        let name = intern_sym("sound-kit");
        let slot = self
            .inner
            .ns
            .session_slot(name)
            .filter(VarSlotRef::is_bound)
            .or_else(|| self.inner.ns.prelude().slot(name))
            .ok_or_else(|| Failure::new(FailCode::UndefinedName, "`sound-kit` is not bound"))?;
        self.deref(&slot)
    }
    fn inst_sound(&mut self, name: KwId) -> Option<Value> {
        self.inner.inst_sound(name)
    }
    fn song_sample_backed(&mut self, sound: &Sound) -> Result<bool, Failure> {
        self.work.borrow_mut().charge(1)?;
        self.inner.song_sample_backed(sound)
    }
}

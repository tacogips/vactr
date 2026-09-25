//! A stub `QueryVm` for pattern tests: callables are `Value::Native(id)`
//! entries backed by Rust closures, and the sound kit is a swappable dict.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ns::namespace::VarSlotRef;
use crate::pattern::eval::QueryVm;
use crate::value::intern::intern_kw;
use crate::value::key::Key;
use crate::value::value::{NativeId, Sound, Value};
use crate::vm::fail::{FailCode, Failure, Origin};

type Callable = Box<dyn FnMut(&[Value]) -> Result<Value, Failure>>;

pub(crate) struct StubVm {
    pub kit: Value,
    pub kit_calls: usize,
    pub calls: usize,
    pub output: Vec<(Origin, Rc<str>)>,
    funcs: BTreeMap<u32, Callable>,
}

impl StubVm {
    pub fn new() -> Self {
        Self {
            kit: builtin_kit(&[
                "bd", "sd", "sn", "hh", "cp", "pluck", "piano", "break", "a", "b", "c", "d",
            ]),
            kit_calls: 0,
            calls: 0,
            output: Vec::new(),
            funcs: BTreeMap::new(),
        }
    }

    /// Registers a callable and returns its value.
    pub fn define(
        &mut self,
        id: u32,
        f: impl FnMut(&[Value]) -> Result<Value, Failure> + 'static,
    ) -> Value {
        self.funcs.insert(id, Box::new(f));
        Value::Native(NativeId::new(id))
    }
}

impl QueryVm for StubVm {
    fn call(&mut self, f: &Value, args: &[Value]) -> Result<Value, Failure> {
        self.calls += 1;
        let Value::Native(id) = f else {
            return Err(Failure::new(FailCode::NotCallable, "stub: not callable"));
        };
        let func = self
            .funcs
            .get_mut(&id.get())
            .ok_or_else(|| Failure::new(FailCode::NotCallable, "stub: unknown native"))?;
        func(args)
    }

    fn deref(&mut self, _r: &VarSlotRef) -> Result<Value, Failure> {
        Err(Failure::new(FailCode::UndefinedName, "stub: no var slots"))
    }

    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        std::mem::take(&mut self.output)
    }

    fn sound_kit(&mut self) -> Result<Value, Failure> {
        self.kit_calls += 1;
        Ok(self.kit.clone())
    }
}

/// A builtin sound value.
pub(crate) fn builtin(name: &str) -> Value {
    Value::Sound(Rc::new(Sound::Builtin(intern_kw(name))))
}

/// A kit mapping each name to its builtin sound.
pub(crate) fn builtin_kit(names: &[&str]) -> Value {
    let map: BTreeMap<Key, Value> = names
        .iter()
        .map(|n| (Key::Kw(intern_kw(n)), builtin(n)))
        .collect();
    Value::dict(map)
}

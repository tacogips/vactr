//! Sound natives (design 7.1.4 "Sounds and the sound kit", 10.1): `s` and
//! its alias `sound`, `sample`, `midi`, and the prelude values
//! `default-sound-kit` and `sound-kit`.
//!
//! `s` stores its source as written and resolves keywords per query: in
//! the `kit:` value when given, else in `QueryVm::sound_kit()` (the
//! session's `sound-kit`, or the prelude's), so a new session binding is
//! heard from the next event on. A second positional argument never
//! reaches here: `n [0 3] > s :bd` is `(s (n [0 3]) :bd)`, an `arity`
//! failure at the call (the checker reports `sound-not-first`).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ns::namespace::Prelude;
use crate::pattern::build::{midi_channel, param_of};
use crate::pattern::combinators::sound::sound as sound_node;
use crate::types::manifest::HostManifest;
use crate::types::ty::KeySet;
use crate::value::intern::{intern_kw, KwId};
use crate::value::key::Key;
use crate::value::value::{Sound, Value};
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::Failure;
use crate::vm::natives::pattern::{named, out};
use crate::vm::natives::{arg, int_of, type_err};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("s", s);
    p.register("sound", s);
    p.register("sample", sample);
    p.register("midi", midi);
    let kit = default_sound_kit();
    p.register_value("default-sound-kit", kit.clone());
    p.register_value("sound-kit", kit);
    p.register_value("digital-kit", digital_kit());
}

/// The builtin kit: one `Sound::Builtin(k)` per key of the host manifest's
/// builtin sound set (sounds and synth templates, 7.1.4).
#[must_use]
pub fn default_sound_kit() -> Value {
    let mut kit = BTreeMap::new();
    if let KeySet::Of(keys) = HostManifest::spec_default().sound_kit_keys() {
        for k in keys {
            let kw = intern_kw(&k);
            kit.insert(Key::Kw(kw), Value::Sound(Rc::new(Sound::Builtin(kw))));
        }
    }
    Value::dict(kit)
}

/// `digital-kit` (design-music 4.1, DDRUM-006): a prelude dict of pure
/// aliases from ordinary sound keywords to the four digital-drum-family
/// templates, usable as `s :bd kit: digital-kit`. Each alias is a
/// `Sound::Builtin` of the template's own name, resolved by the instrument
/// registry exactly the way a direct `s :digital-drum` is (12.8.6), so the
/// alias carries no preset control values of its own.
#[must_use]
pub fn digital_kit() -> Value {
    let mut kit = BTreeMap::new();
    for (alias, template) in [
        ("bd", "digital-drum"),
        ("sd", "digital-snare"),
        ("cy", "digital-metal"),
        ("hh", "digital-hat"),
    ] {
        kit.insert(
            Key::Kw(intern_kw(alias)),
            Value::Sound(Rc::new(Sound::Builtin(intern_kw(template)))),
        );
    }
    Value::dict(kit)
}

/// `s src kit: k`: one positional source and the optional `kit:`.
fn s(cx: &mut NativeCx<'_>, a: &[Value], kw: Kw<'_>) -> R {
    let src = param_of(&arg(a, 0))?;
    let kit = match named(kw, "kit") {
        Some(v) => Some(param_of(v)?),
        None => None,
    };
    out(cx, sound_node(src, kit, None))
}

/// `sample ./x.wav`: a sample sound. No I/O here (TASK-008's
/// `SampleLoader` reads the file).
fn sample(_: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    match arg(a, 0) {
        Value::Path(p) => Ok(Value::Sound(Rc::new(Sound::Sample((*p).clone())))),
        other => Err(type_err(format!(
            "`sample` expects a path, got {}",
            kind_name(&other)
        ))),
    }
}

/// `midi 1`: MIDI out on a channel `1..16`, an instrument.
fn midi(_: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let ch = midi_channel(int_of(&arg(a, 0), "`midi`")?)?;
    Ok(Value::Sound(Rc::new(Sound::MidiOut(ch))))
}

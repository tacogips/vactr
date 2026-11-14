//! Name-level access to the effect catalog (design 12.5, 12.8.8): building
//! an `EffectSpec` from named parameters, as lowering and tests do.

use crate::dsp::graph::{EffectKind, EffectSpec};
use crate::host::wire::Ctl;

use super::{param_ctl, params, ParamDef};

/// A parameter name the kind does not have.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UnknownParam(pub &'static str);

/// The parameter definition of `name` on `kind`.
#[must_use]
pub fn param(kind: EffectKind, name: &str) -> Option<&'static ParamDef> {
    params(kind).iter().find(|p| p.name == name)
}

/// An `EffectSpec` of `kind` with the given named parameters (effect-local
/// ids, `param_ctl`).
///
/// # Errors
/// `UnknownParam` for a name the kind does not have.
pub fn spec(kind: EffectKind, named: &[(&'static str, Ctl)]) -> Result<EffectSpec, UnknownParam> {
    let mut out = Vec::with_capacity(named.len());
    for &(name, ctl) in named {
        let id = param_ctl(kind, name).ok_or(UnknownParam(name))?;
        out.push((id, ctl));
    }
    Ok(EffectSpec {
        kind,
        params: out.into_boxed_slice(),
    })
}

/// An `EffectSpec` of `kind` with every parameter at its default.
#[must_use]
pub fn default_spec(kind: EffectKind) -> EffectSpec {
    EffectSpec {
        kind,
        params: Box::new([]),
    }
}

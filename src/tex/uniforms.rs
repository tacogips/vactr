//! Per-frame uniform resolution (design 9.2-9.3). TASK-006.
//!
//! `UniformPlan` stays on the evaluator side, attached to the slot's
//! evaluator state; it never crosses to the render thread (9.2). Each
//! render tick the evaluator resolves every `UniformSpec` to `f32` and
//! ships only the resulting values through `RenderHost::set_uniforms`
//! (9.3) — that render-side value type is `Uniforms`.

use crate::pattern::eval::{num_f64, value_at, QState, QueryCtx};
use crate::tex::texnode::VParam;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// The evaluator-owned plan for one compiled shader's uniforms (design
/// 9.2): never crosses the render boundary.
#[derive(Clone, Debug)]
pub struct UniformPlan {
    pub specs: Box<[UniformSpec]>,
}

/// One uniform's name (matches `ShaderDesc::uniform_names`) and its
/// (evaluator-owned) value source.
#[derive(Clone, Debug)]
pub struct UniformSpec {
    pub name: String,
    pub src: VParam,
}

/// Render-safe per-frame uniform values only (design 9.2): the render
/// thread receives values and never runs closures.
#[derive(Clone, Debug, PartialEq)]
pub struct Uniforms {
    pub values: Box<[f32]>,
}

impl Uniforms {
    /// The values of a resolved `(name, value)` list, in order.
    #[must_use]
    pub fn from_resolved(resolved: &[(String, f32)]) -> Uniforms {
        Uniforms {
            values: resolved.iter().map(|(_, v)| *v).collect(),
        }
    }
}

/// Resolves every uniform in `plan` at `frame_time` (design 9.3). Each
/// spec's source is forced EXACTLY ONCE per call: a `Late` var/tweak is
/// dereffed once, a `Fn`/`Native` is called once with the frame time, a
/// `Thunk` is called once with no arguments (5.5 "osc is all-Late").
///
/// # Errors
/// The first spec whose value is not a finite number fails the whole
/// frame (the caller keeps the previous `Uniforms`); any deref/call
/// failure propagates the same way.
pub fn resolve_uniforms(
    plan: &UniformPlan,
    frame_time: Ratio64,
    cx: &mut QueryCtx<'_>,
) -> Result<Vec<(String, f32)>, Failure> {
    let mut st = QState::new(cx);
    let mut out = Vec::with_capacity(plan.specs.len());
    for spec in plan.specs.iter() {
        let v = resolve_one(&spec.src, frame_time, &mut st)?;
        let x = num_f64(&v)
            .ok_or_else(|| Failure::new(FailCode::Type, "a uniform must resolve to a number"))?;
        if !x.is_finite() {
            return Err(Failure::new(
                FailCode::Type,
                "a uniform must resolve to a finite number",
            ));
        }
        out.push((spec.name.clone(), x as f32));
    }
    Ok(out)
}

fn resolve_one(src: &VParam, t: Ratio64, st: &mut QState<'_, '_>) -> Result<Value, Failure> {
    match src {
        VParam::Const(x) => Ok(Value::Float64(f64::from(*x))),
        VParam::Late(r) => {
            let v = st.cx.vm.deref(r)?;
            value_at(&v, t, st)
        }
        VParam::Sig(s) => s.value_at(t, st.cx),
        VParam::Pat(p) => st.sample_value(p, t),
        VParam::Fn(f) => value_at(f, t, st),
    }
}

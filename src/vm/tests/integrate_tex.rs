//! INTEGRATE: visual chains through the whole pipeline (design 5.5, 9.1-9.3).
//!
//! The TASK-005 forcing case "a time-varying visual argument stays deferred
//! via its `Late` mask": `osc {* 20 {count-sin time}}` through the REAL
//! registered `osc` native, with a counting native standing in for `sin`
//! inside the thunk. The thunk is not forced at the `osc` call; each
//! `resolve_uniforms` forces it exactly once.

use std::cell::Cell;
use std::rc::Rc;

use crate::ns::namespace::Prelude;
use crate::ns::stage::SlotKey;
use crate::pattern::eval::QueryCtx;
use crate::tex::shader::compile_tex;
use crate::tex::texnode::{TexKind, VParam};
use crate::tex::uniforms::{resolve_uniforms, UniformSpec};
use crate::types::masks::MaskEntry;
use crate::types::natives::{NativeMask, NativeSig, NativeTable};
use crate::value::intern::{intern_kw, intern_sym, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::call::NativeCx;
use crate::vm::fail::{FailCode, Failure};
use crate::vm::query_vm::VmQuery;
use crate::vm::tests::integrate_query::Ev;

thread_local! {
    static CALLS: Cell<u32> = const { Cell::new(0) };
}

/// Counts its calls and returns the count (a value that changes per call).
fn count_sin(_: &mut NativeCx<'_>, _: &[Value], _: &[(KwId, Value)]) -> Result<Value, Failure> {
    let n = CALLS.with(|c| {
        c.set(c.get() + 1);
        c.get()
    });
    Ok(Value::Float64(f64::from(n)))
}

fn calls() -> u32 {
    CALLS.with(Cell::get)
}

fn counting_prelude() -> Prelude {
    let mut p = Prelude::core();
    p.register_custom(
        NativeSig::func(
            "count-sin",
            1,
            1,
            &["fn any -> float"],
            &[NativeMask::Value],
        ),
        count_sin,
    );
    p
}

fn out_o0(h: &Ev) -> Option<Value> {
    h.bound(SlotKey::Named(intern_kw("o0")))
}

/// Resolves every uniform of the plan of `tex` at `t`.
fn resolve(h: &mut Ev, specs: &[UniformSpec], t: Ratio64) -> Vec<f32> {
    let plan = crate::tex::uniforms::UniformPlan {
        specs: specs.to_vec().into_boxed_slice(),
    };
    let (vm, ns) = h.ev.vm_and_ns();
    let mut handle = VmQuery::new(vm, ns);
    let mut cx = QueryCtx::new(&mut handle, &h.cells, 1);
    resolve_uniforms(&plan, t, &mut cx)
        .expect("resolves")
        .into_iter()
        .map(|(_, v)| v)
        .collect()
}

#[test]
fn a_time_varying_visual_argument_stays_deferred_via_its_late_mask() {
    CALLS.with(|c| c.set(0));
    let mut h = Ev::with_prelude(counting_prelude());
    let out = h.run("osc {* 20 {count-sin time}} > out o0");
    assert!(out[0].value.is_ok(), "{:?}", out[0].value);
    assert_eq!(calls(), 0, "not forced at the `osc` call boundary");
    let Some(Value::Tex(t)) = out_o0(&h) else {
        panic!("o0 is bound to a texture");
    };
    assert!(matches!(t.kind, TexKind::Osc));
    assert!(
        matches!(t.params.first(), Some(VParam::Fn(Value::Thunk(_)))),
        "{:?}",
        t.params
    );
    let (_, plan) = compile_tex(&t).expect("compiles");
    let a = resolve(&mut h, &plan.specs, Ratio64::from_int(1));
    assert_eq!(calls(), 1, "forced once per resolution");
    let b = resolve(&mut h, &plan.specs, Ratio64::from_int(2));
    assert_eq!(calls(), 2, "forced once per resolution");
    assert_ne!(a, b, "two frame times give two uniform values");
    assert!(a.contains(&20.0) && b.contains(&40.0), "{a:?} {b:?}");
}

#[test]
fn the_registered_osc_mask_is_its_table_entry_all_late() {
    let h = Ev::new();
    let slot = h.ev.ns().prelude().slot(intern_sym("osc")).expect("osc");
    let Value::Native(id) = slot.get() else {
        panic!("a native");
    };
    let entry = h.ev.ns().prelude().native(id).expect("registered");
    let (tid, sig) = NativeTable::global().get("osc").expect("entry");
    assert_eq!(id, tid);
    assert_eq!(entry.sig.forcing_mask(), sig.forcing_mask());
    assert!(entry
        .sig
        .forcing_mask()
        .0
        .iter()
        .all(|m| *m == MaskEntry::Late));
    assert_eq!(entry.sig.forcing_mask().0.len(), 3);
}

#[test]
fn a_failing_visual_chain_keeps_the_previous_binding() {
    let mut h = Ev::new();
    let ok = h.run("osc 10 > out o0");
    assert!(ok[0].value.is_ok(), "{:?}", ok[0].value);
    let before = out_o0(&h).expect("o0");
    let binds = h.bind_count();
    // A string is not a visual parameter: the chain fails before `out`.
    let bad = h.run("osc 10 > rotate \"x\" > out o0");
    assert_eq!(
        bad[0].value.as_ref().expect_err("fails").code,
        FailCode::Type
    );
    // An unknown output fails in `out` itself, before anything is staged.
    let bad = h.run("osc 10 > rotate 1 > out :o9");
    assert_eq!(
        bad[0].value.as_ref().expect_err("fails").code,
        FailCode::Type
    );
    assert_eq!(h.bind_count(), binds, "nothing staged");
    let (Value::Tex(a), Value::Tex(b)) = (&before, &out_o0(&h).expect("o0")) else {
        panic!("textures");
    };
    assert!(Rc::ptr_eq(a, b), "the previous binding stays");
}

#[test]
fn visual_operators_build_chains() {
    let mut h = Ev::new();
    let v = h
        .last("osc 20 0.1 0.8 > rotate 0.5 > modulate {noise 3} > blend {shape 4} 0.5 > kaleid 4")
        .expect("texture");
    let Value::Tex(t) = v else {
        panic!("a texture");
    };
    let (desc, plan) = compile_tex(&t).expect("compiles");
    assert!(!desc.source.is_empty());
    assert!(!plan.specs.is_empty());
    assert!(matches!(
        h.last("text \"hello\" > scale 0.5 > out o1"),
        Ok(Value::Tex(_))
    ));
    assert!(matches!(
        h.last("add {osc 1} {osc 2} 0.5"),
        Ok(Value::Tex(_))
    ));
    assert_eq!(h.last("add 1 2").expect("number").to_string(), "3");
    let p = h.last("add [0 3] 7").expect("pattern");
    assert_eq!(h.values(&p, 0), ["7", "10"]);
    let p = h.last("sub [10 3] 1").expect("pattern");
    assert_eq!(h.values(&p, 0), ["9", "2"]);
    for src in ["render o1", "use-fps 30", "use-canvas 800 600"] {
        assert_eq!(h.last(src).expect(src).to_string(), "nil");
    }
}

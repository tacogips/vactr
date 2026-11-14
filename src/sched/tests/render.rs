//! G5 per-frame uniforms (design 9.2-9.3, ED-WIRE item 4): `Runtime` keeps
//! the `UniformPlan` `compile_tex` returns per output, and
//! `Runtime::render_frame` resolves it every frame through
//! `RenderHost::set_uniforms`. Drives a real `Evaluator` + `Runtime` with
//! the recording hosts, as `sched.rs`'s own tests do (design 12.8.2).

use super::sched::Rig;
use crate::host::testing::RenderCall;
use crate::tex::texnode::OutId;

const O0: OutId = OutId::new(0);
const O1: OutId = OutId::new(1);

/// Every `SetProgram` call so far.
fn programs(rig: &Rig) -> Vec<OutId> {
    rig.render
        .calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            RenderCall::SetProgram(o, _) => Some(o),
            RenderCall::SetUniforms(..) => None,
        })
        .collect()
}

/// Every `SetUniforms` call so far, with its output and values.
fn uniforms(rig: &Rig) -> Vec<(OutId, Vec<f32>)> {
    rig.render
        .calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            RenderCall::SetUniforms(o, u) => Some((o, u.values.into_vec())),
            RenderCall::SetProgram(..) => None,
        })
        .collect()
}

#[test]
fn render_frame_resolves_the_stored_plan_after_activation_and_clears_it_after_hush() {
    let mut rig = Rig::new();

    // No program before the cycle boundary: `run` only drains, it never
    // ticks, so `activate` has not run yet.
    rig.run("osc 20 > rotate 0.5 > out o0");
    assert!(programs(&rig).is_empty(), "nothing activated before a tick");
    assert!(uniforms(&rig).is_empty());

    // One program after the boundary: the very first tick activates a
    // binding staged before any tick (its boundary is cycle 0).
    rig.run_to(0.0);
    assert_eq!(programs(&rig), vec![O0], "o0 activated once");

    // Every visual-chain numeric parameter is `Late` (design 5.5 "osc is
    // all-Late"), so even the literal `0.5`/`20` arrive as uniforms
    // (`rotate`'s angle, then `osc`'s frequency, in codegen order): one
    // `set_uniforms` call, one value per `uniform_names` entry.
    let faults = rig.rt.render_frame(&mut rig.ev, 0.0);
    assert!(faults.is_empty(), "{faults:?}");
    let u = uniforms(&rig);
    assert_eq!(u.len(), 1);
    assert_eq!(u[0], (O0, vec![0.5, 20.0]));
    // A second frame at a different host time resolves the same literal
    // values again: forced, not folded, but not time-varying either.
    rig.rt.render_frame(&mut rig.ev, 0.4);
    assert_eq!(uniforms(&rig).last(), Some(&(O0, vec![0.5, 20.0])));

    // A genuine time-varying (signal) uniform on a second output: `time` is
    // the prelude's `Sig::Time` value, so `rotate time` resolves to a
    // different `f32` at each frame time, unlike the literal parameters
    // above.
    rig.run("osc 20 > rotate time > out o1");
    rig.run_to(0.01);
    assert_eq!(programs(&rig), vec![O0, O1], "o1 activated too");

    let f1 = rig.rt.render_frame(&mut rig.ev, 0.01);
    assert!(f1.is_empty(), "{f1:?}");
    let f2 = rig.rt.render_frame(&mut rig.ev, 0.5);
    assert!(f2.is_empty(), "{f2:?}");
    let o1_values: Vec<f32> = uniforms(&rig)
        .into_iter()
        .filter(|(o, _)| *o == O1)
        .map(|(_, v)| {
            // Codegen order: `rotate`'s angle, then `osc`'s frequency; the
            // frequency literal `20` is late too (every visual-chain
            // numeric parameter is), so it is also a uniform, but it does
            // not vary across frames the way `time` does.
            assert_eq!(v.len(), 2, "rotate's angle and osc's frequency");
            v[0]
        })
        .collect();
    assert_eq!(o1_values.len(), 2);
    assert_ne!(
        o1_values[0], o1_values[1],
        "two frame times give two uniform values: {o1_values:?}"
    );

    // After `hush`, the outputs are unbound (control.rs already sent the
    // empty program): render_frame sends no uniforms for either.
    rig.run("hush");
    let before = rig.render.calls().len();
    let f3 = rig.rt.render_frame(&mut rig.ev, 0.6);
    assert!(f3.is_empty(), "{f3:?}");
    assert_eq!(
        rig.render.calls().len(),
        before,
        "no uniforms are sent once every output is unbound"
    );
}

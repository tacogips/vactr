//! Visual chain tests (TASK-006, design 9).

mod goldens;

use std::cell::Cell;
use std::rc::Rc;

use crate::pattern::build::value_steps;
use crate::pattern::eval::{InputCells, QueryCtx};
use crate::pattern::signal::Sig;
use crate::pattern::tests::StubVm;
use crate::tex::shader::{compile_tex, ShaderDesc, TextAsset};
use crate::tex::texnode::{pipe, BlendOp, ModKind, TexKind, TexNode, VParam};
use crate::tex::uniforms::resolve_uniforms;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).unwrap()
}

fn osc(params: Vec<VParam>) -> TexNode {
    TexNode::new(TexKind::Osc, params, None)
}

fn noise(params: Vec<VParam>) -> TexNode {
    TexNode::new(TexKind::Noise, params, None)
}

// ---------------------------------------------------------------------
// (c) uniforms resolve per frame from the UniformPlan.
// ---------------------------------------------------------------------

#[test]
fn fn_uniform_is_called_exactly_once_per_resolve_and_varies_with_time() {
    let mut vm = StubVm::new();
    let calls = Rc::new(Cell::new(0u32));
    let calls_inner = Rc::clone(&calls);
    let f = vm.define(1, move |args| {
        calls_inner.set(calls_inner.get() + 1);
        let t = match args.first() {
            Some(Value::Ratio(r)) => r.to_f64(),
            other => panic!("expected a ratio argument, got {other:?}"),
        };
        Ok(Value::Float64(t * 20.0))
    });
    let node = pipe(
        Rc::new(osc(vec![VParam::Fn(f)])),
        TexNode::new(TexKind::Rotate, Vec::new(), None),
        None,
    );
    let (_, plan) = compile_tex(&node).unwrap();
    assert_eq!(plan.specs.len(), 1);

    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, 42);
    let out0 = resolve_uniforms(&plan, r(0, 1), &mut cx).unwrap();
    let out1 = resolve_uniforms(&plan, r(1, 4), &mut cx).unwrap();

    assert_eq!(
        calls.get(),
        2,
        "the thunk must be forced exactly once per resolve_uniforms call"
    );
    assert_ne!(
        out0[0].1, out1[0].1,
        "two frame times must give different resolved values"
    );
}

#[test]
fn sig_uniform_resolves_sine_at_two_frame_times() {
    let node = osc(vec![VParam::Sig(Rc::new(Sig::Sine))]);
    let (_, plan) = compile_tex(&node).unwrap();
    let mut vm = StubVm::new();
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, 42);

    let out0 = resolve_uniforms(&plan, r(0, 1), &mut cx).unwrap();
    let out1 = resolve_uniforms(&plan, r(1, 4), &mut cx).unwrap();

    assert!((out0[0].1 - 0.5).abs() < 1e-6);
    assert!((out1[0].1 - 1.0).abs() < 1e-6);
}

#[test]
fn pat_uniform_resolves_steps_at_two_points() {
    let steps = value_steps(&[Value::Int(1), Value::Int(2)]);
    let node = osc(vec![VParam::Pat(Rc::new(steps))]);
    let (_, plan) = compile_tex(&node).unwrap();
    let mut vm = StubVm::new();
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, 42);

    let out0 = resolve_uniforms(&plan, r(0, 1), &mut cx).unwrap();
    let out1 = resolve_uniforms(&plan, r(1, 2), &mut cx).unwrap();

    assert!((out0[0].1 - 1.0).abs() < 1e-6);
    assert!((out1[0].1 - 2.0).abs() < 1e-6);
}

#[test]
fn non_number_uniform_result_is_a_type_error() {
    let mut vm = StubVm::new();
    let f = vm.define(2, |_args| Ok(Value::str("not a number")));
    let node = osc(vec![VParam::Fn(f)]);
    let (_, plan) = compile_tex(&node).unwrap();
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, 42);

    let err = resolve_uniforms(&plan, r(0, 1), &mut cx).unwrap_err();
    assert_eq!(err.code, FailCode::Type);
}

// ---------------------------------------------------------------------
// (d) structural render-safety: ShaderDesc/TextAsset are plain data.
// ---------------------------------------------------------------------

fn assert_send_static<T: Send + 'static>() {}

#[test]
fn shader_desc_and_text_asset_are_render_safe_plain_data() {
    assert_send_static::<ShaderDesc>();
    assert_send_static::<TextAsset>();

    let asset = TextAsset {
        id: 0,
        text: "hello".to_string(),
    };
    let asset2 = asset.clone();
    assert_eq!(asset, asset2);
    assert_eq!(format!("{asset:?}"), format!("{asset2:?}"));

    let desc = ShaderDesc {
        source: "x".to_string(),
        uniform_names: Box::new([]),
        assets: Box::new([asset]),
    };
    let desc2 = desc.clone();
    assert_eq!(desc, desc2);
}

// ---------------------------------------------------------------------
// (e) a failing chain returns Err with the right FailCode, never panics.
// ---------------------------------------------------------------------

#[test]
fn a_chain_that_does_not_start_with_a_source_is_a_type_error() {
    let bare_rotate = TexNode::new(TexKind::Rotate, Vec::new(), None);
    let err = compile_tex(&bare_rotate).unwrap_err();
    assert_eq!(err.code, FailCode::Type);
}

#[test]
fn too_many_parameters_is_an_arity_error() {
    let too_many = osc(vec![
        VParam::Const(1.0),
        VParam::Const(2.0),
        VParam::Const(3.0),
        VParam::Const(4.0),
    ]);
    let err = compile_tex(&too_many).unwrap_err();
    assert_eq!(err.code, FailCode::Arity);
}

#[test]
fn a_chain_nested_over_64_deep_is_depth_exceeded() {
    let mut node = Rc::new(osc(Vec::new()));
    for _ in 0..80 {
        node = Rc::new(pipe(
            node,
            TexNode::new(TexKind::Rotate, Vec::new(), None),
            None,
        ));
    }
    let err = compile_tex(&node).unwrap_err();
    assert_eq!(err.code, FailCode::DepthExceeded);
}

// ---------------------------------------------------------------------
// (f) blend and modulate chains compile, naming uniforms in encounter
// order when their parameters are non-const.
// ---------------------------------------------------------------------

#[test]
fn blend_chain_names_uniforms_in_encounter_order() {
    let other = noise(vec![VParam::Sig(Rc::new(Sig::Saw))]);
    let blend_node = TexNode::new(
        TexKind::Blend(BlendOp::Add, Rc::new(other)),
        vec![VParam::Sig(Rc::new(Sig::Tri))],
        None,
    );
    let node = pipe(Rc::new(osc(Vec::new())), blend_node, None);
    let (desc, plan) = compile_tex(&node).unwrap();

    assert_eq!(plan.specs.len(), 2);
    assert_eq!(plan.specs[0].name, "u0");
    assert_eq!(plan.specs[1].name, "u1");
    assert!(matches!(plan.specs[0].src, VParam::Sig(ref s) if matches!(**s, Sig::Saw)));
    assert!(matches!(plan.specs[1].src, VParam::Sig(ref s) if matches!(**s, Sig::Tri)));
    assert_eq!(
        &*desc.uniform_names,
        &["u0".to_string(), "u1".to_string()][..]
    );
    assert!(desc.source.contains("tex_add"));
}

#[test]
fn modulate_chain_names_uniforms_in_encounter_order() {
    let other = noise(vec![VParam::Sig(Rc::new(Sig::Saw))]);
    let modulate_node = TexNode::new(
        TexKind::Modulate(ModKind::Modulate, Rc::new(other)),
        vec![VParam::Sig(Rc::new(Sig::Tri))],
        None,
    );
    let node = pipe(Rc::new(osc(Vec::new())), modulate_node, None);
    let (desc, plan) = compile_tex(&node).unwrap();

    assert_eq!(plan.specs.len(), 2);
    assert_eq!(plan.specs[0].name, "u0");
    assert_eq!(plan.specs[1].name, "u1");
    assert!(desc.source.contains("tex_modulate"));
}

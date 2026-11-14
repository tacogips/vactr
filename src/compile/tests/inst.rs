//! Compiling `inst`, `bus` and `master` bodies (12.8.6, B2): implicit
//! control names become `Param` constants in value position only, and an
//! `inst` body never resolves its own name.

use std::rc::Rc;

use crate::compile::proto::FnProto;
use crate::compile::{compile, CompileCx};
use crate::dsp::controls;
use crate::dsp::graph::{UGenKind, UGenSpec};
use crate::ns::load::read_forms;
use crate::ns::namespace::{FormGen, Namespace, SlotKind};
use crate::reader::span::FileId;
use crate::types::check::dsp_def;
use crate::value::intern::name_of_sym;
use crate::value::value::Value;
use crate::vm::natives::full_prelude;

fn proto(ns: &Namespace, src: &str) -> Rc<FnProto> {
    let forms = read_forms(src, FileId::new(1)).expect("reads");
    let mut cx = CompileCx::new(ns, FormGen::new(1));
    compile(&forms[0], &mut cx).expect("compiles")
}

/// The control names of every `Param` constant in the proto tree.
fn params(p: &FnProto, out: &mut Vec<String>) {
    for c in &p.consts {
        if let Value::UGen(n) = c {
            if let UGenKind::Ugen(UGenSpec::Param(id)) = n.kind {
                out.push(controls::row_by_id(id).map_or("?", |r| r.name).to_string());
            }
        }
    }
    for q in &p.protos {
        params(q, out);
    }
}

fn params_of(src: &str) -> Vec<String> {
    let ns = Namespace::new(full_prelude());
    let mut out = Vec::new();
    params(&proto(&ns, src), &mut out);
    out.sort();
    out
}

#[test]
fn free_control_names_in_an_inst_body_are_param_constants() {
    assert_eq!(
        params_of("inst k:\n\tsaw freq > * {env-perc attack release} > * amp"),
        ["amp", "attack", "freq", "release"]
    );
    // Header parameters are locals, not constants.
    assert_eq!(
        params_of("inst k freq amp = 0.5:\n\tsaw freq > * amp"),
        Vec::<String>::new()
    );
}

#[test]
fn call_heads_and_code_outside_a_body_are_untouched() {
    assert_eq!(
        params_of("inst k:\n\tlpf 1 2 > delay 0.1 0.2"),
        Vec::<String>::new()
    );
    assert_eq!(params_of("let x amp"), Vec::<String>::new());
    assert_eq!(params_of("fn f:\n\tamp"), Vec::<String>::new());
}

#[test]
fn bus_and_master_blocks_are_dsp_bodies() {
    assert_eq!(params_of("bus :b:\n\tplate mix: room"), ["room"]);
    assert_eq!(params_of("master:\n\tgain amp"), ["amp"]);
    // The routing control is an ordinary call.
    assert_eq!(params_of("s :bd > bus :b"), Vec::<String>::new());
    let forms = read_forms(
        "bus :b:\n\tplate\nmaster:\n\tlimiter\ns :bd > bus :b\nbus x :k",
        FileId::new(1),
    )
    .expect("reads");
    let heads: Vec<Option<&str>> = forms.iter().map(dsp_def).collect();
    assert_eq!(heads, [Some("bus"), Some("master"), None, None]);
}

#[test]
fn an_inst_body_calls_the_prelude_ugen_of_its_own_name() {
    let ns = Namespace::new(full_prelude());
    let p = proto(&ns, "inst additive:\n\tadditive freq partials: [1 0.5]");
    let body = &p.protos[0];
    let slot = body
        .globals
        .iter()
        .find(|g| &*name_of_sym(g.name()) == "additive")
        .expect("`additive` is referenced");
    assert_eq!(slot.kind(), SlotKind::Prelude);
    // A `fn` still resolves its own name (recursion).
    let p = proto(&ns, "fn additive x:\n\tadditive x");
    let body = &p.protos[0];
    let slot = body
        .globals
        .iter()
        .find(|g| &*name_of_sym(g.name()) == "additive")
        .expect("referenced");
    assert_ne!(slot.kind(), SlotKind::Prelude);
}

//! The compiler: depth guard, path/url constants, `ListProv`, keyword
//! arguments and splats, match through pattern ops, masks from
//! `types/masks.rs` (design 7.1.1, 7.1.5, 8.1-8.2).

use std::rc::Rc;

use crate::compile::{compile, CompileCx};
use crate::ns::namespace::{FormGen, Namespace, Prelude};
use crate::reader::node::{Atom, Node, NodeKind};
use crate::reader::span::{FileId, Span};
use crate::types::diag::{DiagCode, Severity};
use crate::value::value::Value;
use crate::vm::ops::Op;
use crate::vm::tests::{compile_src, Sess, FILE};

fn nested(depth: usize) -> Node {
    let span = Span::new(FileId::new(1), 0, 1);
    let mut n = Node::atom(Atom::Int(1), span);
    for _ in 0..depth {
        let head = Node::atom(Atom::Sym(Rc::from("neg")), span);
        n = Node::new(NodeKind::Call, span, vec![head, n]);
    }
    n
}

#[test]
fn nesting_past_1024_is_nesting_too_deep_and_the_form_does_not_compile() {
    let ns = Namespace::new(Prelude::core());
    let mut cx = CompileCx::new(&ns, FormGen::new(1));
    let d = compile(&nested(1100), &mut cx).expect_err("too deep");
    assert_eq!(d.code, DiagCode::NestingTooDeep);
    assert_eq!(d.severity, Severity::Error);
    // Far past both bounds: still a diagnostic, never a stack overflow.
    let d = compile(&nested(2_000), &mut cx).expect_err("too deep");
    assert_eq!(d.code, DiagCode::NestingTooDeep);
    // A shallow form compiles and runs.
    let ok = compile(&nested(100), &mut cx).expect("compiles");
    let mut s = Sess::new();
    assert_eq!(s.vm.run(ok, &ns).expect("runs").to_string(), "1");
}

fn consts(src: &str) -> Vec<Value> {
    let ns = Namespace::new(Prelude::core());
    compile_src(&ns, src).consts.clone()
}

#[test]
fn path_and_url_literals_are_constants() {
    let c = consts("print ./a/b.wav ../c.wav /abs/x ~/home/y https://example.com/p");
    let paths: Vec<(String, Option<FileId>)> = c
        .iter()
        .filter_map(|v| match v {
            Value::Path(p) => Some((p.text.to_string(), p.file)),
            _ => None,
        })
        .collect();
    assert_eq!(
        paths,
        vec![
            ("./a/b.wav".into(), Some(FILE)),
            ("../c.wav".into(), Some(FILE)),
            ("/abs/x".into(), None),
            ("~/home/y".into(), None),
        ]
    );
    assert!(c
        .iter()
        .any(|v| matches!(v, Value::Url(u) if &**u == "https://example.com/p")));
}

#[test]
fn list_literals_carry_provenance() {
    let mut s = Sess::new();
    let src = "[1 22 333]";
    let v = s.eval(src).expect("list");
    let Value::List(l) = v else { panic!("{v:?}") };
    let prov = l.prov.as_ref().expect("provenance");
    assert_eq!(prov.form_gen, FormGen::new(s.gen));
    let spans: Vec<(u32, u32)> = prov.elems.iter().map(|x| (x.start, x.end)).collect();
    assert_eq!(spans, vec![(1, 2), (3, 5), (6, 9)]);
    // A splat makes element positions dynamic: no provenance.
    let v = s.eval("[0 & [1 2]]").expect("splat");
    let Value::List(l) = v else { panic!() };
    assert!(l.prov.is_none());
    assert_eq!(Value::List(l).to_string(), "[0 1 2]");
}

#[test]
fn keyword_arguments_and_splats() {
    let mut s = Sess::new();
    s.eval("fn f a b = 2:\n\t+ a b\nfn g a b:\n\t- a b")
        .expect("setup");
    for (src, want) in [
        ("f 1", "3"),
        ("f 1 b: 5", "6"),
        ("f b: 5 a: 1", "6"),
        ("let o [b: 10]\nf 1 & o", "11"),
        ("g & [5 2]", "3"),
        ("g 5 & [2]", "3"),
        ("f 1 c: 3", "fail: arity"),
        ("f 1 a: 3", "fail: arity"),
    ] {
        assert_eq!(s.show(src), want, "{src}");
    }
    let ns = Namespace::new(Prelude::core());
    let p = compile_src(&ns, "once 1 at: 4 & [gain: 0.5]");
    assert!(p.code.iter().any(|op| matches!(op, Op::CallKw(_))));
    assert_eq!(p.call_sites.len(), 1);
}

#[test]
fn a_default_is_evaluated_where_the_fn_is_defined() {
    let mut s = Sess::new();
    s.eval("let base 5\nfn f a b = {+ base 1}:\n\t+ a b")
        .expect("setup");
    assert_eq!(s.show("f 1"), "7");
}

#[test]
fn match_compiles_to_pattern_ops() {
    let ns = Namespace::new(Prelude::core());
    let p = compile_src(
        &ns,
        "match 1:\n\t0 -> :a\n\t[x & r] if {> x 1} -> :b\n\t_ -> :c",
    );
    let has = |f: &dyn Fn(&Op) -> bool| p.code.iter().any(f);
    assert!(has(&|o| matches!(o, Op::TestLit(_))));
    assert!(has(&|o| matches!(o, Op::TestLenMin(1))));
    assert!(has(&|o| matches!(o, Op::SplitRest(1))));
    assert!(has(&|o| matches!(o, Op::TestTruthy)));
    assert!(has(&|o| matches!(o, Op::JumpIfNoMatch(_))));
    assert!(has(&|o| matches!(o, Op::Fail(_))));
    assert!(!p
        .code
        .iter()
        .any(|o| matches!(o, Op::Jump(off) | Op::JumpIfNoMatch(off) if *off < 0)));
}

#[test]
fn pattern_forms_match_as_the_language_reference_says() {
    let mut s = Sess::new();
    s.eval("enum shape:\n\tcircle r\n\trect w h\n\tnone")
        .expect("enum");
    let m = "fn m v:\n\tmatch v:\n\t\t0 -> \"zero\"\n\t\t[] -> \"empty\"\n\t\t[a b] -> + a b\n\t\t[amp: a] -> a\n\t\tcircle r if {> r 10} -> \"big circle\"\n\t\tcircle r -> * 3 r r\n\t\trect [x y] h -> * x h\n\t\tnone -> \"none\"\n\t\t:kick | :snare -> \"drum\"\n\t\tx -> x";
    s.eval(m).expect("m");
    for (arg, want) in [
        ("0", "zero"),
        ("[]", "empty"),
        ("[1 2]", "3"),
        ("[amp: 7]", "7"),
        ("{circle 11}", "big circle"),
        ("{circle 2}", "12"),
        ("{rect [2 9] 5}", "10"),
        ("none", "none"),
        (":snare", "drum"),
        ("\"other\"", "other"),
    ] {
        assert_eq!(s.show(&format!("m {arg}")), want, "m {arg}");
    }
}

#[test]
fn every_fn_gets_its_mask_and_latent_forcing_is_reported() {
    let ns = Namespace::new(Prelude::core());
    let mut cx = CompileCx::new(&ns, FormGen::new(1));
    let r = crate::reader::read("fn und f x:\n\tf x", FILE, &crate::reader::AliasEnv::new());
    let k = crate::expand::expand(
        &r.nodes[0],
        &mut crate::expand::ExpandCx::new(r.next_node_id()),
    )
    .expect("expand");
    let p = compile(&k, &mut cx).expect("compile");
    assert_eq!(p.masks.len(), 1);
    assert_eq!(
        p.masks[0].0.as_ref(),
        [
            crate::types::masks::MaskEntry::Fn,
            crate::types::masks::MaskEntry::Undetermined
        ]
    );
    assert_eq!(cx.diags.len(), 1);
    assert_eq!(cx.diags[0].code, DiagCode::LatentForcing);
    assert_eq!(cx.diags[0].severity, Severity::Warning);
}

#[test]
fn top_level_var_and_fn_load_late_and_let_loads_a_snapshot() {
    let mut s = Sess::new();
    s.eval("let a 1\nvar b 2\nfn c k:\n\tk").expect("setup");
    let p = compile_src(&s.ns, "[a b c]");
    let loads: Vec<&Op> = p
        .code
        .iter()
        .filter(|o| matches!(o, Op::LoadGlobal(_) | Op::LoadGlobalRef(_)))
        .collect();
    assert!(matches!(
        loads.as_slice(),
        [
            Op::LoadGlobal(_),
            Op::LoadGlobalRef(_),
            Op::LoadGlobalRef(_)
        ]
    ));
}

#[test]
fn a_proto_table_past_u16_is_a_compile_diagnostic_not_a_wrong_index() {
    let items: Vec<String> = (0..70_000).map(|k| k.to_string()).collect();
    let src = format!("[{}]", items.join(" "));
    let ns = Namespace::new(Prelude::core());
    let r = crate::reader::read(&src, FILE, &crate::reader::AliasEnv::new());
    let k = crate::expand::expand(
        &r.nodes[0],
        &mut crate::expand::ExpandCx::new(r.next_node_id()),
    )
    .expect("expand");
    let d = compile(&k, &mut CompileCx::new(&ns, FormGen::new(1))).expect_err("too large");
    assert_eq!(d.code, DiagCode::NestingTooDeep);
}

/// Reads, expands and compiles one console form.
fn console_form(ns: &Namespace, src: &str) -> Rc<crate::compile::proto::FnProto> {
    let r = crate::reader::read(src, FileId::CONSOLE, &crate::reader::AliasEnv::new());
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    let k = crate::expand::expand(
        &r.nodes[0],
        &mut crate::expand::ExpandCx::new(r.next_node_id()),
    )
    .expect("expand");
    compile(&k, &mut CompileCx::new(ns, FormGen::new(1))).expect("compile")
}

#[test]
fn console_registers_resolve_in_the_console_only() {
    let ns = Namespace::new(Prelude::core());
    ns.set_console_register(1, Value::Int(42));
    assert_eq!(
        ns.console_register(1).map(|v| v.to_string()),
        Some("42".into())
    );
    let mut s = Sess::new();
    let v = s.vm.run(console_form(&ns, "_1"), &ns).expect("runs");
    assert_eq!(v.to_string(), "42");
    let v = s.vm.run(console_form(&ns, "+ _1 1"), &ns).expect("runs");
    assert_eq!(v.to_string(), "43");
    // Registers are not session names.
    assert!(ns.session_names().is_empty());
    assert!(ns.session_value("_1").is_none());

    // An unset register fails `undefined-name` when read.
    let unset = console_form(&ns, "_2");
    assert!(ns.console_register(2).is_none());
    let err = s.vm.run(Rc::clone(&unset), &ns).expect_err("unset");
    assert_eq!(err.code, crate::vm::fail::FailCode::UndefinedName);
    assert!(err.message.contains("_2"), "{}", err.message);
    // Once set, the same compiled read sees it.
    ns.set_console_register(2, Value::Int(7));
    assert_eq!(s.vm.run(unset, &ns).expect("set").to_string(), "7");

    // Outside the console the register does not exist.
    let file_form = Node::atom(Atom::ConsoleReg(1), Span::new(FileId::new(1), 0, 2));
    let p = compile(&file_form, &mut CompileCx::new(&ns, FormGen::new(2))).expect("compile");
    let err = s.vm.run(p, &ns).expect_err("not in a file");
    assert_eq!(err.code, crate::vm::fail::FailCode::UndefinedName);
}

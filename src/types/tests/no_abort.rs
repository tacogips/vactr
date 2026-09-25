//! The checker never aborts (7.1.1, 7.1.5): every spec block checks
//! without panic and types every top-level form, a 1100-deep expression is
//! `nesting-too-deep`, and forcing masks do not depend on the checker
//! (ME-CHECK required tests). The `check` entry is called directly, with
//! no LSP or VM types.

use std::rc::Rc;

use crate::expand::{expand, ExpandCx};
use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::reader::span::{FileId, NodeId, Span};
use crate::reader::{read, AliasEnv};
use crate::types::check;
use crate::types::diag::DiagCode;
use crate::types::manifest::HostManifest;
use crate::types::masks::infer_masks;
use crate::types::ty::CheckEnv;

const LANG_REFERENCE: &str = include_str!("../../../design-docs/specs/lang-reference.md");
const DESIGN_MUSIC: &str = include_str!("../../../design-docs/specs/design-music.md");

/// The code fences of a spec document (```` ```vactrol ```` and
/// ```` ```vact ````).
fn fences(doc: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in doc.lines() {
        match &mut cur {
            None if line.starts_with("```vact") => cur = Some(String::new()),
            Some(_) if line.starts_with("```") => out.extend(cur.take()),
            Some(text) => {
                text.push_str(line);
                text.push('\n');
            }
            None => {}
        }
    }
    out
}

/// The forms of a block that read and expand without a diagnostic.
fn clean_forms(src: &str, file: FileId) -> Vec<Node> {
    let r = read(src, file, &AliasEnv::new());
    let mut cx = ExpandCx::new(r.next_node_id());
    r.nodes
        .iter()
        .filter_map(|n| expand(n, &mut cx).ok())
        .collect()
}

fn spec_blocks() -> Vec<String> {
    let mut blocks = fences(LANG_REFERENCE);
    blocks.extend(fences(DESIGN_MUSIC));
    blocks
}

#[test]
fn every_spec_block_checks_without_abort() {
    let blocks = spec_blocks();
    assert!(blocks.len() >= 10, "found {} blocks", blocks.len());
    let env = CheckEnv::empty();
    let manifest = HostManifest::spec_default();
    let mut total = 0;
    for (k, block) in blocks.iter().enumerate() {
        let file = FileId::new(u32::try_from(k + 1).unwrap_or(1));
        let program = clean_forms(block, file);
        let r = check(&program, &env, &manifest);
        for form in &program {
            assert!(
                r.types.contains_key(&form.id),
                "block {k}: a form has no type"
            );
        }
        total += program.len();
    }
    assert!(total > 100, "only {total} forms checked");
}

/// `(neg (neg (.. (neg 1))))`, `depth` calls deep.
fn deep(depth: u32) -> Node {
    let span = Span::new(FileId::new(1), 0, 1);
    let mut id = 0;
    let mut fresh = || {
        id += 1;
        NodeId::new(id)
    };
    let mut n = Node {
        id: fresh(),
        kind: NodeKind::Atom(Atom::Int(1)),
        span,
        children: Box::new([]),
    };
    for _ in 0..depth {
        let head = Node {
            id: fresh(),
            kind: NodeKind::Atom(Atom::Builtin(Rc::from("neg"))),
            span,
            children: Box::new([]),
        };
        n = Node {
            id: fresh(),
            kind: NodeKind::Call,
            span,
            children: vec![head, n].into_boxed_slice(),
        };
    }
    n
}

#[test]
fn a_1100_deep_expression_is_nesting_too_deep() {
    let n = deep(1100);
    let r = check(
        std::slice::from_ref(&n),
        &CheckEnv::empty(),
        &HostManifest::spec_default(),
    );
    let codes: Vec<DiagCode> = r.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [DiagCode::NestingTooDeep]);
    assert!(r.types.contains_key(&n.id));
    let r = check(
        std::slice::from_ref(&deep(1000)),
        &CheckEnv::empty(),
        &HostManifest::spec_default(),
    );
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

/// `(match true {(-> (false | nil) INNER) (-> (_) 1)})`, `depth` deep: the
/// shape an `if`/`elif` chain expands to.
fn deep_match(depth: u32) -> Node {
    let span = Span::new(FileId::new(1), 0, 1);
    let mut id = 0;
    let mut node = |kind: NodeKind, children: Vec<Node>| {
        id += 1;
        Node {
            id: NodeId::new(id),
            kind,
            span,
            children: children.into_boxed_slice(),
        }
    };
    let mut n = node(NodeKind::Atom(Atom::Int(0)), Vec::new());
    for _ in 0..depth {
        let falsy = vec![
            node(NodeKind::Atom(Atom::Bool(false)), Vec::new()),
            node(NodeKind::Atom(Atom::Op(Op::Bar)), Vec::new()),
            node(NodeKind::Atom(Atom::Nil), Vec::new()),
            n,
        ];
        let no = node(NodeKind::Arrow, falsy);
        let wild = node(NodeKind::Atom(Atom::Wildcard), Vec::new());
        let one = node(NodeKind::Atom(Atom::Int(1)), Vec::new());
        let yes = node(NodeKind::Arrow, vec![wild, one]);
        let head = node(NodeKind::Atom(Atom::Sym(Rc::from("match"))), Vec::new());
        let subject = node(NodeKind::Atom(Atom::Bool(true)), Vec::new());
        let clauses = node(NodeKind::Block, vec![no, yes]);
        n = node(NodeKind::Call, vec![head, subject, clauses]);
    }
    n
}

#[test]
fn a_deep_if_chain_expansion_does_not_overflow() {
    let manifest = HostManifest::spec_default();
    // The depth counts checker recursion: one level per nested `match`.
    let n = deep_match(1100);
    let r = check(std::slice::from_ref(&n), &CheckEnv::empty(), &manifest);
    let codes: Vec<DiagCode> = r.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [DiagCode::NestingTooDeep]);
    let n = deep_match(1000);
    let r = check(std::slice::from_ref(&n), &CheckEnv::empty(), &manifest);
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

#[test]
fn exponentially_growing_types_are_capped() {
    // Each binding's type doubles in size; past `MAX_TY_SIZE` it is `any`.
    let mut src = String::from("let x0 1\n");
    for k in 1..=20 {
        src.push_str(&format!("let x{k} f -> f x{} x{}\n", k - 1, k - 1));
    }
    let program = clean_forms(&src, FileId::new(1));
    assert_eq!(program.len(), 21);
    let r = check(&program, &CheckEnv::empty(), &HostManifest::spec_default());
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

/// Every expanded `fn` node of a form.
fn fns<'n>(n: &'n Node, out: &mut Vec<&'n Node>) {
    if matches!(n.kind, NodeKind::Call) && n.children.first().and_then(Node::sym_name) == Some("fn")
    {
        out.push(n);
    }
    for c in n.children.iter() {
        fns(c, out);
    }
}

#[test]
fn masks_are_identical_with_or_without_the_checker() {
    let env = CheckEnv::empty();
    let manifest = HostManifest::spec_default();
    let mut seen = 0;
    for (k, block) in spec_blocks().iter().enumerate() {
        let program = clean_forms(block, FileId::new(u32::try_from(k + 1).unwrap_or(1)));
        let mut all = Vec::new();
        for form in &program {
            fns(form, &mut all);
        }
        let before: Vec<_> = all.iter().map(|f| infer_masks(f, &env)).collect();
        let _ = check(&program, &env, &manifest);
        let after: Vec<_> = all.iter().map(|f| infer_masks(f, &env)).collect();
        assert_eq!(before, after, "block {k}");
        seen += all.len();
    }
    assert!(seen > 5, "only {seen} fns");
}

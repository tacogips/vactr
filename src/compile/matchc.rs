//! `match` to pattern ops, and everything that binds through a pattern:
//! lambda and `fn` parameters, `let [..]` destructuring, and the `enum` and
//! `struct` constructors whose shapes variant patterns test (design 8.2,
//! lang-reference section 3).

use std::rc::Rc;

use crate::compile::compiler::{literal, too_deep, Compiler, FnDef, Pos, Res};
use crate::compile::proto::{
    field_params, fn_params, struct_fields, Arity, Fb, FbKind, Param, Shape,
};
use crate::compile::sites::SiteCtx;
use crate::ns::namespace::SlotKind;
use crate::reader::node::{Atom, Node, NodeKind, Op as NodeOp};
use crate::types::diag::{DiagCode, Diagnostic, Severity};
use crate::types::masks::{infer_masks, ForcingMask, MaskEntry};
use crate::value::intern::{intern_kw, intern_sym, name_of_sym, KwId};
use crate::value::value::{Value, VariantVal};
use crate::vm::fail::FailCode;
use crate::vm::ops::Op;

/// Splits a clause lhs at top-level `|` into alternatives.
fn alternatives(lhs: &[Node]) -> Vec<&[Node]> {
    lhs.split(|i| matches!(i.kind, NodeKind::Atom(Atom::Op(NodeOp::Bar))))
        .collect()
}

impl Compiler<'_, '_> {
    /// `(match SUBJ {(-> PATTERN [if GUARD] BODY)..})`.
    pub(crate) fn match_form(&mut self, n: &Node) -> Result<(), Diagnostic> {
        let ch = &n.children;
        match ch.get(1) {
            Some(s) => self.expr(s, Pos::Value)?,
            None => self.load_const(Value::Nil),
        }
        self.emit(Op::Force);
        let subj = self.temp(n)?;
        self.emit(Op::StoreLocal(subj));
        let mut ends = Vec::new();
        let clauses = ch.get(2).map_or(&[][..], |b| &b.children[..]);
        for clause in clauses {
            let Some((body, lhs)) = clause.children.split_last() else {
                continue;
            };
            let g = lhs.iter().position(|i| i.sym_name() == Some("if"));
            let (pats, guard) = match g {
                Some(k) => (&lhs[..k], &lhs[k + 1..]),
                None => (lhs, &[][..]),
            };
            self.open_scope();
            let mut fails = Vec::new();
            self.mark(clause.span);
            self.clause_pattern(pats, subj, &mut fails)?;
            for gexpr in guard {
                self.expr(gexpr, Pos::Value)?;
                self.emit(Op::Force);
                self.emit(Op::TestTruthy);
                fails.push(self.jump_slot(Op::JumpIfNoMatch(0)));
            }
            self.expr(body, Pos::Value)?;
            ends.push(self.jump_slot(Op::Jump(0)));
            self.close_scope();
            for f in fails {
                self.patch(f);
            }
        }
        self.mark(n.span);
        self.fail(FailCode::NoMatch);
        for e in ends {
            self.patch(e);
        }
        Ok(())
    }

    /// A clause lhs: alternatives joined by `|`, each a pattern sequence.
    fn clause_pattern(
        &mut self,
        lhs: &[Node],
        subj: u16,
        fails: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        let alts = alternatives(lhs);
        let Some((last, rest)) = alts.split_last() else {
            return Ok(());
        };
        let mut wins = Vec::new();
        for alt in rest {
            let mut alt_fails = Vec::new();
            self.pattern_seq(alt, subj, &mut alt_fails)?;
            wins.push(self.jump_slot(Op::Jump(0)));
            for f in alt_fails {
                self.patch(f);
            }
        }
        self.pattern_seq(last, subj, fails)?;
        for w in wins {
            self.patch(w);
        }
        Ok(())
    }

    /// One alternative: a single pattern, or a variant or struct name
    /// followed by its field patterns.
    fn pattern_seq(
        &mut self,
        items: &[Node],
        subj: u16,
        fails: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        match items {
            [] => Ok(()),
            [one] => self.pattern(one, subj, fails),
            [head, fields @ ..] => self.shape_pattern(head, fields, subj, fails),
        }
    }

    fn test(&mut self, subj: u16, op: Op, fails: &mut Vec<usize>) {
        self.emit(Op::LoadLocal(subj));
        self.emit(op);
        fails.push(self.jump_slot(Op::JumpIfNoMatch(0)));
    }

    /// Emits a test no value passes.
    fn never(&mut self, subj: u16, fails: &mut Vec<usize>) {
        let s = self.shape(Shape {
            ty: intern_sym(""),
            tag: intern_sym(""),
            fields: Box::new([]),
            is_struct: false,
        });
        self.test(subj, Op::TestVariant(s), fails);
    }

    fn shape(&mut self, shape: Shape) -> u16 {
        let fb = self.fb();
        let k = match fb.shapes.iter().position(|s| *s == shape) {
            Some(k) => k,
            None => {
                fb.shapes.push(shape);
                fb.shapes.len() - 1
            }
        };
        self.idx16(k)
    }

    /// Loads `subj`, applies `op` (a projection) and stores into a temp.
    fn project(&mut self, subj: u16, op: Op, at: &Node) -> Result<u16, Diagnostic> {
        self.emit(Op::LoadLocal(subj));
        self.emit(op);
        let t = self.temp(at)?;
        self.emit(Op::StoreLocal(t));
        Ok(t)
    }

    /// Binds `name` to the value in `subj`. A name already bound in this
    /// clause scope (the other side of an or-pattern) reuses its local.
    fn bind(&mut self, name: &str, subj: u16, at: &Node) -> Result<(), Diagnostic> {
        let existing = self
            .fb()
            .scopes
            .last()
            .and_then(|s| s.iter().find(|l| &*l.name == name))
            .map(|l| l.slot);
        let slot = match existing {
            Some(s) => s,
            None => self.new_local(name, false, false, at)?,
        };
        self.emit(Op::LoadLocal(subj));
        self.emit(Op::StoreLocal(slot));
        Ok(())
    }

    /// The shape a name resolves to, when it names a variant, a variant
    /// constructor or a struct constructor itself (the checker's rule: a name
    /// that resolves to a variant is a variant pattern, any other binds, so a
    /// session name merely holding a variant value binds).
    fn shape_of(&mut self, name: &str) -> Option<Shape> {
        let Res::Global(slot) = self.resolve(name) else {
            return None;
        };
        if !slot.is_bound() {
            return None;
        }
        match slot.get() {
            Value::Variant(v) if *name_of_sym(v.tag) == *name => Some(Shape {
                ty: v.enum_ty,
                tag: v.tag,
                fields: v.fields.iter().map(|(k, _)| *k).collect(),
                is_struct: false,
            }),
            Value::Fn(c) if c.proto.name == Some(intern_sym(name)) => c
                .proto
                .ctor
                .and_then(|k| c.proto.shapes.get(usize::from(k)).cloned()),
            _ => None,
        }
    }

    fn pattern(&mut self, p: &Node, subj: u16, fails: &mut Vec<usize>) -> Result<(), Diagnostic> {
        self.enter(p)?;
        let r = self.pattern_inner(p, subj, fails);
        self.leave();
        r
    }

    fn pattern_inner(
        &mut self,
        p: &Node,
        subj: u16,
        fails: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        match &p.kind {
            NodeKind::Atom(Atom::Wildcard) => Ok(()),
            NodeKind::Atom(Atom::Sym(name)) => match self.shape_of(name) {
                Some(shape) => {
                    let op = if shape.is_struct {
                        Op::TestShape
                    } else {
                        Op::TestVariant
                    };
                    let s = self.shape(shape);
                    self.test(subj, op(s), fails);
                    Ok(())
                }
                None => self.bind(name, subj, p),
            },
            NodeKind::Atom(Atom::Path(text)) => {
                let rel = text.starts_with("./") || text.starts_with("../");
                let k = self.konst(Value::path(text, rel.then_some(p.span.file)));
                self.test(subj, Op::TestLit(k), fails);
                Ok(())
            }
            NodeKind::Atom(Atom::Url(text)) => {
                let k = self.konst(Value::url(text));
                self.test(subj, Op::TestLit(k), fails);
                Ok(())
            }
            NodeKind::Atom(a) => {
                match literal(a) {
                    Some(v) => {
                        let k = self.konst(v);
                        self.test(subj, Op::TestLit(k), fails);
                    }
                    None => self.never(subj, fails),
                }
                Ok(())
            }
            NodeKind::List => self.list_pattern(p, subj, fails),
            NodeKind::Pair => self.dict_pattern(std::slice::from_ref(p), subj, fails),
            NodeKind::Call => self.pattern_seq(&p.children, subj, fails),
            NodeKind::Block if p.children.len() == 1 => self.pattern(&p.children[0], subj, fails),
            _ => {
                self.never(subj, fails);
                Ok(())
            }
        }
    }

    fn list_pattern(
        &mut self,
        p: &Node,
        subj: u16,
        fails: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        let items = &p.children;
        if !items.is_empty() && items.iter().all(|i| matches!(i.kind, NodeKind::Pair)) {
            return self.dict_pattern(items, subj, fails);
        }
        let splat = items.iter().position(|i| matches!(i.kind, NodeKind::Splat));
        let fixed = splat.unwrap_or(items.len());
        let n = u16::try_from(fixed).map_err(|_| too_deep(p))?;
        self.test(
            subj,
            if splat.is_some() {
                Op::TestLenMin(n)
            } else {
                Op::TestLen(n)
            },
            fails,
        );
        for (k, item) in items[..fixed].iter().enumerate() {
            let t = self.project(
                subj,
                Op::BindField(u16::try_from(k).map_err(|_| too_deep(p))?),
                item,
            )?;
            self.pattern(item, t, fails)?;
        }
        if let Some(s) = splat {
            if let Some(rest) = items[s].children.first() {
                let t = self.project(subj, Op::SplitRest(n), rest)?;
                self.pattern(rest, t, fails)?;
            }
        }
        Ok(())
    }

    /// `[amp: a pan: p]`: each key must be present; other keys are ignored.
    fn dict_pattern(
        &mut self,
        pairs: &[Node],
        subj: u16,
        fails: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        for pair in pairs {
            let key = match pair.children.first().map(|k| &k.kind) {
                Some(NodeKind::Atom(a)) => literal(a),
                _ => None,
            };
            let Some(key) = key else {
                self.never(subj, fails);
                continue;
            };
            let k = self.konst(key);
            self.test(subj, Op::TestKey(k), fails);
            if let Some(vp) = pair.children.get(1) {
                let t = self.project(subj, Op::GetKey(k), vp)?;
                self.pattern(vp, t, fails)?;
            }
        }
        Ok(())
    }

    /// `circle r`, `rect [x y] h`, `voice [note: p]`.
    fn shape_pattern(
        &mut self,
        head: &Node,
        fields: &[Node],
        subj: u16,
        fails: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        let Some(shape) = head.sym_name().and_then(|n| self.shape_of(n)) else {
            self.never(subj, fails);
            return Ok(());
        };
        let is_struct = shape.is_struct;
        let names = shape.fields.clone();
        let s = self.shape(shape);
        self.test(
            subj,
            if is_struct {
                Op::TestShape(s)
            } else {
                Op::TestVariant(s)
            },
            fails,
        );
        for (k, fp) in fields.iter().enumerate() {
            if is_struct {
                self.pattern(fp, subj, fails)?;
                continue;
            }
            match names.get(k) {
                Some(kw) => {
                    let key = self.konst(Value::Keyword(*kw));
                    let t = self.project(subj, Op::GetKey(key), fp)?;
                    self.pattern(fp, t, fails)?;
                }
                None => self.never(subj, fails),
            }
        }
        Ok(())
    }

    /// `let [a b] v`: an irrefutable binding pattern; a mismatch fails
    /// `no-match`. At the top level each bound name becomes a session slot.
    pub(crate) fn destructure(
        &mut self,
        target: &Node,
        top: bool,
        is_var: bool,
    ) -> Result<(), Diagnostic> {
        let t = self.temp(target)?;
        self.emit(Op::StoreLocal(t));
        let mark = self.fb().scopes.last().map_or(0, Vec::len);
        let mut fails = Vec::new();
        self.pattern(target, t, &mut fails)?;
        if !fails.is_empty() {
            let over = self.jump_slot(Op::Jump(0));
            for f in fails {
                self.patch(f);
            }
            self.mark(target.span);
            self.fail(FailCode::NoMatch);
            self.patch(over);
        }
        let bound: Vec<(Rc<str>, u16)> = self
            .fb()
            .scopes
            .last()
            .map(|s| {
                s[mark..]
                    .iter()
                    .map(|l| (Rc::clone(&l.name), l.slot))
                    .collect()
            })
            .unwrap_or_default();
        for (name, slot) in bound {
            // A binding value is a snapshot: items such as `[x]` for a
            // `var x` hold a `VarRef`, forced here.
            self.emit(Op::LoadLocal(slot));
            self.emit(Op::Force);
            if top {
                let g = self.def_target(&name);
                self.def_global(&g, if is_var { SlotKind::Var } else { SlotKind::Let });
                self.emit(Op::Pop);
            } else {
                if is_var {
                    self.emit(Op::MakeCell);
                    self.mark_var(&name);
                }
                self.emit(Op::StoreLocal(slot));
            }
        }
        self.emit(Op::LoadLocal(t));
        Ok(())
    }

    /// Marks the innermost local `name` as a `var` cell.
    fn mark_var(&mut self, name: &str) {
        if let Some(scope) = self.fb().scopes.last_mut() {
            if let Some(l) = scope.iter_mut().rev().find(|l| &*l.name == name) {
                l.is_var = true;
            }
        }
    }

    /// Records `latent-forcing` for each `Undetermined` parameter (5.5).
    fn latent(&mut self, mask: &ForcingMask, params: &[Param<'_>]) {
        for (entry, p) in mask.0.iter().zip(params) {
            if *entry == MaskEntry::Undetermined {
                self.cx.diags.push(Diagnostic {
                    span: p.node.span,
                    severity: Severity::Warning,
                    code: DiagCode::LatentForcing,
                    message: "this argument's forcing is decided at run time (memoized)".into(),
                    origin: None,
                });
            }
        }
    }

    /// Builds a function body over `params`: positional parameters first,
    /// then keyword parameters; list-pattern parameters are tested in a
    /// prologue. Returns the builder, the mask in parameter order, and the
    /// default nodes in keyword order.
    fn function<'n>(
        &mut self,
        name: Option<Rc<str>>,
        f: &'n Node,
        params: Vec<Param<'n>>,
        mask: ForcingMask,
        body: &mut dyn FnMut(&mut Self) -> Result<(), Diagnostic>,
    ) -> Result<(Fb, ForcingMask, Vec<&'n Node>), Diagnostic> {
        let order: Vec<usize> = (0..params.len())
            .filter(|k| params[*k].default.is_none())
            .chain((0..params.len()).filter(|k| params[*k].default.is_some()))
            .collect();
        let fixed = order
            .iter()
            .filter(|k| params[**k].default.is_none())
            .count();
        let mut entries = Vec::with_capacity(order.len());
        let mut names: Vec<KwId> = Vec::with_capacity(order.len());
        let mut defaults = Vec::new();
        self.fbs.push(Fb::new(name, f.span, FbKind::Func));
        let r = (|| {
            let mut prologue = Vec::new();
            for k in &order {
                let p = &params[*k];
                entries.push(mask.0.get(*k).cloned().unwrap_or(MaskEntry::Value));
                names.push(intern_kw(p.name.as_deref().unwrap_or("")));
                if let Some(d) = p.default {
                    defaults.push(d);
                }
                let slot = self.new_local(p.name.as_deref().unwrap_or(""), false, true, p.node)?;
                if p.name.is_none() && !matches!(p.node.kind, NodeKind::Atom(Atom::Wildcard)) {
                    prologue.push((p.node, slot));
                }
            }
            let fb = self.fb();
            fb.arity = Arity {
                fixed: u8::try_from(fixed).map_err(|_| too_deep(f))?,
                names: names.clone().into_boxed_slice(),
                keys: u8::try_from(order.len() - fixed).map_err(|_| too_deep(f))?,
            };
            for (node, slot) in prologue {
                let mut fails = Vec::new();
                self.pattern(node, slot, &mut fails)?;
                if !fails.is_empty() {
                    let over = self.jump_slot(Op::Jump(0));
                    for x in fails {
                        self.patch(x);
                    }
                    self.mark(node.span);
                    self.fail(FailCode::NoMatch);
                    self.patch(over);
                }
            }
            body(self)?;
            self.emit(Op::Ret);
            Ok(())
        })();
        let fb = self.fbs.pop().ok_or_else(|| too_deep(f))?;
        r?;
        Ok((fb, ForcingMask(entries.into_boxed_slice()), defaults))
    }

    /// Pushes the defaults (after the captures) and makes the closure.
    fn finish_fn(
        &mut self,
        fb: Fb,
        mask: ForcingMask,
        defaults: &[&Node],
        inst: bool,
    ) -> Result<(), Diagnostic> {
        self.load_captures(&fb);
        let site = self.site;
        for d in defaults {
            self.site = if inst {
                SiteCtx::InstDefault
            } else {
                SiteCtx::None
            };
            self.expr(d, Pos::Value)?;
        }
        self.site = site;
        self.close_closure_loaded(fb, mask, false);
        Ok(())
    }

    /// A lambda `(-> (PARAMS..) BODY)`: parameters are patterns.
    pub(crate) fn lambda(&mut self, n: &Node) -> Result<(), Diagnostic> {
        let Some((body, lhs)) = n.children.split_last() else {
            self.load_const(Value::Nil);
            return Ok(());
        };
        let mask = infer_masks(n, self.cx.env());
        let params: Vec<Param<'_>> = lhs
            .iter()
            .map(|p| Param {
                node: p,
                name: p.sym_name().filter(|s| *s != "_").map(Rc::from),
                default: None,
            })
            .collect();
        self.latent(&mask, &params);
        let (fb, mask, defaults) =
            self.function(None, n, params, mask, &mut |c| c.expr(body, Pos::Value))?;
        self.finish_fn(fb, mask, &defaults, false)
    }

    /// `fn NAME HEADER: BODY` (also `inst` and `look`): a session slot at
    /// the top level, a local elsewhere.
    pub(crate) fn def_fn(&mut self, n: &Node, top: bool, kind: FnDef) -> Result<(), Diagnostic> {
        let ch = &n.children;
        let well_formed = ch.len() >= 3
            && ch[1].sym_name().is_some()
            && matches!(ch[ch.len() - 1].kind, NodeKind::Block);
        let Some(name) = ch.get(1).and_then(Node::sym_name).filter(|_| well_formed) else {
            self.fail(FailCode::Type);
            return Ok(());
        };
        let name: Rc<str> = Rc::from(name);
        let params = fn_params(n);
        let mask = infer_masks(n, self.cx.env());
        self.latent(&mask, &params);
        let saved = self.self_def.take();
        let target = top.then(|| self.def_target(&name));
        if let Some(slot) = &target {
            self.self_def = Some((Rc::clone(&name), slot.clone()));
        }
        let body = &ch[ch.len() - 1];
        let r = self.function(Some(Rc::clone(&name)), n, params, mask, &mut |c| {
            c.block(body)
        });
        self.self_def = saved;
        let (fb, mask, defaults) = r?;
        self.finish_fn(fb, mask, &defaults, kind == FnDef::Inst)?;
        self.bind_def(&name, target.as_ref(), SlotKind::Fn, n)
    }

    /// Binds a definition's value (on the stack): `DefGlobal` at the top
    /// level, a local otherwise. The value stays on the stack.
    fn bind_def(
        &mut self,
        name: &str,
        target: Option<&crate::ns::namespace::VarSlotRef>,
        kind: SlotKind,
        at: &Node,
    ) -> Result<(), Diagnostic> {
        match target {
            Some(slot) => self.def_global(slot, kind),
            None => {
                self.emit(Op::Dup);
                let slot = self.new_local(name, false, false, at)?;
                self.emit(Op::StoreLocal(slot));
            }
        }
        Ok(())
    }

    /// A constructor over `fields` for `shape`: required fields positional,
    /// defaulted fields named; the value is built in declared order.
    fn ctor(
        &mut self,
        name: &str,
        at: &Node,
        fields: Vec<Param<'_>>,
        shape: Shape,
    ) -> Result<(), Diagnostic> {
        let mask = ForcingMask(vec![MaskEntry::Value; fields.len()].into_boxed_slice());
        let names: Vec<Rc<str>> = fields
            .iter()
            .map(|p| p.name.clone().unwrap_or_else(|| Rc::from("")))
            .collect();
        let (fb, mask, defaults) =
            self.function(Some(Rc::from(name)), at, fields, mask, &mut |c| {
                for field in &names {
                    match c.resolve(field) {
                        Res::Local(l) => {
                            c.emit(Op::LoadLocal(l.slot));
                        }
                        _ => c.load_const(Value::Nil),
                    }
                }
                let s = c.shape(shape.clone());
                c.fb().ctor = Some(s);
                c.emit(Op::MakeShape(s));
                Ok(())
            })?;
        self.finish_fn(fb, mask, &defaults, false)
    }

    /// `enum NAME: variant..`: a field-less variant is a value, a variant
    /// with fields a constructor. The form's value is `nil`.
    pub(crate) fn def_enum(&mut self, n: &Node, top: bool) -> Result<(), Diagnostic> {
        let ch = &n.children;
        let (Some(ty), Some(block)) = (ch.get(1).and_then(Node::sym_name), ch.get(2)) else {
            self.fail(FailCode::Type);
            return Ok(());
        };
        let enum_ty = intern_sym(ty);
        for line in block.children.iter() {
            let (tag, items) = match &line.kind {
                NodeKind::Call => (
                    line.children.first().and_then(Node::sym_name),
                    &line.children[1..],
                ),
                _ => (line.sym_name(), &[][..]),
            };
            let Some(tag) = tag else { continue };
            let target = top.then(|| self.def_target(tag));
            if items.is_empty() {
                self.load_const(Value::Variant(Rc::new(VariantVal {
                    enum_ty,
                    tag: intern_sym(tag),
                    fields: Box::new([]),
                })));
                self.bind_def(tag, target.as_ref(), SlotKind::Let, line)?;
            } else {
                let fields = field_params(items);
                let shape = Shape {
                    ty: enum_ty,
                    tag: intern_sym(tag),
                    fields: fields
                        .iter()
                        .map(|p| intern_kw(p.name.as_deref().unwrap_or("")))
                        .collect(),
                    is_struct: false,
                };
                self.ctor(tag, line, fields, shape)?;
                self.bind_def(tag, target.as_ref(), SlotKind::Fn, line)?;
            }
            self.emit(Op::Pop);
        }
        self.load_const(Value::Nil);
        Ok(())
    }

    /// `struct NAME: field..`: the constructor `NAME`.
    pub(crate) fn def_struct(&mut self, n: &Node, top: bool) -> Result<(), Diagnostic> {
        let ch = &n.children;
        let (Some(name), Some(block)) = (ch.get(1).and_then(Node::sym_name), ch.get(2)) else {
            self.fail(FailCode::Type);
            return Ok(());
        };
        let fields = struct_fields(block);
        let ty = intern_sym(name);
        let shape = Shape {
            ty,
            tag: ty,
            fields: fields
                .iter()
                .map(|p| intern_kw(p.name.as_deref().unwrap_or("")))
                .collect(),
            is_struct: true,
        };
        let target = top.then(|| self.def_target(name));
        self.ctor(name, n, fields, shape)?;
        self.bind_def(name, target.as_ref(), SlotKind::Fn, n)
    }
}

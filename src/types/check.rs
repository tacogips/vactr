//! The checker entry (design 7, 7.1.1): `check` types one document's
//! expanded forms and returns every diagnostic. It never gates compile or
//! run and never returns early: a node it cannot type is `any`. The same
//! entry serves the compiler, the evaluator, the LSP and the tests, so this
//! module depends on nothing outside `reader/` and `types/`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::reader::node::{Atom, Node, NodeKind};
use crate::reader::span::{NodeId, Span};
use crate::types::diag::{DiagCode, Diagnostic};
use crate::types::manifest::HostManifest;
use crate::types::natives::NativeTable;
use crate::types::scope::{pattern_names, BindExtra, Binding, ScopeKind, Scopes};
use crate::types::ty::{BindKind, CallableSchema, CheckEnv, KeySet, Scheme, Ty, TypeId};
use crate::types::unify::Unifier;

/// The checker's output: a type per expression node (the `TypedInfo` side
/// table for hover) and the diagnostics, in the order found.
#[derive(Clone, Debug, Default)]
pub struct CheckResult {
    pub types: HashMap<NodeId, Ty>,
    pub diags: Vec<Diagnostic>,
    /// Successfully checked portable session function declarations.
    pub callables: BTreeMap<Rc<str>, CallableSchema>,
}

/// Checks the expanded forms of one document (a buffer, a spec block, or
/// one form from `eval_form`). All top-level forms share one session
/// scope; a name already in `env.globals` is the previous session state,
/// so binding it again at the top level is Live redefinition (no
/// diagnostic). Never panics and never stops early; every top-level form
/// gets a `types` entry.
#[must_use]
pub fn check(program: &[Node], env: &CheckEnv, manifest: &HostManifest) -> CheckResult {
    let mut cx = Checker::new(env, manifest);
    cx.prepass(program);
    for form in program {
        cx.too_deep_reported = false;
        let t = match &form.kind {
            NodeKind::Import(decl) => {
                cx.import_collision(&decl.prefix, decl.open, form.span);
                Ty::Nil
            }
            _ => cx.infer(form, 0),
        };
        cx.types.insert(form.id, t);
    }
    let mut callables = BTreeMap::new();
    if !cx
        .diags
        .iter()
        .any(|d| d.severity == crate::types::diag::Severity::Error)
    {
        for form in program {
            if !matches!(form.kind, NodeKind::Call)
                || !matches!(
                    form.children.first().and_then(Node::sym_name),
                    Some("fn" | "let")
                )
            {
                continue;
            }
            let Some(name) = form.children.get(1).and_then(Node::sym_name) else {
                continue;
            };
            let Some(binding) = cx.scopes.session_lookup(name) else {
                continue;
            };
            let Ty::Fn(params, result) = &binding.scheme.ty else {
                continue;
            };
            let positional = params.len();
            let mut all = params.to_vec();
            let mut names = Vec::new();
            if let BindExtra::Fn { keywords, .. } = &binding.extra {
                for (key, ty) in keywords.iter() {
                    names.push(key.clone());
                    all.push(ty.clone());
                }
            }
            let full = Ty::func(all, (**result).clone());
            cx.u.default_nums(&full);
            let signature = cx.u.generalize(&full);
            let schema = CallableSchema {
                positional,
                keywords: names,
                signature,
            };
            if schema.portable() {
                callables.insert(Rc::from(name), schema);
            }
        }
    }
    let mut result = cx.finish();
    result.callables = callables;
    result
}

/// A struct field or an enum variant field.
#[derive(Clone, Debug)]
pub(crate) struct Field {
    pub(crate) name: Rc<str>,
    pub(crate) ty: Ty,
}

/// A declared struct or enum.
#[derive(Clone, Debug)]
pub(crate) enum TypeDef {
    Struct {
        name: Rc<str>,
        fields: Vec<Field>,
    },
    Enum {
        name: Rc<str>,
        variants: Vec<(Rc<str>, Vec<Field>)>,
    },
}

/// The recursion bound over the expanded tree (7.1.5).
pub(crate) const MAX_CHECK_DEPTH: u32 = 1024;

/// The state of one `check` run.
pub(crate) struct Checker<'a> {
    /// The session state before the document (Live redefinition).
    pub(crate) env: &'a CheckEnv,
    /// `env` plus the document's session bindings so far, with their
    /// masks: what `infer_masks` and the forcing checks resolve against.
    pub(crate) live: CheckEnv,
    pub(crate) manifest: &'a HostManifest,
    pub(crate) table: &'static NativeTable,
    pub(crate) u: Unifier,
    pub(crate) scopes: Scopes,
    pub(crate) types: HashMap<NodeId, Ty>,
    pub(crate) diags: Vec<Diagnostic>,
    pub(crate) defs: Vec<TypeDef>,
    /// Declaring node of each struct/enum, so a nested one registers once.
    pub(crate) def_nodes: HashMap<NodeId, TypeId>,
    /// Struct and enum names in the document.
    pub(crate) type_names: BTreeMap<Rc<str>, TypeId>,
    /// Variant name -> (enum, variant index), for every enum in the
    /// document, so a `match` may precede its enum.
    pub(crate) variants: BTreeMap<Rc<str>, (TypeId, usize)>,
    /// `inst` names of the document (valid sound keywords, 7.1.4).
    pub(crate) inst_names: BTreeSet<Rc<str>>,
    /// `inst` header parameter names: control names (M3).
    pub(crate) inst_controls: BTreeSet<Rc<str>>,
    /// The keys of `default-sound-kit`.
    pub(crate) kit_keys: KeySet,
    /// Above 0 every diagnostic is dropped (`inst`/`look` bodies).
    pub(crate) mute: u32,
    /// Above 0 `type-mismatch` is dropped (the arguments of a
    /// `sound-not-first` call already carry one error).
    pub(crate) mute_types: u32,
    /// `nesting-too-deep` is reported once per top-level form.
    pub(crate) too_deep_reported: bool,
}

impl<'a> Checker<'a> {
    fn new(env: &'a CheckEnv, manifest: &'a HostManifest) -> Checker<'a> {
        Checker {
            env,
            live: env.clone(),
            manifest,
            table: NativeTable::global(),
            u: Unifier::new(),
            scopes: Scopes::new(),
            types: HashMap::new(),
            diags: Vec::new(),
            defs: Vec::new(),
            def_nodes: HashMap::new(),
            type_names: BTreeMap::new(),
            variants: BTreeMap::new(),
            inst_names: BTreeSet::new(),
            inst_controls: manifest
                .template_params()
                .into_iter()
                .map(Rc::from)
                .chain(match &manifest.declared_controls {
                    KeySet::Of(names) => names.iter().cloned().collect::<Vec<_>>(),
                    KeySet::Open => Vec::new(),
                })
                .collect(),
            kit_keys: manifest.sound_kit_keys(),
            mute: 0,
            mute_types: 0,
            too_deep_reported: false,
        }
    }

    /// Reports a diagnostic with the code's default severity (7.1.6).
    pub(crate) fn emit(&mut self, code: DiagCode, span: Span, message: impl Into<String>) {
        if self.mute > 0 || (self.mute_types > 0 && code == DiagCode::TypeMismatch) {
            return;
        }
        self.diags.push(Diagnostic {
            span,
            severity: code.default_severity(),
            code,
            message: message.into(),
            origin: None,
        });
    }

    /// Registers the document's structs, enums and `inst` names before any
    /// form is checked: names first, then fields (which may name types).
    fn prepass(&mut self, program: &[Node]) {
        let heads: Vec<(&Node, &str)> = program
            .iter()
            .filter_map(|f| Some((f, def_head(f)?)))
            .collect();
        for (form, head) in &heads {
            match *head {
                "enum" | "struct" => {
                    if let Some(name) = form.children.get(1).and_then(Node::sym_name) {
                        let id = TypeId::new(u32::try_from(self.defs.len()).unwrap_or(u32::MAX));
                        self.def_nodes.insert(form.id, id);
                        self.type_names.insert(Rc::from(name), id);
                        self.defs.push(TypeDef::Struct {
                            name: Rc::from(name),
                            fields: Vec::new(),
                        });
                    }
                }
                "inst" => {
                    if let Some(name) = form.children.get(1).and_then(def_name) {
                        self.inst_names.insert(name);
                    }
                    let header = form.children.iter().skip(2);
                    for item in header.filter(|i| matches!(i.kind, NodeKind::Pair)) {
                        if let Some(name) = def_name(item) {
                            self.inst_controls.insert(name);
                        }
                    }
                }
                _ => {}
            }
        }
        for (form, head) in heads {
            if let Some(id) = self.def_nodes.get(&form.id).copied() {
                self.fill_def(form, head, id);
            }
        }
    }

    /// Registers a struct or enum met outside the prepass (a nested one).
    pub(crate) fn def_id(&mut self, form: &Node, head: &str) -> Option<TypeId> {
        if let Some(id) = self.def_nodes.get(&form.id) {
            return Some(*id);
        }
        let name = form.children.get(1).and_then(Node::sym_name)?;
        let id = TypeId::new(u32::try_from(self.defs.len()).unwrap_or(u32::MAX));
        self.def_nodes.insert(form.id, id);
        self.type_names.insert(Rc::from(name), id);
        self.defs.push(TypeDef::Struct {
            name: Rc::from(name),
            fields: Vec::new(),
        });
        self.fill_def(form, head, id);
        Some(id)
    }

    /// Reads the fields of `struct NAME:` or the variants of `enum NAME:`.
    fn fill_def(&mut self, form: &Node, head: &str, id: TypeId) {
        let name: Rc<str> = Rc::from(form.children.get(1).and_then(Node::sym_name).unwrap_or(""));
        let lines: &[Node] = match form.children.get(2) {
            Some(b) if matches!(b.kind, NodeKind::Block) => &b.children,
            _ => &[],
        };
        let k = usize::try_from(id.get()).unwrap_or(usize::MAX);
        if head == "enum" {
            let mut variants = Vec::new();
            for line in lines.iter() {
                let (vname, fields) = match line.kind {
                    NodeKind::Call => (
                        line.children.first().and_then(Node::sym_name),
                        self.fields_of(line.children.get(1..).unwrap_or(&[])),
                    ),
                    _ => (line.sym_name(), Vec::new()),
                };
                if let Some(vname) = vname {
                    let vname: Rc<str> = Rc::from(vname);
                    self.variants.insert(vname.clone(), (id, variants.len()));
                    variants.push((vname, fields));
                }
            }
            if let Some(slot) = self.defs.get_mut(k) {
                *slot = TypeDef::Enum { name, variants };
            }
        } else {
            let fields = lines.iter().filter_map(|l| self.struct_field(l)).collect();
            if let Some(slot) = self.defs.get_mut(k) {
                *slot = TypeDef::Struct { name, fields };
            }
        }
    }

    /// Variant fields: `r`, `r: float`.
    fn fields_of(&self, items: &[Node]) -> Vec<Field> {
        items
            .iter()
            .filter_map(|item| match &item.kind {
                NodeKind::Atom(Atom::Sym(n)) => Some(Field {
                    name: n.clone(),
                    ty: Ty::Any,
                }),
                NodeKind::Pair => {
                    let (n, ty) = self.annotated(item)?;
                    Some(Field { name: n, ty })
                }
                _ => None,
            })
            .collect()
    }

    /// A struct line: `note`, `amp 1.0`, `note: int` or `amp: float 1.0`.
    fn struct_field(&self, line: &Node) -> Option<Field> {
        match &line.kind {
            NodeKind::Atom(Atom::Sym(n)) => Some(Field {
                name: n.clone(),
                ty: Ty::Any,
            }),
            NodeKind::Pair => {
                let (name, ty) = self.annotated(line)?;
                Some(Field { name, ty })
            }
            NodeKind::Call => match line.children.first() {
                Some(h) if h.sym_name().is_some() => Some(Field {
                    name: Rc::from(h.sym_name().unwrap_or("")),
                    ty: Ty::Any,
                }),
                Some(h) if matches!(h.kind, NodeKind::Pair) => {
                    let (name, ty) = self.annotated(h)?;
                    Some(Field { name, ty })
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// `name: type` read as a pair `[:name type]`.
    pub(crate) fn annotated(&self, pair: &Node) -> Option<(Rc<str>, Ty)> {
        let name = match &pair.children.first()?.kind {
            NodeKind::Atom(Atom::Keyword(n)) => n.clone(),
            _ => return None,
        };
        let ty = pair
            .children
            .get(1)
            .and_then(|t| self.annot_ty(t))
            .unwrap_or(Ty::Any);
        Some((name, ty))
    }

    /// A type written in an annotation: a type name, a struct or enum
    /// name, `[T]` or `[K: V]`. `None` when it is not a type.
    pub(crate) fn annot_ty(&self, n: &Node) -> Option<Ty> {
        match &n.kind {
            NodeKind::Atom(Atom::Sym(name)) => match self.type_names.get(&**name) {
                Some(id) => Some(Ty::Named(*id)),
                None => Scheme::parse(name).map(|s| s.ty),
            },
            NodeKind::Atom(Atom::Keyword(name)) => Scheme::parse(name).map(|s| s.ty),
            NodeKind::List if n.children.len() == 1 => {
                let c = &n.children[0];
                match c.kind {
                    NodeKind::Pair if c.children.len() == 2 => Some(Ty::Dict(
                        Box::new(self.annot_ty(&c.children[0])?),
                        Box::new(self.annot_ty(&c.children[1])?),
                    )),
                    _ => Some(Ty::List(Box::new(self.annot_ty(c)?))),
                }
            }
            _ => None,
        }
    }

    /// The struct or enum definition of `id`.
    pub(crate) fn def(&self, id: TypeId) -> Option<&TypeDef> {
        self.defs.get(usize::try_from(id.get()).ok()?)
    }

    /// The variant `name` as a pattern head: a document enum variant not
    /// shadowed by a user binding, or a session enum variant.
    pub(crate) fn variant(&self, name: &str) -> Option<(TypeId, usize)> {
        match self.scopes.lookup(name) {
            Some(b) => match b.extra {
                crate::types::scope::BindExtra::Variant { enum_id, .. } => {
                    self.variants.get(name).copied().or(Some((enum_id, 0)))
                }
                _ => None,
            },
            None => self.variants.get(name).copied(),
        }
    }

    /// True when `name` is a variant pattern head (document or session).
    pub(crate) fn is_variant_name(&self, name: &str) -> bool {
        self.variant(name).is_some()
            || (self.scopes.lookup(name).is_none()
                && self
                    .env
                    .global(name)
                    .is_some_and(|g| g.kind == BindKind::Enum))
    }

    /// The field count of a variant, when known.
    pub(crate) fn variant_fields(&self, enum_id: TypeId, index: usize) -> Option<&[Field]> {
        match self.def(enum_id)? {
            TypeDef::Enum { variants, .. } => variants.get(index).map(|(_, f)| f.as_slice()),
            TypeDef::Struct { .. } => None,
        }
    }

    /// `import-collision` (warning): an opened import exposes a name that
    /// the prelude or an earlier open also exposes (7.1.6).
    fn import_collision(&mut self, prefix: &Rc<str>, open: bool, span: Span) {
        if !open {
            return;
        }
        let names: Vec<Rc<str>> = match self.env.qualified.get(prefix) {
            Some(pkg) => pkg.keys().cloned().collect(),
            None => return,
        };
        for name in names {
            let earlier = self
                .live
                .opens
                .iter()
                .find(|(p, ns)| p != prefix && ns.contains(&name))
                .map(|(p, _)| p.clone());
            let msg = match earlier {
                Some(p) => format!("`{name}` is exposed by both `{p}` and `{prefix}`"),
                None if self.table.get(&name).is_some() => {
                    format!("`{name}` from `{prefix}` collides with the prelude `{name}`")
                }
                None => continue,
            };
            self.emit(DiagCode::ImportCollision, span, msg);
        }
        let exposed = self
            .env
            .qualified
            .get(prefix)
            .map(|pkg| pkg.keys().cloned().collect())
            .unwrap_or_default();
        self.live.opens.push((prefix.clone(), exposed));
    }

    fn finish(self) -> CheckResult {
        let types = self
            .types
            .iter()
            .map(|(id, t)| (*id, self.u.zonk(t, true)))
            .collect();
        CheckResult {
            types,
            diags: self.diags,
            callables: BTreeMap::new(),
        }
    }
}

/// The definition head of a top-level form: `enum`, `struct`, `inst` or
/// `look`.
pub(crate) fn def_head(form: &Node) -> Option<&'static str> {
    if !matches!(form.kind, NodeKind::Call) {
        return None;
    }
    match form.children.first()?.sym_name()? {
        "enum" => Some("enum"),
        "struct" => Some("struct"),
        "inst" => Some("inst"),
        "look" => Some("look"),
        _ => None,
    }
}

/// `bus :name {block}` or `master {block}`: a bus definition head, whose
/// block is a DSP body (12.8.6).
pub(crate) fn dsp_def(form: &Node) -> Option<&'static str> {
    let ch = &form.children;
    if !matches!(form.kind, NodeKind::Call) || !matches!(ch.last()?.kind, NodeKind::Block) {
        return None;
    }
    match (ch.first()?.sym_name()?, ch.len()) {
        ("master", 2) => Some("master"),
        ("bus", 3) if matches!(ch[1].kind, NodeKind::Atom(Atom::Keyword(_))) => Some("bus"),
        _ => None,
    }
}

/// The name of a definition: `pluck` or the key of `drum: sampler`.
pub(crate) fn def_name(n: &Node) -> Option<Rc<str>> {
    match &n.kind {
        NodeKind::Atom(Atom::Sym(name)) => Some(name.clone()),
        NodeKind::Pair => match &n.children.first()?.kind {
            NodeKind::Atom(Atom::Keyword(name)) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The definition heads `enum`, `struct`, `inst` and `look` (7.1.4).
impl Checker<'_> {
    /// `enum`, `struct`, `inst` and `look` (7.1.4 definition heads).
    pub(crate) fn definition(&mut self, n: &Node, d: u32) -> Ty {
        let head = def_head(n).or_else(|| dsp_def(n)).unwrap_or("");
        match head {
            "enum" => self.enum_form(n),
            "bus" | "master" => {
                // A bus body builds effect nodes: checked for the no-abort
                // property only, like an `inst` body.
                self.mute += 1;
                if let Some(block) = n.children.last() {
                    self.block(block, d, None);
                }
                self.mute -= 1;
                Ty::Nil
            }
            "struct" => {
                let Some(id) = self.def_id(n, "struct") else {
                    return Ty::Nil;
                };
                if let Some(block) = n.children.get(2) {
                    // Field defaults are expressions.
                    for line in block.children.iter() {
                        if matches!(line.kind, NodeKind::Call) {
                            for c in line.children.iter().skip(1) {
                                self.infer(c, d);
                            }
                        }
                    }
                }
                if let Some(name) = n.children.get(1).and_then(Node::sym_name) {
                    let mut b = Binding::mono(BindKind::Struct, Ty::Named(id), n.children[1].span);
                    b.extra = BindExtra::Struct(id);
                    self.declare(&Rc::from(name), &n.children[1], b);
                }
                Ty::Nil
            }
            _ => {
                // `inst`/`look`: header parameters are control names (M3),
                // and the body is checked for the no-abort property only.
                let name = n.children.get(1).and_then(def_name);
                if head == "inst" {
                    if let Some(name) = &name {
                        let b = Binding::mono(BindKind::Inst, Ty::Sound, n.children[1].span);
                        self.declare(name, &n.children[1], b);
                    }
                }
                self.mute += 1;
                let mark = self.scopes.depth();
                self.scopes.push(ScopeKind::Fn);
                for item in n.children.iter().skip(2) {
                    let mut names = Vec::new();
                    pattern_names(item, &|_| false, &mut names, 0);
                    if let NodeKind::Pair = item.kind {
                        if let Some(Atom::Keyword(k)) = item.children.first().and_then(atom_of) {
                            names.push((k.clone(), item));
                        }
                    }
                    for (pname, at) in names {
                        self.scopes
                            .bind(pname, Binding::mono(BindKind::Param, Ty::Any, at.span));
                    }
                }
                for item in n.children.iter().skip(2) {
                    match item.kind {
                        NodeKind::Block => {
                            self.block(item, d, None);
                        }
                        NodeKind::Atom(_) | NodeKind::Pair => {}
                        _ => {
                            self.infer(item, d);
                        }
                    }
                }
                self.scopes.truncate(mark);
                self.mute -= 1;
                // Building the instrument or look is TASK-008's.
                Ty::Any
            }
        }
    }

    /// `(enum NAME {variants})`: the enum and its variants are session
    /// bindings (variant names are global).
    fn enum_form(&mut self, n: &Node) -> Ty {
        let Some(id) = self.def_id(n, "enum") else {
            return Ty::Nil;
        };
        let named = Ty::Named(id);
        if let Some(name) = n.children.get(1).and_then(Node::sym_name) {
            let b = Binding::mono(BindKind::Enum, named.clone(), n.children[1].span);
            self.declare(&Rc::from(name), &n.children[1], b);
        }
        let variants = match self.def(id) {
            Some(TypeDef::Enum { variants, .. }) => variants.clone(),
            _ => Vec::new(),
        };
        let lines: Vec<&Node> = n
            .children
            .get(2)
            .map(|b| b.children.iter().collect())
            .unwrap_or_default();
        for (k, (vname, fields)) in variants.iter().enumerate() {
            let at = lines
                .get(k)
                .map(|l| match l.kind {
                    NodeKind::Call => l.children.first().unwrap_or(l),
                    _ => *l,
                })
                .unwrap_or(&n.children[1]);
            let ty = if fields.is_empty() {
                named.clone()
            } else {
                Ty::func(fields.iter().map(|f| f.ty.clone()).collect(), named.clone())
            };
            let mut b = Binding::mono(BindKind::Enum, ty, at.span);
            b.extra = BindExtra::Variant { enum_id: id };
            self.declare(vname, at, b);
        }
        Ty::Nil
    }
}

fn atom_of(n: &Node) -> Option<&Atom> {
    match &n.kind {
        NodeKind::Atom(a) => Some(a),
        _ => None,
    }
}

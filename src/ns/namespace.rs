//! Namespaces, late-bound slots and the prelude (design 5.6, 5.7).
//!
//! Scopes form the chain `prelude -> session -> fn/block` (20 Q1, Decided
//! 2026-09-25). The prelude is a read-only table of natives and prelude
//! values. A session binding of a prelude name is a NEW session slot that
//! shadows it; nothing writes a prelude slot. Function and block scopes are
//! the compiler's (frame locals). Name resolution happens at compile time
//! and fixes the slot a form reads.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use crate::ns::pkg::{ImportBinding, PkgNs};
use crate::ns::tweak::TweakTable;
use crate::types::masks::{CalleeRef, ForcingMask};
use crate::types::natives::{NativeKind, NativeSig, NativeTable};
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo};
use crate::value::intern::{intern_sym, name_of_sym, SymId};
use crate::value::value::{NativeId, Sound, Value};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::natives::NativeFn;

id_newtype!(
    /// The generation of a top-level form, bumped on each re-evaluation.
    FormGen(u64)
);

/// What introduced a slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SlotKind {
    /// A `let` (or a definition head: `enum`, `struct`, `inst`, `look`).
    Let,
    /// A `var`: the only kind `upd` may write.
    Var,
    Fn,
    /// An anonymous tweak slot (section 13).
    Tweak,
    /// A read-only prelude native or value.
    Prelude,
}

thread_local! {
    static NEXT_SLOT: Cell<u64> = const { Cell::new(1) };
}

/// The id the next slot will get: slots created from now on have ids at
/// or above it (Query mode uses this to tell its own local cells apart).
#[must_use]
pub fn slot_id_mark() -> u64 {
    NEXT_SLOT.with(Cell::get)
}

fn next_slot_id() -> u64 {
    NEXT_SLOT.with(|n| {
        let id = n.get();
        n.set(id.wrapping_add(1));
        id
    })
}

/// One slot. `version` increments on every write (5.6).
pub struct VarSlot {
    pub name: SymId,
    kind: Cell<SlotKind>,
    pub value: RefCell<Value>,
    pub version: Cell<u64>,
    bound: Cell<bool>,
    /// A frame-local `var` cell: never a top-level slot, never recorded
    /// as an eager read.
    local: bool,
    owner: Cell<Option<FormGen>>,
    id: u64,
}

/// A shared reference to a slot. `Value::VarRef` holds one (5.6).
#[derive(Clone)]
pub struct VarSlotRef(pub Rc<VarSlot>);

impl VarSlotRef {
    fn make(name: SymId, kind: SlotKind, value: Value, bound: bool, local: bool) -> VarSlotRef {
        VarSlotRef(Rc::new(VarSlot {
            name,
            kind: Cell::new(kind),
            value: RefCell::new(value),
            version: Cell::new(0),
            bound: Cell::new(bound),
            local,
            owner: Cell::new(None),
            id: next_slot_id(),
        }))
    }

    /// A bound top-level slot outside any namespace (tests, tweak slots).
    #[must_use]
    pub fn new(name: SymId, kind: SlotKind, value: Value) -> VarSlotRef {
        VarSlotRef::make(name, kind, value, true, false)
    }

    /// A frame-local `var` cell.
    #[must_use]
    pub fn local_cell(name: SymId, value: Value) -> VarSlotRef {
        VarSlotRef::make(name, SlotKind::Var, value, true, true)
    }

    /// The current value (a clone).
    #[must_use]
    pub fn get(&self) -> Value {
        self.0.value.borrow().clone()
    }

    #[must_use]
    pub fn name(&self) -> SymId {
        self.0.name
    }

    #[must_use]
    pub fn kind(&self) -> SlotKind {
        self.0.kind.get()
    }

    #[must_use]
    pub fn version(&self) -> u64 {
        self.0.version.get()
    }

    /// False for a name reserved by a forward reference but not yet defined.
    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.0.bound.get()
    }

    #[must_use]
    pub fn is_local(&self) -> bool {
        self.0.local
    }

    /// A process-unique identity, stable for the slot's lifetime.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.0.id
    }

    /// The form generation that last defined the slot.
    #[must_use]
    pub fn owner(&self) -> Option<FormGen> {
        self.0.owner.get()
    }

    /// True when both refer to the same slot.
    #[must_use]
    pub fn same(&self, other: &VarSlotRef) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Writes the value and bumps the version. No kind check: callers
    /// (definition, `upd` after its check, tweak writes) own the rule.
    pub fn set(&self, value: Value) {
        *self.0.value.borrow_mut() = value;
        self.0.version.set(self.0.version.get().wrapping_add(1));
    }

    /// Restores a value and version exactly (the reactive pass journal).
    pub fn restore(&self, value: Value, version: u64) {
        *self.0.value.borrow_mut() = value;
        self.0.version.set(version);
    }

    fn define(&self, kind: SlotKind, value: Value, gen: Option<FormGen>) {
        self.0.kind.set(kind);
        self.0.bound.set(true);
        self.0.owner.set(gen);
        self.set(value);
    }
}

impl fmt::Debug for VarSlotRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VarSlotRef({} {:?} v{})",
            name_of_sym(self.0.name),
            self.0.kind.get(),
            self.0.version.get()
        )
    }
}

/// A registered native: its signature and implementation.
#[derive(Clone, Copy)]
pub struct NativeEntry {
    pub sig: NativeSig,
    pub f: NativeFn,
}

impl fmt::Debug for NativeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NativeEntry({})", self.sig.name)
    }
}

/// The read-only prelude: natives registered against their `NativeTable`
/// entries, custom natives (tests) after the table ids, and prelude values.
#[derive(Debug, Default)]
pub struct Prelude {
    slots: BTreeMap<SymId, VarSlotRef>,
    natives: Vec<Option<NativeEntry>>,
    errors: Vec<String>,
}

impl Prelude {
    /// A prelude with nothing registered.
    #[must_use]
    pub fn empty() -> Prelude {
        Prelude::default()
    }

    /// The core prelude of this wave (lang-reference section 5).
    #[must_use]
    pub fn core() -> Prelude {
        let mut p = Prelude::empty();
        crate::vm::natives::register_core(&mut p);
        p
    }

    fn put_slot(&mut self, name: &str, value: Value) {
        let sym = intern_sym(name);
        let slot = VarSlotRef::new(sym, SlotKind::Prelude, value);
        self.slots.insert(sym, slot);
    }

    /// Registers the implementation of the table entry `name`. A missing
    /// entry or a prelude-value entry is recorded in `errors` (a test
    /// asserts there are none) instead of panicking.
    pub fn register(&mut self, name: &str, f: NativeFn) -> Option<NativeId> {
        let table = NativeTable::global();
        let Some((id, sig)) = table.get(name) else {
            self.errors
                .push(format!("`{name}` has no native table entry"));
            return None;
        };
        if sig.kind != NativeKind::Function {
            self.errors.push(format!("`{name}` is a prelude value"));
            return None;
        }
        let k = usize::try_from(id.get()).ok()?;
        if self.natives.len() <= k {
            self.natives.resize(k + 1, None);
        }
        if self.natives[k].is_some() {
            self.errors.push(format!("`{name}` is registered twice"));
        }
        self.natives[k] = Some(NativeEntry { sig: *sig, f });
        self.put_slot(name, Value::Native(id));
        Some(id)
    }

    /// Registers a native that is not in the table (tests; ids follow the
    /// table). Its signature supplies the arity and mask.
    pub fn register_custom(&mut self, sig: NativeSig, f: NativeFn) -> NativeId {
        let base = NativeTable::global().len();
        let k = self.natives.len().max(base);
        self.natives.resize(k + 1, None);
        self.natives[k] = Some(NativeEntry { sig, f });
        let id = NativeId::new(u32::try_from(k).unwrap_or(u32::MAX));
        self.put_slot(sig.name, Value::Native(id));
        id
    }

    /// Binds a prelude value (`default-sound-kit`, `sound-kit`, signals).
    pub fn register_value(&mut self, name: &str, value: Value) {
        self.put_slot(name, value);
    }

    /// The prelude slot of `name`.
    #[must_use]
    pub fn slot(&self, name: SymId) -> Option<VarSlotRef> {
        self.slots.get(&name).cloned()
    }

    /// The registered native `id`.
    #[must_use]
    pub fn native(&self, id: NativeId) -> Option<&NativeEntry> {
        let k = usize::try_from(id.get()).ok()?;
        self.natives.get(k)?.as_ref()
    }

    /// Every registered native, in id order.
    pub fn natives(&self) -> impl Iterator<Item = (NativeId, &NativeEntry)> + '_ {
        self.natives.iter().enumerate().filter_map(|(k, e)| {
            let id = NativeId::new(u32::try_from(k).ok()?);
            e.as_ref().map(|e| (id, e))
        })
    }

    /// Registration problems (a table test asserts this is empty).
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Every prelude name.
    pub fn names(&self) -> impl Iterator<Item = SymId> + '_ {
        self.slots.keys().copied()
    }
}

/// Where a name resolved (5.7 lookup order after locals).
#[derive(Clone, Debug)]
pub enum Resolved {
    Session(VarSlotRef),
    /// A name of the most recent opened import that exposes it.
    Open {
        prefix: SymId,
        slot: VarSlotRef,
    },
    Prelude(VarSlotRef),
}

impl Resolved {
    /// The slot.
    #[must_use]
    pub fn slot(&self) -> &VarSlotRef {
        match self {
            Resolved::Session(s) | Resolved::Open { slot: s, .. } | Resolved::Prelude(s) => s,
        }
    }
}

/// A session (or package, or `load`) namespace over a shared prelude.
#[derive(Debug)]
pub struct Namespace {
    prelude: Rc<Prelude>,
    session: RefCell<BTreeMap<SymId, VarSlotRef>>,
    /// Definition order of session names (first definition).
    order: RefCell<Vec<SymId>>,
    /// Import bindings in import order, each with its package namespace.
    imports: RefCell<Vec<(ImportBinding, Rc<PkgNs>)>>,
    tweaks: RefCell<TweakTable>,
    /// Console registers `_n` (14.5.10): kept apart from session names, so
    /// they are never in `session_names`, never rebound, and only a console
    /// read resolves them.
    console: RefCell<BTreeMap<u32, VarSlotRef>>,
}

impl Namespace {
    /// A fresh session whose parent is `prelude`.
    #[must_use]
    pub fn new(prelude: Prelude) -> Namespace {
        Namespace::with_prelude(Rc::new(prelude))
    }

    /// A fresh session sharing an existing prelude (`load`, packages).
    #[must_use]
    pub fn with_prelude(prelude: Rc<Prelude>) -> Namespace {
        Namespace {
            prelude,
            session: RefCell::new(BTreeMap::new()),
            order: RefCell::new(Vec::new()),
            imports: RefCell::new(Vec::new()),
            tweaks: RefCell::new(TweakTable::default()),
            console: RefCell::new(BTreeMap::new()),
        }
    }

    /// The shared prelude.
    #[must_use]
    pub fn prelude(&self) -> &Rc<Prelude> {
        &self.prelude
    }

    /// The session slot of `name`, bound or reserved.
    #[must_use]
    pub fn session_slot(&self, name: SymId) -> Option<VarSlotRef> {
        self.session.borrow().get(&name).cloned()
    }

    /// The value of the session binding `name` (not the prelude's).
    #[must_use]
    pub fn session_value(&self, name: &str) -> Option<Value> {
        self.session_slot(intern_sym(name))
            .filter(VarSlotRef::is_bound)
            .map(|s| s.get())
    }

    /// Resolves `name`: session, then opened imports (most recent first),
    /// then the prelude, then a reserved (unbound) session slot.
    #[must_use]
    pub fn lookup(&self, name: SymId) -> Option<Resolved> {
        let reserved = match self.session_slot(name) {
            Some(s) if s.is_bound() => return Some(Resolved::Session(s)),
            other => other,
        };
        for (binding, pkg) in self.imports.borrow().iter().rev() {
            if binding.open {
                if let Some(slot) = pkg.ns.session_slot(name).filter(VarSlotRef::is_bound) {
                    return Some(Resolved::Open {
                        prefix: binding.prefix,
                        slot,
                    });
                }
            }
        }
        if let Some(slot) = self.prelude.slot(name) {
            return Some(Resolved::Prelude(slot));
        }
        reserved.map(Resolved::Session)
    }

    /// `prefix.name` through the import bound to `prefix`.
    #[must_use]
    pub fn lookup_qualified(&self, prefix: SymId, name: SymId) -> Option<VarSlotRef> {
        let imports = self.imports.borrow();
        let (_, pkg) = imports.iter().rev().find(|(b, _)| b.prefix == prefix)?;
        pkg.ns.session_slot(name).filter(VarSlotRef::is_bound)
    }

    /// The session slot for `name`, reserving an unbound one if needed (a
    /// forward reference, or the target of a definition being compiled).
    pub fn reserve(&self, name: SymId) -> VarSlotRef {
        if let Some(s) = self.session_slot(name) {
            return s;
        }
        let slot = VarSlotRef::make(name, SlotKind::Let, Value::Nil, false, false);
        self.session.borrow_mut().insert(name, slot.clone());
        self.order.borrow_mut().push(name);
        slot
    }

    /// Defines `name` in the SESSION scope: a new slot, or Live
    /// redefinition of the existing session slot (never a prelude write).
    pub fn define(&self, name: SymId, kind: SlotKind, value: Value, gen: FormGen) -> VarSlotRef {
        let slot = self.reserve(name);
        slot.define(kind, value, Some(gen));
        slot
    }

    /// Defines through an already-resolved session slot (`DefGlobal`).
    pub fn define_slot(&self, slot: &VarSlotRef, kind: SlotKind, value: Value, gen: FormGen) {
        slot.define(kind, value, Some(gen));
    }

    /// Live redefinition of an existing bound session slot: replaces the
    /// value and bumps the version, keeping the kind.
    ///
    /// # Errors
    /// `undefined-name` when the session does not bind `name`.
    pub fn redefine(&self, name: SymId, value: Value) -> Result<VarSlotRef, Failure> {
        match self.session_slot(name).filter(VarSlotRef::is_bound) {
            Some(slot) => {
                slot.set(value);
                Ok(slot)
            }
            None => Err(Failure::new(
                FailCode::UndefinedName,
                format!("`{}` is not defined", name_of_sym(name)),
            )),
        }
    }

    /// `upd`: writes a `var` slot.
    ///
    /// # Errors
    /// `upd-immutable` for any other kind, prelude slots included;
    /// `undefined-name` for an unbound (reserved) slot.
    pub fn write_var(&self, slot: &VarSlotRef, value: Value) -> Result<(), Failure> {
        if !slot.is_bound() {
            return Err(Failure::new(
                FailCode::UndefinedName,
                format!("`{}` is not defined", name_of_sym(slot.name())),
            ));
        }
        if slot.kind() != SlotKind::Var {
            return Err(Failure::new(
                FailCode::UpdImmutable,
                format!("`{}` is not a var", name_of_sym(slot.name())),
            ));
        }
        slot.set(value);
        Ok(())
    }

    /// Binds an import. A binding with the same prefix or package replaces
    /// the earlier one, so re-import replaces the whole `PkgNs` (5.7).
    pub fn import(&self, binding: ImportBinding, pkg: Rc<PkgNs>) {
        let mut imports = self.imports.borrow_mut();
        imports.retain(|(b, _)| b.prefix != binding.prefix && b.pkg != binding.pkg);
        imports.push((binding, pkg));
    }

    /// The import bound to `prefix`.
    #[must_use]
    pub fn package(&self, prefix: SymId) -> Option<Rc<PkgNs>> {
        let imports = self.imports.borrow();
        imports
            .iter()
            .rev()
            .find(|(b, _)| b.prefix == prefix)
            .map(|(_, p)| Rc::clone(p))
    }

    /// Bound session names in definition order.
    #[must_use]
    pub fn session_names(&self) -> Vec<SymId> {
        let session = self.session.borrow();
        self.order
            .borrow()
            .iter()
            .filter(|n| session.get(n).is_some_and(VarSlotRef::is_bound))
            .copied()
            .collect()
    }

    /// Binds the console register `_n` to `v` (the REPL's result of entry
    /// `n`).
    pub fn set_console_register(&self, n: u32, v: Value) {
        self.console_slot(n).define(SlotKind::Let, v, None);
    }

    /// The value of the console register `_n`, when it is set.
    #[must_use]
    pub fn console_register(&self, n: u32) -> Option<Value> {
        self.console
            .borrow()
            .get(&n)
            .filter(|s| s.is_bound())
            .map(VarSlotRef::get)
    }

    /// The slot of the console register `_n`, reserving an unbound one (an
    /// unset register) if needed. The compiler reads registers through it.
    pub(crate) fn console_slot(&self, n: u32) -> VarSlotRef {
        self.console
            .borrow_mut()
            .entry(n)
            .or_insert_with(|| {
                let name = intern_sym(&format!("_{n}"));
                VarSlotRef::make(name, SlotKind::Let, Value::Nil, false, false)
            })
            .clone()
    }

    /// The tweak-site table.
    #[must_use]
    pub fn tweaks(&self) -> &RefCell<TweakTable> {
        &self.tweaks
    }

    /// The forcing mask of a callable value against this namespace.
    #[must_use]
    pub fn mask_of_value(&self, v: &Value) -> Option<ForcingMask> {
        let mut v = v.clone();
        for _ in 0..8 {
            match v {
                Value::Fn(c) => return Some(c.mask.clone()),
                Value::Native(id) => return self.native_mask(id),
                Value::VarRef(slot) => v = slot.get(),
                _ => return None,
            }
        }
        None
    }

    /// The mask of the native `id` (registered or table entry).
    #[must_use]
    pub fn native_mask(&self, id: NativeId) -> Option<ForcingMask> {
        match self.prelude.native(id) {
            Some(e) => Some(e.sig.forcing_mask()),
            None => NativeTable::global().sig(id).map(NativeSig::forcing_mask),
        }
    }

    /// The CURRENT mask a `Forward` link points at (the 5.5 chase lookup).
    #[must_use]
    pub fn callee_mask(&self, callee: &CalleeRef) -> Option<ForcingMask> {
        match callee {
            CalleeRef::Native(id) => self.native_mask(*id),
            CalleeRef::Global(name) => {
                let r = self.lookup(intern_sym(name))?;
                let slot = r.slot();
                if !slot.is_bound() {
                    return None;
                }
                self.mask_of_value(&slot.get())
            }
            CalleeRef::Qualified { prefix, name } => {
                let slot = self.lookup_qualified(intern_sym(prefix), intern_sym(name))?;
                self.mask_of_value(&slot.get())
            }
        }
    }

    fn global_info(&self, slot: &VarSlotRef) -> GlobalInfo {
        let kind = match slot.kind() {
            SlotKind::Var => BindKind::Var,
            // A realized `inst` is a sound (12.8.6).
            SlotKind::Fn if matches!(slot.get(), Value::Sound(s) if matches!(*s, Sound::Inst(_))) => {
                BindKind::Inst
            }
            SlotKind::Fn => BindKind::Fn,
            _ => BindKind::Let,
        };
        GlobalInfo {
            kind,
            scheme: None,
            mask: self.mask_of_value(&slot.get()),
            span: None,
        }
    }

    /// The session state as the checker and `infer_masks` see it: bound
    /// session names with their current masks, the import prefixes and the
    /// opened imports (oldest first).
    #[must_use]
    pub fn check_env(&self) -> CheckEnv {
        let mut env = CheckEnv::empty();
        for (name, slot) in self.session.borrow().iter() {
            if slot.is_bound() {
                env.globals
                    .insert(name_of_sym(*name), self.global_info(slot));
            }
        }
        for (binding, pkg) in self.imports.borrow().iter() {
            let prefix = name_of_sym(binding.prefix);
            let mut names = BTreeMap::new();
            for name in pkg.ns.session_names() {
                if let Some(slot) = pkg.ns.session_slot(name) {
                    names.insert(name_of_sym(name), pkg.ns.global_info(&slot));
                }
            }
            if binding.open {
                env.opens
                    .push((prefix.clone(), names.keys().cloned().collect()));
            }
            env.qualified.insert(prefix, names);
        }
        env
    }
}

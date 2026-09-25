//! The native signature table (design 7.1.3): one entry per prelude name in
//! scope for issue #2, indexed by `NativeId` in table order.
//!
//! The checker reads the schemes, `infer_masks` reads the masks, and the VM
//! registers each implementation against its entry. Core entries live here
//! (lang-reference section 5); domain entries (patterns, controls, signals,
//! sounds, visuals) live in `natives_domain.rs`. The synthesis, effect, bus
//! and granular vocabulary (TASK-008) and package loading (TASK-009) are
//! absent, so the checker reports them as `undefined-name`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::types::masks::{ForcingMask, MaskEntry};
use crate::types::natives_domain;
use crate::types::ty::{Scheme, Ty};
use crate::value::value::NativeId;

/// A prelude function, or a prelude value such as `sine` or `sound-kit`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NativeKind {
    Function,
    Value,
}

/// A native parameter's forcing. Natives are never `Forward` or
/// `Undetermined`, so this is a `const`-friendly subset of `MaskEntry`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NativeMask {
    Value,
    Fn,
    Late,
}

impl NativeMask {
    /// The mask entry.
    #[must_use]
    pub const fn entry(self) -> MaskEntry {
        match self {
            NativeMask::Value => MaskEntry::Value,
            NativeMask::Fn => MaskEntry::Fn,
            NativeMask::Late => MaskEntry::Late,
        }
    }
}

/// A host capability a native needs (`beyond-capability`, 7.1.6).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HostCap {
    MidiIn,
    MidiOut,
    Analysis,
    Render,
}

/// One entry of the table.
///
/// `ty` holds the schemes in the notation of `Scheme::parse`; more than one
/// only for the fixed overload group `scale` and `shape` (7.1.4), in
/// subject-type order. Parameters past `min_args` are optional. For a
/// variadic native (`max_args == None`) the last parameter type and the last
/// mask entry apply to every further argument. `keywords` are the named
/// arguments (`kit:` of `s`); they are not in the mask.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NativeSig {
    pub name: &'static str,
    pub kind: NativeKind,
    pub min_args: u8,
    pub max_args: Option<u8>,
    pub keywords: &'static [&'static str],
    pub ty: &'static [&'static str],
    pub mask: &'static [NativeMask],
    pub effectful: bool,
    pub needs: &'static [HostCap],
}

impl NativeSig {
    /// A function with `min..=max` positional arguments.
    #[must_use]
    pub const fn func(
        name: &'static str,
        min_args: u8,
        max_args: u8,
        ty: &'static [&'static str],
        mask: &'static [NativeMask],
    ) -> NativeSig {
        NativeSig {
            name,
            kind: NativeKind::Function,
            min_args,
            max_args: Some(max_args),
            keywords: &[],
            ty,
            mask,
            effectful: false,
            needs: &[],
        }
    }

    /// A prelude value of type `ty`.
    #[must_use]
    pub const fn value(name: &'static str, ty: &'static [&'static str]) -> NativeSig {
        NativeSig {
            name,
            kind: NativeKind::Value,
            min_args: 0,
            max_args: Some(0),
            keywords: &[],
            ty,
            mask: &[],
            effectful: false,
            needs: &[],
        }
    }

    /// No upper arity bound.
    #[must_use]
    pub const fn rest(self) -> NativeSig {
        NativeSig {
            max_args: None,
            ..self
        }
    }

    /// With named arguments.
    #[must_use]
    pub const fn kw(self, keywords: &'static [&'static str]) -> NativeSig {
        NativeSig { keywords, ..self }
    }

    /// A host-visible effect (staged; `effect-in-query` inside a query).
    #[must_use]
    pub const fn effect(self) -> NativeSig {
        NativeSig {
            effectful: true,
            ..self
        }
    }

    /// Needs these host capabilities.
    #[must_use]
    pub const fn needs(self, needs: &'static [HostCap]) -> NativeSig {
        NativeSig { needs, ..self }
    }

    /// The type schemes. An entry that does not parse (a table bug; a test
    /// rejects it) reads as `any` instead of panicking.
    #[must_use]
    pub fn schemes(&self) -> Vec<Scheme> {
        self.ty
            .iter()
            .map(|t| Scheme::parse(t).unwrap_or_else(|| Scheme::mono(Ty::Any)))
            .collect()
    }

    /// True for the `scale`/`shape` overload group.
    #[must_use]
    pub fn is_overloaded(&self) -> bool {
        self.ty.len() > 1
    }

    /// The mask as a `ForcingMask`.
    #[must_use]
    pub fn forcing_mask(&self) -> ForcingMask {
        ForcingMask(self.mask.iter().map(|m| m.entry()).collect())
    }

    /// The forcing of argument `pos`: past the mask, the last entry for a
    /// variadic native and `None` otherwise (an arity error).
    #[must_use]
    pub fn entry_at(&self, pos: usize) -> Option<NativeMask> {
        match self.mask.get(pos) {
            Some(m) => Some(*m),
            None if self.max_args.is_none() => self.mask.last().copied(),
            None => None,
        }
    }
}

/// The prelude table, built once from the static entry lists.
#[derive(Debug)]
pub struct NativeTable {
    entries: Vec<&'static NativeSig>,
    index: BTreeMap<&'static str, NativeId>,
}

impl NativeTable {
    /// The table of every in-scope prelude name.
    #[must_use]
    pub fn global() -> &'static NativeTable {
        static TABLE: OnceLock<NativeTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            let lists: [&'static [NativeSig]; 2] = [CORE, natives_domain::DOMAIN];
            let mut table = NativeTable {
                entries: Vec::new(),
                index: BTreeMap::new(),
            };
            for sig in lists.into_iter().flatten() {
                let id = NativeId::new(u32::try_from(table.entries.len()).unwrap_or(u32::MAX));
                table.entries.push(sig);
                table.index.entry(sig.name).or_insert(id);
            }
            table
        })
    }

    /// The entry named `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<(NativeId, &'static NativeSig)> {
        let id = *self.index.get(name)?;
        Some((id, self.sig(id)?))
    }

    /// The entry with this id.
    #[must_use]
    pub fn sig(&self, id: NativeId) -> Option<&'static NativeSig> {
        let k = usize::try_from(id.get()).ok()?;
        self.entries.get(k).copied()
    }

    /// Every entry in id order.
    pub fn iter(&self) -> impl Iterator<Item = (NativeId, &'static NativeSig)> + '_ {
        self.entries
            .iter()
            .enumerate()
            .map(|(k, sig)| (NativeId::new(u32::try_from(k).unwrap_or(u32::MAX)), *sig))
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of distinct names (equal to `len` when names are unique).
    #[must_use]
    pub fn name_count(&self) -> usize {
        self.index.len()
    }
}

use NativeMask::{Fn as F, Value as V};

const fn f(
    name: &'static str,
    min: u8,
    max: u8,
    ty: &'static [&'static str],
    mask: &'static [NativeMask],
) -> NativeSig {
    NativeSig::func(name, min, max, ty, mask)
}

/// Named arguments of `once`: the offset, and any control as an override
/// (`once {s :bd} gain: g`, lang-reference section 3).
const ONCE_KEYWORDS: [&str; natives_domain::CONTROLS.len() + 1] = {
    let mut out = [""; natives_domain::CONTROLS.len() + 1];
    out[0] = "at";
    let mut k = 0;
    while k < natives_domain::CONTROLS.len() {
        out[k + 1] = natives_domain::CONTROLS[k];
        k += 1;
    }
    out
};

const LIST_FN: &[NativeMask] = &[V, F];
const ARITH: &[&str] = &["fn 'a 'a -> 'a"];
const COMPARE: &[&str] = &["fn 'a 'a -> bool"];
const SLOT: &[&str] = &["fn 'a -> 'a"];

/// The lang-reference section 5 core prelude, the operators, and the
/// expander's `Builtin` heads (`or`, `neg`, `concat`, `map`; 6.5.5).
/// Arithmetic schemes are `'a`-generic: the numeric lattice (int, float,
/// ratio widening, 6.5.3) is the unifier's (ME-CHECK).
static CORE: &[NativeSig] = &[
    // Operators (lang-reference section 1; `->`, `&`, `|`, `?` are syntax).
    f("+", 1, 2, ARITH, &[V, V]).rest(),
    f("-", 1, 2, ARITH, &[V, V]).rest(),
    f("*", 1, 2, ARITH, &[V, V]).rest(),
    f("/", 1, 2, ARITH, &[V, V]).rest(),
    f("=", 2, 2, COMPARE, &[V, V]),
    f("<", 2, 2, COMPARE, &[V, V]),
    f(">", 2, 2, COMPARE, &[V, V]),
    f("<=", 2, 2, COMPARE, &[V, V]),
    f(">=", 2, 2, COMPARE, &[V, V]),
    f("..", 1, 2, &["fn int int -> [int]"], &[V, V]),
    // Truthiness (lang-reference section 2; `?` expands to `or`).
    f("or", 1, 1, &["fn 'a -> 'a"], &[V]).rest(),
    f("and", 1, 1, &["fn 'a -> 'a"], &[V]).rest(),
    f("not", 1, 1, &["fn any -> bool"], &[V]),
    // Lists and dicts.
    f("put", 1, 2, &["fn 'a any -> 'a"], &[V, V]).rest(),
    f("join", 1, 1, &["fn ['a] -> 'a"], &[V]),
    f("len", 1, 1, &["fn any -> int"], &[V]),
    f("first", 1, 1, &["fn ['a] -> ?'a"], &[V]),
    f("last", 1, 1, &["fn ['a] -> ?'a"], &[V]),
    f("tail", 1, 1, &["fn ['a] -> ['a]"], &[V]),
    f("reverse", 1, 1, &["fn ['a] -> ['a]"], &[V]),
    f("sort", 1, 1, &["fn ['a] -> ['a]"], &[V]),
    f("map", 2, 2, &["fn ['a] (fn 'a -> 'b) -> ['b]"], LIST_FN),
    f("filter", 2, 2, &["fn ['a] (fn 'a -> 'b) -> ['a]"], LIST_FN),
    f(
        "reduce",
        3,
        3,
        &["fn ['a] 'b (fn 'b 'a -> 'b) -> 'b"],
        &[V, V, F],
    ),
    f("find", 2, 2, &["fn ['a] (fn 'a -> 'b) -> ?'a"], LIST_FN),
    f("any", 2, 2, &["fn ['a] (fn 'a -> 'b) -> bool"], LIST_FN),
    f("all", 2, 2, &["fn ['a] (fn 'a -> 'b) -> bool"], LIST_FN),
    f("take", 2, 2, &["fn ['a] int -> ['a]"], &[V, V]),
    f(
        "take-while",
        2,
        2,
        &["fn ['a] (fn 'a -> 'b) -> ['a]"],
        LIST_FN,
    ),
    f("drop", 2, 2, &["fn ['a] int -> ['a]"], &[V, V]),
    f("enumerate", 1, 1, &["fn ['a] -> [[any]]"], &[V]),
    f("repeat", 2, 2, &["fn 'a int -> ['a]"], &[V, V]),
    f("dict", 1, 1, &["fn [any] -> [any: any]"], &[V]),
    // Values.
    f("is-nil", 1, 1, &["fn any -> bool"], &[V]),
    f("is-list", 1, 1, &["fn any -> bool"], &[V]),
    f("int", 1, 1, &["fn 'a -> int"], &[V]),
    f("int64", 1, 1, &["fn 'a -> int64"], &[V]),
    f("float", 1, 1, &["fn 'a -> float"], &[V]),
    f("round", 1, 1, &["fn 'a -> int"], &[V]),
    f("neg", 1, 1, &["fn 'a -> 'a"], &[V]),
    f("mod", 2, 2, ARITH, &[V, V]),
    f("sin", 1, 1, &["fn 'a -> 'a"], &[V]),
    f("cos", 1, 1, &["fn 'a -> 'a"], &[V]),
    f("min", 1, 1, &["fn 'a -> 'a"], &[V]).rest(),
    f("max", 1, 1, &["fn 'a -> 'a"], &[V]).rest(),
    f("abs", 1, 1, &["fn 'a -> 'a"], &[V]),
    // Console and strings (`concat` is the interpolation head).
    f("print", 0, 1, &["fn any -> nil"], &[V]).rest().effect(),
    f("concat", 0, 1, &["fn any -> string"], &[V]).rest(),
    // Time and slots (`bus`, `master` are TASK-008).
    f("use-bpm", 1, 1, &["fn 'a -> nil"], &[V]).effect(),
    f("use-cycle", 1, 1, &["fn 'a -> nil"], &[V]).effect(),
    f("use-clock", 1, 1, &["fn keyword -> nil"], &[V]).effect(),
    f("midi-clock-out", 1, 1, &["fn bool -> nil"], &[V])
        .effect()
        .needs(&[HostCap::MidiOut]),
    f("once", 1, 1, &["fn 'a -> nil"], &[V])
        .kw(&ONCE_KEYWORDS)
        .effect(),
    f("at", 2, 2, &["fn 'a (fn -> 'b) -> nil"], &[V, F]).effect(),
    f("stop", 1, 1, &["fn keyword -> nil"], &[V]).effect(),
    f("hush", 0, 0, &["fn -> nil"], &[]).effect(),
    f("d1", 1, 1, SLOT, &[V]).effect(),
    f("d2", 1, 1, SLOT, &[V]).effect(),
    f("d3", 1, 1, SLOT, &[V]).effect(),
    f("d4", 1, 1, SLOT, &[V]).effect(),
    f("d5", 1, 1, SLOT, &[V]).effect(),
    f("d6", 1, 1, SLOT, &[V]).effect(),
    f("d7", 1, 1, SLOT, &[V]).effect(),
    f("d8", 1, 1, SLOT, &[V]).effect(),
    f("d9", 1, 1, SLOT, &[V]).effect(),
    f("slot", 2, 2, &["fn keyword 'a -> 'a"], &[V, V])
        .kw(&natives_domain::CONTROLS)
        .effect(),
    // Source loading (7.1.3, 7.1.4): a fresh result variable per call site.
    f("load", 1, 1, &["fn path -> 'a"], &[V]).effect(),
];

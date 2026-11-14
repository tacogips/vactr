//! Keyword and identifier interning (design 5.2, 6.5.2).
//!
//! The table is thread-local to the evaluator thread, so `Ord for Key` can
//! resolve keyword names without a context argument. An id has no meaning
//! on another thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

id_newtype!(
    /// An interned keyword (`:kick`).
    KwId(u32)
);

id_newtype!(
    /// An interned identifier.
    SymId(u32)
);

/// A string interner: each distinct text gets one `u32` id.
#[derive(Debug, Default)]
pub struct Interner {
    names: Vec<Rc<str>>,
    map: HashMap<Rc<str>, u32>,
}

impl Interner {
    /// An empty interner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the id of `text`, adding it on first use.
    pub fn intern(&mut self, text: &str) -> u32 {
        if let Some(&id) = self.map.get(text) {
            return id;
        }
        // More than u32::MAX distinct names cannot be addressed; the last id is reused.
        let id = u32::try_from(self.names.len()).unwrap_or(u32::MAX);
        let name: Rc<str> = Rc::from(text);
        self.names.push(Rc::clone(&name));
        self.map.insert(name, id);
        id
    }

    /// The text of `id`, or `None` for an unknown id.
    #[must_use]
    pub fn resolve(&self, id: u32) -> Option<Rc<str>> {
        let idx = usize::try_from(id).ok()?;
        self.names.get(idx).cloned()
    }
}

thread_local! {
    static INTERNER: RefCell<Interner> = RefCell::new(Interner::new());
}

fn intern_raw(text: &str) -> u32 {
    INTERNER
        .try_with(|cell| {
            cell.try_borrow_mut()
                .map(|mut i| i.intern(text))
                .unwrap_or(u32::MAX)
        })
        .unwrap_or(u32::MAX)
}

fn name_raw(id: u32) -> Rc<str> {
    INTERNER
        .try_with(|cell| cell.try_borrow().ok().and_then(|i| i.resolve(id)))
        .ok()
        .flatten()
        .unwrap_or_else(|| Rc::from(""))
}

/// Interns a keyword name (without the leading `:`).
#[must_use]
pub fn intern_kw(name: &str) -> KwId {
    KwId::new(intern_raw(name))
}

/// Interns an identifier.
#[must_use]
pub fn intern_sym(name: &str) -> SymId {
    SymId::new(intern_raw(name))
}

/// The name of a keyword; the empty string for an unknown id.
#[must_use]
pub fn name_of_kw(id: KwId) -> Rc<str> {
    name_raw(id.get())
}

/// The name of an identifier; the empty string for an unknown id.
#[must_use]
pub fn name_of_sym(id: SymId) -> Rc<str> {
    name_raw(id.get())
}

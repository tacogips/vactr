//! Context-aware completion shared by native, LSP and wasm consumers.

mod context;
mod rank;
mod scope;
mod sources;

use std::cell::RefCell;

use crate::types::manifest::HostManifest;
use serde::Serialize;

pub const DEFAULT_LIMIT: usize = 100;
pub const MAX_LIMIT: usize = 500;
pub const STATUS_OK: u32 = 0;
pub const STATUS_NOT_UTF8: u32 = 2;
pub const STATUS_BAD_CURSOR: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextKind {
    None,
    Keyword,
    Qualified,
    #[serde(rename = "pipe")]
    PipeTarget,
    Head,
    PairKey,
    Argument,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateKind {
    Local,
    Function,
    Value,
    Variable,
    Type,
    Keyword,
    Control,
    Key,
    Module,
    Qualified,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub label: String,
    pub kind: CandidateKind,
    pub detail: String,
    pub insert: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Completion {
    pub context: ContextKind,
    pub from: usize,
    pub to: usize,
    pub items: Vec<Candidate>,
    pub incomplete: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageNames {
    pub prefix: String,
    pub path: String,
    pub names: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocName {
    pub label: String,
    pub kind: CandidateKind,
    pub detail: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub manifest: HostManifest,
    pub packages: Vec<PackageNames>,
    pub document: Vec<DocName>,
}
impl Snapshot {
    #[must_use]
    pub fn builtin() -> Snapshot {
        Snapshot {
            manifest: HostManifest::spec_default(),
            packages: Vec::new(),
            document: Vec::new(),
        }
    }
}
impl Completion {
    #[must_use]
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct Json<'a> {
            v: u8,
            context: ContextKind,
            from: usize,
            to: usize,
            incomplete: bool,
            items: &'a [Candidate],
        }
        serde_json::to_string(&Json {
            v: 1,
            context: self.context,
            from: self.from,
            to: self.to,
            incomplete: self.incomplete,
            items: &self.items,
        })
        .unwrap_or_else(|_| {
            "{\"v\":1,\"context\":\"none\",\"from\":0,\"to\":0,\"incomplete\":false,\"items\":[]}"
                .to_owned()
        })
    }
}

#[must_use]
pub fn complete(text: &str, cursor: usize, snap: &Snapshot, limit: usize) -> Completion {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor = cursor.saturating_sub(1);
    }
    let scanned = context::scan(text, cursor);
    if scanned.context == ContextKind::None {
        return Completion {
            context: ContextKind::None,
            from: scanned.from,
            to: cursor,
            items: Vec::new(),
            incomplete: false,
        };
    }
    let locals = scope::locals(text, cursor);
    let mut ranked = sources::candidates(text, cursor, snap, scanned.context, &locals);
    let prefix = text.get(scanned.from..cursor).unwrap_or("");
    let max = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.clamp(1, MAX_LIMIT)
    };
    let (items, incomplete) = rank::rank(&mut ranked, prefix, scanned.context, max);
    Completion {
        context: scanned.context,
        from: scanned.from,
        to: cursor,
        items,
        incomplete,
    }
}

thread_local! { static BUILTIN: RefCell<Option<Snapshot>> = const { RefCell::new(None) }; }
#[must_use]
pub fn complete_bytes(input: &[u8], cursor: u32, limit: u32) -> (u32, Vec<u8>) {
    let Ok(text) = std::str::from_utf8(input) else {
        return (STATUS_NOT_UTF8, Vec::new());
    };
    if usize::try_from(cursor).map_or(true, |n| n > input.len()) {
        return (STATUS_BAD_CURSOR, Vec::new());
    }
    let output = BUILTIN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let snapshot = slot.get_or_insert_with(Snapshot::builtin);
        complete(
            text,
            cursor as usize,
            snapshot,
            if limit == 0 {
                DEFAULT_LIMIT
            } else {
                limit as usize
            },
        )
        .to_json()
        .into_bytes()
    });
    (STATUS_OK, output)
}

#[cfg(test)]
mod tests;

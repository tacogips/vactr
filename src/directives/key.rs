//! Binding identity (design 13.5 "the binding key is the FULL resolved
//! selector path", 14.5.8).
//!
//! `BindingKey { label, site: Option<(name, ordinal)>, param }`. The label is
//! position-free, the param lexical, and the ordinal is re-derived per
//! revision: `KeyTable::migrate` maps each keyed site's last span through the
//! `doc-changed` change set and counts the same-named call sites again. A
//! clean mapping onto a same-named site migrates the key (ordinal
//! rewritten); anything else marks it `Stale`. It never guesses.

use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use crate::directives::DirectiveTable;
use crate::reader::span::Span;
use crate::session::changes::{ChangeSet, Mapped};

/// A labeled binding's document-scoped identity.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BindingKey {
    pub label: Rc<str>,
    /// `None` for a declared-definition parameter; `Some((call-site name,
    /// 1-based ordinal))` for a parameter of a call site.
    pub site: Option<(Rc<str>, u16)>,
    pub param: Rc<str>,
}

impl BindingKey {
    /// A definition-parameter key (`analog.cutoff`).
    #[must_use]
    pub fn param(label: &str, param: &str) -> BindingKey {
        BindingKey {
            label: Rc::from(label),
            site: None,
            param: Rc::from(param),
        }
    }

    /// A call-site parameter key (`hats.lpf.2.cutoff`).
    #[must_use]
    pub fn site(label: &str, site: &str, ordinal: u16, param: &str) -> BindingKey {
        BindingKey {
            label: Rc::from(label),
            site: Some((Rc::from(site), ordinal)),
            param: Rc::from(param),
        }
    }
}

impl fmt::Display for BindingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.site {
            Some((site, n)) => write!(f, "{}.{site}.{n}.{}", self.label, self.param),
            None => write!(f, "{}.{}", self.label, self.param),
        }
    }
}

/// Why a key spelling does not parse.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyParseError(pub String);

impl fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not `label.param` or `label.site.n.param`",
            self.0
        )
    }
}

impl std::error::Error for KeyParseError {}

impl FromStr for BindingKey {
    type Err = KeyParseError;

    fn from_str(s: &str) -> Result<BindingKey, KeyParseError> {
        let parts: Vec<&str> = s.split('.').collect();
        let bad = || KeyParseError(s.to_string());
        if parts.iter().any(|p| p.is_empty()) {
            return Err(bad());
        }
        match parts.as_slice() {
            [label, param] => Ok(BindingKey::param(label, param)),
            [label, site, n, param] => {
                let n: u16 = n.parse().ok().filter(|n| *n >= 1).ok_or_else(bad)?;
                Ok(BindingKey::site(label, site, n, param))
            }
            _ => Err(bad()),
        }
    }
}

/// A binding's identity: its key, or positional provenance (a call-site
/// head or parameter span) when it has no usable label.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum BindingIdent {
    Key(BindingKey),
    Positional { span: (u32, u32), param: Rc<str> },
}

impl BindingIdent {
    /// A positional identity at `span`.
    #[must_use]
    pub fn positional(span: Span, param: &str) -> BindingIdent {
        BindingIdent::Positional {
            span: (span.start, span.end),
            param: Rc::from(param),
        }
    }
}

/// Whether a key is still bound to a site.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum KeyState {
    Live,
    /// The mapping broke: data kept, re-confirmation required.
    Stale,
}

/// One tracked key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyEntry {
    /// The last resolved span (call-site head or parameter).
    pub span: (u32, u32),
    /// The revision the span belongs to.
    pub revision: u64,
    pub state: KeyState,
}

/// What `migrate` did to one key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyMove {
    pub from: BindingKey,
    pub to: BindingKey,
    pub state: KeyState,
}

/// Each key's last resolved span and revision.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyTable {
    entries: BTreeMap<BindingKey, KeyEntry>,
    /// Stale keys whose spelling a migrated Live key took over; kept, never
    /// dropped silently.
    superseded: Vec<(BindingKey, KeyEntry)>,
    revision: u64,
}

impl KeyTable {
    /// A table holding every key `table` resolves, at `revision`.
    #[must_use]
    pub fn from_table(table: &DirectiveTable, revision: u64) -> KeyTable {
        let mut keys = KeyTable {
            entries: BTreeMap::new(),
            superseded: Vec::new(),
            revision,
        };
        keys.record_new(table);
        keys
    }

    /// The revision of the latest recorded spans.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The entry of `key`.
    #[must_use]
    pub fn get(&self, key: &BindingKey) -> Option<&KeyEntry> {
        self.entries.get(key)
    }

    /// The state of `key` (`None` when untracked).
    #[must_use]
    pub fn state(&self, key: &BindingKey) -> Option<KeyState> {
        self.entries.get(key).map(|e| e.state)
    }

    /// Every tracked key, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&BindingKey, &KeyEntry)> {
        self.entries.iter()
    }

    /// Stale keys whose spelling a migrated Live key took over.
    #[must_use]
    pub fn superseded(&self) -> &[(BindingKey, KeyEntry)] {
        &self.superseded
    }

    /// Adds the keys of `table` not tracked yet, Live at this revision.
    fn record_new(&mut self, table: &DirectiveTable) {
        for r in &table.resolved {
            if let Some(key) = &r.key {
                self.entries.entry(key.clone()).or_insert(KeyEntry {
                    span: (r.target_span.start, r.target_span.end),
                    revision: self.revision,
                    state: KeyState::Live,
                });
            }
        }
    }

    /// Migrates every key to the next revision; see [`KeyTable::migrate_moves`].
    pub fn migrate(
        &mut self,
        changes: &ChangeSet,
        new_table: &DirectiveTable,
    ) -> Vec<(BindingKey, KeyState)> {
        self.migrate_moves(changes, new_table)
            .into_iter()
            .map(|m| (m.to, m.state))
            .collect()
    }

    /// Maps each Live keyed span through `changes` and re-resolves it
    /// against `new_table` (the next revision):
    /// - a definition-parameter key stays Live while its label still
    ///   resolves to a definition declaring the parameter;
    /// - a call-site key whose head span maps cleanly onto a same-named call
    ///   site under the same label MIGRATES, its ordinal re-counted;
    /// - a `Touched` mapping, or one that does not land on a same-named
    ///   site, marks the key `Stale` and keeps its data.
    ///
    /// Stale keys stay stale. Keys new in `new_table` are added Live. The
    /// result lists what happened to every previously tracked key.
    pub fn migrate_moves(
        &mut self,
        changes: &ChangeSet,
        new_table: &DirectiveTable,
    ) -> Vec<KeyMove> {
        let next = self.revision.saturating_add(1);
        let old = std::mem::take(&mut self.entries);
        let mut live = Vec::new();
        let mut stale = Vec::new();
        for (key, entry) in old {
            let found = if entry.state == KeyState::Stale {
                None
            } else {
                migrate_one(&key, &entry, changes, new_table)
            };
            match found {
                Some((to, span)) => live.push((key, to, span)),
                None => stale.push((key, entry)),
            }
        }
        let mut moves = Vec::new();
        for (from, to, span) in live {
            self.entries.insert(
                to.clone(),
                KeyEntry {
                    span,
                    revision: next,
                    state: KeyState::Live,
                },
            );
            moves.push(KeyMove {
                from,
                to,
                state: KeyState::Live,
            });
        }
        for (key, entry) in stale {
            let entry = KeyEntry {
                state: KeyState::Stale,
                ..entry
            };
            if self.entries.contains_key(&key) {
                // A migrated Live key now owns this spelling.
                self.superseded.push((key.clone(), entry));
            } else {
                self.entries.insert(key.clone(), entry);
            }
            moves.push(KeyMove {
                from: key.clone(),
                to: key,
                state: KeyState::Stale,
            });
        }
        self.revision = next;
        self.record_new(new_table);
        moves
    }

    /// Re-confirms a stale key at its current resolution in `table` (the
    /// editor's explicit re-confirmation). Returns false when `table` does
    /// not resolve the key.
    pub fn confirm(&mut self, key: &BindingKey, table: &DirectiveTable) -> bool {
        let Some(r) = table.resolved.iter().find(|r| r.key.as_ref() == Some(key)) else {
            return false;
        };
        self.entries.insert(
            key.clone(),
            KeyEntry {
                span: (r.target_span.start, r.target_span.end),
                revision: self.revision,
                state: KeyState::Live,
            },
        );
        true
    }
}

/// The new key and span of one Live key, or `None` when it breaks.
fn migrate_one(
    key: &BindingKey,
    entry: &KeyEntry,
    changes: &ChangeSet,
    new_table: &DirectiveTable,
) -> Option<(BindingKey, (u32, u32))> {
    let Some((site, _)) = &key.site else {
        // Label-addressed definition parameter: position-free.
        return new_table
            .definition_param(&key.label, &key.param)
            .map(|span| (key.clone(), (span.start, span.end)));
    };
    let Mapped::Moved { start, end } = changes.map_span(entry.span.0, entry.span.1) else {
        return None;
    };
    let n = new_table.site_ordinal(&key.label, site, (start, end))?;
    let to = BindingKey {
        label: Rc::clone(&key.label),
        site: Some((Rc::clone(site), n)),
        param: Rc::clone(&key.param),
    };
    Some((to, (start, end)))
}

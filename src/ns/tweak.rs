//! Tweak sites and their tiers (design section 13).
//!
//! A numeric literal in the Decided site set compiles to `LoadTweak`: an
//! anonymous slot of kind `Tweak`, initialized to the literal, late-bound
//! like any var. The compiler classifies the tier from the form's
//! structure; the reactive layer (ME-REACTIVE) re-evaluates `Reeval` owners
//! and downgrades non-replayable owners to `Manual`.

use std::collections::BTreeMap;

use crate::ns::namespace::{FormGen, VarSlotRef};
use crate::reader::span::Span;
use crate::value::num::NumKind;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

id_newtype!(
    /// A tweak slot.
    TweakId(u32)
);

/// The numeric type of a site's literal.
pub type NumTy = NumKind;

/// How a controller write reaches the sound (section 13 "Site tiers").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SiteTier {
    /// Captured late by a pattern, control, signal or `inst` parameter:
    /// the write is heard with no re-evaluation.
    Direct,
    /// Flowed through computation: the owning form is rebuilt.
    Reeval,
    /// The owner is non-replayable or no longer current.
    Manual,
}

/// Which Decided site kind a site is (section 13 "Site scope").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SiteOrigin {
    PatternLiteral,
    Binding,
    InstDefault,
}

/// One tweak site.
#[derive(Clone, Debug)]
pub struct TweakSite {
    pub id: TweakId,
    pub span: Span,
    pub slot: VarSlotRef,
    pub initial: Value,
    pub ty: NumTy,
    pub tier: SiteTier,
    pub form_gen: FormGen,
    pub origin: SiteOrigin,
    /// The literal's index among the form's sites, in source order (the
    /// stable path used by override migration).
    pub index: u32,
}

/// Every live tweak site, by id.
#[derive(Debug, Default)]
pub struct TweakTable {
    sites: BTreeMap<TweakId, TweakSite>,
    next: u32,
}

impl TweakTable {
    /// A fresh id.
    pub fn alloc(&mut self) -> TweakId {
        let id = TweakId::new(self.next);
        self.next = self.next.wrapping_add(1);
        id
    }

    /// Records a site.
    pub fn insert(&mut self, site: TweakSite) {
        self.sites.insert(site.id, site);
    }

    /// The site `id`.
    #[must_use]
    pub fn get(&self, id: TweakId) -> Option<&TweakSite> {
        self.sites.get(&id)
    }

    /// Sets a site's tier (the reactive layer marks `Manual`).
    pub fn set_tier(&mut self, id: TweakId, tier: SiteTier) {
        if let Some(site) = self.sites.get_mut(&id) {
            site.tier = tier;
        }
    }

    /// The sites of one form generation, in source order.
    #[must_use]
    pub fn sites_of(&self, gen: FormGen) -> Vec<TweakSite> {
        let mut out: Vec<TweakSite> = self
            .sites
            .values()
            .filter(|s| s.form_gen == gen)
            .cloned()
            .collect();
        out.sort_by_key(|s| s.index);
        out
    }

    /// Drops the sites of a superseded generation.
    pub fn retire(&mut self, gen: FormGen) {
        self.sites.retain(|_, s| s.form_gen != gen);
    }

    /// Every site.
    pub fn iter(&self) -> impl Iterator<Item = &TweakSite> + '_ {
        self.sites.values()
    }

    /// `set-tweak`: writes the site's slot. A `Direct` site is heard at the
    /// next read with no re-evaluation; the reactive layer rebuilds the
    /// owner of a `Reeval` site.
    ///
    /// # Errors
    /// `undefined-name` for an unknown id, `type` for a non-number.
    pub fn write(&self, id: TweakId, value: Value) -> Result<&TweakSite, Failure> {
        let Some(site) = self.sites.get(&id) else {
            return Err(Failure::new(FailCode::UndefinedName, "no such tweak site"));
        };
        if NumKind::of(&value).is_none() {
            return Err(Failure::new(FailCode::Type, "a tweak value is a number"));
        }
        site.slot.set(value);
        Ok(site)
    }
}

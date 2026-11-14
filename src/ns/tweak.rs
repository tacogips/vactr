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
    by_generation: BTreeMap<FormGen, Vec<TweakId>>,
    by_span_start: BTreeMap<(u32, u32, u32, TweakId), TweakId>,
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
        let id = site.id;
        let span = site.span;
        let generation = site.form_gen;
        if let Some(previous) = self.sites.insert(id, site) {
            self.remove_indexes(&previous);
        }
        self.by_generation.entry(generation).or_default().push(id);
        self.by_span_start
            .insert((span.file.get(), span.start, span.end, id), id);
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
            .by_generation
            .get(&gen)
            .into_iter()
            .flatten()
            .filter_map(|id| self.sites.get(id).cloned())
            .collect();
        out.sort_by_key(|s| (s.index, s.id));
        out
    }

    fn migration_candidates(
        &self,
        span: crate::reader::span::Span,
    ) -> impl Iterator<Item = &TweakSite> + '_ {
        let start = (span.file.get(), span.start, 0, TweakId::new(0));
        let end = (span.file.get(), span.end, u32::MAX, TweakId::new(u32::MAX));
        self.by_span_start
            .range(start..=end)
            .filter_map(|(_, id)| self.sites.get(id))
    }

    fn is_migration_candidate(
        site: &TweakSite,
        span: crate::reader::span::Span,
        before: FormGen,
    ) -> bool {
        site.form_gen < before && site.span.end <= span.end && site.span.start >= span.start
    }

    /// Finds the latest prior-generation tweak contained by `span`.
    #[cfg(test)]
    pub(super) fn migration_generation_in_span(
        &self,
        span: crate::reader::span::Span,
        before: FormGen,
    ) -> (Option<FormGen>, usize) {
        let mut visited = 0;
        let latest = self
            .migration_candidates(span)
            .inspect(|_| visited += 1)
            .filter(|site| Self::is_migration_candidate(site, span, before))
            .max_by_key(|site| (site.form_gen, site.id))
            .map(|site| site.form_gen);
        (latest, visited)
    }

    /// Production lookup without test instrumentation.
    #[cfg(not(test))]
    pub(super) fn migration_generation_in_span(
        &self,
        span: crate::reader::span::Span,
        before: FormGen,
    ) -> Option<FormGen> {
        self.migration_candidates(span)
            .filter(|site| Self::is_migration_candidate(site, span, before))
            .max_by_key(|site| (site.form_gen, site.id))
            .map(|site| site.form_gen)
    }

    fn remove_indexes(&mut self, site: &TweakSite) {
        let span_key = (
            site.span.file.get(),
            site.span.start,
            site.span.end,
            site.id,
        );
        self.by_span_start.remove(&span_key);
        if let Some(ids) = self.by_generation.get_mut(&site.form_gen) {
            ids.retain(|id| *id != site.id);
            if ids.is_empty() {
                self.by_generation.remove(&site.form_gen);
            }
        }
    }

    /// Drops the sites of a superseded generation.
    pub fn retire(&mut self, gen: FormGen) {
        if let Some(ids) = self.by_generation.remove(&gen) {
            for id in ids {
                if let Some(site) = self.sites.remove(&id) {
                    self.by_span_start.remove(&(
                        site.span.file.get(),
                        site.span.start,
                        site.span.end,
                        site.id,
                    ));
                }
            }
        }
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

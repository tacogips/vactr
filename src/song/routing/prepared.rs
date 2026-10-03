//! Owned immutable route plan and privately attested original site bindings.
use super::*;
use crate::dsp::caps::CapabilitySet;
use crate::pattern::eval::song_clock::ProjectionBudget;
use crate::song::snapshot::occupancy::route_view::{
    RouteAuthorityView, TrustedPolicyCopy, TrustedRouteCopy, TrustedSiteCopy,
};
use crate::song::snapshot::{FrozenPattern, FrozenSelectedSource};
use crate::song::{SongLimits, SongSettings};
use crate::vm::fail::{FailCode, Failure};
use std::rc::Rc;

pub(crate) struct PreparedRoutes {
    plan: SongRoutePlan,
    authority: Rc<RouteAuthorityView>,
    copy: TrustedRouteCopy,
}
#[derive(Clone, Copy)]
pub(crate) struct PreparedSiteRef<'a> {
    owner: &'a PreparedRoutes,
    binding: &'a TrustedSiteCopy,
    scope: usize,
    track: KwId,
}
pub(crate) struct PreparedPolicyRef<'a> {
    site: PreparedSiteRef<'a>,
    binding: &'a TrustedPolicyCopy,
    source: usize,
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}
pub(crate) fn prepare_routes_issued(
    authority: Rc<RouteAuthorityView>,
    settings: SongSettings,
    caps: &CapabilitySet,
    available: &SongHostCapacities,
    limits: SongLimits,
    remaining: &mut u32,
    depth: u32,
) -> Result<PreparedRoutes, Failure> {
    if settings != *authority.original().settings() {
        return Err(invalid("route settings differ from original Song"));
    }
    let copy = authority.trusted_copy(limits, remaining, depth)?;
    let plan = super::prepare::prepare_routes_metered(
        copy.inventory(),
        settings,
        authority.resource_count(),
        authority.pcm_bytes(),
        caps,
        available,
        limits,
        remaining,
        depth,
    )?;
    Ok(PreparedRoutes {
        plan,
        authority,
        copy,
    })
}
impl PreparedRoutes {
    pub(crate) fn plan(&self) -> &SongRoutePlan {
        &self.plan
    }
    pub(crate) fn site(
        &self,
        scope: usize,
        track: KwId,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<PreparedSiteRef<'_>, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(self.copy.sites().len() as u64 + 1)?;
        let mut found = None;
        for binding in self.copy.sites() {
            if binding.matches(&self.authority, scope, track) {
                if found.replace(binding).is_some() {
                    return Err(invalid("ambiguous prepared site"));
                }
            }
        }
        let binding = found.ok_or_else(|| invalid("missing prepared site"))?;
        binding.resolve(
            &self.authority,
            &self.copy,
            scope,
            track,
            limits,
            remaining,
            depth,
        )?;
        Ok(PreparedSiteRef {
            owner: self,
            binding,
            scope,
            track,
        })
    }
    pub(crate) fn policy<'a>(
        &'a self,
        site: PreparedSiteRef<'a>,
        source: usize,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<PreparedPolicyRef<'a>, Failure> {
        if !std::ptr::eq(site.owner, self) {
            return Err(invalid("foreign prepared site owner"));
        }
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(self.copy.policies().len() as u64 + 1)?;
        let binding = self
            .copy
            .policies()
            .iter()
            .find(|p| p.matches(site.binding, source))
            .ok_or_else(|| invalid("missing prepared policy"))?;
        binding.resolve(
            &self.authority,
            &self.copy,
            site.binding,
            source,
            limits,
            remaining,
            depth,
        )?;
        Ok(PreparedPolicyRef {
            site,
            binding,
            source,
        })
    }
}
impl<'a> PreparedSiteRef<'a> {
    pub(crate) fn authority(&self) -> &'a RouteAuthorityView {
        &self.owner.authority
    }
    pub(crate) fn scope(&self) -> usize {
        self.scope
    }
    pub(crate) fn track(&self) -> KwId {
        self.track
    }
    pub(crate) fn bind_original(
        &self,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<&'a FrozenPattern, Failure> {
        self.binding.resolve(
            &self.owner.authority,
            &self.owner.copy,
            self.scope,
            self.track,
            limits,
            remaining,
            depth,
        )
    }
}
impl PreparedPolicyRef<'_> {
    pub(crate) fn bind_original(
        &self,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<&FrozenSelectedSource, Failure> {
        self.binding.resolve(
            &self.site.owner.authority,
            &self.site.owner.copy,
            self.site.binding,
            self.source,
            limits,
            remaining,
            depth,
        )
    }
}

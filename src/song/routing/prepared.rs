//! Owned immutable route plan and privately attested original site bindings.
use super::prepare::PreparationLedger;
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
    let plan = super::prepare::prepare_routes_metered(super::prepare::MeteredPreparation {
        inventory: copy.inventory(),
        settings,
        resource_count: authority.resource_count(),
        pcm_bytes: authority.pcm_bytes(),
        caps,
        available,
        limits,
        remaining,
        depth,
        ledger: PreparationLedger::Issued,
    })?;
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
                let Some(_) = found.replace(binding) else {
                    continue;
                };
                return Err(invalid("ambiguous prepared site"));
            }
        }
        let binding = found.ok_or_else(|| invalid("missing prepared site"))?;
        binding.resolve(
            (&self.authority, &self.copy, scope, track),
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
            (&self.authority, &self.copy, site.binding, source),
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
    pub(crate) fn authenticates_request(
        &self,
        request: &crate::song::snapshot::occupancy::CanonicalIndexRequest,
    ) -> bool {
        self.binding
            .authenticates_record(&self.owner.authority, request, self.scope, self.track)
    }
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
            (
                &self.owner.authority,
                &self.owner.copy,
                self.scope,
                self.track,
            ),
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
            (
                &self.site.owner.authority,
                &self.site.owner.copy,
                self.site.binding,
                self.source,
            ),
            limits,
            remaining,
            depth,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::query::TimeSpan;
    use crate::song::routing::SongHostCapacities;
    use crate::song::snapshot::occupancy::route_view::{
        capture_test_route_authority, RouteAuthorityView,
    };
    use crate::song::{SongLimits, SongSettings};
    use crate::value::intern::intern_kw;
    use crate::value::Ratio64;
    use std::rc::Rc;

    const SLICE: &str = "fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {beat -> s :analog > chord [:c :five]} 2 [cut nil]\nlet base {part [drums: {s :analog}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song";
    const FX_SLICE: &str = "bus :room:\n\tplate mix: 0.5\nfn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {beat -> s :analog > chord [:c :five]} 2 [cut nil]\nlet base {part [drums: {s :analog}] duration: 2}\nlet fx {instrument-fx base :drums :analog :room}\nlet selected {transform-instrument fx :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song";
    const POLICY: &str = "fn inner p:\n\tfast p 1\nfn outer p:\n\tfast p 1\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inside {transform-instrument base :drums :analog inner}\nlet selected {transform-instrument inside :drums :analog outer}\nsong selected tail-seconds: 0 > play-song";
    const PLAIN: &str =
        "let base {part [drums: {s :analog}] duration: 2}\nsong base tail-seconds: 0 > play-song";

    fn limits() -> SongLimits {
        SongLimits {
            max_nodes: 1_000_000,
            ..SongLimits::default()
        }
    }
    fn capacities() -> SongHostCapacities {
        SongHostCapacities {
            sample_rate: 48_000,
            cell_slots: 4096,
            voice_slots: 8,
            template_slots: 256,
            bus_slots: 256,
            sample_resources: 256,
            pcm_bytes: 16_000_000,
            voice_frames: 8 * 192_000,
            bus_frames: 128_000_000,
            ack_slots: 1024,
        }
    }
    fn legacy_plan(code: &str) -> crate::song::routing::SongRoutePlan {
        use crate::session::song::{evaluate_song_candidate, CandidateBuildCtx};
        use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
        use crate::song::{prepare_song, SnapshotEpoch};
        use std::collections::BTreeMap;
        let assets =
            DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let context = CandidateBuildCtx {
            assets: &assets,
            asset_limits: SongAssetLimits {
                max_resources: 64,
                max_pcm_bytes: 1_000_000,
                max_source_files: 32,
                max_source_bytes: 100_000,
                max_banks: 32,
                max_walk_nodes: 100_000,
                max_walk_depth: 256,
            },
            lock: None,
            cache: None,
        };
        let prepared = prepare_song(
            evaluate_song_candidate(code, "route-legacy.vact", 7, SnapshotEpoch(92), &context)
                .unwrap(),
        )
        .unwrap();
        crate::song::routing::prepare_routes(
            prepared.snapshot(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacities(),
        )
        .unwrap()
    }
    fn issued(
        code: &str,
    ) -> (
        Rc<RouteAuthorityView>,
        crate::song::snapshot::issued::FrozenIssuedBatch,
    ) {
        let policy = limits();
        let mut remaining = policy.max_nodes;
        capture_test_route_authority(code, policy, &mut remaining, 0).unwrap()
    }
    fn prepared(authority: Rc<RouteAuthorityView>, remaining: &mut u32) -> PreparedRoutes {
        prepare_routes_issued(
            authority.clone(),
            *authority.original().settings(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacities(),
            limits(),
            remaining,
            0,
        )
        .unwrap()
    }
    fn root_track(view: &RouteAuthorityView) -> (usize, crate::value::intern::KwId) {
        (view.inventory().root_part, intern_kw("drums"))
    }
    fn slice_site(
        view: &RouteAuthorityView,
    ) -> (
        usize,
        crate::value::intern::KwId,
        crate::reader::span::NodeId,
        Vec<crate::song::source_uses::FrozenUseTraceTerm>,
    ) {
        use crate::song::source_uses::FrozenUseOperation;
        let (scope, track) = root_track(view);
        let payload = match &view.inventory().parts[scope].node {
            crate::song::snapshot::FrozenPartNode::Edit {
                edit: crate::song::snapshot::FrozenEdit::Transform { payload, .. },
                ..
            } => payload,
            _ => panic!("fixture is a transformed Slice payload"),
        };
        let recipe = payload.index_timing().unwrap();
        let mut pending = vec![(recipe.root(), Vec::new())];
        while let Some((index, prefix)) = pending.pop() {
            let node = &recipe.nodes()[index as usize];
            if node.operation() == FrozenUseOperation::Slice {
                return (scope, track, node.issuer(), prefix);
            }
            for edge in node.children() {
                let mut next = prefix.clone();
                next.extend_from_slice(edge.trace());
                pending.push((edge.child(), next));
            }
        }
        panic!("fixture has an actual Slice site")
    }
    fn topology_descriptors(inventory: &crate::song::snapshot::FrozenRoutingInventory) -> String {
        use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
        format!(
            "{:?}",
            inventory
                .parts
                .iter()
                .map(|part| {
                    let node = match &part.node {
                        P::Capture(entries) => format!("capture:{entries:?}"),
                        P::Sequence(children) => format!("sequence:{children:?}"),
                        P::Repeat {
                            child,
                            count,
                            seed_mode,
                        } => {
                            format!("repeat:{child}:{count}:{seed_mode:?}")
                        }
                        P::Edit { source, edit } => match edit {
                            E::InstrumentFx {
                                track,
                                family,
                                template,
                            } => format!("fx:{source}:{track:?}:{family:?}:{template:?}"),
                            E::Transform {
                                track,
                                family,
                                payload,
                                ..
                            } => format!("transform:{source}:{track:?}:{family:?}:{payload:?}"),
                            E::Replace { track, payload } => {
                                format!("replace:{source}:{track:?}:{payload:?}")
                            }
                            E::Overwrite {
                                track,
                                region,
                                payload,
                            } => format!("overwrite:{source}:{track:?}:{region:?}:{payload:?}"),
                            E::Delete(handle) => format!("delete:{source}:{handle:?}"),
                        },
                    };
                    (part.duration, &part.tracks, node)
                })
                .collect::<Vec<_>>()
        )
    }
    fn plan_with(authority: Rc<RouteAuthorityView>) -> PreparedRoutes {
        let mut remaining = limits().max_nodes;
        prepared(authority, &mut remaining)
    }

    #[test]
    fn real_slice_authority_survives_prepared_song_drop_and_binds_copied_site_policy() {
        let (view, batch) = issued(SLICE);
        assert!(!batch.events().is_empty());
        let routes = plan_with(view.clone());
        let (scope, track, _, _) = slice_site(&view);
        let mut left = limits().max_nodes;
        let site = routes.site(scope, track, limits(), &mut left, 0).unwrap();
        assert!(site
            .bind_original(limits(), &mut left, 0)
            .unwrap()
            .index_timing()
            .is_some());

        let (policy_view, _) = issued(POLICY);
        let policy_routes = plan_with(policy_view.clone());
        let (policy_scope, policy_track) = root_track(&policy_view);
        let policy_site = policy_routes
            .site(policy_scope, policy_track, limits(), &mut left, 0)
            .unwrap();
        let policy = policy_routes
            .policy(policy_site, 0, limits(), &mut left, 0)
            .unwrap();
        assert!(!policy
            .bind_original(limits(), &mut left, 0)
            .unwrap()
            .family
            .is_empty());
    }

    #[test]
    fn retained_records_match_slice_sites_and_full_part_windows() {
        let (view, _) = issued(SLICE);
        let records = view.test_record_summaries();
        assert_eq!(
            records.len(),
            1,
            "one retained record for the actual Slice site"
        );
        assert!(records.iter().all(|(_, window)| {
            *window == TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(2)).unwrap()
        }));
    }

    #[test]
    fn no_slice_issues_empty_view_and_matches_legacy_branches() {
        let (view, _) = issued(PLAIN);
        assert!(view.test_record_summaries().is_empty());
        let mut remaining = limits().max_nodes;
        let issued = prepared(view.clone(), &mut remaining);
        let legacy = legacy_plan(PLAIN);
        assert_eq!(
            format!("{:?}", legacy.branches),
            format!("{:?}", issued.plan().branches)
        );
        assert_eq!(
            format!("{:?}", legacy.tracks),
            format!("{:?}", issued.plan().tracks)
        );
        assert_eq!(
            format!("{:?}", legacy.source_covers),
            format!("{:?}", issued.plan().source_covers)
        );
        assert_eq!(
            format!("{:?}", legacy.nested_source_covers),
            format!("{:?}", issued.plan().nested_source_covers)
        );
        assert_eq!(
            topology_descriptors(&legacy.topology),
            topology_descriptors(&issued.plan().topology)
        );
    }

    #[test]
    fn legacy_preparation_preserves_the_issued_plan_topology_and_ledger() {
        let (view, _) = issued(FX_SLICE);
        let routes = plan_with(view);
        let legacy = legacy_plan(FX_SLICE);
        assert_eq!(
            format!("{:?}", routes.plan().branches),
            format!("{:?}", legacy.branches)
        );
        assert_eq!(
            format!("{:?}", routes.plan().tracks),
            format!("{:?}", legacy.tracks)
        );
        assert_eq!(
            format!("{:?}", routes.plan().source_covers),
            format!("{:?}", legacy.source_covers)
        );
        assert_eq!(
            format!("{:?}", routes.plan().nested_source_covers),
            format!("{:?}", legacy.nested_source_covers)
        );
        assert_eq!(
            topology_descriptors(&routes.plan().topology),
            topology_descriptors(&legacy.topology)
        );
    }

    #[test]
    fn mismatched_settings_refuse_before_trusted_copy() {
        let (view, _) = issued(SLICE);
        let original = *view.original().settings();
        for settings in [
            SongSettings {
                bpm: original.bpm.checked_add(Ratio64::ONE).unwrap(),
                ..original
            },
            SongSettings {
                tail_seconds: original.tail_seconds.checked_add(Ratio64::ONE).unwrap(),
                ..original
            },
            SongSettings {
                seed: original.seed.wrapping_add(1),
                ..original
            },
        ] {
            let mut remaining = limits().max_nodes;
            let before = remaining;
            assert!(prepare_routes_issued(
                view.clone(),
                settings,
                &crate::dsp::caps::CapabilitySet::native(),
                &capacities(),
                limits(),
                &mut remaining,
                0
            )
            .is_err());
            assert_eq!(
                remaining, before,
                "settings mismatch is refused before topology copy"
            );
        }
    }

    #[test]
    fn foreign_site_and_swapped_equal_looking_policy_bindings_are_refused() {
        let (view_a, _) = issued(POLICY);
        let (view_b, _) = issued(POLICY);
        let a = plan_with(view_a.clone());
        let b = plan_with(view_b);
        let (scope, track) = root_track(&view_a);
        let mut left = limits().max_nodes;
        let local = a.site(scope, track, limits(), &mut left, 0).unwrap();
        assert!(!a
            .policy(local, 0, limits(), &mut left, 0)
            .unwrap()
            .bind_original(limits(), &mut left, 0)
            .unwrap()
            .family
            .is_empty());
        let foreign = b.site(scope, track, limits(), &mut left, 0).unwrap();
        assert!(a.policy(foreign, 0, limits(), &mut left, 0).is_err());
        let binding = b.copy.policies().first().expect("fixture policy");
        let swapped = PreparedPolicyRef {
            site: local,
            binding,
            source: 0,
        };
        assert!(swapped.bind_original(limits(), &mut left, 0).is_err());
        let mut copied_plan = a.plan().clone();
        copied_plan.topology.parts.clear();
        assert!(
            a.site(scope, track, limits(), &mut left, 0).is_ok(),
            "mutating an independent public plan copy grants no replacement authority"
        );
    }

    #[test]
    fn exact_work_succeeds_and_one_less_refuses_without_publishing_prepared_routes() {
        let (view, _) = issued(FX_SLICE);
        let mut measure = limits().max_nodes;
        let _ = prepared(view.clone(), &mut measure);
        let exact_cost = limits().max_nodes - measure;
        let mut exact = exact_cost;
        let _routes = prepared(view.clone(), &mut exact);
        assert_eq!(exact, 0);
        let mut short = exact_cost - 1;
        let before = short;
        assert!(prepare_routes_issued(
            view.clone(),
            *view.original().settings(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacities(),
            limits(),
            &mut short,
            0
        )
        .is_err());
        assert!(short < before, "one-less failure retains consumed work");
    }

    #[test]
    fn depth_boundary_is_checked_for_authority_and_owned_copy() {
        let (view, _) = issued(PLAIN);
        let mut exact = limits().max_nodes;
        assert!(view
            .trusted_copy(limits(), &mut exact, limits().max_depth - 1)
            .is_ok());
        let mut refused = limits().max_nodes;
        assert!(view
            .trusted_copy(limits(), &mut refused, limits().max_depth)
            .is_err());
    }

    #[test]
    fn retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits() {
        use crate::pattern::eval::song_observation::CanonicalIndexCollector;
        use crate::song::snapshot::occupancy::lookup::authority::bind_issued_owner;
        let (view, batch) = issued(SLICE);
        let (scope, track, issuer, prefix) = slice_site(&view);
        let routes = plan_with(view.clone());
        let mut remaining = limits().max_nodes;
        let site = routes
            .site(scope, track, limits(), &mut remaining, 0)
            .unwrap();
        let window = TimeSpan::new(
            Ratio64::ZERO,
            view.inventory().parts[view.inventory().root_part].duration,
        )
        .unwrap();
        let mut success = None;
        for event in batch.events() {
            for seal in event.invocations() {
                let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
                work.borrow_mut().original = Some(view.original().clone());
                work.borrow_mut().charge(7).unwrap();
                let before = work.borrow().remaining();
                if bind_issued_owner(
                    site,
                    issuer,
                    &prefix,
                    window,
                    batch.transcript(),
                    seal,
                    &work,
                    0,
                )
                .is_ok()
                {
                    assert!(work.borrow().remaining() < before);
                    success = Some(seal.clone());
                    break;
                }
            }
            if success.is_some() {
                break;
            }
        }
        let seal = success.expect("a genuine queried invocation binds to its retained Slice owner");

        let (_foreign_view, foreign_batch) = issued(SLICE);
        let foreign_transcript = foreign_batch.transcript().clone();
        let foreign_work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
        foreign_work.borrow_mut().original = Some(view.original().clone());
        foreign_work.borrow_mut().charge(7).unwrap();
        let before = foreign_work.borrow().remaining();
        assert!(bind_issued_owner(
            site,
            issuer,
            &prefix,
            window,
            &foreign_transcript,
            &seal,
            &foreign_work,
            0
        )
        .is_err());
        assert!(foreign_work.borrow().remaining() < before);
    }
}

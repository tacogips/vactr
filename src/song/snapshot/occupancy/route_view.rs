//! Immutable original-to-view ownership. Seals have no backedges to their owners.
use super::*;
use crate::pattern::eval::{song_clock::ProjectionBudget, song_observation::OwnerInvocation};
use crate::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenPattern, FrozenRoutingInventory};
use crate::song::source_uses::FrozenUseMapping;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PayloadSlot {
    Capture { part: usize, track: usize },
    Edit { part: usize },
}
struct SiteSeal;
struct PolicySeal;
struct ViewPolicy {
    source: usize,
    seal: Rc<PolicySeal>,
}
struct ViewSite {
    slot: PayloadSlot,
    track: KwId,
    seal: Rc<SiteSeal>,
    policies: Vec<ViewPolicy>,
}
pub(super) struct PublishedRetainedIndex {
    pub(super) request: CanonicalIndexRequest,
    site: Rc<SiteSeal>,
    pub(super) invocations: Vec<Rc<OwnerInvocation>>,
}
pub(crate) struct RouteAuthorityView {
    original: Rc<Song>,
    inventory: Rc<FrozenRoutingInventory>,
    resource_count: usize,
    pcm_bytes: u64,
    sites: Vec<ViewSite>,
    records: Vec<PublishedRetainedIndex>,
}
pub(crate) struct TrustedRouteCopy {
    inventory: FrozenRoutingInventory,
    sites: Vec<TrustedSiteCopy>,
    policies: Vec<TrustedPolicyCopy>,
}
impl TrustedRouteCopy {
    pub(crate) fn inventory(&self) -> &FrozenRoutingInventory {
        &self.inventory
    }
    pub(crate) fn sites(&self) -> &[TrustedSiteCopy] {
        &self.sites
    }
    pub(crate) fn policies(&self) -> &[TrustedPolicyCopy] {
        &self.policies
    }
}
pub(crate) struct TrustedSiteCopy {
    slot: PayloadSlot,
    site: usize,
    seal: Rc<SiteSeal>,
}
pub(crate) struct TrustedPolicyCopy {
    site: usize,
    source: usize,
    seal: Rc<PolicySeal>,
}
fn slot_payload(
    inventory: &FrozenRoutingInventory,
    slot: PayloadSlot,
) -> Result<&FrozenPattern, Failure> {
    match slot {
        PayloadSlot::Capture { part, track } => match &inventory
            .parts
            .get(part)
            .ok_or_else(|| invalid("missing copied Part"))?
            .node
        {
            FrozenPartNode::Capture(tracks) => tracks
                .get(track)
                .map(|(_, payload)| payload)
                .ok_or_else(|| invalid("missing copied track")),
            _ => Err(invalid("copied capture kind mismatch")),
        },
        PayloadSlot::Edit { part } => match &inventory
            .parts
            .get(part)
            .ok_or_else(|| invalid("missing copied edit"))?
            .node
        {
            FrozenPartNode::Edit {
                edit:
                    FrozenEdit::Replace { payload, .. }
                    | FrozenEdit::Transform { payload, .. }
                    | FrozenEdit::Overwrite { payload, .. },
                ..
            } => Ok(payload),
            _ => Err(invalid("copied edit kind mismatch")),
        },
    }
}
fn copy_pattern_work(
    payload: &FrozenPattern,
    budget: &mut ProjectionBudget<'_>,
) -> Result<(), Failure> {
    budget.charge(
        5 + payload.families.len() as u64
            + payload.named_buses.len() as u64
            + payload.sources.len() as u64
            + payload.source_uses.nodes.len() as u64,
    )?;
    for source in &payload.sources {
        budget.charge(source.family.len() as u64 + 1)?;
    }
    for node in &payload.source_uses.nodes {
        budget.charge(node.edges.len() as u64 + 1)?;
        if let FrozenUseMapping::Slices { starts, .. } = &node.mapping {
            budget.charge(starts.len() as u64)?;
        }
        for edge in &node.edges {
            budget.charge(edge.trace.len() as u64 + edge.layout.len() as u64 + 1)?;
        }
    }
    Ok(())
}
/// Precharge exactly the owned vector data; graph/recipe/text Rc and Arc clones are shallow.
pub(crate) fn copy_inventory(
    inventory: &FrozenRoutingInventory,
    limits: SongLimits,
    remaining: &mut u32,
    depth: u32,
) -> Result<FrozenRoutingInventory, Failure> {
    let mut budget = ProjectionBudget::new(limits, remaining)?;
    budget.enter(depth)?;
    budget.charge(
        7 + inventory.parts.len() as u64
            + inventory.instruments.len() as u64
            + inventory.buses.len() as u64
            + inventory.sources.len() as u64,
    )?;
    for instrument in &inventory.instruments {
        budget.charge(instrument.parameters.len() as u64 + instrument.defaults.len() as u64 + 3)?;
    }
    for part in &inventory.parts {
        budget.charge(part.tracks.len() as u64 + 3)?;
        match &part.node {
            FrozenPartNode::Capture(tracks) => {
                budget.charge(tracks.len() as u64)?;
                for (_, payload) in tracks {
                    copy_pattern_work(payload, &mut budget)?;
                }
            }
            FrozenPartNode::Sequence(children) => budget.charge(children.len() as u64)?,
            FrozenPartNode::Repeat { .. } => budget.charge(3)?,
            FrozenPartNode::Edit { edit, .. } => match edit {
                FrozenEdit::Replace { payload, .. }
                | FrozenEdit::Transform { payload, .. }
                | FrozenEdit::Overwrite { payload, .. } => {
                    copy_pattern_work(payload, &mut budget)?;
                    if let FrozenEdit::Transform { family, .. } = edit {
                        budget.charge(family.len() as u64)?;
                    }
                }
                FrozenEdit::InstrumentFx { family, .. } => {
                    budget.charge(family.len() as u64 + 2)?
                }
                FrozenEdit::Delete(handle) => budget.charge(
                    handle.occurrence().producer_ordinals.len() as u64
                        + handle.placement().0.len() as u64
                        + 3,
                )?,
            },
        }
    }
    budget.charge(
        inventory.cells.values().len() as u64
            + inventory.cells.references().len() as u64
            + inventory.cells.analysis_ranges().len() as u64
            + inventory.cells.analysis_banks().len() as u64
            + inventory.resources.entries().len() as u64,
    )?;
    Ok(inventory.clone())
}
impl RouteAuthorityView {
    pub(crate) fn original(&self) -> &Rc<Song> {
        &self.original
    }
    pub(crate) fn inventory(&self) -> &FrozenRoutingInventory {
        &self.inventory
    }
    pub(crate) fn resource_count(&self) -> usize {
        self.resource_count
    }
    pub(crate) fn pcm_bytes(&self) -> u64 {
        self.pcm_bytes
    }
    pub(super) fn records(&self) -> &[PublishedRetainedIndex] {
        &self.records
    }
    pub(crate) fn trusted_copy(
        &self,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<TrustedRouteCopy, Failure> {
        let copied = copy_inventory(&self.inventory, limits, remaining, depth)?;
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(self.sites.len() as u64 + 1)?;
        let mut sites = Vec::with_capacity(self.sites.len());
        let mut policies = Vec::new();
        for (site_index, site) in self.sites.iter().enumerate() {
            slot_payload(&copied, site.slot)?;
            sites.push(TrustedSiteCopy {
                slot: site.slot,
                site: site_index,
                seal: site.seal.clone(),
            });
            budget.charge(site.policies.len() as u64 + 1)?;
            for policy in &site.policies {
                policies.push(TrustedPolicyCopy {
                    site: site_index,
                    source: policy.source,
                    seal: policy.seal.clone(),
                });
            }
        }
        Ok(TrustedRouteCopy {
            inventory: copied,
            sites,
            policies,
        })
    }
}
impl TrustedSiteCopy {
    pub(crate) fn resolve<'a>(
        &self,
        view: &'a RouteAuthorityView,
        copy: &TrustedRouteCopy,
        scope: usize,
        track: KwId,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<&'a FrozenPattern, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(copy.sites.len() as u64 + 3)?;
        let site = view
            .sites
            .get(self.site)
            .ok_or_else(|| invalid("foreign copied site"))?;
        let ownscope = match self.slot {
            PayloadSlot::Capture { part, .. } | PayloadSlot::Edit { part } => part,
        };
        if ownscope != scope
            || site.track != track
            || site.slot != self.slot
            || !Rc::ptr_eq(&site.seal, &self.seal)
        {
            return Err(invalid("foreign copied site binding"));
        }
        if !copy.sites.iter().any(|binding| std::ptr::eq(binding, self)) {
            return Err(invalid("foreign immutable copy owner"));
        }
        slot_payload(&copy.inventory, self.slot)?;
        slot_payload(&view.inventory, site.slot)
    }
}
pub(crate) fn publish_route_authority(
    snapshot: &SongSnapshot,
    limits: SongLimits,
    remaining: &mut u32,
    depth: u32,
) -> Result<Rc<RouteAuthorityView>, Failure> {
    let mut sites = Vec::new();
    {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        for (part_index, part) in snapshot.routing.parts.iter().enumerate() {
            budget.charge(1)?;
            let mut include = |slot, track, payload: &FrozenPattern| -> Result<(), Failure> {
                budget.charge(payload.sources.len() as u64 + 3)?;
                let policies = payload
                    .sources
                    .iter()
                    .enumerate()
                    .map(|(source, _)| ViewPolicy {
                        source,
                        seal: Rc::new(PolicySeal),
                    })
                    .collect();
                sites.push(ViewSite {
                    slot,
                    track,
                    seal: Rc::new(SiteSeal),
                    policies,
                });
                Ok(())
            };
            match &part.node {
                FrozenPartNode::Capture(tracks) => {
                    for (track_index, (track, payload)) in tracks.iter().enumerate() {
                        include(
                            PayloadSlot::Capture {
                                part: part_index,
                                track: track_index,
                            },
                            *track,
                            payload,
                        )?;
                    }
                }
                FrozenPartNode::Edit {
                    edit:
                        FrozenEdit::Replace { track, payload }
                        | FrozenEdit::Transform { track, payload, .. }
                        | FrozenEdit::Overwrite { track, payload, .. },
                    ..
                } => include(PayloadSlot::Edit { part: part_index }, *track, payload)?,
                _ => {}
            }
        }
    }
    let mut records = Vec::new();
    {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        for record in &snapshot.occupancy {
            lookup::authority::authenticate_request(snapshot, &record.request, depth, &mut budget)?;
            let original_payload = lookup::authority::payload_for(
                lookup::LookupAuthority::Snapshot(snapshot),
                &record.request,
                &mut budget,
            )?;
            budget.charge(
                sites.len() as u64
                    + record.invocations.len() as u64
                    + record.request.prefix.len() as u64
                    + 3,
            )?;
            let mut matched = None;
            for site in &sites {
                if std::ptr::eq(
                    slot_payload(&snapshot.routing, site.slot)?,
                    original_payload,
                ) {
                    if matched.replace(site).is_some() {
                        return Err(invalid("ambiguous original site"));
                    }
                }
            }
            let site = matched.ok_or_else(|| invalid("missing retained original site"))?;
            for invocation in &record.invocations {
                if !invocation.authentic_original(&snapshot.song)
                    || invocation.lookup_depth() > limits.max_depth
                {
                    return Err(invalid("foreign retained invocation"));
                }
            }
            let r = &record.request;
            records.push(PublishedRetainedIndex {
                request: CanonicalIndexRequest {
                    original: r.original.clone(),
                    scope: r.scope,
                    track: r.track,
                    revision: r.revision,
                    root: r.root,
                    recipe: r.recipe.clone(),
                    issuer: r.issuer,
                    prefix: r.prefix.clone(),
                    window: r.window,
                    depth: r.depth,
                },
                site: site.seal.clone(),
                invocations: record.invocations.clone(),
            });
        }
    }
    let inventory = Rc::new(copy_inventory(&snapshot.routing, limits, remaining, depth)?);
    Ok(Rc::new(RouteAuthorityView {
        original: snapshot.song.clone(),
        inventory,
        resource_count: snapshot.resource_count(),
        pcm_bytes: snapshot.pcm_bytes(),
        sites,
        records,
    }))
}
impl SongSnapshot {
    pub(crate) fn issue_route_authority(
        &self,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<Rc<RouteAuthorityView>, Failure> {
        publish_route_authority(self, limits, remaining, depth)
    }
}

impl TrustedSiteCopy {
    pub(crate) fn matches(&self, view: &RouteAuthorityView, scope: usize, track: KwId) -> bool {
        let own_scope = match self.slot {
            PayloadSlot::Capture { part, .. } | PayloadSlot::Edit { part } => part,
        };
        own_scope == scope
            && view
                .sites
                .get(self.site)
                .is_some_and(|site| site.track == track)
    }
}
impl TrustedPolicyCopy {
    pub(crate) fn resolve<'a>(
        &self,
        view: &'a RouteAuthorityView,
        copy: &TrustedRouteCopy,
        site: &TrustedSiteCopy,
        source: usize,
        limits: SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<&'a crate::song::snapshot::FrozenSelectedSource, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(copy.policies.len() as u64 + copy.sites.len() as u64 + 3)?;
        if !copy.policies.iter().any(|p| std::ptr::eq(p, self))
            || !copy.sites.iter().any(|p| std::ptr::eq(p, site))
            || self.site != site.site
            || self.source != source
        {
            return Err(invalid("foreign copied source policy"));
        }
        let original = view
            .sites
            .get(self.site)
            .ok_or_else(|| invalid("missing original site"))?;
        budget.charge(original.policies.len() as u64 + 1)?;
        if !Rc::ptr_eq(&original.seal, &site.seal)
            || !original
                .policies
                .iter()
                .any(|p| p.source == source && Rc::ptr_eq(&p.seal, &self.seal))
        {
            return Err(invalid("foreign original source policy"));
        }
        slot_payload(&view.inventory, original.slot)?
            .sources
            .get(source)
            .ok_or_else(|| invalid("missing original selected source"))
    }
    pub(crate) fn matches(&self, site: &TrustedSiteCopy, source: usize) -> bool {
        self.site == site.site && self.source == source
    }
}

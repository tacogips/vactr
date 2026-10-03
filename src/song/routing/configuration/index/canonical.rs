//! Immutable original-invocation Index geometry; no evaluator access.
#[cfg(test)]
use super::super::super::index::{
    bind_slice_operands, output_operand_owner, prepare_slice_operands,
};
use super::super::super::index::{BoundSliceOperands, PreparedSliceAddress};
use super::super::super::source::{invalid, ResolutionBudget};
use crate::pattern::query::{sect, TimeSpan};
use crate::song::snapshot::occupancy::{
    lookup::{bind_member, RetainedIndexAddress, RetainedProjectedIndexRow},
    CanonicalIndexRequest,
};
use crate::song::snapshot::FrozenSelectedSource;
use crate::vm::fail::Failure;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))] // Genuine consumer; production route-authority wiring is the next bounded phase.
pub(in crate::song::routing) enum RetainedSourceMembership {
    Empty,
    NonEmpty,
}
#[cfg_attr(not(test), allow(dead_code))] // Genuine consumer; production route-authority wiring is the next bounded phase.
pub(in crate::song::routing) struct PreparedIndexGeometry<'s> {
    rows: Vec<RetainedProjectedIndexRow<'s>>,
    groups: Vec<(usize, Vec<TimeSpan>)>,
    membership: Option<RetainedSourceMembership>,
}
impl PreparedIndexGeometry<'_> {
    #[cfg_attr(not(test), allow(dead_code))] // Genuine consumer; production route-authority wiring is the next bounded phase.
    pub(in crate::song::routing) fn groups(&self) -> impl Iterator<Item = &[TimeSpan]> {
        self.groups
            .iter()
            .map(|(_, components)| components.as_slice())
    }
    #[cfg_attr(not(test), allow(dead_code))] // Genuine consumer; production route-authority wiring is the next bounded phase.
    pub(in crate::song::routing) fn membership(&self) -> Option<RetainedSourceMembership> {
        self.membership
    }
}
#[cfg_attr(not(test), allow(dead_code))] // Genuine consumer; production route-authority wiring is the next bounded phase.
fn insert_union(
    components: &mut Vec<TimeSpan>,
    mut interval: TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<(), Failure> {
    let mut i = 0;
    while i < components.len() {
        budget.charge(1)?;
        let previous = components[i];
        if previous.end < interval.begin {
            i += 1;
            continue;
        }
        if interval.end < previous.begin {
            break;
        }
        interval = TimeSpan::new(
            previous.begin.min(interval.begin),
            previous.end.max(interval.end),
        )?;
        budget.charge(components.len() - i)?;
        components.remove(i);
    }
    budget.charge(components.len() - i + 1)?;
    components.insert(i, interval);
    Ok(())
}
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))] // Genuine consumer; production route-authority wiring is the next bounded phase.
pub(in crate::song::routing) fn canonical_prepared_index_geometry<'s>(
    prepared: &PreparedSliceAddress<'_>,
    request: &CanonicalIndexRequest,
    address: &RetainedIndexAddress<'s>,
    selected: Option<&FrozenSelectedSource>,
    source: TimeSpan,
    owner: TimeSpan,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<PreparedIndexGeometry<'s>, Failure> {
    budget.enter(depth)?;
    let limits = budget.limits();
    let issuer = prepared
        .recipe
        .nodes()
        .get(prepared.slice as usize)
        .ok_or_else(|| invalid("prepared issuer absent"))?
        .issuer();
    budget.with_remaining(|left| {
        address.validate_prepared_site(
            request,
            prepared.owner.payload(),
            issuer,
            prepared.prefix,
            depth,
            limits,
            left,
        )
    })?;
    let limits = budget.limits();
    let membership = if let Some(selected) = selected {
        Some(
            if budget.with_remaining(|left| address.empty_source(selected, depth, limits, left))? {
                RetainedSourceMembership::Empty
            } else {
                RetainedSourceMembership::NonEmpty
            },
        )
    } else {
        if !budget
            .with_remaining(|left| address.source_boundaries(depth, limits, left))?
            .is_empty()
        {
            return Err(invalid("selected source classification is required"));
        }
        None
    };
    let limits = budget.limits();
    let rows =
        budget.with_remaining(|left| address.related_rows(request, owner, depth, limits, left))?;
    let mut groups: Vec<(usize, Vec<TimeSpan>)> = Vec::new();
    for (ordinal, row) in rows.iter().enumerate() {
        budget.charge(1)?;
        let limits = budget.limits();
        if !budget.with_remaining(|left| row.source_eligible(depth, limits, left))? {
            continue;
        }
        // Direct issuer windows are local to that issuer. Selected-source
        // windows require the authenticated corresponding source-hop basis.
        let limits = budget.limits();
        if !budget.with_remaining(|left| {
            address.row_source_window(row, selected, source, depth, limits, left)
        })? {
            continue;
        }
        let mut group = None;
        for (index, (representative, _)) in groups.iter().enumerate() {
            let limits = budget.limits();
            if budget.with_remaining(|left| {
                row.same_configuration_group(&rows[*representative], depth, limits, left)
            })? {
                group = Some(index);
                break;
            }
        }
        let index = if let Some(index) = group {
            index
        } else {
            budget.charge(1)?;
            groups.push((ordinal, Vec::new()));
            groups.len() - 1
        };
        insert_union(&mut groups[index].1, row.footprint().whole, budget)?;
    }
    // Only after connected union do we clip to this actual caller owner.
    for (_, components) in &mut groups {
        let mut i = 0;
        while i < components.len() {
            budget.charge(1)?;
            if let Some(clipped) = sect(components[i], owner) {
                components[i] = clipped;
                i += 1;
            } else {
                budget.charge(components.len() - i)?;
                components.remove(i);
            }
        }
    }
    Ok(PreparedIndexGeometry {
        rows,
        groups,
        membership,
    })
}
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))] // Genuine consumer; production route-authority wiring is the next bounded phase.
pub(in crate::song::routing) fn canonical_index_configuration(
    bound: &BoundSliceOperands<'_>,
    request: &CanonicalIndexRequest,
    address: &RetainedIndexAddress<'_>,
    selected: Option<&FrozenSelectedSource>,
    source: TimeSpan,
    owner: TimeSpan,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    let limits = budget.limits();
    if let Some(selected) = selected {
        budget.with_remaining(|left| {
            bind_member(
                address,
                selected,
                bound.timing.subject_handle(),
                depth,
                limits,
                left,
            )
        })?;
    }
    let geometry = canonical_prepared_index_geometry(
        &bound.prepared,
        request,
        address,
        selected,
        source,
        owner,
        depth,
        budget,
    )?;
    let limits = budget.limits();
    let own_rows = budget.with_remaining(|left| address.rows(depth, limits, left))?;
    let mut matched = None;
    for row in &own_rows {
        let producer = row
            .original_producer()
            .ok_or_else(|| invalid("Index producer absent"))?;
        budget.charge(producer.steps.len() + bound.timing.index_trace().len() + 1)?;
        if producer.steps == bound.timing.index_trace()
            && row.original_whole() == bound.timing.index_whole()
            && row.issuer_start() == bound.timing.sample_start()
            && matched.replace(row).is_some()
        {
            return Err(invalid("ambiguous addressed Index event"));
        }
    }
    let matched = matched.ok_or_else(|| invalid("issued Index absent from retained invocation"))?;
    let limits = budget.limits();
    if !budget.with_remaining(|left| matched.source_eligible(depth, limits, left))? {
        return Ok(None);
    }
    let limits = budget.limits();
    if !budget.with_remaining(|left| {
        address.row_source_window(matched, selected, source, depth, limits, left)
    })? {
        return Ok(None);
    }
    for (representative, components) in &geometry.groups {
        let limits = budget.limits();
        if budget.with_remaining(|left| {
            matched.same_configuration_group(&geometry.rows[*representative], depth, limits, left)
        })? {
            for component in components {
                budget.charge(1)?;
                if sect(*component, matched.footprint().whole).is_some() {
                    return Ok(Some(*component));
                }
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
impl crate::song::SongSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retained_geometry_fixture_impl(
        &self,
        site: (
            usize,
            crate::value::intern::KwId,
            crate::reader::span::NodeId,
            &[crate::song::source_uses::FrozenUseTraceTerm],
            TimeSpan,
        ),
        address: &crate::song::snapshot::occupancy::lookup::RetainedIndexAddress<'_>,
        selected: Option<&crate::song::snapshot::FrozenSelectedSource>,
        timing: Option<&crate::song::source_uses::FrozenSliceTiming>,
        source: TimeSpan,
        owner: TimeSpan,
        depth: u32,
        limits: crate::song::SongLimits,
        remaining: &mut u32,
    ) -> Result<super::super::super::index::GeometryFixtureOutput, Failure> {
        let mut budget = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: *remaining,
            ..limits
        });
        let result = (|| {
            let request = budget.with_remaining(|left| {
                self.canonical_index_request(
                    site.0, site.1, site.2, site.3, site.4, depth, limits, left,
                )
            })?;
            let output = output_operand_owner(self.routing(), site.0, site.1, &mut budget)?;
            let prepared = prepare_slice_operands(output, site.2, site.3, depth, &mut budget)?;
            let bound = if let Some(timing) = timing {
                let bound = bind_slice_operands(
                    prepared,
                    timing,
                    timing.subject_handle(),
                    depth,
                    &mut budget,
                )?;
                canonical_index_configuration(
                    &bound,
                    &request,
                    address,
                    selected,
                    source,
                    owner,
                    depth,
                    &mut budget,
                )?
            } else {
                None
            };
            let geometry = canonical_prepared_index_geometry(
                &prepared,
                &request,
                address,
                selected,
                source,
                owner,
                depth,
                &mut budget,
            )?;
            let mut groups = Vec::new();
            for components in geometry.groups() {
                budget.charge(components.len() + 1)?;
                groups.push(components.to_vec());
            }
            Ok((
                groups,
                geometry
                    .membership()
                    .map(|membership| membership == RetainedSourceMembership::Empty),
                bound,
            ))
        })();
        budget.with_remaining(|left| {
            *remaining = *left;
            Ok(())
        })?;
        result
    }
}

//! Certified copied control references and ordered owner-local analysis writers.
//! Physical epoch banks, host capacities and activation are downstream contracts.
use super::FrozenRoutingInventory;
use crate::dsp::cells::CellId;
use crate::dsp::effects;
use crate::dsp::graph::{AnalyzerKind, BusDef, BusId, EffectKind, EffectSpec, InstDef, UGenSpec};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;
use crate::value::intern::KwId;
use crate::vm::fail::{FailCode, Failure};
use std::collections::BTreeMap;

/// An isolated logical graph owner, not a physical host generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrozenCellOwner {
    Instrument(KwId),
    Bus(BusId),
    Master(BusId),
}
/// The exact declaration site containing a cell reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenCellSite {
    Header { parameter: CtlId },
    NodeParameter { node: u16, parameter: CtlId },
    EmbeddedEffect { node: u16, parameter: CtlId },
    BusEffect { effect: u16, parameter: CtlId },
}
/// A finite scalar copied from an isolated candidate default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrozenCellValue {
    pub cell: CellId,
    pub value: f32,
}
/// A retained binding; sharing a scalar does not erase its graph sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenCellReference {
    pub owner: FrozenCellOwner,
    pub site: FrozenCellSite,
    pub cell: CellId,
}
/// An ordered writer. Overlaps retain existing last-writer behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenAnalysisRange {
    pub owner: FrozenCellOwner,
    pub effect: u16,
    pub writer_order: u32,
    pub kind: AnalyzerKind,
    pub logical_start: u32,
    pub width: u32,
}
/// Direct logical addressing preserves holes and overlapping writers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenAnalysisBank {
    pub owner: FrozenCellOwner,
    pub slots: u32,
}
/// Only trusted capture can construct a nonempty certified inventory.
#[derive(Clone, Debug, Default)]
pub struct FrozenCellInventory {
    values: Vec<FrozenCellValue>,
    references: Vec<FrozenCellReference>,
    analysis: Vec<FrozenAnalysisRange>,
    analysis_banks: Vec<FrozenAnalysisBank>,
    consumed_work: u32,
    logical_analysis_slots: u64,
}
fn invalid(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::BeyondCapability, message)
}
fn charge(remaining: &mut u32, count: usize) -> Result<(), Failure> {
    let count = u32::try_from(count).map_err(|_| invalid("cell inventory work overflow"))?;
    *remaining = remaining.checked_sub(count).ok_or_else(|| {
        Failure::new(
            FailCode::FuelExhausted,
            "frozen cell inventory work exhausted",
        )
    })?;
    Ok(())
}
fn map_charge(remaining: &mut u32, len: usize) -> Result<(), Failure> {
    // Logical lookup/insertion admission, including allocation of one map node.
    charge(remaining, (usize::BITS - len.leading_zeros()) as usize + 2)
}
impl FrozenCellInventory {
    pub fn values(&self) -> &[FrozenCellValue] {
        &self.values
    }
    pub fn references(&self) -> &[FrozenCellReference] {
        &self.references
    }
    pub fn analysis_ranges(&self) -> &[FrozenAnalysisRange] {
        &self.analysis
    }
    pub fn analysis_banks(&self) -> &[FrozenAnalysisBank] {
        &self.analysis_banks
    }
    pub const fn consumed_work(&self) -> u32 {
        self.consumed_work
    }
    pub fn logical_control_slots(&self) -> usize {
        self.values.len()
    }
    pub const fn logical_analysis_slots(&self) -> u64 {
        self.logical_analysis_slots
    }
    pub fn value(&self, cell: CellId) -> Option<f32> {
        self.values
            .binary_search_by_key(&cell, |v| v.cell)
            .ok()
            .map(|i| self.values[i].value)
    }
    /// Recertifies hostile public routing DTOs against complete graph contents.
    pub fn validate_graphs(
        &self,
        routing: &FrozenRoutingInventory,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        let expected = Self::capture(routing, remaining)?;
        charge(
            remaining,
            self.values
                .len()
                .checked_add(self.references.len())
                .and_then(|n| n.checked_add(self.analysis.len()))
                .and_then(|n| n.checked_add(self.analysis_banks.len()))
                .ok_or_else(|| invalid("cell inventory comparison overflow"))?,
        )?;
        if self.values != expected.values
            || self.references != expected.references
            || self.analysis != expected.analysis
            || self.analysis_banks != expected.analysis_banks
            || self.logical_analysis_slots != expected.logical_analysis_slots
        {
            return Err(invalid(
                "frozen cell inventory does not match copied graphs",
            ));
        }
        Ok(())
    }
    /// Header/global event controls cannot address effect-local analyzer ids.
    /// Cell-valued headers must use the exact certified header binding.
    pub fn validate_event_controls(
        &self,
        owner: FrozenCellOwner,
        controls: &[(CtlId, Ctl)],
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        charge(remaining, self.analysis_banks.len())?;
        if !self.analysis_banks.iter().any(|bank| bank.owner == owner) {
            return Err(invalid(format!("unknown event graph owner {owner:?}")));
        }
        charge(remaining, controls.len())?;
        for &(parameter, ctl) in controls {
            match ctl {
                Ctl::Const(value) if value.is_finite() => {}
                Ctl::Const(_) => {
                    return Err(invalid(format!("nonfinite event control for {owner:?}")));
                }
                Ctl::Cell(cell) => {
                    let mut found = false;
                    for reference in &self.references {
                        charge(remaining, 1)?;
                        if reference.owner == owner
                            && reference.cell == cell
                            && reference.site == (FrozenCellSite::Header { parameter })
                        {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(invalid(format!(
                            "uncertified event cell {cell:?} for {owner:?}/{parameter:?}"
                        )));
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn capture(
        routing: &FrozenRoutingInventory,
        remaining: &mut u32,
    ) -> Result<Self, Failure> {
        let initial = *remaining;
        charge(remaining, routing.instruments.len())?;
        let mut values = BTreeMap::new();
        for instrument in &routing.instruments {
            charge(remaining, instrument.defaults.len())?;
            for &(cell, value) in &instrument.defaults {
                if !value.is_finite() {
                    return Err(invalid(format!(
                        "nonfinite default {cell:?} in {:?}",
                        instrument.name
                    )));
                }
                map_charge(remaining, values.len())?;
                if let Some(old) = values.insert(cell, value) {
                    if old != value {
                        return Err(invalid(format!("conflicting copied default {cell:?}")));
                    }
                }
            }
        }
        charge(remaining, values.len())?;
        let copied = values
            .into_iter()
            .map(|(cell, value)| FrozenCellValue { cell, value })
            .collect();
        let mut out = Self {
            values: copied,
            ..Default::default()
        };
        let mut owners = BTreeMap::new();
        for instrument in &routing.instruments {
            let owner = FrozenCellOwner::Instrument(instrument.name);
            if admit_owner(
                &mut owners,
                owner,
                Graph::Instrument(&instrument.graph),
                remaining,
            )? {
                out.instrument(owner, &instrument.graph, remaining)?;
            }
        }
        charge(remaining, routing.buses.len())?;
        for (name, bus) in &routing.buses {
            let owner = if name.is_some() {
                FrozenCellOwner::Bus(bus.id)
            } else {
                FrozenCellOwner::Master(bus.id)
            };
            if admit_owner(&mut owners, owner, Graph::Bus(bus), remaining)? {
                out.bus(owner, bus, remaining)?;
            }
        }
        for bank in &out.analysis_banks {
            charge(remaining, 1)?;
            out.logical_analysis_slots = out
                .logical_analysis_slots
                .checked_add(u64::from(bank.slots))
                .ok_or_else(|| invalid("analysis bank total overflow"))?;
        }
        out.consumed_work = initial
            .checked_sub(*remaining)
            .ok_or_else(|| invalid("cell work accounting overflow"))?;
        Ok(out)
    }
    fn reference(
        &mut self,
        owner: FrozenCellOwner,
        site: FrozenCellSite,
        ctl: Ctl,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        charge(remaining, 1)?;
        if let Ctl::Cell(cell) = ctl {
            map_charge(remaining, self.values.len())?;
            if self.value(cell).is_none() {
                return Err(invalid(format!(
                    "missing copied cell {cell:?} for {owner:?}/{site:?}"
                )));
            }
            charge(remaining, 1)?;
            self.references
                .push(FrozenCellReference { owner, site, cell });
        }
        Ok(())
    }
    fn instrument(
        &mut self,
        owner: FrozenCellOwner,
        graph: &InstDef,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        charge(remaining, graph.params.len())?;
        for &(parameter, ctl) in &graph.params {
            self.reference(owner, FrozenCellSite::Header { parameter }, ctl, remaining)?;
        }
        charge(remaining, graph.node_params.len())?;
        for &(node, parameter, ctl) in &graph.node_params {
            if usize::from(node) >= graph.nodes.len() {
                return Err(invalid(format!("invalid node cell site {owner:?}/{node}")));
            }
            self.reference(
                owner,
                FrozenCellSite::NodeParameter { node, parameter },
                ctl,
                remaining,
            )?;
        }
        charge(remaining, graph.nodes.len())?;
        let mut accumulation = AnalyzerAccumulation::default();
        let execution = graph_execution_order(graph, remaining)?;
        for &node in execution.iter().take(graph.nodes.len()) {
            if let UGenSpec::Effect(effect) = &graph.nodes[usize::from(node)] {
                charge(remaining, effect.params.len())?;
                for &(parameter, ctl) in &effect.params {
                    self.reference(
                        owner,
                        FrozenCellSite::EmbeddedEffect { node, parameter },
                        ctl,
                        remaining,
                    )?;
                }
                if matches!(effect.kind, EffectKind::Analyzer(_)) {
                    charge(remaining, graph.node_params.len())?;
                }
                // RawGraph emits embedded params first, then explicit per-node overrides.
                let extras = graph
                    .node_params
                    .iter()
                    .filter_map(|&(n, id, ctl)| (n == node).then_some((id, ctl)));
                self.analyzer(owner, node, effect, extras, &mut accumulation, remaining)?;
            }
        }
        self.bank(owner, accumulation.highwater, remaining)
    }
    fn bus(
        &mut self,
        owner: FrozenCellOwner,
        graph: &BusDef,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        charge(remaining, graph.chain.len())?;
        let mut accumulation = AnalyzerAccumulation::default();
        for (index, effect) in graph.chain.iter().enumerate() {
            let effect_index =
                u16::try_from(index).map_err(|_| invalid("bus effect index overflow"))?;
            charge(remaining, effect.params.len())?;
            for &(parameter, ctl) in &effect.params {
                self.reference(
                    owner,
                    FrozenCellSite::BusEffect {
                        effect: effect_index,
                        parameter,
                    },
                    ctl,
                    remaining,
                )?;
            }
            self.analyzer(
                owner,
                effect_index,
                effect,
                std::iter::empty(),
                &mut accumulation,
                remaining,
            )?;
        }
        self.bank(owner, accumulation.highwater, remaining)
    }
    fn bank(
        &mut self,
        owner: FrozenCellOwner,
        slots: u32,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        charge(remaining, 1)?;
        self.analysis_banks
            .push(FrozenAnalysisBank { owner, slots });
        Ok(())
    }
    fn analyzer(
        &mut self,
        owner: FrozenCellOwner,
        site: u16,
        effect: &EffectSpec,
        extras: impl Iterator<Item = (CtlId, Ctl)>,
        accumulation: &mut AnalyzerAccumulation,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        let EffectKind::Analyzer(kind) = effect.kind else {
            return Ok(());
        };
        let defs = effects::params(effect.kind);
        let (index, definition) = defs
            .iter()
            .enumerate()
            .find(|(_, d)| d.name == "id")
            .ok_or_else(|| invalid("analyzer id schema missing"))?;
        let mut raw = definition.default;
        for (id, ctl) in effect.params.iter().copied().chain(extras) {
            charge(remaining, 1)?;
            if effects::param_index(effect.kind, id) == Some(index) {
                raw = match ctl {
                    Ctl::Const(v) => v,
                    Ctl::Cell(c) => self.value(c).ok_or_else(|| {
                        invalid(format!("missing analyzer id cell {c:?} for {owner:?}"))
                    })?,
                };
            }
        }
        if !raw.is_finite() {
            return Err(invalid(format!(
                "nonfinite analyzer id for {owner:?}/{site}"
            )));
        }
        let normalized = definition.clamp(raw).max(0.0).trunc();
        if !normalized.is_finite() || f64::from(normalized) > f64::from(u32::MAX) {
            return Err(invalid("analyzer address overflow"));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let logical_start = normalized as u32;
        let width = u32::try_from(effects::analyzer::cells(kind))
            .map_err(|_| invalid("analyzer width overflow"))?;
        let end = logical_start
            .checked_add(width)
            .ok_or_else(|| invalid("analyzer range overflow"))?;
        accumulation.highwater = accumulation.highwater.max(end);
        charge(remaining, 1)?;
        self.analysis.push(FrozenAnalysisRange {
            owner,
            effect: site,
            writer_order: accumulation.writer_order,
            kind,
            logical_start,
            width,
        });
        accumulation.writer_order = accumulation
            .writer_order
            .checked_add(1)
            .ok_or_else(|| invalid("analyzer writer order overflow"))?;
        Ok(())
    }
}
#[derive(Default)]
struct AnalyzerAccumulation {
    writer_order: u32,
    highwater: u32,
}
#[derive(Clone, Copy)]
enum Graph<'a> {
    Instrument(&'a InstDef),
    Bus(&'a BusDef),
}
fn admit_owner<'a>(
    owners: &mut BTreeMap<FrozenCellOwner, Graph<'a>>,
    owner: FrozenCellOwner,
    graph: Graph<'a>,
    remaining: &mut u32,
) -> Result<bool, Failure> {
    map_charge(remaining, owners.len())?;
    if let Some(old) = owners.get(&owner) {
        // Equality can traverse a shared graph: charge its declared collections first.
        let equal = match (old, graph) {
            (Graph::Instrument(a), Graph::Instrument(b)) => {
                charge(
                    remaining,
                    a.params
                        .len()
                        .checked_add(a.nodes.len())
                        .and_then(|n| n.checked_add(a.edges.len()))
                        .and_then(|n| n.checked_add(a.node_params.len()))
                        .ok_or_else(|| invalid("graph alias work overflow"))?,
                )?;
                charge(
                    remaining,
                    b.params
                        .len()
                        .checked_add(b.nodes.len())
                        .and_then(|n| n.checked_add(b.edges.len()))
                        .and_then(|n| n.checked_add(b.node_params.len()))
                        .ok_or_else(|| invalid("graph alias work overflow"))?,
                )?;
                for spec in a.nodes.iter().chain(b.nodes.iter()) {
                    if let UGenSpec::Effect(effect) = spec {
                        charge(remaining, effect.params.len())?;
                    }
                }
                *a == b
            }
            (Graph::Bus(a), Graph::Bus(b)) => {
                charge(
                    remaining,
                    a.chain
                        .len()
                        .checked_add(b.chain.len())
                        .ok_or_else(|| invalid("bus alias work overflow"))?,
                )?;
                for effect in a.chain.iter().chain(b.chain.iter()) {
                    charge(remaining, effect.params.len())?;
                }
                *a == b
            }
            _ => false,
        };
        if !equal {
            return Err(invalid(format!("ambiguous graph owner {owner:?}")));
        }
        return Ok(false);
    }
    owners.insert(owner, graph);
    Ok(true)
}

/// Matches Template::build queue tie-breaking, with charged fixed-capacity work.
pub(crate) fn graph_execution_order(
    graph: &InstDef,
    remaining: &mut u32,
) -> Result<[u16; crate::dsp::graph::NODE_CAP], Failure> {
    const CAP: usize = crate::dsp::graph::NODE_CAP;
    let n = graph.nodes.len();
    if n > CAP || graph.edges.len() > crate::dsp::ugen::MAX_EDGES {
        return Err(invalid("cell graph exceeds template topology capacity"));
    }
    charge(
        remaining,
        n.checked_mul(2)
            .and_then(|v| v.checked_add(graph.edges.len()))
            .ok_or_else(|| invalid("topology admission overflow"))?,
    )?;
    let mut indegree = [0u16; CAP];
    for edge in &graph.edges {
        let (from, to) = (usize::from(edge.from), usize::from(edge.to));
        if from >= n || to >= n {
            return Err(invalid("cell graph edge outside node range"));
        }
        indegree[to] = indegree[to]
            .checked_add(1)
            .ok_or_else(|| invalid("cell graph indegree overflow"))?;
    }
    let mut order = [0u16; CAP];
    let mut len = 0;
    for (index, &degree) in indegree.iter().enumerate().take(n) {
        if degree == 0 {
            order[len] = u16::try_from(index).map_err(|_| invalid("cell graph index overflow"))?;
            len += 1;
        }
    }
    let mut head = 0;
    while head < len {
        charge(
            remaining,
            graph
                .edges
                .len()
                .checked_add(1)
                .ok_or_else(|| invalid("topology scan overflow"))?,
        )?;
        let node = order[head];
        head += 1;
        for edge in graph.edges.iter().filter(|edge| edge.from == node) {
            let target = usize::from(edge.to);
            indegree[target] = indegree[target]
                .checked_sub(1)
                .ok_or_else(|| invalid("cell graph degree underflow"))?;
            if indegree[target] == 0 {
                order[len] = edge.to;
                len += 1;
            }
        }
    }
    if len < n {
        return Err(invalid("cell graph topology contains a cycle"));
    }
    Ok(order)
}

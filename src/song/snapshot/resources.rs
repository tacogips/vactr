//! Issued graph declarations bound to closed PCM, never inferred from numeric IDs.
use super::FrozenRoutingInventory;
use crate::dsp::build::{DeclaredGraphResource, GraphResourceInput, GraphResourceSite};
use crate::dsp::graph::{BusId, EffectKind, InstId, UGenSpec};
use crate::host::caps::{SampleLoader, SampleSrc};
use crate::host::wire::Ctl;
use crate::ns::insts::InstRegistry;
use crate::song::assets::{PinnedSongAssets, SongAssetSelector};
use crate::vm::fail::{FailCode, Failure};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrozenGraphOwner {
    Instrument(InstId),
    Bus(BusId),
    Master(BusId),
}
/// Immutable issued binding. An empty default cannot issue a required resource.
/// ```compile_fail
/// use vactr::song::snapshot::FrozenGraphResource;
/// fn forge(mut r: FrozenGraphResource) { r.source = todo!(); }
/// ```
#[derive(Clone, Debug)]
pub struct FrozenGraphResource {
    owner: FrozenGraphOwner,
    site: GraphResourceSite,
    source: SampleSrc,
}
impl FrozenGraphResource {
    #[must_use]
    pub fn owner(&self) -> &FrozenGraphOwner {
        &self.owner
    }
    #[must_use]
    pub fn site(&self) -> &GraphResourceSite {
        &self.site
    }
    #[must_use]
    pub fn source(&self) -> &SampleSrc {
        &self.source
    }
}
#[derive(Clone, Debug, Default)]
pub struct FrozenGraphResources {
    entries: Vec<FrozenGraphResource>,
    consumed_work: u32,
}
fn fail(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}
fn admit(remaining: &mut u32, n: usize) -> Result<(), Failure> {
    let n = u32::try_from(n)
        .map_err(|_| Failure::new(FailCode::Overflow, "graph resource work overflow"))?;
    *remaining = remaining
        .checked_sub(n)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "graph resource work exhausted"))?;
    Ok(())
}
fn bank(record: &DeclaredGraphResource) -> Result<crate::value::intern::KwId, Failure> {
    match record.input() {
        GraphResourceInput::Bank(kw) => Ok(*kw),
        GraphResourceInput::UnresolvedNumeric | GraphResourceInput::UnresolvedDynamic => {
            Err(fail("fixed graph resource has no closed bank declaration"))
        }
    }
}
fn include_inst_resource(
    owner: InstId,
    record: &DeclaredGraphResource,
    event_banks: &BTreeSet<(InstId, crate::value::intern::KwId)>,
    remaining: &mut u32,
) -> Result<bool, Failure> {
    admit(remaining, 1)?;
    if matches!(record.site(), GraphResourceSite::Header { parameter } if *parameter == crate::dsp::ugen::BANK)
    {
        return Ok(event_banks.contains(&(owner, bank(record)?)));
    }
    Ok(true)
}
fn required(ctl: Option<&Ctl>) -> bool {
    match ctl {
        None => false,
        Some(Ctl::Const(x)) if *x < 0.0 => false,
        _ => true,
    }
}
fn check_fixed(
    records: &[DeclaredGraphResource],
    site: GraphResourceSite,
    ctl: Option<&Ctl>,
    remaining: &mut u32,
) -> Result<(), Failure> {
    admit(remaining, records.len())?;
    let mut matching = records.iter().filter(|r| *r.site() == site);
    let first = matching.next();
    let duplicate = matching.next().is_some();
    if !required(ctl) {
        return if first.is_none() {
            Ok(())
        } else {
            Err(fail("disabled fixed site has issued resource"))
        };
    }
    if first.is_none() || duplicate {
        return Err(fail("fixed graph site lacks unique issued authority"));
    }
    bank(first.ok_or_else(|| fail("fixed authority absent"))?)?;
    if !matches!(ctl, Some(Ctl::Const(x)) if *x == 0.0) {
        return Err(fail("fixed graph placeholder changed"));
    }
    Ok(())
}
fn validate_inst(
    graph: &crate::dsp::graph::InstDef,
    records: &[DeclaredGraphResource],
    remaining: &mut u32,
) -> Result<(), Failure> {
    let ir = crate::dsp::effects::param_ctl(EffectKind::Convolution, "ir")
        .ok_or_else(|| fail("IR control missing"))?;
    admit(
        remaining,
        graph.nodes.len() + graph.node_params.len() + graph.params.len() + records.len(),
    )?;
    for (index, node) in graph.nodes.iter().enumerate() {
        if let UGenSpec::Effect(effect) = node {
            if effect.kind == EffectKind::Convolution {
                let node = u16::try_from(index).map_err(|_| fail("fixed node index overflow"))?;
                admit(remaining, effect.params.len() + graph.node_params.len())?;
                let ctl = graph
                    .node_params
                    .iter()
                    .find_map(|(i, c, v)| (*i == node && *c == ir).then_some(v))
                    .or_else(|| {
                        effect
                            .params
                            .iter()
                            .find_map(|(c, v)| (*c == ir).then_some(v))
                    });
                check_fixed(
                    records,
                    GraphResourceSite::EmbeddedEffect {
                        node,
                        parameter: ir,
                    },
                    ctl,
                    remaining,
                )?;
            }
        }
    }
    for record in records {
        bank(record)?;
        admit(remaining, graph.params.len())?;
        let valid = match record.site() {
            GraphResourceSite::Header { parameter } => graph
                .params
                .iter()
                .any(|(id, ctl)| id == parameter && matches!(ctl,Ctl::Const(x) if *x==0.0)),
            GraphResourceSite::EmbeddedEffect { node, parameter } => {
                *parameter == ir
                    && matches!(graph.nodes.get(usize::from(*node)),Some(UGenSpec::Effect(e)) if e.kind==EffectKind::Convolution)
            }
            GraphResourceSite::BusEffect { .. } => false,
        };
        if !valid {
            return Err(fail("issued instrument resource site changed"));
        }
    }
    Ok(())
}
fn validate_bus(
    graph: &crate::dsp::graph::BusDef,
    records: &[DeclaredGraphResource],
    remaining: &mut u32,
) -> Result<(), Failure> {
    let ir = crate::dsp::effects::param_ctl(EffectKind::Convolution, "ir")
        .ok_or_else(|| fail("IR control missing"))?;
    admit(remaining, graph.chain.len() + records.len())?;
    for (index, effect) in graph.chain.iter().enumerate() {
        admit(remaining, effect.params.len())?;
        if effect.kind == EffectKind::Convolution {
            let effect_index =
                u16::try_from(index).map_err(|_| fail("fixed effect index overflow"))?;
            check_fixed(
                records,
                GraphResourceSite::BusEffect {
                    effect: effect_index,
                    parameter: ir,
                },
                effect
                    .params
                    .iter()
                    .find_map(|(c, v)| (*c == ir).then_some(v)),
                remaining,
            )?;
        }
    }
    for record in records {
        bank(record)?;
        if !matches!(record.site(),GraphResourceSite::BusEffect {effect,parameter} if *parameter==ir && graph.chain.get(usize::from(*effect)).is_some_and(|e|e.kind==EffectKind::Convolution))
        {
            return Err(fail("issued bus resource site changed"));
        }
    }
    Ok(())
}
impl FrozenGraphResources {
    #[must_use]
    pub fn entries(&self) -> &[FrozenGraphResource] {
        &self.entries
    }
    #[must_use]
    pub const fn consumed_work(&self) -> u32 {
        self.consumed_work
    }
    pub(crate) fn pin_selections(
        registry: &InstRegistry,
        instruments: &[InstId],
        event_banks: &BTreeSet<(InstId, crate::value::intern::KwId)>,
        remaining: &mut u32,
    ) -> Result<Vec<SongAssetSelector>, Failure> {
        admit(remaining, instruments.len())?;
        let mut out = Vec::new();
        for id in instruments {
            let entry = registry
                .entry(*id)
                .ok_or_else(|| fail("retained resource instrument missing"))?;
            let issued = registry.inst_resources(*id);
            if issued.is_some_and(|r| !Arc::ptr_eq(r.graph(), &entry.def)) {
                return Err(fail("stale instrument resource authority"));
            }
            let records = issued.map_or(&[][..], |r| r.resources());
            validate_inst(&entry.def, records, remaining)?;
            for record in records {
                if include_inst_resource(*id, record, event_banks, remaining)? {
                    admit(remaining, 1)?;
                    out.push(SongAssetSelector::Bank(bank(record)?));
                }
            }
        }
        for (_, entry) in registry.buses() {
            admit(remaining, 1)?;
            let records = registry
                .bus_resources(entry.id)
                .map_or(&[][..], |r| r.resources());
            validate_bus(&entry.def, records, remaining)?;
            for record in records {
                admit(remaining, 1)?;
                out.push(SongAssetSelector::Bank(bank(record)?));
            }
        }
        Ok(out)
    }
    pub(crate) fn capture(
        inventory: &FrozenRoutingInventory,
        registry: &InstRegistry,
        assets: &mut PinnedSongAssets,
        event_banks: &BTreeSet<(InstId, crate::value::intern::KwId)>,
        remaining: &mut u32,
    ) -> Result<Self, Failure> {
        let before = *remaining;
        let mut out = Self::default();
        for inst in &inventory.instruments {
            admit(remaining, 1)?;
            let entry = registry
                .entry(inst.graph.id)
                .ok_or_else(|| fail("frozen resource instrument missing"))?;
            if !Arc::ptr_eq(&entry.def, &inst.graph) {
                return Err(fail("frozen instrument differs from issued graph"));
            }
            let record = registry.inst_resources(inst.graph.id);
            if record.is_some_and(|r| !Arc::ptr_eq(r.graph(), &inst.graph)) {
                return Err(fail("stale instrument resource authority"));
            }
            let records = record.map_or(&[][..], |r| r.resources());
            validate_inst(&inst.graph, records, remaining)?;
            out.append(
                FrozenGraphOwner::Instrument(inst.graph.id),
                records,
                assets,
                event_banks,
                remaining,
            )?;
        }
        for (name, graph) in &inventory.buses {
            admit(remaining, 1)?;
            let record = registry.bus_resources(graph.id);
            if record.is_none()
                && graph
                    .chain
                    .iter()
                    .any(|effect| effect.kind == EffectKind::Convolution)
            {
                return Err(fail("bus fixed resources lack issued graph"));
            }
            if record.is_some_and(|r| !Arc::ptr_eq(r.graph(), graph)) {
                return Err(fail("stale bus resource authority"));
            }
            let records = record.map_or(&[][..], |r| r.resources());
            validate_bus(graph, records, remaining)?;
            let owner = if name.is_some() {
                FrozenGraphOwner::Bus(graph.id)
            } else {
                FrozenGraphOwner::Master(graph.id)
            };
            out.append(owner, records, assets, event_banks, remaining)?;
        }
        out.consumed_work = before
            .checked_sub(*remaining)
            .ok_or_else(|| fail("resource accounting underflow"))?;
        Ok(out)
    }
    fn append(
        &mut self,
        owner: FrozenGraphOwner,
        records: &[DeclaredGraphResource],
        assets: &mut PinnedSongAssets,
        event_banks: &BTreeSet<(InstId, crate::value::intern::KwId)>,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        for record in records {
            if let FrozenGraphOwner::Instrument(id) = &owner {
                if !include_inst_resource(*id, record, event_banks, remaining)? {
                    continue;
                }
            }
            admit(remaining, 1)?;
            let source = SampleSrc::Bank {
                kw: bank(record)?,
                index: 0,
            };
            assets.load(&source)?;
            self.entries.push(FrozenGraphResource {
                owner: owner.clone(),
                site: record.site().clone(),
                source,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ns::{
        evaluator::Evaluator, load::NoopHost, namespace::Prelude, stage::RecordingSink,
    };
    use crate::reader::span::FileId;
    use crate::song::assets::{
        DecodedSongAssetFactory, SongAssetFactory, SongAssetLimits, SongSourceFile,
    };
    use crate::value::intern_kw;
    use crate::value::value::PathVal;
    use std::collections::BTreeMap;
    fn evaluator() -> Evaluator {
        let mut ev = Evaluator::new(
            Prelude::core(),
            Box::new(NoopHost),
            Box::new(RecordingSink::default()),
        );
        ev.insts()
            .unwrap()
            .borrow_mut()
            .enable_closed_song_resources();
        for row in ev
            .eval_str(
                "inst closed:\n\tsin-osc freq > convolution ir: :fixed",
                FileId::CONSOLE,
            )
            .unwrap()
        {
            assert!(row.value.is_ok(), "{:?}", row.value);
            assert!(row
                .diags
                .iter()
                .all(|d| d.severity != crate::types::Severity::Error));
        }
        ev
    }
    fn closed_assets() -> PinnedSongAssets {
        let factory = DecodedSongAssetFactory::new(
            BTreeMap::from([(
                "ir.wav".into(),
                Arc::new(crate::host::caps::SampleData {
                    rate: 48000,
                    channels: 2,
                    frames: vec![0.25, 0.5].into(),
                }),
            )]),
            BTreeMap::from([(intern_kw("fixed"), vec!["ir.wav".into()])]),
            BTreeMap::new(),
        );
        let limits = SongAssetLimits {
            max_resources: 8,
            max_pcm_bytes: 1024,
            max_source_files: 8,
            max_source_bytes: 10000,
            max_banks: 8,
            max_walk_nodes: 100000,
            max_walk_depth: 256,
        };
        let mut prepared = factory
            .begin(
                SongSourceFile {
                    file: FileId::new(0),
                    path: PathVal {
                        text: "test.vact".into(),
                        file: None,
                    },
                },
                limits,
            )
            .unwrap();
        prepared
            .pin(&[SongAssetSelector::Bank(intern_kw("fixed"))])
            .unwrap();
        prepared.close().unwrap()
    }
    fn inventory(ev: &Evaluator) -> FrozenRoutingInventory {
        let registry = ev.insts().unwrap();
        let registry = registry.borrow();
        let entry = registry
            .entry(registry.id_of(intern_kw("closed")).unwrap())
            .unwrap();
        FrozenRoutingInventory {
            instruments: vec![super::super::FrozenInstrument {
                name: entry.name,
                graph: entry.def.clone(),
                parameters: entry.params.clone(),
                defaults: Vec::new(),
                resource: entry.resource,
            }],
            ..Default::default()
        }
    }
    #[test]
    fn issued_arc_site_and_placeholder_are_required_not_numeric_zero() {
        let ev = evaluator();
        let r = ev.insts().unwrap();
        let r = r.borrow();
        let mut assets = closed_assets();
        let original = inventory(&ev);
        let mut remaining = 10000;
        let captured = FrozenGraphResources::capture(
            &original,
            &r,
            &mut assets,
            &BTreeSet::new(),
            &mut remaining,
        )
        .unwrap();
        assert_eq!(captured.entries.len(), 1);
        assert_eq!(captured.consumed_work, 10000 - remaining);
        let cloned = captured.clone();
        assert_eq!(cloned.consumed_work, captured.consumed_work);
        let mut budget = captured.consumed_work;
        assert!(FrozenGraphResources::capture(
            &original,
            &r,
            &mut assets,
            &BTreeSet::new(),
            &mut budget
        )
        .is_ok());
        assert_eq!(budget, 0);
        let mut short = captured.consumed_work - 1;
        assert_eq!(
            FrozenGraphResources::capture(&original, &r, &mut assets, &BTreeSet::new(), &mut short)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        let mut hostile = original.clone();
        hostile.instruments[0].graph = Arc::new((*hostile.instruments[0].graph).clone());
        assert!(FrozenGraphResources::capture(
            &hostile,
            &r,
            &mut assets,
            &BTreeSet::new(),
            &mut 10000
        )
        .is_err());
        let mut graph = (*original.instruments[0].graph).clone();
        if let UGenSpec::Effect(effect) = graph.nodes.last_mut().unwrap() {
            effect.params[0].1 = Ctl::Const(1.0);
        } else {
            panic!("actual effect node");
        }
        hostile.instruments[0].graph = Arc::new(graph);
        assert!(FrozenGraphResources::capture(
            &hostile,
            &r,
            &mut assets,
            &BTreeSet::new(),
            &mut 10000
        )
        .is_err());
    }
    #[test]
    fn selected_default_and_overridden_bank_keep_fixed_ir() {
        use crate::host::caps::{InstResolver, Route};
        use crate::value::value::Sound;
        let mut ev = evaluator();
        for form in ev.eval_str(
            "inst sampler bank: keyword = :fixed:\n\tsample-play bank n: 0 > convolution ir: :fixed",
            FileId::CONSOLE,
        ).unwrap() { assert!(form.value.is_ok(), "{:?}", form.value); }
        let registry = ev.insts().unwrap();
        let r = registry.borrow();
        let id = r.id_of(intern_kw("sampler")).unwrap();
        let entry = r.entry(id).unwrap();
        let original = FrozenRoutingInventory {
            instruments: vec![super::super::FrozenInstrument {
                name: entry.name,
                graph: entry.def.clone(),
                parameters: entry.params.clone(),
                defaults: Vec::new(),
                resource: entry.resource,
            }],
            ..Default::default()
        };
        for (sound, expected) in [
            (Sound::Inst(id), 2),
            (Sound::Builtin(intern_kw("other-bank")), 1),
        ] {
            let Route::Audio {
                inst,
                sample: Some(SampleSrc::Bank { kw, .. }),
            } = r.route(&sound).unwrap()
            else {
                panic!("actual bank event route");
            };
            assert_eq!(inst, id);
            let proof = BTreeSet::from([(inst, kw)]);
            let pins = FrozenGraphResources::pin_selections(&r, &[id], &proof, &mut 10000).unwrap();
            assert_eq!(pins.len(), expected);
            assert!(pins
                .iter()
                .all(|pin| matches!(pin, SongAssetSelector::Bank(k) if *k == intern_kw("fixed"))));
            let mut assets = closed_assets();
            let bindings =
                FrozenGraphResources::capture(&original, &r, &mut assets, &proof, &mut 10000)
                    .unwrap();
            assert_eq!(bindings.entries().len(), expected);
            assert!(bindings
                .entries()
                .iter()
                .any(|b| matches!(b.site(), GraphResourceSite::EmbeddedEffect { .. })));
            assert_eq!(
                bindings
                    .entries()
                    .iter()
                    .filter(|b| matches!(b.site(), GraphResourceSite::Header { .. }))
                    .count(),
                expected - 1
            );
        }
        // A skipped BANK header does not bypass graph/issued identity validation.
        let mut hostile = original.clone();
        hostile.instruments[0].graph = Arc::new((*entry.def).clone());
        assert!(FrozenGraphResources::capture(
            &hostile,
            &r,
            &mut closed_assets(),
            &BTreeSet::new(),
            &mut 10000
        )
        .is_err());
        let records = r.inst_resources(id).unwrap().resources();
        let mut graph = (*entry.def).clone();
        let (_, ctl) = graph
            .params
            .iter_mut()
            .find(|(ctl, _)| *ctl == crate::dsp::ugen::BANK)
            .unwrap();
        *ctl = Ctl::Const(1.0);
        assert!(validate_inst(&graph, records, &mut 10000).is_err());
    }
    #[test]
    fn private_site_mismatch_cannot_rebind_issued_bank() {
        let ev = evaluator();
        let registry = ev.insts().unwrap();
        let registry = registry.borrow();
        let id = registry.id_of(intern_kw("closed")).unwrap();
        let entry = registry.entry(id).unwrap();
        let records = registry.inst_resources(id).unwrap().resources();
        let ir = crate::dsp::effects::param_ctl(EffectKind::Convolution, "ir").unwrap();
        assert!(check_fixed(
            records,
            GraphResourceSite::EmbeddedEffect {
                node: u16::MAX,
                parameter: ir
            },
            Some(&Ctl::Const(0.0)),
            &mut 10000
        )
        .is_err());
        let missing: &[DeclaredGraphResource] = &[];
        assert!(validate_inst(&entry.def, missing, &mut 10000).is_err());
    }
}

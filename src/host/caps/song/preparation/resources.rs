//! Closed graph provenance and owner-local physical resource assembly.
use super::*;
use crate::dsp::build::GraphResourceSite;
use crate::dsp::graph::{BankRef, BusDef, BusId, GranSrc, InstId, TableRef, UGenSpec};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;
use crate::song::snapshot::{FrozenCellOwner, FrozenGraphOwner, FrozenRoutingInventory};

fn source_words(source: &SampleSrc) -> Result<usize, Failure> {
    let bytes = match source {
        SampleSrc::Path(path) => path.text.len(),
        _ => 0,
    };
    bytes
        .checked_add(4)
        .ok_or_else(|| fail("sample descriptor work overflow"))
}
fn float_id(id: u32) -> Result<f32, Failure> {
    let encoded = id as f32;
    if f64::from(encoded) != f64::from(id) {
        return Err(fail("resource ID cannot be represented in graph controls"));
    }
    Ok(encoded)
}
fn replace(params: &mut [(CtlId, Ctl)], parameter: CtlId, id: u32) -> Result<(), Failure> {
    let value = params
        .iter_mut()
        .find(|(key, _)| *key == parameter)
        .ok_or_else(|| fail("issued resource declaration site missing"))?;
    value.1 = Ctl::Const(float_id(id)?);
    Ok(())
}
struct Builder<'a> {
    topology: &'a FrozenRoutingInventory,
    routes: &'a SongRoutePlan,
    limits: &'a SongPreparationLimits,
    remaining: &'a mut u32,
    epoch: SnapshotEpoch,
    track_keys: Vec<(crate::value::intern::KwId, SongLeaseKey)>,
    next: u32,
    assembly: Assembly,
}
impl Builder<'_> {
    fn key(&mut self, kind: SongResourceKind) -> Result<SongLeaseKey, Failure> {
        if self.next > self.limits.max_resources {
            return Err(fail("complete resource lease limit exceeded"));
        }
        charge(self.remaining, 32)?;
        let key = SongLeaseKey {
            epoch: self.epoch,
            resource: SongResourceRef {
                id: self.next,
                generation: 1,
            },
            kind,
        };
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| fail("resource identity exhausted"))?;
        Ok(key)
    }
    fn sample_id(&mut self, source: &SampleSrc) -> Result<u32, Failure> {
        let words = self
            .assembly
            .samples
            .iter()
            .try_fold(source_words(source)?, |total, binding| {
                total.checked_add(source_words(&binding.source).ok()?)
            })
            .ok_or_else(|| fail("sample lookup work overflow"))?;
        charge(self.remaining, words)?;
        self.assembly
            .samples
            .iter()
            .find(|s| s.source == *source)
            .map(|s| s.key.resource.id)
            .ok_or_else(|| fail("closed graph PCM binding missing"))
    }
    fn banks(
        &mut self,
        graph: SongLeaseKey,
        owner: FrozenCellOwner,
    ) -> Result<SongGraphBanks, Failure> {
        let topology = self.topology;
        let inventory = &topology.cells;
        charge(self.remaining, inventory.references().len())?;
        let mut cells = Vec::new();
        for reference in inventory.references().iter().filter(|r| r.owner == owner) {
            charge(
                self.remaining,
                cells
                    .len()
                    .checked_add(inventory.values().len())
                    .ok_or_else(|| fail("cell work overflow"))?,
            )?;
            if cells
                .iter()
                .any(|c: &crate::song::snapshot::FrozenCellValue| c.cell == reference.cell)
            {
                continue;
            }
            let value = inventory
                .value(reference.cell)
                .ok_or_else(|| fail("closed cell default missing"))?;
            if !value.is_finite() {
                return Err(fail("nonfinite closed cell value"));
            }
            charge(self.remaining, 2)?;
            cells.push(crate::song::snapshot::FrozenCellValue {
                cell: reference.cell,
                value,
            });
        }
        let controls = if cells.is_empty() {
            None
        } else {
            let key = self.key(SongResourceKind::ControlCells)?;
            self.assembly.resources.push(Resource {
                key,
                upload: None,
                admitted: false,
                ready: false,
                dispatched: false,
                returned: false,
                cells,
                analysis: 0,
                banks: None,
            });
            Some(key)
        };
        charge(self.remaining, inventory.analysis_banks().len())?;
        let slots = inventory
            .analysis_banks()
            .iter()
            .find(|b| b.owner == owner)
            .map_or(0, |b| b.slots);
        let analysis = if slots == 0 {
            None
        } else {
            let key = self.key(SongResourceKind::AnalysisBank)?;
            self.assembly.resources.push(Resource {
                key,
                upload: None,
                admitted: false,
                ready: false,
                dispatched: false,
                returned: false,
                cells: Vec::new(),
                analysis: slots,
                banks: None,
            });
            Some(key)
        };
        Ok(SongGraphBanks {
            graph,
            controls,
            analysis,
        })
    }
    fn bus(
        &mut self,
        graph: &BusDef,
        target: SongDetectorTarget,
        kind: SongResourceKind,
    ) -> Result<SongPhysicalStage, Failure> {
        let key = if let SongDetectorTarget::Track(track) = target {
            self.track_keys
                .iter()
                .find(|(id, _)| *id == track)
                .map(|(_, key)| *key)
                .ok_or_else(|| fail("physical track identity missing"))?
        } else {
            self.key(kind)?
        };
        let owner = if kind == SongResourceKind::Master {
            FrozenCellOwner::Master(graph.id)
        } else {
            FrozenCellOwner::Bus(graph.id)
        };
        let banks = self.banks(key, owner)?;
        let graph_owner = if kind == SongResourceKind::Master {
            FrozenGraphOwner::Master(graph.id)
        } else {
            FrozenGraphOwner::Bus(graph.id)
        };
        let words = graph
            .chain
            .iter()
            .try_fold(1_usize, |n, e| {
                n.checked_add(e.params.len()).and_then(|n| n.checked_add(1))
            })
            .ok_or_else(|| fail("graph work overflow"))?;
        charge(
            self.remaining,
            words
                .checked_mul(64)
                .ok_or_else(|| fail("graph work overflow"))?,
        )?;
        let mut def = graph.clone();
        def.id = BusId::new(key.resource.id);
        let topology = self.topology;
        charge(self.remaining, topology.resources.entries().len())?;
        for binding in topology
            .resources
            .entries()
            .iter()
            .filter(|r| *r.owner() == graph_owner)
        {
            let id = self.sample_id(binding.source())?;
            let GraphResourceSite::BusEffect { effect, parameter } = *binding.site() else {
                return Err(fail("wrong graph resource site"));
            };
            let spec = def
                .chain
                .get_mut(usize::from(effect))
                .ok_or_else(|| fail("graph effect site missing"))?;
            charge(self.remaining, spec.params.len())?;
            replace(&mut spec.params, parameter, id)?;
        }
        // Sidechain bindings target the actual graph instance, never its name.
        let routes = self.routes;
        charge(self.remaining, routes.sidechains.len())?;
        for route in routes.sidechains.iter().filter(|r| r.target == target) {
            charge(self.remaining, self.track_keys.len())?;
            let source = self
                .track_keys
                .iter()
                .find(|(track, _)| *track == route.source_track)
                .ok_or_else(|| fail("physical detector source stage missing"))?;
            let effect = def
                .chain
                .get_mut(usize::try_from(route.unit).map_err(|_| fail("effect index overflow"))?)
                .ok_or_else(|| fail("detector effect site missing"))?;
            charge(self.remaining, effect.params.len())?;
            replace(
                &mut effect.params,
                crate::dsp::effects::SIDECHAIN_BUS_CTL,
                source.1.resource.id,
            )?;
        }
        let mut bytes = Vec::new();
        crate::dsp::arena::encode_bus(&def, kind == SongResourceKind::Master, &mut bytes)
            .map_err(|_| fail("closed bus cannot encode"))?;
        if bytes.len() as u64 > self.limits.max_graph_bytes
            || bytes.len() + 22 > crate::dsp::ring::INBOX_SLOT_BYTES
        {
            return Err(fail("bus graph exceeds atomic upload limit"));
        }
        let def = Arc::new(def);
        let handle = if kind == SongResourceKind::Master {
            GraphHandle::Master(def)
        } else {
            GraphHandle::Bus { id: def.id, def }
        };
        self.assembly.resources.push(Resource {
            key,
            upload: Some(Upload::Graph {
                graph: handle,
                native: None,
                materialized: false,
            }),
            admitted: false,
            ready: false,
            dispatched: false,
            returned: false,
            cells: Vec::new(),
            analysis: 0,
            banks: Some(banks),
        });
        Ok(SongPhysicalStage {
            target,
            lease: key,
            banks,
        })
    }
    fn instrument(
        &mut self,
        branch: &SongBranchRoute,
    ) -> Result<(SongLeaseKey, SongGraphBanks), Failure> {
        charge(self.remaining, self.topology.instruments.len())?;
        let instrument = self
            .topology
            .instruments
            .iter()
            .find(|i| i.graph.id == branch.resolved_instrument)
            .ok_or_else(|| fail("closed instrument graph missing"))?;
        let name = instrument.name;
        let resource_name = instrument.resource;
        let graph = Arc::clone(&instrument.graph);
        let key = self.key(SongResourceKind::Instrument)?;
        let banks = self.banks(key, FrozenCellOwner::Instrument(name))?;

        let words = graph
            .nodes
            .len()
            .checked_add(graph.params.len())
            .and_then(|n| n.checked_add(graph.edges.len()))
            .and_then(|n| n.checked_add(graph.node_params.len()))
            .ok_or_else(|| fail("graph work overflow"))?;
        charge(
            self.remaining,
            words
                .checked_mul(64)
                .ok_or_else(|| fail("graph work overflow"))?,
        )?;
        let embedded = graph
            .nodes
            .iter()
            .try_fold(0_usize, |n, node| {
                n.checked_add(match node {
                    UGenSpec::Effect(effect) => effect.params.len(),
                    // Clone copies the full fixed arrays, including unused entries.
                    UGenSpec::StageLinked { data: Some(_) } => 145,
                    UGenSpec::FrameKeyframe { data: Some(_) } => 321,
                    _ => 0,
                })
            })
            .ok_or_else(|| fail("embedded graph work overflow"))?;
        charge(
            self.remaining,
            embedded
                .checked_mul(64)
                .ok_or_else(|| fail("embedded graph work overflow"))?,
        )?;
        let mut def = (*graph).clone();
        def.id = InstId::new(key.resource.id);
        let mut bank_id = None;
        let mut table_id = None;
        let mut source_id = None;
        let table_ctl = crate::dsp::controls::row("table")
            .ok_or_else(|| fail("table control missing"))?
            .ctl;
        let source_ctl = crate::dsp::controls::row("source")
            .ok_or_else(|| fail("source control missing"))?
            .ctl;
        let mut selected_parameter = None;
        let topology = self.topology;
        charge(self.remaining, topology.resources.entries().len())?;
        for binding in topology
            .resources
            .entries()
            .iter()
            .filter(|r| *r.owner() == FrozenGraphOwner::Instrument(graph.id))
        {
            let id = self.sample_id(binding.source())?;
            match *binding.site() {
                GraphResourceSite::Header { parameter } => {
                    charge(self.remaining, def.params.len())?;
                    replace(&mut def.params, parameter, id)?;
                    if parameter == crate::dsp::ugen::BANK {
                        bank_id = Some(id);
                    }
                    if parameter == table_ctl {
                        table_id = Some(id);
                    }
                    if parameter == source_ctl {
                        source_id = Some(id);
                    }
                    if matches!(binding.source(), SampleSrc::Bank { kw, .. } if Some(*kw) == resource_name)
                    {
                        selected_parameter = Some(parameter);
                    }
                }
                GraphResourceSite::EmbeddedEffect { node, parameter } => {
                    let Some(UGenSpec::Effect(effect)) = def.nodes.get_mut(usize::from(node))
                    else {
                        return Err(fail("issued embedded FX missing"));
                    };
                    charge(
                        self.remaining,
                        effect
                            .params
                            .len()
                            .checked_add(def.node_params.len())
                            .ok_or_else(|| fail("graph work overflow"))?,
                    )?;
                    replace(&mut effect.params, parameter, id)?;
                    for (index, ctl, value) in def.node_params.iter_mut() {
                        if *index == node && *ctl == parameter {
                            *value = Ctl::Const(float_id(id)?);
                        }
                    }
                }
                GraphResourceSite::BusEffect { .. } => {
                    return Err(fail("wrong instrument resource site"))
                }
            }
        }
        if let Some(source) = &branch.sample {
            let id = self.sample_id(source)?;
            charge(self.remaining, def.params.len())?;
            let parameter = selected_parameter
                .or_else(|| {
                    def.params
                        .iter()
                        .any(|(ctl, _)| *ctl == crate::dsp::ugen::BANK)
                        .then_some(crate::dsp::ugen::BANK)
                })
                .ok_or_else(|| fail("selected sample header authority missing"))?;
            replace(&mut def.params, parameter, id)?;
            if parameter == crate::dsp::ugen::BANK {
                bank_id = Some(id);
            } else if parameter == table_ctl {
                table_id = Some(id);
            } else if parameter == source_ctl {
                source_id = Some(id);
            } else {
                return Err(fail("unknown selected resource header control"));
            }
        }
        for node in def.nodes.iter_mut() {
            match node {
                UGenSpec::SamplePlay(bank) => {
                    *bank =
                        BankRef::new(bank_id.ok_or_else(|| fail("sample node lacks issued BANK"))?)
                }
                UGenSpec::Wavetable(table) => {
                    *table = TableRef::new(
                        table_id.ok_or_else(|| fail("table node lacks issued table header"))?,
                    )
                }
                UGenSpec::Granular(GranSrc::Sample(bank)) => {
                    *bank = BankRef::new(
                        source_id.ok_or_else(|| fail("granular node lacks issued source"))?,
                    )
                }
                UGenSpec::Granular(GranSrc::Table(table)) => {
                    *table = TableRef::new(
                        source_id.ok_or_else(|| fail("granular node lacks issued source"))?,
                    )
                }
                _ => {}
            }
        }
        let mut bytes = Vec::new();
        crate::dsp::arena::encode_inst(&def, &mut bytes)
            .map_err(|_| fail("closed instrument cannot encode"))?;
        if bytes.len() as u64 > self.limits.max_graph_bytes
            || bytes.len() + 22 > crate::dsp::ring::INBOX_SLOT_BYTES
        {
            return Err(fail("instrument graph exceeds atomic upload limit"));
        }
        let def = Arc::new(def);
        self.assembly.resources.push(Resource {
            key,
            upload: Some(Upload::Graph {
                graph: GraphHandle::Inst { id: def.id, def },
                native: None,
                materialized: false,
            }),
            admitted: false,
            ready: false,
            dispatched: false,
            returned: false,
            cells: Vec::new(),
            analysis: 0,
            banks: Some(banks),
        });
        Ok((key, banks))
    }
}

pub(super) fn build(
    prepared: &mut PreparedSong,
    routes: &SongRoutePlan,
    limits: &SongPreparationLimits,
    clock: SongHostClock,
    remaining: &mut u32,
) -> Result<Assembly, Failure> {
    let topology = &routes.topology;
    charge(remaining, limits.song.max_nodes as usize)?;
    let buses = materialize_route_buses(routes, limits.song)?;
    let mut sources = Vec::new();
    for source in routes
        .branches
        .iter()
        .filter_map(|branch| branch.sample.as_ref())
    {
        let cost = source_words(source)?
            .checked_mul(
                sources
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| fail("sample association work overflow"))?,
            )
            .ok_or_else(|| fail("sample association work overflow"))?;
        charge(remaining, cost)?;
        if !sources.contains(source) {
            sources.push(source.clone());
        }
    }
    for binding in topology.resources.entries() {
        charge(
            remaining,
            routes
                .branches
                .len()
                .checked_add(buses.len())
                .ok_or_else(|| fail("resource owner inspection overflow"))?,
        )?;
        let active = match *binding.owner() {
            FrozenGraphOwner::Instrument(id) => routes
                .branches
                .iter()
                .any(|b| b.resolved_instrument == id && b.reserved_generations != 0),
            FrozenGraphOwner::Bus(id) => buses
                .iter()
                .any(|b| b.target != SongDetectorTarget::Master && b.template.id == id),
            FrozenGraphOwner::Master(id) => buses
                .iter()
                .any(|b| b.target == SongDetectorTarget::Master && b.template.id == id),
        };
        if !active {
            continue;
        }
        let source = binding.source();
        let cost = source_words(source)?
            .checked_mul(
                sources
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| fail("sample association work overflow"))?,
            )
            .ok_or_else(|| fail("sample association work overflow"))?;
        charge(remaining, cost)?;
        if !sources.contains(source) {
            sources.push(source.clone());
        }
    }
    let mut builder = Builder {
        topology,
        routes,
        limits,
        remaining,
        epoch: prepared.epoch(),
        next: 1,
        track_keys: Vec::new(),
        assembly: Assembly::default(),
    };
    for source in sources {
        let data = prepared.sample(&source)?;
        charge(builder.remaining, data.frames.len())?;
        if !(8000..=192000).contains(&data.rate)
            || !matches!(data.channels, 1 | 2)
            || data.frames.len() % usize::from(data.channels) != 0
            || u32::try_from(data.frames.len()).is_err()
            || data.frames.iter().any(|v| !v.is_finite())
        {
            return Err(fail("closed PCM geometry malformed"));
        }
        // Retain one lease per exact immutable decoded allocation, including
        // aliases between an event bank and fixed IR/table declarations.
        charge(builder.remaining, builder.assembly.resources.len())?;
        let existing = builder
            .assembly
            .resources
            .iter()
            .find_map(|r| match &r.upload {
                Some(Upload::Sample(other)) if Arc::ptr_eq(other, &data) => Some(r.key),
                _ => None,
            });
        let key = if let Some(key) = existing {
            key
        } else {
            let key = builder.key(SongResourceKind::Sample)?;
            builder.assembly.resources.push(Resource {
                key,
                upload: Some(Upload::Sample(data)),
                admitted: false,
                ready: false,
                dispatched: false,
                returned: false,
                cells: Vec::new(),
                analysis: 0,
                banks: None,
            });
            key
        };
        builder.assembly.samples.push(SampleBinding { source, key });
    }
    // Make every detector-source track visible in the owner map before bus
    // rewriting. Their immutable IDs are assigned before any graph is staged.
    charge(
        builder.remaining,
        routes
            .tracks
            .len()
            .checked_mul(2)
            .ok_or_else(|| fail("track mapping work overflow"))?,
    )?;
    for track in &routes.tracks {
        let key = builder.key(SongResourceKind::Track)?;
        builder.track_keys.push((track.track, key));
    }
    for bus in buses
        .iter()
        .filter(|b| matches!(b.target, SongDetectorTarget::Track(_)))
    {
        let stage = builder.bus(&bus.template, bus.target, SongResourceKind::Track)?;
        builder.assembly.stages.push(stage);
    }
    let master = buses
        .iter()
        .find(|b| b.target == SongDetectorTarget::Master)
        .ok_or_else(|| fail("master stage missing"))?;
    let master = builder.bus(&master.template, master.target, SongResourceKind::Master)?;
    builder.assembly.stages.push(master);
    let mut physical = 0_u32;
    for branch in &routes.branches {
        let last_generation = demand::generation_ceiling(branch)?;
        if last_generation == 0 {
            continue;
        }
        let fx = buses
            .iter()
            .find(|b| b.target == SongDetectorTarget::Branch(branch.id))
            .ok_or_else(|| fail("private FX stage missing"))?;
        for slot in 0..branch.reserved_generations {
            charge(builder.remaining, 1)?;
            let (instrument, banks) = builder.instrument(branch)?;
            let private_fx = builder.bus(&fx.template, fx.target, SongResourceKind::PrivateFx)?;
            let track = builder
                .assembly
                .stages
                .iter()
                .find(|s| s.target == SongDetectorTarget::Track(branch.track))
                .ok_or_else(|| fail("track stage missing"))?;
            let initial = SongReusableBranch {
                config: SongBranchConfig {
                    epoch: builder.epoch,
                    branch: SongBranchId(physical),
                    generation: 1,
                    family: branch.resolved_instrument.get(),
                    track: branch.track.get(),
                    instrument: instrument.resource,
                    private_fx: Some(private_fx.lease.resource),
                    track_template: Some(track.lease.resource),
                    master: Some(master.lease.resource),
                    transition_frame: clock.frame,
                    tail_deadline: u64::MAX,
                },
                last_generation,
            };
            builder.assembly.pools.push(SongPhysicalBranch {
                logical: branch.id,
                slot,
                initial,
                banks,
            });
            let mut private_fx = private_fx;
            private_fx.target = SongDetectorTarget::Branch(SongBranchId(physical));
            builder.assembly.stages.push(private_fx);
            physical = physical
                .checked_add(1)
                .ok_or_else(|| fail("physical branch ID overflow"))?;
        }
    }
    Ok(builder.assembly)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_control_ids_require_exact_float_representation() {
        assert_eq!(float_id(1 << 24).unwrap(), (1 << 24) as f32);
        assert!(float_id((1 << 24) + 1).is_err());
        assert!(float_id(u32::MAX).is_err());
        assert_eq!(
            float_id(u32::MAX - 255).unwrap() as f64,
            f64::from(u32::MAX - 255)
        );
    }
}

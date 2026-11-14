//! Immutable symbolic route preparation, separate from POD transport.
use super::components::outer_placement_overlap;
use super::*;
use crate::dsp::caps::CapabilitySet;
use crate::song::snapshot::SongSnapshot;
use crate::vm::fail::{FailCode, Failure};

mod builder;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PreparationLedger {
    Legacy,
    Issued,
}
pub(super) struct MeteredPreparation<'a> {
    pub inventory: &'a FrozenRoutingInventory,
    pub settings: crate::song::SongSettings,
    pub resource_count: usize,
    pub pcm_bytes: u64,
    pub caps: &'a CapabilitySet,
    pub available: &'a SongHostCapacities,
    pub limits: crate::song::SongLimits,
    pub remaining: &'a mut u32,
    pub depth: u32,
    pub ledger: PreparationLedger,
}

/// Compile and admit the complete symbolic configuration set before activation.
/// Does not evaluate music, expand repeats, allocate host IDs, or install graphs.
pub fn prepare_routes(
    snapshot: &SongSnapshot,
    caps: &CapabilitySet,
    available: &SongHostCapacities,
) -> Result<SongRoutePlan, Failure> {
    let limits = crate::song::SongLimits {
        max_nodes: 1_000_000,
        max_depth: 256,
        ..Default::default()
    };
    let mut remaining = limits.max_nodes;
    prepare_routes_metered(MeteredPreparation {
        inventory: snapshot.routing(),
        settings: snapshot.settings(),
        resource_count: snapshot.resource_count(),
        pcm_bytes: snapshot.pcm_bytes(),
        caps,
        available,
        limits,
        remaining: &mut remaining,
        depth: 0,
        ledger: PreparationLedger::Legacy,
    })
}
/// Compile from the authenticated immutable inventory using the caller's original ledger.
pub(super) fn prepare_routes_metered(
    request: MeteredPreparation<'_>,
) -> Result<SongRoutePlan, Failure> {
    let MeteredPreparation {
        inventory,
        settings,
        resource_count,
        pcm_bytes,
        caps,
        available,
        limits,
        remaining,
        depth,
        ledger,
    } = request;
    limits.validate()?;
    if *remaining > limits.max_nodes {
        return Err(Failure::new(
            FailCode::Type,
            "route counter exceeds original limit",
        ));
    }
    if depth >= limits.max_depth {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "route preparation depth exhausted",
        ));
    }
    available.validate_rate()?;
    let root = inventory
        .parts
        .get(inventory.root_part)
        .ok_or_else(|| route_failure("missing root Part"))?;
    if ledger == PreparationLedger::Issued {
        // Bound track/template selection by track count times each bus candidate plus no match.
        precharge(
            remaining,
            product(
                root.tracks.len(),
                inventory
                    .buses
                    .len()
                    .checked_add(1)
                    .ok_or_else(capacity_overflow)?,
            )?,
        )?;
    }
    let mut tracks = Vec::new();
    let mut next_bus = inventory
        .buses
        .iter()
        .map(|(_, b)| b.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(capacity_overflow)?;
    for &track in &root.tracks {
        let template = inventory
            .buses
            .iter()
            .find_map(|(name, def)| (*name == Some(track)).then(|| def.clone()));
        tracks.push(SongTrackRoute {
            track,
            destination: BusId::new(next_bus),
            template,
        });
        next_bus = next_bus.checked_add(1).ok_or_else(capacity_overflow)?;
    }
    let mut builder = RouteBuilder {
        inventory,
        available,
        tracks: &tracks,
        branches: Vec::new(),
        source_covers: Vec::new(),
        tail_cycles: settings
            .tail_seconds
            .checked_div(settings.seconds_per_cycle()?)?,
        work: 0,
        quota: *remaining,
        limits,
        min_duration: None,
    };
    let walked = builder.walk(inventory.root_part, 1, &mut Vec::new(), &[], &[], depth);
    *remaining = builder
        .quota
        .checked_sub(builder.work)
        .ok_or_else(capacity_overflow)?;
    walked?;
    let nested_source_covers = match ledger {
        PreparationLedger::Legacy => {
            nested::prepare_nested_covers(inventory, &builder.source_covers, remaining)?
        }
        PreparationLedger::Issued => nested::prepare_nested_covers_metered(
            inventory,
            &builder.source_covers,
            limits,
            remaining,
            depth,
        )?,
    };
    let source_covers = builder.source_covers;
    let mut branches = builder.branches;
    let min_duration = builder.min_duration;
    let seconds = settings.seconds_per_cycle()?;
    let overlap = match min_duration {
        Some(duration) => {
            let ratio = settings
                .tail_seconds
                .checked_div(duration.checked_mul(seconds)?)?;
            u64::try_from(
                ratio
                    .floor()
                    .checked_add(i64::from(ratio.frac() != Ratio64::ZERO))
                    .ok_or_else(capacity_overflow)?,
            )
            .map_err(|_| capacity_overflow())?
            .checked_add(1)
            .ok_or_else(capacity_overflow)?
        }
        None => 1,
    };
    // Every potential configuration can own generations across this many
    // successive minimum-duration placement boundaries, including old tails.
    let mut max_live_generations = 0u32;
    if ledger == PreparationLedger::Issued {
        // Bound reservation accounting by one visit per branch and final aggregation.
        precharge(
            remaining,
            branches
                .len()
                .checked_add(1)
                .ok_or_else(capacity_overflow)?,
        )?;
    }
    for route in &mut branches {
        route.reserved_generations = u32::try_from(
            route.occurrences.min(match route.source_cover {
                Some(index) => source_covers
                    .get(index as usize)
                    .ok_or_else(|| route_failure("invalid source cover"))?
                    .live_generation_bound
                    .checked_mul(outer_placement_overlap(
                        inventory,
                        &route.placement,
                        route.track,
                        builder.tail_cycles,
                        remaining,
                    )?)
                    .ok_or_else(capacity_overflow)?,
                None => overlap,
            }),
        )
        .map_err(|_| capacity_overflow())?;
        max_live_generations = max_live_generations
            .checked_add(route.reserved_generations)
            .ok_or_else(capacity_overflow)?;
    }
    if ledger == PreparationLedger::Issued {
        // Bound the master selector scan by every bus plus the no-match case.
        precharge(
            remaining,
            inventory
                .buses
                .len()
                .checked_add(1)
                .ok_or_else(capacity_overflow)?,
        )?;
    }
    let master = inventory
        .buses
        .iter()
        .find_map(|(name, def)| name.is_none().then(|| def.clone()));
    if ledger == PreparationLedger::Issued {
        // Bound detector graph/control scans and detector validation by tracks, branches, buses and controls.
        let controls = inventory.buses.iter().try_fold(0usize, |sum, (_, bus)| {
            bus.chain.iter().try_fold(sum, |sum, unit| {
                sum.checked_add(unit.params.len())
                    .ok_or_else(capacity_overflow)
            })
        })?;
        let bound = tracks
            .len()
            .checked_add(branches.len())
            .and_then(|n| n.checked_add(inventory.buses.len()))
            .and_then(|n| n.checked_add(controls))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(capacity_overflow)?;
        precharge(remaining, bound)?;
    }
    let sidechains = sidechain_routes(inventory, &tracks, master.as_deref(), &branches)?;
    let sr = available.sample_rate as f32;
    if ledger == PreparationLedger::Issued {
        // Bound frame aggregation by all tracks, branches and the master graph.
        precharge(
            remaining,
            tracks
                .len()
                .checked_add(branches.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(capacity_overflow)?,
        )?;
    }
    let mut bus_frames = 0u64;
    for track in &tracks {
        if ledger == PreparationLedger::Issued {
            if let Some(graph) = track.template.as_deref() {
                precharge(remaining, bus_compile_bound(graph)?)?;
            }
        }
        bus_frames = add64(
            bus_frames,
            chain_frames(track.template.as_deref(), sr, caps)?,
        )?;
    }
    if ledger == PreparationLedger::Issued {
        if let Some(graph) = master.as_deref() {
            precharge(remaining, bus_compile_bound(graph)?)?;
        }
    }
    bus_frames = add64(bus_frames, chain_frames(master.as_deref(), sr, caps)?)?;
    let private_count = max_live_generations;
    let branch_delay_frames = mul64(
        u64::from(max_live_generations),
        mul64(
            u64::from(available.sample_rate)
                .checked_mul(4)
                .ok_or_else(capacity_overflow)?
                .checked_add(4)
                .ok_or_else(capacity_overflow)?,
            2,
        )?,
    )?;
    for route in &branches {
        let graph = route
            .effect_template
            .map(|name| named_bus(inventory, name))
            .transpose()?;
        if ledger == PreparationLedger::Issued {
            if let Some(graph) = graph {
                precharge(remaining, bus_compile_bound(graph)?)?;
            }
        }
        bus_frames = add64(
            bus_frames,
            mul64(
                chain_frames(graph, sr, caps)?,
                u64::from(route.reserved_generations),
            )?,
        )?;
    }
    bus_frames = add64(bus_frames, branch_delay_frames)?;
    // Validate the measured fixed shared pool, not a second snapshot lease.
    let voice_slots = available.voice_slots.min(u32::from(caps.max_voices));
    if voice_slots == 0 {
        return Err(capacity_failure("voice_slots"));
    }
    let env = crate::dsp::ugen::BuildEnv {
        sr,
        caps: *caps,
        voice_mem: usize::try_from(available.voice_frames / u64::from(voice_slots))
            .map_err(|_| capacity_overflow())?,
    };
    let mut max_voice_frames = 0usize;
    let mut cells = std::collections::BTreeSet::new();
    for instrument in &inventory.instruments {
        if ledger == PreparationLedger::Issued {
            // Bound graph compilation traversal and diagnostics by all graph tables.
            let graph = &instrument.graph;
            let bound = graph
                .nodes
                .len()
                .checked_add(graph.edges.len())
                .and_then(|n| n.checked_add(graph.node_params.len()))
                .and_then(|n| n.checked_add(graph.params.len()))
                .and_then(|n| n.checked_add(1))
                .ok_or_else(capacity_overflow)?;
            precharge(remaining, bound)?;
        }
        let template = crate::dsp::ugen::Template::from_inst(&instrument.graph, &env)
            .map_err(|e| route_failure(&format!("instrument compilation: {e:?}")))?;
        max_voice_frames = max_voice_frames.max(template.mem_total);
        for (id, _) in &instrument.defaults {
            cells.insert(*id);
        }
    }
    // Bus default cell references are candidate-local too; remapping must cover
    // every copied graph, not just instrument defaults.
    for (_, bus) in &inventory.buses {
        if ledger == PreparationLedger::Issued {
            // Bound each bus default-cell scan by its chain units and parameter cells.
            precharge(remaining, bus_compile_bound(bus)?)?;
        }
        for unit in bus.chain.iter() {
            for (_, ctl) in unit.params.iter() {
                if let crate::host::wire::Ctl::Cell(id) = ctl {
                    cells.insert(*id);
                }
            }
        }
    }
    let bus_slots = u32::try_from(tracks.len())
        .map_err(|_| capacity_overflow())?
        .checked_add(1)
        .and_then(|n| n.checked_add(private_count))
        .ok_or_else(capacity_overflow)?;
    let sample_resources = u32::try_from(resource_count).map_err(|_| capacity_overflow())?;
    let cell_slots = u32::try_from(cells.len()).map_err(|_| capacity_overflow())?;
    let template_slots = max_live_generations;
    let ack_slots = template_slots
        .checked_add(bus_slots)
        .and_then(|n| n.checked_add(sample_resources))
        .and_then(|n| n.checked_add(2))
        .ok_or_else(capacity_overflow)?;
    let required = SongHostCapacities {
        sample_rate: available.sample_rate,
        cell_slots,
        voice_slots,
        template_slots,
        bus_slots,
        sample_resources,
        pcm_bytes,
        voice_frames: mul64(max_voice_frames as u64, u64::from(voice_slots))?,
        bus_frames,
        ack_slots,
    };
    available.admit(required).map_err(|failure| {
        branches.first().map_or_else(
            || failure.clone(),
            |owner| {
                owner_failure(
                    failure.clone(),
                    inventory,
                    owner.track,
                    &owner.instrument,
                    owner.scope_part,
                    &owner.placement,
                )
            },
        )
    })?;
    let required_bytes = add64(
        required.pcm_bytes,
        mul64(add64(required.voice_frames, required.bus_frames)?, 4)?,
    )?;
    Ok(SongRoutePlan {
        source_covers,
        nested_source_covers,
        branches,
        tracks,
        master,
        sidechains,
        max_live_generations,
        required_bytes,
        branch_delay_frames,
        required,
        topology: match ledger {
            PreparationLedger::Legacy => inventory.clone(),
            PreparationLedger::Issued => {
                crate::song::snapshot::occupancy::route_view::copy_inventory(
                    inventory, limits, remaining, depth,
                )?
            }
        },
    })
}
fn precharge(remaining: &mut u32, cost: usize) -> Result<(), Failure> {
    let cost = u32::try_from(cost).map_err(|_| capacity_overflow())?;
    *remaining = remaining
        .checked_sub(cost)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "song route input work exhausted"))?;
    Ok(())
}
fn product(left: usize, right: usize) -> Result<usize, Failure> {
    left.checked_mul(right).ok_or_else(capacity_overflow)
}
fn bus_compile_bound(bus: &BusDef) -> Result<usize, Failure> {
    bus.chain.iter().try_fold(1usize, |sum, unit| {
        sum.checked_add(1)
            .and_then(|n| n.checked_add(unit.params.len()))
            .ok_or_else(capacity_overflow)
    })
}
fn route_failure(message: &str) -> Failure {
    Failure::new(FailCode::BeyondCapability, format!("song route: {message}"))
}
fn owner_failure(
    mut failure: Failure,
    inventory: &FrozenRoutingInventory,
    track: KwId,
    family: &FrozenSound,
    scope: usize,
    path: &[SongRoutePlacement],
) -> Failure {
    let label = match family {
        FrozenSound::Builtin(name) => format!(":{}", crate::value::intern::name_of_kw(*name)),
        FrozenSound::Instrument(id) => inventory
            .instruments
            .iter()
            .find(|i| i.graph.id == *id)
            .map(|i| format!("inst:{}", crate::value::intern::name_of_kw(i.name)))
            .unwrap_or_else(|| format!("inst:{}", id.get())),
        FrozenSound::Sample { path, .. } => format!("sample:{}", path),
        FrozenSound::Buffer(id) => format!("buffer:{id}"),
    };
    let label: String = label.chars().take(96).collect();
    let track: String = crate::value::intern::name_of_kw(track)
        .chars()
        .take(96)
        .collect();
    failure.message = format!(
        "{}; track={track}, family={label}, scope_part={scope}, placement={:?} ({} edges)",
        failure.message,
        &path[..path.len().min(4)],
        path.len()
    );
    failure
}
fn add64(a: u64, b: u64) -> Result<u64, Failure> {
    a.checked_add(b).ok_or_else(capacity_overflow)
}
fn mul64(a: u64, b: u64) -> Result<u64, Failure> {
    a.checked_mul(b).ok_or_else(capacity_overflow)
}
fn named_bus(inventory: &FrozenRoutingInventory, name: KwId) -> Result<&BusDef, Failure> {
    inventory
        .buses
        .iter()
        .find_map(|(key, def)| (*key == Some(name)).then_some(def.as_ref()))
        .ok_or_else(|| route_failure("explicit bus/FX is undeclared"))
}
fn chain_frames(graph: Option<&BusDef>, sr: f32, caps: &CapabilitySet) -> Result<u64, Failure> {
    let room = crate::dsp::effects::mem_len(crate::dsp::graph::EffectKind::Room, sr, caps) as u64;
    let mut frames = room;
    if let Some(graph) = graph {
        let template = crate::dsp::bus::BusTemplate::from_def(graph)
            .map_err(|e| route_failure(&format!("bus compilation: {e:?}")))?;
        for k in 0..template.n {
            frames = add64(
                frames,
                crate::dsp::effects::mem_len(template.kinds[k], sr, caps) as u64,
            )?;
        }
    }
    Ok(frames.max(mul64(room, 4)?))
}
fn sidechain_routes(
    inventory: &FrozenRoutingInventory,
    tracks: &[SongTrackRoute],
    master: Option<&BusDef>,
    branches: &[SongBranchRoute],
) -> Result<Vec<SongSidechainRoute>, Failure> {
    let mut graphs = Vec::new();
    for stage in tracks {
        if let Some(graph) = stage.template.as_deref() {
            graphs.push((graph, SongDetectorTarget::Track(stage.track)));
        }
    }
    if let Some(graph) = master {
        graphs.push((graph, SongDetectorTarget::Master));
    }
    for branch in branches {
        if let Some(name) = branch.effect_template {
            graphs.push((
                named_bus(inventory, name)?,
                SongDetectorTarget::Branch(branch.id),
            ));
        }
    }
    let mut out = Vec::new();
    for (graph, target) in graphs {
        // Preserve malformed/self selector rejection from actual compilation.
        crate::dsp::bus::BusTemplate::from_def(graph)
            .map_err(|e| route_failure(&format!("detector template: {e:?}")))?;
        for (unit, effect) in graph.chain.iter().enumerate() {
            for (id, control) in effect.params.iter() {
                if *id != crate::dsp::effects::SIDECHAIN_BUS_CTL {
                    continue;
                }
                let source = crate::dsp::effects::sidechain_bus(*control)
                    .ok_or_else(|| route_failure("malformed detector selector"))?;
                let name = inventory
                    .buses
                    .iter()
                    .find_map(|(name, definition)| (definition.id == source).then_some(*name))
                    .flatten()
                    .ok_or_else(|| {
                        route_failure("detector source is not a declared named track")
                    })?;
                let stage = tracks
                    .iter()
                    .find(|stage| stage.track == name)
                    .ok_or_else(|| {
                        route_failure("detector source is outside declared song tracks")
                    })?;
                out.push(SongSidechainRoute {
                    template: graph.id,
                    target,
                    unit: u32::try_from(unit).map_err(|_| capacity_overflow())?,
                    source_track: name,
                    source_destination: stage.destination,
                });
            }
        }
    }
    source::validate_detectors(tracks, branches, &out)?;
    Ok(out)
}
struct RouteBuilder<'a> {
    inventory: &'a FrozenRoutingInventory,
    available: &'a SongHostCapacities,
    tracks: &'a [SongTrackRoute],
    branches: Vec<SongBranchRoute>,
    source_covers: Vec<SongSourceRouteCover>,
    tail_cycles: Ratio64,
    work: u32,
    quota: u32,
    limits: crate::song::SongLimits,
    min_duration: Option<Ratio64>,
}

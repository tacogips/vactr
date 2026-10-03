//! Acknowledged physical family gates and immutable semantic comparison.
use super::*;
impl Runtime {
    pub(crate) fn active_song_families(&self, epoch: SnapshotEpoch) -> Option<Vec<FrozenSound>> {
        let owner = self.song.owners.iter().find(|o| {
            o.transport.epoch() == epoch
                && o.transport.applied_activation().is_some()
                && matches!(
                    o.transport.state(),
                    SongTransportState::Playing | SongTransportState::Draining
                )
        })?;
        if owner.transport.epoch() != epoch {
            return None;
        }
        let mut sounds = Vec::new();
        for record in &owner.families {
            if !sounds.contains(&record.sound) {
                sounds.push(record.sound.clone());
            }
        }
        Some(sounds)
    }
    pub(crate) fn request_song_mute(
        &mut self,
        epoch: SnapshotEpoch,
        sounds: &[FrozenSound],
        muted: bool,
        request: u64,
    ) -> Result<(), Failure> {
        if self.song.mutes.len() >= 32 || self.song.mutes.iter().any(|p| p.epoch == epoch) {
            return Err(song_failure("mute request already pending"));
        }
        let owner = self
            .song
            .owners
            .iter()
            .rev()
            .find(|o| {
                o.transport.applied_activation().is_some()
                    && matches!(
                        o.transport.state(),
                        SongTransportState::Playing | SongTransportState::Draining
                    )
            })
            .ok_or_else(|| song_failure("unknown acknowledged active song epoch"))?;
        if owner.transport.epoch() != epoch {
            return Err(song_failure("stale active song epoch"));
        }
        if self.song.owners.iter().any(|new| {
            new.replacement
                .as_ref()
                .is_some_and(|p| !p.definitive() && p.previous == epoch)
        }) {
            return Err(song_failure(
                "mute serialized behind frozen replacement overlay",
            ));
        }
        let mut families = Vec::new();
        for sound in sounds {
            let mut found = false;
            for record in &owner.families {
                let instrument = &record.instrument;
                if &record.sound == sound {
                    found = true;
                    if !families
                        .iter()
                        .any(|f: &MuteFamily| f.instrument == *instrument)
                    {
                        families.push(MuteFamily {
                            instrument: *instrument,
                            posted: false,
                            acknowledged: false,
                        });
                    }
                }
            }
            if !found {
                return Err(song_failure("selector is not a certified active family"));
            }
        }
        if families.is_empty() {
            return Err(song_failure("empty mute selector"));
        }
        let clock = self.hosts.audio.song_clock().or(self
            .song
            .clock
            .ok_or_else(|| song_failure("exact song clock pending")))?;
        let frame = future_frame(clock, self.cfg.commit_lead)?;
        self.song.mutes.push(PendingMute {
            failed: false,
            request,
            epoch,
            muted,
            frame,
            application_frame: frame,
            families,
        });
        Ok(())
    }
    /// Queues the complete frozen selector; success is published only after DSP ACKs.
    pub fn mute_instrument(
        &mut self,
        epoch: SnapshotEpoch,
        selector: InstrumentSelector,
        muted: bool,
    ) -> Result<(), Failure> {
        let sounds = selector
            .family()
            .iter()
            .map(FrozenSound::from_sound)
            .collect::<Result<Vec<_>, _>>()?;
        self.request_song_mute(epoch, &sounds, muted, 0)
    }
}

use crate::dsp::build::GraphResourceSite;
use crate::dsp::cells::CellId;
use crate::dsp::graph::{BankRef, EffectKind, GranSrc, InstDef, InstId, TableRef, UGenSpec};
use crate::host::caps::{SampleData, SampleSrc};
use crate::host::wire::Ctl;
use crate::song::snapshot::{FrozenCellInventory, FrozenCellOwner, FrozenGraphOwner};
use std::{rc::Rc, sync::Arc};

pub(super) struct FrozenFamilyCertificate {
    name: crate::value::intern::KwId,
    parameters: Vec<crate::dsp::controls::DeclaredParam>,
    analysis: Vec<(u16, crate::dsp::graph::AnalyzerKind, u32, u32)>,
    graph: InstDef,
    values: Vec<u32>,
    assets: Vec<FrozenAssetBinding>,
}
struct FrozenAssetBinding {
    site: Option<GraphResourceSite>,
    logical_source: Rc<str>,
    members: Vec<Arc<SampleData>>,
    wrapping: bool,
}
pub(super) fn charge(remaining: &mut u32, count: usize) -> Result<(), Failure> {
    let count = u32::try_from(count).map_err(|_| song_failure("controller work overflow"))?;
    *remaining = remaining
        .checked_sub(count)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "song controller work exhausted"))?;
    Ok(())
}
fn normalize_control(
    control: &mut Ctl,
    cells: &FrozenCellInventory,
    ids: &mut Vec<CellId>,
    values: &mut Vec<u32>,
    remaining: &mut u32,
) -> Result<(), Failure> {
    charge(
        remaining,
        ids.len()
            .checked_add(cells.values().len())
            .ok_or_else(|| song_failure("cell work overflow"))?,
    )?;
    if let Ctl::Cell(cell) = control {
        let ordinal = match ids.iter().position(|id| id == cell) {
            Some(ordinal) => ordinal,
            None => {
                let value = cells
                    .value(*cell)
                    .ok_or_else(|| song_failure("missing frozen default"))?;
                charge(remaining, 2)?;
                ids.push(*cell);
                values.push(value.to_bits());
                ids.len() - 1
            }
        };
        *cell =
            CellId::new(u32::try_from(ordinal).map_err(|_| song_failure("cell ordinal overflow"))?);
    }
    Ok(())
}
fn normalize_graph(
    original: &InstDef,
    cells: &FrozenCellInventory,
    remaining: &mut u32,
) -> Result<(InstDef, Vec<u32>), Failure> {
    charge(
        remaining,
        original.params.len()
            + original.nodes.len()
            + original.edges.len()
            + original.node_params.len(),
    )?;
    for node in &original.nodes {
        match node {
            UGenSpec::Effect(effect) => charge(remaining, effect.params.len())?,
            UGenSpec::StageLinked { data: Some(_) } => charge(remaining, 36 * 4)?,
            UGenSpec::FrameKeyframe { data: Some(_) } => charge(remaining, 64 * 5)?,
            _ => {}
        }
    }
    let mut graph = original.clone();
    graph.id = InstId::new(0);
    let mut ids = Vec::new();
    let mut values = Vec::new();
    for (_, ctl) in &mut graph.params {
        normalize_control(ctl, cells, &mut ids, &mut values, remaining)?;
    }
    for (_, _, ctl) in &mut graph.node_params {
        normalize_control(ctl, cells, &mut ids, &mut values, remaining)?;
    }
    for node in &mut graph.nodes {
        match node {
            UGenSpec::SamplePlay(bank) => *bank = BankRef::new(0),
            UGenSpec::Wavetable(table) => *table = TableRef::new(0),
            UGenSpec::Granular(GranSrc::Sample(bank)) => *bank = BankRef::new(0),
            UGenSpec::Granular(GranSrc::Table(table)) => *table = TableRef::new(0),
            UGenSpec::Effect(effect) => {
                for (_, ctl) in &mut effect.params {
                    normalize_control(ctl, cells, &mut ids, &mut values, remaining)?;
                }
            }
            _ => {}
        }
    }
    Ok((graph, values))
}
fn asset_binding(
    prepared: &PreparedSong,
    site: Option<GraphResourceSite>,
    source: &SampleSrc,
    remaining: &mut u32,
) -> Result<Option<FrozenAssetBinding>, Failure> {
    let snapshot = prepared.snapshot();
    let mut members = Vec::new();
    let (logical_source, wrapping) = match source {
        SampleSrc::Bank { kw, .. } => {
            let Some((count, wrapping)) = snapshot.closed_bank_geometry(*kw) else {
                return Ok(None);
            };
            charge(
                remaining,
                usize::try_from(count).map_err(|_| song_failure("bank count overflow"))?,
            )?;
            for index in 0..count {
                charge(
                    remaining,
                    snapshot
                        .resource_count()
                        .checked_add(1)
                        .ok_or_else(|| song_failure("asset work overflow"))?,
                )?;
                members.push(snapshot.closed_sample(&SampleSrc::Bank { kw: *kw, index })?);
            }
            let name = crate::value::intern::name_of_kw(*kw);
            charge(remaining, name.len())?;
            (Rc::<str>::from(name.as_ref()), wrapping)
        }
        SampleSrc::Path(path) => {
            charge(
                remaining,
                snapshot.routing().sources.len() + path.text.len(),
            )?;
            let declaring = if std::path::Path::new(path.text.as_ref()).is_absolute() {
                "".to_owned()
            } else {
                let Some(file) = path.file else {
                    return Ok(None);
                };
                let Some(source) = snapshot.routing().sources.iter().find(|s| s.file == file)
                else {
                    return Ok(None);
                };
                charge(remaining, source.path.len())?;
                source.path.to_string()
            };
            charge(
                remaining,
                snapshot.resource_count() + declaring.len() + path.text.len() + 1,
            )?;
            members.push(snapshot.closed_sample(source)?);
            (
                Rc::<str>::from(format!("{declaring}\0{}", path.text)),
                false,
            )
        }
        SampleSrc::Buffer { .. } => return Ok(None),
    };
    Ok(Some(FrozenAssetBinding {
        site,
        logical_source,
        members,
        wrapping,
    }))
}
pub(super) fn certify_frozen_family(
    prepared: &PreparedSong,
    sound: &FrozenSound,
    id: InstId,
    remaining: &mut u32,
) -> Result<Option<FrozenFamilyCertificate>, Failure> {
    let routing = prepared.snapshot().routing();
    charge(remaining, routing.instruments.len())?;
    let Some(instrument) = routing.instruments.iter().find(|i| i.graph.id == id) else {
        return Ok(None);
    };
    let (mut graph, values) = normalize_graph(&instrument.graph, &routing.cells, remaining)?;
    charge(remaining, routing.resources.entries().len())?;
    let mut assets = Vec::new();
    for binding in routing
        .resources
        .entries()
        .iter()
        .filter(|r| *r.owner() == FrozenGraphOwner::Instrument(id))
    {
        let Some(asset) = asset_binding(
            prepared,
            Some(binding.site().clone()),
            binding.source(),
            remaining,
        )?
        else {
            return Ok(None);
        };
        charge(remaining, 1)?;
        assets.push(asset);
    }
    let event_source = match sound {
        FrozenSound::Sample { path, file } => Some(SampleSrc::Path(crate::value::value::PathVal {
            text: Rc::clone(path),
            file: *file,
        })),
        FrozenSound::Builtin(kw) if prepared.snapshot().closed_bank_geometry(*kw).is_some() => {
            Some(SampleSrc::Bank { kw: *kw, index: 0 })
        }
        FrozenSound::Buffer(_) => return Ok(None),
        _ => None,
    };
    if let Some(source) = event_source {
        let Some(asset) = asset_binding(prepared, None, &source, remaining)? else {
            return Ok(None);
        };
        charge(remaining, 1)?;
        assets.push(asset);
    }
    if assets.is_empty()
        && graph.nodes.iter().any(|n| {
            matches!(
                n,
                UGenSpec::SamplePlay(_)
                    | UGenSpec::Wavetable(_)
                    | UGenSpec::Granular(GranSrc::Sample(_) | GranSrc::Table(_))
            )
        })
    {
        return Ok(None);
    }
    charge(remaining, routing.cells.analysis_ranges().len())?;
    let owner = FrozenCellOwner::Instrument(instrument.name);
    if !normalize_analyzers(&mut graph, instrument.name, &routing.cells, remaining)? {
        return Ok(None);
    }
    let base = routing
        .cells
        .analysis_ranges()
        .iter()
        .filter(|r| r.owner == owner)
        .map(|r| r.logical_start)
        .min()
        .unwrap_or(0);
    let mut analysis = Vec::new();
    for range in routing
        .cells
        .analysis_ranges()
        .iter()
        .filter(|r| r.owner == owner)
    {
        charge(remaining, 1)?;
        analysis.push((
            range.effect,
            range.kind,
            range.logical_start - base,
            range.width,
        ));
    }
    charge(remaining, instrument.parameters.len())?;
    Ok(Some(FrozenFamilyCertificate {
        name: instrument.name,
        parameters: instrument.parameters.clone(),
        graph,
        values,
        assets,
        analysis,
    }))
}
pub(super) fn equivalent_frozen_family(
    old: &FrozenFamilyCertificate,
    next: &FrozenFamilyCertificate,
    remaining: &mut u32,
) -> Result<bool, Failure> {
    charge(
        remaining,
        old.graph.nodes.len()
            + old.graph.edges.len()
            + old.graph.params.len()
            + old.graph.node_params.len()
            + old.values.len()
            + old.assets.len(),
    )?;
    for node in &old.graph.nodes {
        match node {
            UGenSpec::Effect(effect) => charge(remaining, effect.params.len())?,
            UGenSpec::StageLinked { data: Some(_) } => charge(remaining, 36 * 4)?,
            UGenSpec::FrameKeyframe { data: Some(_) } => charge(remaining, 64 * 5)?,
            _ => {}
        }
    }
    charge(remaining, old.parameters.len() + old.analysis.len())?;
    if old.parameters != next.parameters || old.analysis != next.analysis {
        return Ok(false);
    }
    if old.name != next.name
        || old.graph != next.graph
        || old.values != next.values
        || old.assets.len() != next.assets.len()
    {
        return Ok(false);
    }
    for (a, b) in old.assets.iter().zip(&next.assets) {
        charge(
            remaining,
            a.logical_source.len() + b.logical_source.len() + 1,
        )?;
        if a.site != b.site
            || a.logical_source != b.logical_source
            || a.wrapping != b.wrapping
            || a.members.len() != b.members.len()
        {
            return Ok(false);
        }
        for (a, b) in a.members.iter().zip(&b.members) {
            charge(remaining, 1)?;
            if a.rate != b.rate || a.channels != b.channels || a.frames.len() != b.frames.len() {
                return Ok(false);
            }
            if !Arc::ptr_eq(a, b) {
                charge(remaining, a.frames.len())?;
                if a.frames
                    .iter()
                    .zip(&b.frames)
                    .any(|(a, b)| a.to_bits() != b.to_bits())
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}
pub(super) fn build_family_records(
    ready: &SongReadyBundle,
    remaining: &mut u32,
) -> Result<Vec<FamilyRecord>, Failure> {
    let mut out: Vec<FamilyRecord> = Vec::new();
    charge(remaining, ready.routes().branches.len())?;
    for branch in &ready.routes().branches {
        charge(remaining, out.len())?;
        if out.iter().any(|f| {
            f.sound == branch.instrument && f.instrument == branch.resolved_instrument.get()
        }) {
            continue;
        }
        let certificate = certify_frozen_family(
            ready.prepared(),
            &branch.instrument,
            branch.resolved_instrument,
            remaining,
        )?;
        charge(remaining, 1)?;
        out.push(FamilyRecord {
            sound: branch.instrument.clone(),
            instrument: branch.resolved_instrument.get(),
            certificate,
            muted: false,
        });
    }
    Ok(out)
}

fn normalize_analyzers(
    graph: &mut InstDef,
    instrument: crate::value::intern::KwId,
    cells: &FrozenCellInventory,
    remaining: &mut u32,
) -> Result<bool, Failure> {
    let owner = FrozenCellOwner::Instrument(instrument);
    charge(remaining, cells.analysis_ranges().len())?;
    let base = cells
        .analysis_ranges()
        .iter()
        .filter(|r| r.owner == owner)
        .map(|r| r.logical_start)
        .min()
        .unwrap_or(0);
    for (node, spec) in graph.nodes.iter_mut().enumerate() {
        let UGenSpec::Effect(effect) = spec else {
            continue;
        };
        if !matches!(effect.kind, EffectKind::Analyzer(_)) {
            continue;
        }
        let id = crate::dsp::effects::param_ctl(effect.kind, "id")
            .ok_or_else(|| song_failure("analyzer id schema missing"))?;
        charge(
            remaining,
            effect.params.len() + graph.node_params.len() + cells.analysis_ranges().len(),
        )?;
        if effect
            .params
            .iter()
            .any(|(ctl, value)| *ctl == id && matches!(value, Ctl::Cell(_)))
            || graph.node_params.iter().any(|(n, ctl, value)| {
                usize::from(*n) == node && *ctl == id && matches!(value, Ctl::Cell(_))
            })
        {
            return Ok(false);
        }
        let Some(range) = cells
            .analysis_ranges()
            .iter()
            .find(|r| r.owner == owner && usize::from(r.effect) == node)
        else {
            return Ok(false);
        };
        let normalized = range.logical_start - base;
        let value = normalized as f32;
        if f64::from(value) != f64::from(normalized) {
            return Ok(false);
        }
        for (ctl, v) in &mut effect.params {
            if *ctl == id {
                *v = Ctl::Const(value);
            }
        }
        for (n, ctl, v) in &mut graph.node_params {
            if usize::from(*n) == node && *ctl == id {
                *v = Ctl::Const(value);
            }
        }
    }
    Ok(true)
}

impl Runtime {
    pub(super) fn drive_song_mutes(&mut self) {
        for pending in &mut self.song.mutes {
            if pending.failed {
                continue;
            }
            let Some(owner) = self.song.owners.iter_mut().find(|o| {
                o.transport.epoch() == pending.epoch
                    && matches!(
                        o.transport.state(),
                        SongTransportState::Playing | SongTransportState::Draining
                    )
            }) else {
                continue;
            };
            for family in &mut pending.families {
                if !family.posted {
                    let command = SongMute {
                        epoch: pending.epoch,
                        instrument: family.instrument,
                        muted: pending.muted,
                        frame: pending.frame,
                    };
                    match owner.transport.mute(self.hosts.audio.as_mut(), command) {
                        Ok(()) => family.posted = true,
                        Err(refusal) => {
                            if !matches!(refusal.error, SongSubmitError::Backpressure) {
                                pending.failed = true;
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}

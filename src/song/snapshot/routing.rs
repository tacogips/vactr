//! Copied immutable routing DTOs, re-exported at snapshot for compatibility.
use super::{FrozenPattern, FrozenSound};
use crate::value::ratio::Ratio64;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct FrozenInstrument {
    pub name: crate::value::intern::KwId,
    pub graph: std::sync::Arc<crate::dsp::graph::InstDef>,
    pub parameters: Vec<crate::dsp::controls::DeclaredParam>,
    pub defaults: Vec<(crate::dsp::cells::CellId, f32)>,
    pub resource: Option<crate::value::intern::KwId>,
}
#[derive(Clone, Debug)]
pub enum FrozenPartNode {
    Capture(Vec<(crate::value::intern::KwId, FrozenPattern)>),
    Sequence(Vec<(Ratio64, usize)>),
    Repeat {
        child: usize,
        count: u32,
        seed_mode: crate::song::RepeatSeedMode,
    },
    Edit {
        source: usize,
        edit: FrozenEdit,
    },
}
#[derive(Clone, Debug)]
pub enum FrozenEdit {
    Replace {
        track: crate::value::intern::KwId,
        payload: FrozenPattern,
    },
    Transform {
        track: crate::value::intern::KwId,
        family: Vec<FrozenSound>,
        cutoff: crate::song::PartRevision,
        payload: FrozenPattern,
    },
    Delete(crate::song::EventHandle),
    Overwrite {
        track: crate::value::intern::KwId,
        region: crate::pattern::query::TimeSpan,
        payload: FrozenPattern,
    },
    InstrumentFx {
        track: crate::value::intern::KwId,
        family: Vec<FrozenSound>,
        template: crate::value::intern::KwId,
    },
}
#[derive(Clone, Debug)]
pub struct FrozenPart {
    pub revision: crate::song::PartRevision,
    pub duration: Ratio64,
    pub tracks: Vec<crate::value::intern::KwId>,
    pub node: FrozenPartNode,
}
/// Copied immutable route contracts. Host generations/capacity admission are
/// deliberately absent until the routing/installation phases.
#[derive(Clone, Debug, Default)]
pub struct FrozenRoutingInventory {
    pub instruments: Vec<FrozenInstrument>,
    pub buses: Vec<(
        Option<crate::value::intern::KwId>,
        std::sync::Arc<crate::dsp::graph::BusDef>,
    )>,
    pub parts: Vec<FrozenPart>,
    pub root_part: usize,
    pub sources: Vec<FrozenSource>,
    pub cells: super::FrozenCellInventory,
    pub resources: super::FrozenGraphResources,
}
#[derive(Clone, Debug)]
pub struct FrozenSource {
    pub file: crate::reader::span::FileId,
    pub path: Rc<str>,
    pub text: Rc<str>,
}

#[derive(Clone, Debug)]
pub struct FrozenAudioRoute {
    pub instrument: crate::dsp::graph::InstId,
    pub sample: Option<crate::host::caps::SampleSrc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenTrackStage {
    pub track: crate::value::intern::KwId,
    pub template: Option<crate::dsp::graph::BusId>,
}
impl FrozenRoutingInventory {
    /// Every track owns a distinct sum stage. Missing same-name templates mean
    /// neutral processing, never a direct-master routing fallback.
    pub fn track_stage(&self, track: crate::value::intern::KwId) -> FrozenTrackStage {
        FrozenTrackStage {
            track,
            template: self
                .buses
                .iter()
                .find_map(|(name, graph)| (*name == Some(track)).then_some(graph.id)),
        }
    }
}

//! The host that does nothing (design 11.5).
//!
//! `NoopHost` implements every capability: sends are dropped, reads and
//! routes fail `host-unavailable`, `now()` is 0.0 and no input ever arrives.
//! It is the dry-run target and the default source loader (`ns::load`
//! re-exports it).

use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::graph::{InstDef, InstId};
use crate::host::caps::{
    AudioHost, GraphHandle, HostSigs, InstResolver, MidiEvent, MidiHost, MidiInEvent, MidiInHost,
    OscEvent, OscHost, RenderHost, Route, SampleData, SampleLoader, SampleSrc, SignalInput,
};
use crate::host::wire::{AudioEvent, CtlMsg, HostMsg, SlotControl};
use crate::ns::load::SourceLoader;
use crate::reader::span::FileId;
use crate::tex::shader::ShaderDesc;
use crate::tex::texnode::OutId;
use crate::tex::uniforms::Uniforms;
use crate::value::intern::name_of_kw;
use crate::value::value::{PathVal, Sound};
use crate::vm::fail::{FailCode, Failure};

/// No host: every capability is inert.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopHost;

impl SourceLoader for NoopHost {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        Err(Failure::new(
            FailCode::HostUnavailable,
            format!("no host can read `{}`", path.text),
        ))
    }
}

impl AudioHost for NoopHost {
    fn send(&mut self, _: AudioEvent) {}
    fn control(&mut self, _: SlotControl) {}
    fn post(&mut self, _: CtlMsg) {}
    fn drain(&mut self, _: &mut Vec<HostMsg>) {}
    fn now(&self) -> f64 {
        0.0
    }
    fn swap_graph(&mut self, _: GraphHandle) {}
    fn install_sample(&mut self, _: u32, _: Arc<SampleData>) {}
    fn retire_sample(&mut self, _: u32) {}
    fn analysis(&self) -> HostSigs {
        HostSigs::default()
    }
}

impl MidiHost for NoopHost {
    fn send(&mut self, _: MidiEvent) {}
    fn control(&mut self, _: SlotControl) {}
}

impl OscHost for NoopHost {
    fn send(&mut self, _: OscEvent) {}
    fn control(&mut self, _: SlotControl) {}
}

impl RenderHost for NoopHost {
    fn set_program(&mut self, _: OutId, _: ShaderDesc) {}
    fn set_uniforms(&mut self, _: OutId, _: &Uniforms) {}
}

impl MidiInHost for NoopHost {
    fn poll(&mut self) -> &[MidiInEvent] {
        &[]
    }
}

impl InstResolver for NoopHost {
    fn route(&self, _: &Sound) -> Result<Route, Failure> {
        Err(Failure::new(
            FailCode::HostUnavailable,
            "no instrument registry is installed",
        ))
    }
    fn inst(&self, _: InstId) -> Option<Arc<InstDef>> {
        None
    }
    fn signal_inputs(&self) -> Vec<SignalInput> {
        Vec::new()
    }
}

impl SampleLoader for NoopHost {
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let what = match src {
            SampleSrc::Bank { kw, index } => format!(":{} {index}", name_of_kw(*kw)),
            SampleSrc::Path(p) => p.text.to_string(),
        };
        Err(Failure::new(
            FailCode::HostUnavailable,
            format!("no host can load the sample `{what}`"),
        ))
    }
}

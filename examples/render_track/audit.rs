//! Public host proxy auditing install acknowledgments and runtime counters.
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;
use vactr::host::caps::{AudioHost, GraphHandle, HostSigs, SampleData};
use vactr::host::native::{audio::GRAPH_RESOURCE_BASE, NativeAudioHost};
use vactr::host::wire::{AudioEvent, CtlMsg, HostMsg, SlotControl};
use vactr::types::diag::Severity;

pub(super) struct AudioAudit {
    host: NativeAudioHost,
    next_graph: u32,
    pending: BTreeSet<u32>,
    forwarded: Vec<HostMsg>,
    failures: Vec<String>,
    rate: u32,
    pub events: u64,
    pub cutoff: Option<f64>,
}
impl AudioAudit {
    pub fn new(host: NativeAudioHost, rate: u32) -> Self {
        Self {
            host,
            next_graph: GRAPH_RESOURCE_BASE,
            pending: BTreeSet::new(),
            forwarded: Vec::new(),
            failures: Vec::new(),
            rate,
            events: 0,
            cutoff: None,
        }
    }
    fn collect(&mut self) {
        let mut messages = Vec::new();
        self.host.drain(&mut messages);
        for message in &messages {
            match *message {
                HostMsg::Installed { resource, .. } => {
                    self.pending.remove(&resource);
                }
                HostMsg::Counters {
                    late,
                    dropped,
                    stolen,
                    skipped,
                } if late != 0 || dropped != 0 || stolen != 0 || skipped != 0 => self.failures.push(format!(
                    "audio counters: late={late}, dropped={dropped}, stolen={stolen}, skipped={skipped}"
                )),
                _ => {}
            }
        }
        self.forwarded.extend(messages);
        for diagnostic in self.host.take_diagnostics() {
            if diagnostic.severity == Severity::Error {
                self.failures
                    .push(format!("{}: {}", diagnostic.code, diagnostic.message));
            }
        }
    }
    pub fn check(&mut self) -> super::Result<()> {
        self.collect();
        if let Some(failure) = self.failures.first() {
            Err(failure.clone())
        } else {
            Ok(())
        }
    }
    pub fn check_installs(&mut self) -> super::Result<()> {
        self.check()?;
        if self.pending.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "audio engine did not acknowledge graph installs: {:?}",
                self.pending
            ))
        }
    }
    pub fn installs_ready(&mut self) -> super::Result<bool> {
        self.check()?;
        Ok(self.pending.is_empty())
    }
}
pub(super) struct CheckedHost(pub Rc<RefCell<AudioAudit>>);
impl AudioHost for CheckedHost {
    fn send(&mut self, mut event: AudioEvent) {
        let mut audit = self.0.borrow_mut();
        event.time = sample_time(event.time, audit.rate);
        if audit.cutoff.is_some_and(|end| event.time >= end) {
            return;
        }
        audit.events += 1;
        audit.host.send(event);
    }
    fn control(&mut self, control: SlotControl) {
        self.0.borrow_mut().host.control(control);
    }
    fn post(&mut self, message: CtlMsg) {
        self.0.borrow_mut().host.post(message);
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        let mut audit = self.0.borrow_mut();
        audit.collect();
        out.append(&mut audit.forwarded);
    }
    fn now(&self) -> f64 {
        self.0.borrow().host.now()
    }
    fn swap_graph(&mut self, graph: GraphHandle) {
        let mut audit = self.0.borrow_mut();
        let resource = audit.next_graph;
        audit.next_graph += 1;
        audit.pending.insert(resource);
        audit.host.swap_graph(graph);
    }
    fn install_sample(&mut self, resource: u32, data: Arc<SampleData>) {
        self.0.borrow_mut().host.install_sample(resource, data);
    }
    fn retire_sample(&mut self, resource: u32) {
        self.0.borrow_mut().host.retire_sample(resource);
    }
    fn analysis(&self) -> HostSigs {
        self.0.borrow().host.analysis()
    }
}

pub(super) fn sample_time(time: f64, rate: u32) -> f64 {
    let frame = (time * f64::from(rate)).round();
    if frame == 0.0 {
        return 0.0;
    }
    // The DSP rounds event offsets to sample positions. Put the timestamp
    // safely inside that sample's rounding interval: an exact boundary may
    // differ by an ULP between start+n/rate and (start_frame+n)/rate. A quarter
    // sample prevents early admission or a false late counter while rounding
    // to the same intended sample. True past events remain past and faulted.
    (frame + 0.25) / f64::from(rate)
}

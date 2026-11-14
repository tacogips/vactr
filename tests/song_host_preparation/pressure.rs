//! Actual sender pressure, allocation retention and finite physical pooling.
use super::*;
use vactr::dsp::ring::{NativeInstall, NativeSongInstall};
use vactr::host::caps::{GraphHandle, HostSigs, SongCommandRefusal, SongSubmitError};
use vactr::host::wire::{AudioEvent, Ctl, CtlMsg, SlotControl};
use vactr::vm::fail::Failure;

fn bound_browser_pressure() {
    assert_eq!(
        browser_abi::outbox_len(),
        0,
        "must retain queued actual records"
    );
    browser_abi::fix_outbox(4096);
}
fn fill_commands(host: &mut dyn AudioHost) -> usize {
    let command = SongCommand::Mute(SongMute {
        epoch: SnapshotEpoch(9999),
        instrument: 1,
        muted: true,
        frame: 0,
    });
    for count in 0..8192 {
        if let Err(refusal) = host.try_song_command(command) {
            assert!(matches!(refusal.error, SongSubmitError::Backpressure));
            assert_eq!(refusal.command, command);
            return count;
        }
    }
    panic!("actual bounded command transport did not fill");
}
fn upload_pointer(install: &NativeSongInstall) -> usize {
    match &install.payload {
        NativeInstall::Inst { template, .. } => std::ptr::from_ref(template.as_ref()) as usize,
        NativeInstall::Bus { template, .. } => std::ptr::from_ref(template.as_ref()) as usize,
        NativeInstall::Sample { data, .. } => Arc::as_ptr(data) as usize,
    }
}
struct GateHost {
    inner: Box<dyn AudioHost>,
    gate: bool,
    bytes: bool,
    pointers: Rc<RefCell<Vec<usize>>>,
    materializations: Rc<std::cell::Cell<usize>>,
}
impl AudioHost for GateHost {
    fn try_song_command(&mut self, c: SongCommand) -> Result<(), SongCommandRefusal> {
        self.inner.try_song_command(c)
    }
    fn materialize_song_native(
        &self,
        k: SongLeaseKey,
        g: &GraphHandle,
    ) -> Result<Option<NativeSongInstall>, Failure> {
        self.materializations.set(self.materializations.get() + 1);
        self.inner.materialize_song_native(k, g)
    }
    fn submit_song_native(&mut self, install: NativeSongInstall) -> Result<(), NativeSongInstall> {
        if install.lease.kind == SongResourceKind::Instrument {
            self.pointers.borrow_mut().push(upload_pointer(&install));
            if self.gate {
                self.gate = false;
                if self.bytes {
                    bound_browser_pressure();
                }
                assert!(fill_commands(self.inner.as_mut()) > 0);
            }
        }
        self.inner.submit_song_native(install)
    }
    fn try_song_graph(&mut self, k: SongLeaseKey, g: &GraphHandle) -> Result<(), SongSubmitError> {
        if let GraphHandle::Inst { def, .. } = g {
            self.pointers.borrow_mut().push(Arc::as_ptr(def) as usize);
            if self.gate {
                self.gate = false;
                if self.bytes {
                    bound_browser_pressure();
                }
                assert!(fill_commands(self.inner.as_mut()) > 0);
            }
        }
        self.inner.try_song_graph(k, g)
    }
    fn try_song_sample(
        &mut self,
        lease: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), vactr::host::caps::SongSampleRefusal> {
        self.inner.try_song_sample(lease, data)
    }
    fn song_sample_sender_capacity(
        &self,
    ) -> Result<vactr::host::caps::SongSampleSenderCapacity, Failure> {
        self.inner.song_sample_sender_capacity()
    }
    fn submit_song_sample(
        &mut self,
        k: SongLeaseKey,
        d: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        self.inner.submit_song_sample(k, d)
    }
    fn song_clock(&self) -> Result<SongHostClock, Failure> {
        self.inner.song_clock()
    }
    fn poll_msg(&mut self) -> Result<Option<HostMsg>, Failure> {
        self.inner.poll_msg()
    }
    fn send(&mut self, e: AudioEvent) {
        self.inner.send(e)
    }
    fn control(&mut self, c: SlotControl) {
        self.inner.control(c)
    }
    fn post(&mut self, c: CtlMsg) {
        self.inner.post(c)
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        self.inner.drain(out)
    }
    fn now(&self) -> f64 {
        self.inner.now()
    }
    fn swap_graph(&mut self, g: GraphHandle) {
        self.inner.swap_graph(g)
    }
    fn install_sample(&mut self, id: u32, d: Arc<SampleData>) {
        self.inner.install_sample(id, d)
    }
    fn retire_sample(&mut self, id: u32) {
        self.inner.retire_sample(id)
    }
    fn analysis(&self) -> HostSigs {
        self.inner.analysis()
    }
}
#[test]
fn actual_queue_pressure_retains_once_materialized_native_box_and_browser_arc() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let pointers = Rc::new(RefCell::new(Vec::new()));
        let materializations = Rc::new(std::cell::Cell::new(0));
        let inner = std::mem::replace(&mut rig.host, Box::new(vactr::host::noop::NoopHost));
        rig.host = Box::new(GateHost {
            inner,
            gate: true,
            bytes,
            pointers: Rc::clone(&pointers),
            materializations: Rc::clone(&materializations),
        });
        let epoch = SnapshotEpoch(410);
        let mut owner = SongHostPreparation::begin(candidate(PLAIN, epoch), limits(rig.caps))
            .ok()
            .unwrap();
        let ready = loop {
            owner.submit(rig.host.as_mut()).unwrap();
            for msg in rig.tick() {
                if let HostMsg::Song(ack) = msg {
                    if ack.epoch() == epoch {
                        owner.receive(ack).unwrap();
                    } else {
                        assert_eq!(owner.receive(ack), Err(ack));
                    }
                }
            }
            if owner.progress() == SongPreparationProgress::Ready {
                break owner.take_ready().unwrap();
            }
            assert!(rig.frame < 64_000, "bounded pressure retry stalled");
        };
        let addresses = pointers.borrow();
        assert!(addresses.len() >= 2);
        assert_eq!(
            addresses[0], addresses[1],
            "refused upload must retain original allocation"
        );
        let graphs = ready
            .resources()
            .iter()
            .filter(|key| {
                matches!(
                    key.kind,
                    SongResourceKind::Instrument
                        | SongResourceKind::PrivateFx
                        | SongResourceKind::Track
                        | SongResourceKind::Master
                )
            })
            .count();
        assert_eq!(
            materializations.get(),
            graphs,
            "one compile decision per actual uploaded graph"
        );
        drop(addresses);
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup);
        assert!(cleanup.take_cancelled().is_ok());
    }
}
fn start_old_event(rig: &mut Rig, ready: &SongReadyBundle) {
    let pool = ready.pools()[0];
    let mut event = AudioEvent::new(
        0.,
        vactr::sched::slots::SlotId::new(1),
        1,
        vactr::dsp::graph::InstId::new(pool.initial.config.instrument.id),
    );
    event
        .push_ctl(vactr::dsp::ugen::AMP, Ctl::Const(0.2))
        .unwrap();
    event
        .push_ctl(vactr::sched::slots::CtlId::new(49), Ctl::Const(1.))
        .unwrap();
    rig.host
        .try_song_command(SongCommand::Event(SongAudioEvent {
            epoch: ready.prepared().epoch(),
            branch: pool.initial.config.branch,
            generation: 1,
            frame: rig.frame,
            event,
        }))
        .unwrap();
    rig.tick();
    assert!(rig.last.iter().any(|sample| sample.abs() > 0.00001));
}
#[test]
fn refused_exclusive_activation_retains_ready_and_old_active_owner() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut owner =
            SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(411)), limits(rig.caps))
                .ok()
                .unwrap();
        let mut old = rig.complete(&mut owner);
        old.submit_activation(
            rig.host.as_mut(),
            SongActivation {
                epoch: SnapshotEpoch(411),
                frame: rig.frame,
            },
        )
        .unwrap();
        for _ in 0..16 {
            for msg in rig.tick() {
                if let HostMsg::Song(ack) = msg {
                    if ack.epoch() == SnapshotEpoch(411) {
                        old.receive_activation(ack).unwrap();
                    } else {
                        assert_eq!(old.receive_activation(ack), Err(ack));
                    }
                }
            }
            if old.applied_activation().is_some() {
                break;
            }
        }
        assert!(
            old.applied_activation().is_some(),
            "actual Applied receipt never arrived"
        );
        start_old_event(&mut rig, &old);
        let mut owner =
            SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(412)), limits(rig.caps))
                .ok()
                .unwrap();
        let mut next = rig.complete(&mut owner);
        let command = SongActivation {
            epoch: SnapshotEpoch(412),
            frame: rig
                .frame
                .checked_add(8192)
                .expect("bounded activation horizon"),
        };
        if bytes {
            bound_browser_pressure();
        }
        let filled = fill_commands(rig.host.as_mut());
        assert!(filled > 0);
        // Native has 1024 records and 8192 ACK slots; Arena admits 16 records
        // per callback into 128 ACK slots. Its fixed 4096-byte outbox is smaller.
        // Even the slower 16-record relay needs at most 64 pressure callbacks.
        assert!(filled <= vactr::host::native::audio::CONTROL_CAPACITY);
        let refused = next
            .submit_activation(rig.host.as_mut(), command)
            .unwrap_err();
        assert!(matches!(refused.error, SongSubmitError::Backpressure));
        assert_eq!(refused.command, SongCommand::Activate(command));
        assert_eq!(next.prepared().state(), SongPreparationState::Ready);
        assert_eq!(old.prepared().state(), SongPreparationState::Applied);
        assert!(next.applied_activation().is_none());
        let mut rejected = 0;
        for _ in 0..256 {
            for msg in rig.tick() {
                if let HostMsg::Song(ack) = msg {
                    if ack.epoch() == SnapshotEpoch(9999) {
                        assert_eq!(
                            ack,
                            SongHostAck::Rejected {
                                epoch: SnapshotEpoch(9999),
                                reason: SongRejectCode::StaleEpoch
                            }
                        );
                        rejected += 1;
                    }
                    assert_eq!(next.receive_activation(ack), Err(ack));
                }
            }
            assert!(rig.last.iter().any(|sample| sample.abs() > 0.00001));
            if rejected == filled {
                break;
            }
        }
        assert_eq!(
            rejected, filled,
            "every actual queued pressure owner has a receipt"
        );
        assert!(
            rig.frame < command.frame,
            "exact refused command must remain future"
        );
        assert_eq!(next.prepared().state(), SongPreparationState::Ready);
        assert_eq!(old.prepared().state(), SongPreparationState::Applied);
        next.submit_activation(rig.host.as_mut(), command).unwrap();
        for _ in 0..1024 {
            for msg in rig.tick() {
                if let HostMsg::Song(ack) = msg {
                    if ack.epoch() == SnapshotEpoch(412) {
                        next.receive_activation(ack).unwrap();
                    } else {
                        assert_eq!(next.receive_activation(ack), Err(ack));
                    }
                }
            }
            assert!(rig.last.iter().any(|sample| sample.abs() > 0.00001));
            if next.applied_activation().is_some() {
                break;
            }
        }
        let applied = next
            .applied_activation()
            .expect("actual Applied after future deadline");
        assert!(applied.frame >= command.frame);
        for ready in [old, next] {
            let epoch = ready.prepared().epoch();
            let end = rig.frame + 16;
            rig.host
                .try_song_command(SongCommand::Endpoints(SongEndpoints {
                    epoch,
                    arrangement: end,
                    tail_deadline: end,
                }))
                .unwrap();
            let mut cleanup = SongHostPreparation::retire(ready);
            rig.cleanup(&mut cleanup);
            assert!(cleanup.take_retired().is_ok());
        }
    }
}
#[test]
fn cancellation_during_actual_partial_upload_waits_for_all_owned_returns() {
    let mut rig = Rig::new(true);
    let epoch = SnapshotEpoch(413);
    let mut owner = SongHostPreparation::begin(
        candidate_with_pcm(CLOSED, epoch, 2 * browser_messages::SLICE_FLOATS),
        limits(rig.caps),
    )
    .ok()
    .unwrap();
    let mut cancelled = false;
    for _ in 0..2048 {
        let _ = owner.submit(rig.host.as_mut());
        for msg in rig.tick() {
            if let HostMsg::Song(ack) = msg {
                if matches!(ack, SongHostAck::SliceAccepted { .. }) && !cancelled {
                    owner.cancel().unwrap();
                    cancelled = true;
                    assert!(owner.take_cancelled().is_err());
                }
                if let Err(unhandled) = owner.receive(ack) {
                    assert!(matches!(unhandled, SongHostAck::SliceAccepted { .. }));
                }
            }
        }
        if owner.progress() == SongPreparationProgress::Cancelled {
            break;
        }
    }
    assert!(cancelled);
    assert_eq!(owner.progress(), SongPreparationProgress::Cancelled);
    assert!(owner.take_cancelled().is_ok());
    assert_eq!(
        rig.receipts
            .iter()
            .filter(|ack| **ack == SongHostAck::PreparationCancelled(epoch))
            .count(),
        1
    );
    assert!(!rig
        .receipts
        .iter()
        .any(|ack| *ack == SongHostAck::Ready(epoch)));
}
#[test]
fn finite_repeat_pools_do_not_expand_logical_placements() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let text="inst tone:\n\tsin-osc 440\nsong {part-repeat {part [tone: {s :tone}] duration: 1} 1000000} tail-seconds: 0 > play-song";
        let mut owner =
            SongHostPreparation::begin(candidate(text, SnapshotEpoch(414)), limits(rig.caps))
                .ok()
                .unwrap();
        let ready = rig.complete(&mut owner);
        assert!(!ready.routes().branches.is_empty());
        assert_eq!(ready.pools().len(), ready.routes().branches.len());
        for route in &ready.routes().branches {
            assert_eq!(route.occurrences, 1_000_000);
            assert_eq!(route.reserved_generations, 1);
            let pools: Vec<_> = ready
                .pools()
                .iter()
                .filter(|pool| pool.logical == route.id)
                .collect();
            assert_eq!(pools.len(), 1);
            assert_eq!(pools[0].initial.last_generation, 1_000_000);
        }
        assert!(ready.resources().len() < 16);
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup);
    }
}

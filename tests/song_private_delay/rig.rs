//! Genuine Native/Arena graph and PCM ingress; callback memory probe stays armed only while processing.
use super::*;
pub(super) struct Rig {
    pub engine: Engine,
    pub tx: ring::Producer<NativeRecord>,
    controls: ring::Consumer<NativeRecord>,
    pub inbox: ByteInbox,
    pub bytes: bool,
    _event_tx: ring::EventProducer,
    events: ring::EventConsumer,
    pub acks: ring::AckProducer,
    pub received: ring::AckConsumer,
    pub garbage: ring::Producer<Garbage>,
    pub returned: ring::Consumer<Garbage>,
    cells: AtomicCells,
    pub receipts: Vec<HostMsg>,
    pub frame: u64,
    pub caps: CapabilitySet,
    pub baseline: SongHostCapacities,
    pub native_owners: Vec<(SongLeaseKey, usize)>,
    pub returned_owners: Vec<(SongLeaseKey, usize)>,
}
impl Rig {
    pub fn new(bytes: bool, ack_slots: usize, sr: f32, bus_seconds: f32) -> Self {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 4;
        let mut cfg = EngineConfig::new(
            &caps,
            sr,
            64,
            if bytes {
                StoreKind::Arena { bytes: 16384 }
            } else {
                StoreKind::NativeArc
            },
        );
        cfg.bus_seconds = bus_seconds;
        cfg.bus_slots = 8;
        cfg.template_slots = 4;
        cfg.event_capacity = 64;
        cfg.voice_seconds = 0.25;
        let mut engine = Engine::with_config(cfg);
        let (acks, received) = SpscRing::split(ack_slots);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 2,
                    leases: 16,
                    branches: 4,
                    control_slots: 8,
                    analysis_slots: 8,
                    native_pcm_bytes: 16384,
                    critical_receipts: 1,
                },
                &acks,
            )
            .unwrap();
        let baseline = engine.song_remaining_capacities().unwrap();
        let (tx, controls) = SpscRing::split(64);
        let (event_tx, events) = EventRing::split(8);
        let (garbage, returned) = SpscRing::split(1);
        Self {
            engine,
            tx,
            controls,
            inbox: ByteInbox::new(),
            bytes,
            _event_tx: event_tx,
            events,
            acks,
            received,
            garbage,
            returned,
            cells: AtomicCells::new(64),
            receipts: Vec::new(),
            frame: 0,
            caps,
            baseline,
            native_owners: Vec::new(),
            returned_owners: Vec::new(),
        }
    }
    pub fn command(&mut self, command: SongCommand) {
        if self.bytes {
            let mut data = [0; CtlMsg::MAX_LEN];
            let n = CtlMsg::Song(command).encode(&mut data);
            assert!(n > 0 && self.inbox.push(&data[..n]));
        } else {
            self.tx
                .push(NativeRecord::Msg(CtlMsg::Song(command)))
                .ok()
                .unwrap();
        }
    }
    pub fn process(&mut self, frames: usize, drain: bool) -> Vec<f32> {
        let mut output = vec![0.; frames * 2];
        CALLBACK_MEMORY.with(|c| {
            assert!(c.get().is_none());
            c.set(Some((0, 0)));
        });
        if self.bytes {
            self.engine.process(
                &mut EngineIo {
                    events: &mut self.events,
                    controls: &mut self.inbox,
                    acks: &mut self.acks,
                    cells: &mut self.cells,
                    garbage: Some(&mut self.garbage),
                },
                &mut output,
                frames,
            );
        } else {
            self.engine.process(
                &mut EngineIo {
                    events: &mut self.events,
                    controls: &mut self.controls,
                    acks: &mut self.acks,
                    cells: &mut self.cells,
                    garbage: Some(&mut self.garbage),
                },
                &mut output,
                frames,
            );
        }
        let counts = CALLBACK_MEMORY.with(|c| c.replace(None).unwrap());
        assert_eq!(
            counts,
            (0, 0),
            "actual delay callback allocation/destruction"
        );
        self.frame += frames as u64;
        if drain {
            self.drain();
        }
        output
    }
    pub fn drain(&mut self) {
        while let Some(m) = self.received.pop() {
            self.receipts.push(m);
        }
        while let Some(g) = self.returned.pop() {
            if let Garbage::SongInstall(ref install) = g {
                let address = match &install.payload {
                    NativeInstall::Inst { template, .. } => {
                        std::ptr::from_ref(template.as_ref()) as usize
                    }
                    NativeInstall::Bus { template, .. } => {
                        std::ptr::from_ref(template.as_ref()) as usize
                    }
                    NativeInstall::Sample { data, .. } => Arc::as_ptr(data) as usize,
                };
                self.returned_owners.push((install.lease, address));
            }
            drop(g);
        }
    }
    pub fn graph(&mut self, lease: SongLeaseKey, inst: Option<&InstDef>, bus: Option<&BusDef>) {
        if self.bytes {
            let mut graph = Vec::new();
            if let Some(def) = inst {
                encode_inst(def, &mut graph).unwrap();
            } else {
                encode_bus(
                    bus.unwrap(),
                    lease.kind == SongResourceKind::Master,
                    &mut graph,
                )
                .unwrap();
            }
            let mut data = vec![0; graph.len() + 64];
            let n = ring::encode_song_graph_record(lease, &graph, &mut data);
            assert!(n > 0 && self.inbox.push(&data[..n]));
        } else {
            let payload = if let Some(def) = inst {
                NativeInstall::Inst {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    template: Template::from_inst(def, &self.engine.build_env()).unwrap(),
                }
            } else {
                NativeInstall::Bus {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    master: lease.kind == SongResourceKind::Master,
                    template: Box::new(BusTemplate::from_def(bus.unwrap()).unwrap()),
                }
            };
            let address = match &payload {
                NativeInstall::Inst { template, .. } => {
                    std::ptr::from_ref(template.as_ref()) as usize
                }
                NativeInstall::Bus { template, .. } => {
                    std::ptr::from_ref(template.as_ref()) as usize
                }
                NativeInstall::Sample { data, .. } => Arc::as_ptr(data) as usize,
            };
            self.native_owners.push((lease, address));
            self.tx
                .push(NativeRecord::SongInstall(NativeSongInstall {
                    lease,
                    payload,
                }))
                .ok()
                .unwrap();
        }
        for _ in 0..4 {
            assert!(self.process(1, true).iter().all(|v| *v == 0.));
        }
        assert!(self
            .receipts
            .contains(&HostMsg::Song(SongHostAck::ResourceReady {
                epoch: lease.epoch,
                resource: lease.resource
            })));
    }
    pub fn prepared(bytes: bool, private_gain: f32, track_gain: f32, master_gain: f32) -> Self {
        let mut r = Self::new(bytes, 4, 48000., 12.);
        let owners = [
            key(1, SongResourceKind::Instrument),
            key(2, SongResourceKind::Instrument),
            key(3, SongResourceKind::PrivateFx),
            key(4, SongResourceKind::PrivateFx),
            key(5, SongResourceKind::Track),
            key(6, SongResourceKind::Master),
            key(7, SongResourceKind::Sample),
            key(8, SongResourceKind::ControlCells),
        ];
        r.engine
            .begin_song_preparation(SongStagePreparation {
                preparation: SongPreparation {
                    epoch: SnapshotEpoch(71),
                    branches: 2,
                    resources: owners.len() as u32,
                    required: SongHostCapacities {
                        sample_rate: 48000,
                        voice_slots: 2,
                        ..Default::default()
                    },
                },
                analysis_required: SongAnalysisCapacity { slots: 0 },
            })
            .unwrap();
        for owner in owners {
            r.engine
                .reserve_song_resource(SongResourceReservation {
                    epoch: owner.epoch,
                    resource: owner.resource,
                    kind: owner.kind,
                })
                .unwrap();
        }
        r.command(SongCommand::InitCells(SongCellInit {
            lease: owners[7],
            cell: CellId::new(42),
            value: 0.001,
        }));
        r.command(SongCommand::InitCells(SongCellInit {
            lease: owners[7],
            cell: CellId::new(43),
            value: 0.5,
        }));
        for inst in &owners[..2] {
            r.command(SongCommand::BindGraphBanks(SongGraphBanks {
                graph: *inst,
                controls: Some(owners[7]),
                analysis: None,
            }));
        }
        r.process(1, true);
        let mut frames = vec![0.; 512];
        frames[16..20].fill(0.5);
        let pcm = Arc::new(SampleData {
            rate: 48000,
            channels: 1,
            frames: frames.into_boxed_slice(),
        });
        if bytes {
            let mut data = [0; 1024];
            let n = ring::encode_song_sample_begin(owners[6], 512, 1, 48000, &mut data);
            assert!(r.inbox.push(&data[..n]));
            r.process(1, true);
            for (i, slice) in pcm.frames.chunks(128).enumerate() {
                let n = ring::encode_song_slice(owners[6], (i * 128) as u32, slice, &mut data);
                assert!(r.inbox.push(&data[..n]));
                r.process(1, true);
            }
        } else {
            r.native_owners
                .push((owners[6], Arc::as_ptr(&pcm) as usize));
            r.tx.push(NativeRecord::SongInstall(NativeSongInstall {
                lease: owners[6],
                payload: NativeInstall::Sample {
                    resource: 7,
                    gen: 5,
                    data: pcm,
                },
            }))
            .ok()
            .unwrap();
        }
        for _ in 0..4 {
            assert!(r.process(1, true).iter().all(|v| *v == 0.));
        }
        assert!(r
            .receipts
            .contains(&HostMsg::Song(SongHostAck::ResourceReady {
                epoch: owners[6].epoch,
                resource: owners[6].resource
            })));
        for (i, owner) in owners[..2].iter().enumerate() {
            let def = InstDef {
                id: InstId::new(301 + i as u32),
                params: Box::new([]),
                nodes: vec![UGenSpec::SamplePlay(BankRef::new(7))].into_boxed_slice(),
                edges: Box::new([]),
                node_params: Box::new([]),
            };
            r.graph(*owner, Some(&def), None);
        }
        for (owner, gain) in [
            (owners[2], private_gain),
            (owners[3], private_gain),
            (owners[4], track_gain),
            (owners[5], master_gain),
        ] {
            let def = BusDef {
                id: BusId::new(owner.resource.id),
                chain: linear_gain_chain(gain),
            };
            r.graph(owner, None, Some(&def));
        }
        for i in 0..2 {
            r.engine
                .stage_song_branch(SongBranchConfig {
                    epoch: SnapshotEpoch(71),
                    branch: SongBranchId(i),
                    generation: 17 + i,
                    family: i,
                    track: 5,
                    instrument: owners[i as usize].resource,
                    private_fx: Some(owners[2 + i as usize].resource),
                    track_template: Some(owners[4].resource),
                    master: Some(owners[5].resource),
                    transition_frame: 0,
                    tail_deadline: u64::MAX,
                })
                .unwrap();
        }
        r.command(SongCommand::SealPreparation(SnapshotEpoch(71)));
        r.process(1, true);
        assert!(r
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Ready(SnapshotEpoch(71)))));
        r
    }
    pub fn event(&mut self, branch: u32, frame: u64, send: f32, time: Ctl, feedback: Ctl) {
        let mut audio = AudioEvent::new(0., SlotId::new(branch), 1, InstId::new(301 + branch));
        for (id, value) in [
            (38, Ctl::Const(send)),
            (39, time),
            (40, feedback),
            (12, Ctl::Const(0.001)),
        ] {
            audio.push_ctl(CtlId::new(id), value).unwrap();
        }
        self.command(SongCommand::Event(SongAudioEvent {
            epoch: SnapshotEpoch(71),
            branch: SongBranchId(branch),
            generation: 17 + branch,
            frame,
            event: audio,
        }));
    }
    pub fn malformed_event(
        &mut self,
        branch: u32,
        frame: u64,
        send: f32,
        time: f32,
        feedback: f32,
    ) {
        let values = [send, time, feedback];
        let invalid = values
            .iter()
            .position(|v| !v.is_finite())
            .expect("nonfinite fixture");
        let mut audio = AudioEvent::new(0., SlotId::new(branch), 1, InstId::new(301 + branch));
        for (id, value) in [(38, send), (39, time), (40, feedback), (12, 0.001)] {
            audio.push_ctl(CtlId::new(id), Ctl::Const(value)).unwrap();
        }
        let mut event = SongAudioEvent {
            epoch: SnapshotEpoch(71),
            branch: SongBranchId(branch),
            generation: 17 + branch,
            frame,
            event: audio,
        };
        assert!(!SongCommand::Event(event).valid());
        if !self.bytes {
            // Real native ingress intentionally reaches Engine's POD validator.
            self.command(SongCommand::Event(event));
            return;
        }
        let mut record = [0; CtlMsg::MAX_LEN];
        let queued = self.inbox.len();
        assert_eq!(
            CtlMsg::Song(SongCommand::Event(event)).encode(&mut record),
            0
        );
        assert_eq!(
            self.inbox.len(),
            queued,
            "checked encoder never queues nonfinite controls"
        );
        for (_, ctl) in &mut event.event.ctl[..usize::from(event.event.n_ctl)] {
            if matches!(*ctl, Ctl::Const(v) if !v.is_finite()) {
                *ctl = Ctl::Const(0.);
            }
        }
        let n = CtlMsg::Song(SongCommand::Event(event)).encode(&mut record);
        assert!(n >= AudioEvent::ENCODED_LEN);
        let mut encoded_event = [0; AudioEvent::ENCODED_LEN];
        assert_eq!(event.event.encode(&mut encoded_event), encoded_event.len());
        let event_at = n - encoded_event.len();
        assert_eq!(&record[event_at..n], encoded_event.as_slice());
        // Fixed existing AudioEvent codec: each control is id2 + tag1 + word4.
        let header = AudioEvent::ENCODED_LEN - vactr::host::wire::MAX_CTLS * 7;
        let local_word = header + invalid * 7 + 3;
        assert_eq!(
            &encoded_event[local_word..local_word + 4],
            &0f32.to_bits().to_le_bytes()
        );
        record[event_at + local_word..event_at + local_word + 4]
            .copy_from_slice(&values[invalid].to_bits().to_le_bytes());
        assert!(CtlMsg::decode(&record[..n]).is_err());
        assert!(
            self.inbox.push(&record[..n]),
            "real malformed record reaches ByteInbox boundary"
        );
    }
    pub fn activate(&mut self, frame: u64) {
        self.command(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(71),
            frame,
        }));
    }
    pub fn audio(&mut self, frames: usize, partition: usize) -> Vec<f32> {
        let mut out = Vec::new();
        let mut left = frames;
        while left > 0 {
            let n = left.min(partition);
            out.extend(self.process(n, true));
            left -= n;
        }
        out
    }
}

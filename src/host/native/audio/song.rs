//! Song-specific checked admission and upload ownership.
use super::*;

impl NativeAudioHost {
    #[allow(clippy::result_large_err)] // Preserve the original complete command on refusal.
    pub(super) fn try_song_command_checked(
        &mut self,
        command: crate::song::routing::SongCommand,
    ) -> Result<(), crate::host::caps::SongCommandRefusal> {
        use crate::host::caps::{SongCommandRefusal, SongSubmitError};
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut bytes);
        if n == 0 {
            return Err(SongCommandRefusal {
                command,
                error: SongSubmitError::Invalid(crate::vm::fail::Failure::new(
                    crate::vm::fail::FailCode::Type,
                    "invalid song command",
                )),
            });
        }
        self.song_capacity
            .post(&mut self.controls, NativeRecord::Msg(CtlMsg::Song(command)))
            .map_err(|_| SongCommandRefusal {
                command,
                error: SongSubmitError::Backpressure,
            })
    }

    pub(super) fn song_native(
        &mut self,
        install: crate::dsp::ring::NativeSongInstall,
    ) -> Result<(), crate::dsp::ring::NativeSongInstall> {
        if !install.valid() {
            return Err(install);
        }
        match self
            .song_capacity
            .post(&mut self.controls, NativeRecord::SongInstall(install))
        {
            Ok(()) => Ok(()),
            Err(NativeRecord::SongInstall(install)) => Err(install),
            Err(_) => unreachable!("song native enqueue identity"),
        }
    }
    pub(super) fn materialize_song_graph(
        &self,
        lease: crate::song::routing::SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<crate::dsp::ring::NativeSongInstall, crate::vm::fail::Failure> {
        use crate::song::routing::SongResourceKind as Kind;
        use crate::vm::fail::{FailCode, Failure};
        let fail = |message: &str| Failure::new(FailCode::HostUnavailable, message);
        let payload = match graph {
            GraphHandle::Inst { def, .. } if lease.kind == Kind::Instrument => {
                NativeInstall::Inst {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    template: Template::from_inst(def, &self.env).map_err(|e| fail(e.message()))?,
                }
            }
            GraphHandle::Bus { def, .. } if matches!(lease.kind, Kind::PrivateFx | Kind::Track) => {
                NativeInstall::Bus {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    master: false,
                    template: Box::new(BusTemplate::from_def(def).map_err(|e| fail(e.message()))?),
                }
            }
            GraphHandle::Master(def) if lease.kind == Kind::Master => NativeInstall::Bus {
                resource: lease.resource.id,
                gen: lease.resource.generation,
                master: true,
                template: Box::new(BusTemplate::from_def(def).map_err(|e| fail(e.message()))?),
            },
            _ => return Err(fail("song graph lease kind mismatch")),
        };
        let install = crate::dsp::ring::NativeSongInstall { lease, payload };
        if !install.valid() {
            return Err(fail("invalid song graph payload ownership"));
        }
        Ok(install)
    }
    pub(super) fn song_graph(
        &mut self,
        lease: crate::song::routing::SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<(), crate::vm::fail::Failure> {
        let install = self.materialize_song_graph(lease, graph)?;
        self.submit_song_native(install).map_err(|_returned| {
            crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::HostUnavailable,
                "song native control queue full",
            )
        })
    }
    pub(super) fn song_sample_checked(
        &mut self,
        lease: crate::song::routing::SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), crate::host::caps::SongSampleRefusal> {
        use crate::host::caps::{SongSampleRefusal, SongSubmitError};
        if lease.kind != crate::song::routing::SongResourceKind::Sample
            || !(1..=2).contains(&data.channels)
            || !(8000..=192000).contains(&data.rate)
            || data.frames.len() % usize::from(data.channels) != 0
            || data.frames.len().checked_mul(4).is_none()
            || data.frames.iter().any(|value| !value.is_finite())
        {
            return Err(SongSampleRefusal {
                lease,
                data,
                error: SongSubmitError::Invalid(crate::vm::fail::Failure::new(
                    crate::vm::fail::FailCode::Type,
                    "invalid native song sample geometry",
                )),
            });
        }
        let install = crate::dsp::ring::NativeSongInstall {
            lease,
            payload: NativeInstall::Sample {
                resource: lease.resource.id,
                gen: lease.resource.generation,
                data,
            },
        };
        match self.submit_song_native(install) {
            Ok(()) => Ok(()),
            Err(crate::dsp::ring::NativeSongInstall {
                payload: NativeInstall::Sample { data, .. },
                ..
            }) => Err(SongSampleRefusal {
                lease,
                data,
                error: SongSubmitError::Backpressure,
            }),
            Err(_) => unreachable!("song sample enqueue identity"),
        }
    }
    pub(super) fn song_sample(
        &mut self,
        lease: crate::song::routing::SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        self.song_sample_checked(lease, data)
            .map_err(|refusal| refusal.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{cells::CellId, graph::InstId};
    use crate::host::caps::SongSubmitError;
    use crate::sched::slots::SlotId;
    use crate::song::{routing::*, SnapshotEpoch};
    fn lease(kind: SongResourceKind) -> SongLeaseKey {
        SongLeaseKey {
            epoch: SnapshotEpoch(u64::MAX),
            resource: SongResourceRef {
                id: u32::MAX,
                generation: u32::MAX,
            },
            kind,
        }
    }
    fn commands() -> Vec<SongCommand> {
        let epoch = SnapshotEpoch(u64::MAX);
        let resource = lease(SongResourceKind::Instrument).resource;
        let mut event = AudioEvent::new(
            0.125,
            SlotId::new(u32::MAX),
            u32::MAX,
            InstId::new(u32::MAX),
        );
        event
            .push_ctl(
                crate::sched::slots::CtlId::new(u16::MAX),
                crate::host::wire::Ctl::Const(-0.0),
            )
            .unwrap();
        event
            .push_ctl(
                crate::sched::slots::CtlId::new(1),
                crate::host::wire::Ctl::Cell(CellId::new(u32::MAX)),
            )
            .unwrap();
        let preparation = SongPreparation {
            epoch,
            branches: 1,
            resources: 1,
            required: SongHostCapacities {
                sample_rate: 48000,
                ..SongHostCapacities::default()
            },
        };
        vec![
            SongCommand::RequestCapacity(epoch),
            SongCommand::BeginStaging(SongStagePreparation {
                preparation,
                analysis_required: SongAnalysisCapacity { slots: 1 },
            }),
            SongCommand::InitCells(SongCellInit {
                lease: lease(SongResourceKind::ControlCells),
                cell: CellId::new(0),
                value: 0.25,
            }),
            SongCommand::ReserveAnalysis(SongAnalysisReservation {
                lease: lease(SongResourceKind::AnalysisBank),
                slots: 1,
            }),
            SongCommand::CancelPreparation(epoch),
            SongCommand::CancelLease(lease(SongResourceKind::Sample)),
            SongCommand::BeginPreparation(preparation),
            SongCommand::ReserveResource(SongResourceReservation {
                epoch,
                resource,
                kind: SongResourceKind::Instrument,
            }),
            SongCommand::ConfigureBranch(SongBranchConfig {
                epoch,
                branch: SongBranchId(2),
                generation: 3,
                family: 4,
                track: 5,
                instrument: resource,
                private_fx: Some(resource),
                track_template: None,
                master: Some(resource),
                transition_frame: u64::MAX - 1,
                tail_deadline: u64::MAX,
            }),
            SongCommand::SealPreparation(epoch),
            SongCommand::Prepare(epoch),
            SongCommand::Activate(SongActivation {
                epoch,
                frame: u64::MAX,
            }),
            SongCommand::Mute(SongMute {
                epoch,
                instrument: 5,
                muted: true,
                frame: u64::MAX,
            }),
            SongCommand::Endpoints(SongEndpoints {
                epoch,
                arrangement: u64::MAX - 1,
                tail_deadline: u64::MAX,
            }),
            SongCommand::Release(SongBranchRelease {
                epoch,
                branch: SongBranchId(2),
                generation: 3,
                frame: u64::MAX - 1,
                tail_deadline: u64::MAX,
            }),
            SongCommand::Event(SongAudioEvent {
                epoch,
                branch: SongBranchId(2),
                generation: 3,
                frame: u64::MAX,
                event,
            }),
            SongCommand::BindGraphBanks(SongGraphBanks {
                graph: lease(SongResourceKind::Instrument),
                controls: Some(lease(SongResourceKind::ControlCells)),
                analysis: Some(lease(SongResourceKind::AnalysisBank)),
            }),
        ]
    }

    #[test]
    fn native_all_commands_refuse_and_retry_without_duplicate_records() {
        let (mut host, mut side) = NativeAudioHost::headless(48000, CapabilitySet::browser(), 1);
        while side.controls.pop().is_some() {}
        let all = commands();
        assert_eq!(all.len(), 17);
        for command in all {
            let mut count = 0;
            while host
                .controls
                .push(NativeRecord::Msg(CtlMsg::Song(SongCommand::Prepare(
                    SnapshotEpoch(17),
                ))))
                .is_ok()
            {
                count += 1;
            }
            assert!(count > 0);
            let dropped = host.dropped();
            let refusal = host.try_song_command(command).unwrap_err();
            assert_eq!(refusal.command, command);
            assert!(matches!(refusal.error, SongSubmitError::Backpressure));
            assert_eq!(host.dropped(), dropped);
            let second = host.try_song_command(refusal.command).unwrap_err();
            assert!(matches!(second.error, SongSubmitError::Backpressure));
            let mut expected = [0; CtlMsg::MAX_LEN];
            let n = CtlMsg::Song(command).encode(&mut expected);
            for refused in [refusal.command, second.command] {
                let mut actual = [0; CtlMsg::MAX_LEN];
                let len = CtlMsg::Song(refused).encode(&mut actual);
                assert_eq!(&actual[..len], &expected[..n]);
            }
            assert_eq!(host.dropped(), dropped);
            assert_eq!(side.controls.len(), count);
            for _ in 0..count {
                assert!(matches!(
                    side.controls.pop(),
                    Some(NativeRecord::Msg(CtlMsg::Song(SongCommand::Prepare(
                        SnapshotEpoch(17)
                    ))))
                ));
            }
            assert!(side.controls.pop().is_none());
            host.try_song_command(refusal.command).unwrap();
            match side.controls.pop() {
                Some(NativeRecord::Msg(message)) => {
                    let mut expected = [0; CtlMsg::MAX_LEN];
                    let mut actual = [0; CtlMsg::MAX_LEN];
                    let n = CtlMsg::Song(command).encode(&mut expected);
                    let len = message.encode(&mut actual);
                    assert_eq!(&actual[..len], &expected[..n]);
                }
                _ => panic!("complete command"),
            }
            assert!(side.controls.pop().is_none());
        }
        let invalid = SongCommand::Endpoints(SongEndpoints {
            epoch: SnapshotEpoch(1),
            arrangement: 2,
            tail_deadline: 1,
        });
        assert!(matches!(
            host.try_song_command(invalid).unwrap_err().error,
            SongSubmitError::Invalid(_)
        ));
        assert!(side.controls.pop().is_none());
    }
    #[test]
    fn native_refused_sample_preserves_original_allocation() {
        let (mut host, mut side) = NativeAudioHost::headless(48000, CapabilitySet::browser(), 1);
        while side.controls.pop().is_some() {}
        while host
            .controls
            .push(NativeRecord::Msg(CtlMsg::Song(SongCommand::Prepare(
                SnapshotEpoch(17),
            ))))
            .is_ok()
        {}
        let key = lease(SongResourceKind::Sample);
        let data = Arc::new(SampleData {
            rate: 48000,
            channels: 1,
            frames: vec![0.5; 8].into_boxed_slice(),
        });
        let pointer = Arc::as_ptr(&data);
        let returned = host.submit_song_sample(key, data).unwrap_err();
        assert_eq!(Arc::as_ptr(&returned), pointer);
        while side.controls.pop().is_some() {}
        host.submit_song_sample(key, returned).unwrap();
        match side.controls.pop().unwrap() {
            NativeRecord::SongInstall(crate::dsp::ring::NativeSongInstall {
                lease,
                payload: NativeInstall::Sample { data, .. },
            }) => {
                assert_eq!(lease, key);
                assert_eq!(Arc::as_ptr(&data), pointer);
            }
            _ => panic!("sample ownership"),
        }
        assert!(side.controls.pop().is_none());
    }
    #[test]
    fn native_refused_box_preserves_original_allocation() {
        let (mut host, mut side) = NativeAudioHost::headless(48000, CapabilitySet::browser(), 1);
        while side.controls.pop().is_some() {}
        while host
            .controls
            .push(NativeRecord::Msg(CtlMsg::Song(SongCommand::Prepare(
                SnapshotEpoch(17),
            ))))
            .is_ok()
        {}
        let key = lease(SongResourceKind::PrivateFx);
        let template = Box::new(BusTemplate::new());
        let pointer = std::ptr::from_ref(template.as_ref());
        let install = crate::dsp::ring::NativeSongInstall {
            lease: key,
            payload: NativeInstall::Bus {
                resource: key.resource.id,
                gen: key.resource.generation,
                master: false,
                template,
            },
        };
        let returned = host.submit_song_native(install).unwrap_err();
        match &returned.payload {
            NativeInstall::Bus { template, .. } => {
                assert_eq!(std::ptr::from_ref(template.as_ref()), pointer)
            }
            _ => panic!("box identity"),
        }
        assert_eq!(returned.lease, key);
        while side.controls.pop().is_some() {}
        assert!(host.submit_song_native(returned).is_ok());
        match side.controls.pop().unwrap() {
            NativeRecord::SongInstall(install) => {
                assert_eq!(install.lease, key);
                match install.payload {
                    NativeInstall::Bus { template, .. } => {
                        assert_eq!(std::ptr::from_ref(template.as_ref()), pointer)
                    }
                    _ => panic!("box identity"),
                }
            }
            _ => panic!("install"),
        }
        assert!(side.controls.pop().is_none());
    }
    #[test]
    fn native_refused_instrument_box_preserves_original_allocation() {
        use crate::dsp::graph::{InstDef, UGenSpec};
        let (mut host, mut side) = NativeAudioHost::headless(48000, CapabilitySet::browser(), 1);
        while side.controls.pop().is_some() {}
        while host
            .controls
            .push(NativeRecord::Msg(CtlMsg::Song(SongCommand::Prepare(
                SnapshotEpoch(17),
            ))))
            .is_ok()
        {}
        let key = lease(SongResourceKind::Instrument);
        let def = InstDef {
            id: InstId::new(301),
            params: Vec::new().into_boxed_slice(),
            nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
            edges: Vec::new().into_boxed_slice(),
            node_params: Vec::new().into_boxed_slice(),
        };
        let template = Template::from_inst(&def, &host.env).unwrap();
        let pointer = std::ptr::from_ref(template.as_ref());
        let install = crate::dsp::ring::NativeSongInstall {
            lease: key,
            payload: NativeInstall::Inst {
                resource: key.resource.id,
                gen: key.resource.generation,
                template,
            },
        };
        assert!(install.valid());
        let returned = host.submit_song_native(install).unwrap_err();
        assert_eq!(returned.lease, key);
        match &returned.payload {
            NativeInstall::Inst {
                resource,
                gen,
                template,
            } => {
                assert_eq!(
                    (*resource, *gen),
                    (key.resource.id, key.resource.generation)
                );
                assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
            }
            _ => panic!("instrument box identity"),
        }
        while side.controls.pop().is_some() {}
        assert!(host.submit_song_native(returned).is_ok());
        match side.controls.pop().unwrap() {
            NativeRecord::SongInstall(install) => {
                assert_eq!(install.lease, key);
                match install.payload {
                    NativeInstall::Inst {
                        resource,
                        gen,
                        template,
                    } => {
                        assert_eq!((resource, gen), (key.resource.id, key.resource.generation));
                        assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
                    }
                    _ => panic!("instrument box identity"),
                }
            }
            _ => panic!("instrument install"),
        }
        assert!(side.controls.pop().is_none());
    }

    fn materialization_config(memory_seconds: f32) -> EngineConfig {
        let mut caps = CapabilitySet::native();
        caps.max_voices = 1;
        let mut cfg = EngineConfig::new(&caps, 32_768., 16, StoreKind::NativeArc);
        cfg.voice_seconds = memory_seconds;
        cfg.template_slots = 3;
        cfg.bus_slots = 3;
        cfg.orbits = 1;
        cfg.orbit_delay_seconds = 0.01;
        cfg
    }
    fn materialization_graph() -> GraphHandle {
        use crate::dsp::graph::{InstDef, UGenSpec};
        GraphHandle::Inst {
            id: InstId::new(7),
            def: Arc::new(InstDef {
                id: InstId::new(7),
                params: Box::new([]),
                nodes: vec![UGenSpec::SpectrumPair, UGenSpec::SpectrumPair].into_boxed_slice(),
                edges: Box::new([]),
                node_params: Box::new([]),
            }),
        }
    }
    fn materialized_pointer(install: &crate::dsp::ring::NativeSongInstall) -> *const () {
        match &install.payload {
            NativeInstall::Inst { template, .. } => std::ptr::from_ref(template.as_ref()).cast(),
            NativeInstall::Bus { template, .. } => std::ptr::from_ref(template.as_ref()).cast(),
            NativeInstall::Sample { .. } => panic!("graph materialization"),
        }
    }
    fn render_materialization(
        side: &mut AudioSide,
        host: &mut NativeAudioHost,
    ) -> Vec<SongHostAck> {
        side.render(&mut [0.; 32], 2);
        let acks = &mut host.acks;
        let capacity = &mut host.song_capacity;
        std::iter::from_fn(|| acks.pop())
            .filter_map(|message| capacity.observe(message))
            .filter_map(|m| {
                if let HostMsg::Song(ack) = m {
                    Some(ack)
                } else {
                    None
                }
            })
            .collect()
    }
    #[test]
    fn native_materialization_uses_actual_allocated_voice_memory() {
        let graph = materialization_graph();
        for (seconds, physical, fits) in [(96. / 32_768., 96, true), (95. / 32_768., 95, false)] {
            let (host, side) =
                NativeAudioHost::pair(materialization_config(seconds), AtomicCells::new(8));
            assert_eq!(side.engine.build_env().sr, 32_768.);
            assert_eq!(side.engine.build_env().voice_mem, physical);
            let result = host.materialize_song_native(lease(SongResourceKind::Instrument), &graph);
            if fits {
                let install = result.unwrap().unwrap();
                assert_eq!(install.lease, lease(SongResourceKind::Instrument));
                match install.payload {
                    NativeInstall::Inst { template, .. } => {
                        assert_eq!(template.mem_total, physical)
                    }
                    _ => panic!("actual instrument compilation"),
                }
            } else {
                assert!(result.is_err(), "actual one-float-short engine must refuse");
            }
        }
    }
    #[test]
    fn materialization_rejects_wrong_kind_without_enqueue_or_fallback() {
        let (mut host, mut side) =
            NativeAudioHost::pair(materialization_config(96. / 32_768.), AtomicCells::new(8));
        // Clear only the constructor's genuine internal observation before the purity check.
        render_materialization(&mut side, &mut host);
        for kind in [
            SongResourceKind::Sample,
            SongResourceKind::Track,
            SongResourceKind::Master,
            SongResourceKind::ControlCells,
            SongResourceKind::AnalysisBank,
        ] {
            assert!(host
                .materialize_song_native(lease(kind), &materialization_graph())
                .is_err());
        }
        let mut bad = materialization_graph();
        if let GraphHandle::Inst { def, .. } = &mut bad {
            Arc::make_mut(def).edges = vec![crate::dsp::graph::Edge {
                from: 9,
                to: 0,
                port: 0,
                output: 0,
            }]
            .into_boxed_slice();
        }
        assert!(host
            .materialize_song_native(lease(SongResourceKind::Instrument), &bad)
            .is_err());
        assert!(side.controls.pop().is_none());
        assert!(host.acks.pop().is_none());
        assert!(host
            .materialize_song_native(
                lease(SongResourceKind::Instrument),
                &materialization_graph()
            )
            .unwrap()
            .is_some());
        assert!(side.controls.pop().is_none());
        let def = Arc::new(crate::dsp::graph::BusDef {
            id: crate::dsp::graph::BusId::new(9),
            chain: Box::new([]),
        });
        for (graph, kind, master) in [
            (
                GraphHandle::Bus {
                    id: def.id,
                    def: Arc::clone(&def),
                },
                SongResourceKind::PrivateFx,
                false,
            ),
            (
                GraphHandle::Bus {
                    id: def.id,
                    def: Arc::clone(&def),
                },
                SongResourceKind::Track,
                false,
            ),
            (
                GraphHandle::Master(Arc::clone(&def)),
                SongResourceKind::Master,
                true,
            ),
        ] {
            let install = host
                .materialize_song_native(lease(kind), &graph)
                .unwrap()
                .unwrap();
            assert_eq!(install.lease, lease(kind));
            match install.payload {
                NativeInstall::Bus {
                    resource,
                    gen,
                    master: actual,
                    template,
                } => {
                    assert_eq!((resource, gen, actual), (u32::MAX, u32::MAX, master));
                    assert_eq!(template.bus, def.id);
                }
                _ => panic!("complete graph option payload"),
            }
            assert!(side.controls.pop().is_none());
        }
    }
    #[test]
    fn native_materialization_preserves_instrument_and_bus_boxes_under_pressure() {
        use crate::dsp::graph::{BusDef, BusId};
        let (mut host, mut side) =
            NativeAudioHost::pair(materialization_config(96. / 32_768.), AtomicCells::new(8));
        render_materialization(&mut side, &mut host);
        let graphs = [
            materialization_graph(),
            GraphHandle::Bus {
                id: BusId::new(11),
                def: Arc::new(BusDef {
                    id: BusId::new(11),
                    chain: Box::new([]),
                }),
            },
        ];
        for (index, graph) in graphs.iter().enumerate() {
            let kind = if index == 0 {
                SongResourceKind::Instrument
            } else {
                SongResourceKind::PrivateFx
            };
            let key = SongLeaseKey {
                epoch: SnapshotEpoch(u64::MAX - u64::try_from(index).unwrap()),
                ..lease(kind)
            };
            host.try_song_command(SongCommand::BeginStaging(SongStagePreparation {
                preparation: SongPreparation {
                    epoch: key.epoch,
                    branches: 0,
                    resources: 1,
                    required: SongHostCapacities {
                        sample_rate: 32_768,
                        ..SongHostCapacities::default()
                    },
                },
                analysis_required: SongAnalysisCapacity { slots: 0 },
            }))
            .unwrap();
            host.try_song_command(SongCommand::ReserveResource(SongResourceReservation {
                epoch: key.epoch,
                resource: key.resource,
                kind: key.kind,
            }))
            .unwrap();
            render_materialization(&mut side, &mut host);
            let mut retained = host.materialize_song_native(key, graph).unwrap().unwrap();
            let pointer = materialized_pointer(&retained);
            while host
                .controls
                .push(NativeRecord::Msg(CtlMsg::Song(SongCommand::RequestClock(
                    SongClockRequest {
                        epoch: key.epoch,
                        request: 1,
                    },
                ))))
                .is_ok()
            {}
            for _ in 0..2 {
                retained = match host.submit_song_native(retained) {
                    Err(owner) => owner,
                    Ok(()) => panic!("full real queue"),
                };
                assert_eq!(retained.lease, key);
                assert_eq!(materialized_pointer(&retained), pointer);
            }
            // Drain actual queued clock requests via the callback, including critical ACK pressure.
            for _ in 0..CONTROL_CAPACITY {
                render_materialization(&mut side, &mut host);
                if side.controls.is_empty() {
                    break;
                }
            }
            assert!(side.controls.is_empty());
            for _ in 0..4 {
                render_materialization(&mut side, &mut host);
            }
            assert!(host.submit_song_native(retained).is_ok());
            let mut receipts = Vec::new();
            for _ in 0..8 {
                receipts.extend(render_materialization(&mut side, &mut host));
            }
            assert_eq!(
                receipts
                    .iter()
                    .filter(|ack| **ack
                        == SongHostAck::ResourceReady {
                            epoch: key.epoch,
                            resource: key.resource
                        })
                    .count(),
                1
            );
            host.try_song_command(SongCommand::SealPreparation(key.epoch))
                .unwrap();
            assert!(render_materialization(&mut side, &mut host)
                .contains(&SongHostAck::Ready(key.epoch)));
            host.try_song_command(SongCommand::CancelPreparation(key.epoch))
                .unwrap();
            for _ in 0..8 {
                receipts.extend(render_materialization(&mut side, &mut host));
            }
            let returned = host.garbage.pop().expect("actual off-thread owned garbage");
            match returned {
                Garbage::SongInstall(owner) => {
                    assert_eq!(owner.lease, key);
                    assert_eq!(materialized_pointer(&owner), pointer);
                    drop(owner); // This test/control thread owns destruction.
                }
                _ => panic!("exact song upload return"),
            }
            assert!(host.garbage.pop().is_none());
            assert_eq!(
                receipts
                    .iter()
                    .filter(|ack| **ack == SongHostAck::LeaseReturned(key))
                    .count(),
                1
            );
        }
    }
    #[test]
    fn native_materialization_clock_rejects_actual_fractional_rate() {
        let mut cfg = materialization_config(0.1);
        cfg.sample_rate = 32_768.5;
        let (mut host, mut side) = NativeAudioHost::pair(cfg, AtomicCells::new(8));
        assert_eq!(side.engine.build_env().sr, 32_768.5);
        assert_eq!(host.now(), 0.);
        assert!(host.song_clock().is_err());
        render_materialization(&mut side, &mut host); // Constructor capacity request is a separate observation.
        assert!(side.controls.is_empty());
        assert!(host.acks.pop().is_none());
        let prior = host.clock.frames();
        assert!(host.song_clock().is_err());
        assert_eq!(host.clock.frames(), prior);
        assert!(side.controls.is_empty());
        let request = SongClockRequest {
            epoch: SnapshotEpoch(u64::MAX),
            request: u64::MAX,
        };
        host.try_song_command(SongCommand::RequestClock(request))
            .unwrap();
        assert_eq!(
            render_materialization(&mut side, &mut host),
            vec![SongHostAck::ClockRejected(SongClockFailure {
                request,
                reason: SongRejectCode::Malformed,
            })]
        );
        assert_eq!(host.now(), 32. / 32_768.);
        let (host, side) = NativeAudioHost::pair(materialization_config(0.1), AtomicCells::new(8));
        assert_eq!(side.engine.build_env().sr, 32_768.);
        host.clock.advance(u64::MAX);
        assert_eq!(
            host.song_clock().unwrap(),
            SongHostClock {
                frame: u64::MAX,
                sample_rate: 32_768
            }
        );
    }
}

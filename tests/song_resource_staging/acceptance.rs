//! Physical acceptance fixtures: no metadata-only proof of graph fit.
use super::*;

#[test]
fn physical_slot_and_pcm_fit_rejects_one_short_without_owner_loss() {
    use vactr::dsp::graph::{InstDef, InstId, UGenSpec};
    use vactr::dsp::ugen::Template;
    let mut rig = Rig::new(StoreKind::NativeArc);
    let graph = key(17, 10, SongResourceKind::Track);
    let inst = key(17, 11, SongResourceKind::Instrument);
    let pcm = key(17, 12, SongResourceKind::Sample);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, 17, 3))
        .unwrap();
    for lease in [graph, inst, pcm] {
        reserve(&mut rig.engine, lease);
    }
    let regions = rig.engine.song_bus_regions();
    let largest = regions.iter().map(|r| r.frames).max().unwrap();
    let aggregate: u64 = regions.iter().map(|r| r.frames).sum();
    let env = rig.engine.build_env();
    let chain = [EffectKind::Delay; 3];
    let chain_frames = chain
        .iter()
        .try_fold(0_u64, |total, &kind| {
            total.checked_add(
                u64::try_from(vactr::dsp::effects::mem_len(kind, env.sr, &env.caps)).unwrap(),
            )
        })
        .unwrap();
    let room = u64::try_from(vactr::dsp::effects::mem_len(
        EffectKind::Room,
        env.sr,
        &env.caps,
    ))
    .unwrap()
    .min(largest / 4);
    let need = chain_frames.checked_add(room).unwrap();
    assert!(
        need > largest && need <= aggregate,
        "real slot={largest}, aggregate={aggregate}, need={need}"
    );
    let mut bus = Box::new(BusTemplate::new());
    for kind in chain {
        bus.push(kind).unwrap();
    }
    let pointer = std::ptr::from_ref(bus.as_ref());
    let before = rig.engine.song_remaining_capacities().unwrap();
    let returned = rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: graph,
            payload: NativeInstall::Bus {
                resource: graph.resource.id,
                gen: graph.resource.generation,
                master: false,
                template: bus,
            },
        })
        .expect_err("aggregate memory cannot replace one contiguous bus region");
    let NativeInstall::Bus { template, .. } = returned.payload else {
        panic!("original bus")
    };
    assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
    assert_eq!(rig.engine.song_remaining_capacities().unwrap(), before);
    assert!(rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: graph,
            payload: NativeInstall::Bus {
                resource: graph.resource.id,
                gen: graph.resource.generation,
                master: false,
                template: Box::new(BusTemplate::new()),
            }
        })
        .is_ok());
    let def = InstDef {
        id: InstId::new(100),
        params: Box::new([]),
        nodes: vec![UGenSpec::Delay].into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    };
    let available = usize::try_from(
        rig.engine
            .song_voice_regions()
            .iter()
            .map(|r| r.frames)
            .max()
            .unwrap(),
    )
    .unwrap();
    let mut bigger = env;
    bigger.voice_mem = available + 1;
    let template = Template::from_inst(&def, &bigger).unwrap();
    assert_eq!(template.mem_total, available + 1);
    let pointer = std::ptr::from_ref(template.as_ref());
    let returned = rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: inst,
            payload: NativeInstall::Inst {
                resource: inst.resource.id,
                gen: inst.resource.generation,
                template,
            },
        })
        .expect_err("compiled voice exceeds every real voice region by one");
    let NativeInstall::Inst { template, .. } = returned.payload else {
        panic!("original instrument")
    };
    assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
    bigger.voice_mem = available;
    let fitting = Template::from_inst(&def, &bigger).unwrap();
    assert_eq!(fitting.mem_total, available);
    assert!(rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: inst,
            payload: NativeInstall::Inst {
                resource: inst.resource.id,
                gen: inst.resource.generation,
                template: fitting,
            }
        })
        .is_ok());
    let free = rig.engine.song_remaining_capacities().unwrap().pcm_bytes;
    let exact = usize::try_from(free / 4).unwrap();
    let too_large = sample(exact + 1);
    let retained = Arc::clone(&too_large);
    let returned = rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: pcm,
            payload: NativeInstall::Sample {
                resource: pcm.resource.id,
                gen: pcm.resource.generation,
                data: too_large,
            },
        })
        .expect_err("actual NativeArc PCM cap plus one sample rejects");
    let NativeInstall::Sample { data, .. } = returned.payload else {
        panic!("original PCM")
    };
    assert!(Arc::ptr_eq(&data, &retained));
    assert_eq!(
        rig.engine.song_remaining_capacities().unwrap().pcm_bytes,
        free
    );
    assert!(rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: pcm,
            payload: NativeInstall::Sample {
                resource: pcm.resource.id,
                gen: pcm.resource.generation,
                data: sample(exact),
            }
        })
        .is_ok());
    assert_eq!(rig.engine.song_remaining_capacities().unwrap().pcm_bytes, 0);
    assert!(rig.engine.store().get(pcm.resource.id).is_none());
}

#[test]
fn fragmented_pcm_rejects_aggregate_fit_and_preserves_extents() {
    let mut store = SampleStore::new(StoreKind::Arena { bytes: 64 });
    store.begin(1, 1, 4, 1, 48_000).unwrap();
    store.begin(2, 1, 4, 1, 48_000).unwrap();
    assert!(store.retire(1));
    store.release(1).unwrap();
    let before = store.song_free_extents().to_vec();
    let total: u64 = before.iter().map(|r| r.frames).sum();
    let largest = before.iter().map(|r| r.frames).max().unwrap();
    assert_eq!((largest, total), (8, 12));
    let lease = key(19, 3, SongResourceKind::Sample);
    let fault = store.song_reserve(lease, request(lease, 9)).unwrap_err();
    assert_eq!(fault.code, FaultCode::ArenaExhausted);
    assert_eq!(fault.resource, lease.resource.id);
    assert_eq!(store.song_free_extents(), before);
    store.song_reserve(lease, request(lease, 8)).unwrap();
    assert_eq!(
        store
            .song_free_extents()
            .iter()
            .map(|r| r.frames)
            .sum::<u64>(),
        4
    );
    store.song_cancel(lease).unwrap();
    assert_eq!(store.song_free_extents(), before);
}

pub(super) fn closure_inst(pcm: u32, logical: CellId) -> vactr::dsp::graph::InstDef {
    use vactr::dsp::graph::{BankRef, Edge, EffectSpec, InstDef, InstId, TableRef, UGenSpec};
    use vactr::dsp::ugen::{catalog, Node, AMP, BANK};
    let fx = |kind, name, value| {
        UGenSpec::Effect(EffectSpec {
            kind,
            params: vec![(vactr::dsp::effects::param_ctl(kind, name).unwrap(), value)]
                .into_boxed_slice(),
        })
    };
    InstDef {
        id: InstId::new(301),
        params: vec![(AMP, Ctl::Cell(logical)), (BANK, Ctl::Const(pcm as f32))].into_boxed_slice(),
        nodes: vec![
            UGenSpec::SinOsc,
            UGenSpec::SamplePlay(BankRef::new(pcm)),
            UGenSpec::Wavetable(TableRef::new(pcm)),
            fx(EffectKind::Gain, "gain", Ctl::Cell(logical)),
            fx(EffectKind::Convolution, "ir", Ctl::Const(pcm as f32)),
            fx(
                EffectKind::Analyzer(AnalyzerKind::Level),
                "id",
                Ctl::Const(0.),
            ),
        ]
        .into_boxed_slice(),
        edges: vec![
            Edge {
                from: 1,
                to: 3,
                port: 0,
                output: 0,
            },
            Edge {
                from: 3,
                to: 4,
                port: 0,
                output: 0,
            },
            Edge {
                from: 4,
                to: 5,
                port: 0,
                output: 0,
            },
        ]
        .into_boxed_slice(),
        node_params: vec![(
            0,
            catalog::port_ctl(&Node::SinOsc, 0).unwrap(),
            Ctl::Cell(logical),
        )]
        .into_boxed_slice(),
    }
}
fn closure_bus(pcm: u32, logical: CellId) -> vactr::dsp::graph::BusDef {
    use vactr::dsp::graph::{BusDef, BusId, EffectSpec};
    BusDef {
        id: BusId::new(91),
        chain: [
            (EffectKind::Gain, "gain", Ctl::Cell(logical)),
            (EffectKind::Convolution, "ir", Ctl::Const(pcm as f32)),
            (
                EffectKind::Analyzer(AnalyzerKind::Level),
                "id",
                Ctl::Const(0.),
            ),
        ]
        .into_iter()
        .map(|(kind, name, value)| EffectSpec {
            kind,
            params: vec![(vactr::dsp::effects::param_ctl(kind, name).unwrap(), value)]
                .into_boxed_slice(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice(),
    }
}
fn run_byte(rig: &mut Rig, inbox: &mut ByteInbox) -> [f32; 32] {
    let mut out = [0.; 32];
    rig.engine.process(
        &mut EngineIo {
            events: &mut rig.events,
            controls: inbox,
            acks: &mut rig.acks,
            cells: &mut rig.cells,
            garbage: Some(&mut rig.garbage),
        },
        &mut out,
        16,
    );
    out
}
fn collect_receipts(rig: &mut Rig, rounds: usize) -> (Vec<SongHostAck>, Vec<Garbage>) {
    let mut acks = Vec::new();
    let mut garbage = Vec::new();
    for _ in 0..rounds {
        assert!(rig.step().iter().all(|x| *x == 0.));
        acks.extend(rig.acks());
        while let Some(owner) = rig.returned.pop() {
            garbage.push(owner);
        }
    }
    (acks, garbage)
}
#[test]
fn native_complete_graph_closure_adopts_remaps_and_seals_silently() {
    complete_graph_closure(false);
}
#[test]
fn byte_complete_graph_closure_adopts_remaps_and_seals_silently() {
    complete_graph_closure(true);
}
fn complete_graph_closure(bytes: bool) {
    use vactr::dsp::arena::{encode_bus, encode_inst};
    use vactr::dsp::graph::{BusId, InstId};
    use vactr::dsp::ugen::{Node, Src, Template, AMP, BANK};
    let mut rig = Rig::new(if bytes {
        StoreKind::Arena { bytes: 4096 }
    } else {
        StoreKind::NativeArc
    });
    let baseline = rig.engine.song_remaining_capacities().unwrap();
    let analysis_before = rig.engine.song_analysis_capacity().unwrap();
    let master = rig.engine.buses().master();
    let owners = [
        SongResourceKind::Instrument,
        SongResourceKind::Track,
        SongResourceKind::Sample,
        SongResourceKind::ControlCells,
        SongResourceKind::AnalysisBank,
    ]
    .map(|kind| key(u64::MAX - 1, 51 + kind as u32, kind));
    let [inst, bus, pcm, control, analysis] = owners;
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, inst.epoch.0, 5))
        .unwrap();
    for owner in owners {
        reserve(&mut rig.engine, owner);
    }
    let logical = CellId::new(42);
    rig.command(SongCommand::InitCells(SongCellInit {
        lease: control,
        cell: logical,
        value: 0.5,
    }));
    rig.command(SongCommand::ReserveAnalysis(SongAnalysisReservation {
        lease: analysis,
        slots: 64,
    }));
    for graph in [inst, bus] {
        rig.command(SongCommand::BindGraphBanks(SongGraphBanks {
            graph,
            controls: Some(control),
            analysis: Some(analysis),
        }));
    }
    let (mut receipts, initial_garbage) = collect_receipts(&mut rig, 8);
    assert!(initial_garbage.is_empty());
    let physical_cell = rig.engine.song_control_cell(control, logical).unwrap();
    assert_ne!(physical_cell, logical);
    let data = sample(64);
    let sample_pointer = Arc::as_ptr(&data);
    let mut inbox = ByteInbox::new();
    let mut encoded = [0_u8; 8192];
    if bytes {
        let n = ring::encode_song_sample_begin(pcm, 64, 1, 48_000, &mut encoded);
        assert!(n > 0 && inbox.push(&encoded[..n]));
        let n = ring::encode_song_slice(pcm, 0, &data.frames, &mut encoded);
        assert!(n > 0 && inbox.push(&encoded[..n]));
        assert!(run_byte(&mut rig, &mut inbox).iter().all(|x| *x == 0.));
        receipts.extend(rig.acks());
    } else {
        rig.tx
            .push(NativeRecord::SongInstall(NativeSongInstall {
                lease: pcm,
                payload: NativeInstall::Sample {
                    resource: pcm.resource.id,
                    gen: pcm.resource.generation,
                    data: Arc::clone(&data),
                },
            }))
            .ok()
            .unwrap();
    }
    receipts.extend(collect_receipts(&mut rig, 8).0);
    assert!(rig.engine.store().get(pcm.resource.id).is_none());
    let def = closure_inst(pcm.resource.id, logical);
    let template = Template::from_inst(&def, &rig.engine.build_env()).unwrap();
    assert!(template.mem_total as u64 <= rig.engine.song_voice_regions()[0].frames);
    let convolution_memory = vactr::dsp::effects::mem_len(
        EffectKind::Convolution,
        rig.engine.build_env().sr,
        &rig.engine.build_env().caps,
    );
    assert!(template.nodes[..template.n_nodes]
        .iter()
        .any(|node| matches!(
            node.node,
            Node::Effect {
                kind: EffectKind::Convolution,
                ..
            }
        ) && node.mem_len
            == u32::try_from(convolution_memory).expect("actual memory fits node extent")));
    let inst_pointer = std::ptr::from_ref(template.as_ref());
    let bus_def = closure_bus(pcm.resource.id, logical);
    let bus_template = Box::new(BusTemplate::from_def(&bus_def).unwrap());
    let bus_pointer = std::ptr::from_ref(bus_template.as_ref());
    let largest = rig
        .engine
        .song_bus_regions()
        .iter()
        .map(|r| r.frames)
        .max()
        .unwrap();
    let env = rig.engine.build_env();
    let effects = bus_def
        .chain
        .iter()
        .try_fold(0_u64, |sum, fx| {
            sum.checked_add(vactr::dsp::effects::mem_len(fx.kind, env.sr, &env.caps) as u64)
        })
        .unwrap();
    let room =
        (vactr::dsp::effects::mem_len(EffectKind::Room, env.sr, &env.caps) as u64).min(largest / 4);
    assert!(effects.checked_add(room).unwrap() <= largest);
    if bytes {
        let mut graph = Vec::new();
        encode_inst(&def, &mut graph).unwrap();
        let n = ring::encode_song_graph_record(inst, &graph, &mut encoded);
        assert!(n > 0 && inbox.push(&encoded[..n]));
        assert!(run_byte(&mut rig, &mut inbox).iter().all(|x| *x == 0.));
        receipts.extend(rig.acks());
        encode_bus(&bus_def, false, &mut graph).unwrap();
        let n = ring::encode_song_graph_record(bus, &graph, &mut encoded);
        assert!(n > 0 && inbox.push(&encoded[..n]));
        assert!(run_byte(&mut rig, &mut inbox).iter().all(|x| *x == 0.));
        receipts.extend(rig.acks());
    } else {
        for install in [
            NativeSongInstall {
                lease: inst,
                payload: NativeInstall::Inst {
                    resource: inst.resource.id,
                    gen: inst.resource.generation,
                    template,
                },
            },
            NativeSongInstall {
                lease: bus,
                payload: NativeInstall::Bus {
                    resource: bus.resource.id,
                    gen: bus.resource.generation,
                    master: false,
                    template: bus_template,
                },
            },
        ] {
            rig.tx
                .push(NativeRecord::SongInstall(install))
                .ok()
                .unwrap();
        }
    }
    receipts.extend(collect_receipts(&mut rig, 8).0);
    for owner in [pcm, inst, bus] {
        assert_eq!(
            receipts
                .iter()
                .filter(|ack| **ack
                    == SongHostAck::ResourceReady {
                        epoch: owner.epoch,
                        resource: owner.resource
                    })
                .count(),
            1
        );
    }
    assert!(!receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::Rejected { .. })));
    assert!(rig.engine.template(InstId::new(301)).is_none());
    assert!(rig.engine.template(InstId::new(inst.resource.id)).is_none());
    assert!(rig.engine.buses().find(BusId::new(91)).is_none());
    assert_eq!(rig.engine.buses().master(), master);
    assert_eq!(
        rig.engine
            .buses()
            .slots
            .iter()
            .filter(|s| s.state == SlotState::Staged)
            .count(),
        1
    );
    rig.command(SongCommand::SealPreparation(inst.epoch));
    receipts.extend(collect_receipts(&mut rig, 8).0);
    assert_eq!(
        receipts
            .iter()
            .filter(|ack| **ack == SongHostAck::Ready(inst.epoch))
            .count(),
        1
    );
    rig.command(SongCommand::CancelPreparation(inst.epoch));
    let (retired, garbage) = collect_receipts(&mut rig, 40);
    for owner in owners {
        assert_eq!(
            retired
                .iter()
                .filter(|ack| **ack == SongHostAck::LeaseReturned(owner))
                .count(),
            1
        );
    }
    assert_eq!(
        retired
            .iter()
            .filter(|ack| **ack == SongHostAck::PreparationCancelled(inst.epoch))
            .count(),
        1
    );
    if !bytes {
        let mut returned = 0;
        for owner in garbage {
            let Garbage::SongInstall(owner) = owner else {
                panic!("exact graph owner")
            };
            assert!(owners.contains(&owner.lease));
            match owner.payload {
                NativeInstall::Sample { data, .. } => {
                    assert_eq!(Arc::as_ptr(&data), sample_pointer)
                }
                NativeInstall::Bus { template, .. } => {
                    assert_eq!(std::ptr::from_ref(template.as_ref()), bus_pointer);
                    let analyzer = EffectKind::Analyzer(AnalyzerKind::Level);
                    let id = vactr::dsp::effects::param_ctl(analyzer, "id").unwrap();
                    assert_eq!(template.kinds[2], analyzer);
                    assert!(template.params[2][..usize::from(template.n_params[2])]
                        .contains(&(id, Ctl::Const(0.))));
                    let gain = vactr::dsp::effects::param_ctl(EffectKind::Gain, "gain").unwrap();
                    let ir = vactr::dsp::effects::param_ctl(EffectKind::Convolution, "ir").unwrap();
                    assert!(template.params[0][..usize::from(template.n_params[0])]
                        .contains(&(gain, Ctl::Cell(physical_cell))));
                    assert!(template.params[1][..usize::from(template.n_params[1])]
                        .iter()
                        .any(|(id, ctl)| *id == ir
                            && matches!(ctl,
                            Ctl::Const(value) if *value != pcm.resource.id as f32)));
                }
                NativeInstall::Inst { template, .. } => {
                    assert_eq!(std::ptr::from_ref(template.as_ref()), inst_pointer);
                    assert!(template.params[..template.n_params]
                        .contains(&(AMP, Ctl::Cell(physical_cell))));
                    assert!(template.nodes[..template.n_nodes]
                        .iter()
                        .any(|node| node.node == Node::SinOsc
                            && node.inputs.contains(&Src::Cell(physical_cell))));
                    let physical = template.refs[0];
                    assert_ne!(physical, pcm.resource.id);
                    assert!(template.params[..template.n_params]
                        .contains(&(BANK, Ctl::Const(physical as f32))));
                    assert!(template.nodes[..template.n_nodes]
                        .iter()
                        .any(|n| matches!(n.node, Node::SamplePlay(k) if k.get() == physical)));
                    assert!(template.nodes[..template.n_nodes]
                        .iter()
                        .any(|n| matches!(n.node, Node::Wavetable(k) if k.get() == physical)));
                    let gain = vactr::dsp::effects::param_ctl(EffectKind::Gain, "gain").unwrap();
                    let ir = vactr::dsp::effects::param_ctl(EffectKind::Convolution, "ir").unwrap();
                    assert!(template.fx_params[..template.n_fx]
                        .iter()
                        .any(|p| p.contains(&(gain, Ctl::Cell(physical_cell)))));
                    assert!(template.fx_params[..template.n_fx]
                        .iter()
                        .any(|p| p.contains(&(ir, Ctl::Const(physical as f32)))));
                }
            }
            returned += 1;
        }
        assert_eq!(returned, 3);
    } else {
        assert!(garbage.is_empty());
    }
    assert_eq!(rig.engine.song_remaining_capacities().unwrap(), baseline);
    assert_eq!(
        rig.engine.song_analysis_capacity().unwrap(),
        analysis_before
    );
}

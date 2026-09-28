//! BE-DSP tests (design 12.8.9, TASK-008): headless renders of the audio
//! engine. Every `Engine::process` call runs inside `alloc_probe::armed`
//! and asserts zero allocations (`Rig::step`).

mod analog_pair;
mod analyzer;
mod arena;
mod braids_cloud;
mod braids_digital;
mod braids_filter;
mod braids_five;
mod braids_fm;
mod braids_formant;
mod braids_noise;
mod braids_percussion;
mod braids_physical;
mod braids_struck;
mod braids_subsync;
mod braids_triple;
mod braids_wave_bank;
mod braids_wave_line;
mod bus;
mod caps;
mod catalog;
mod cells;
mod chip_pair;
mod chord_pair;
mod clock_noise_pair;
mod cross_mod;
mod dual_kick;
mod dynamic_control;
mod effects;
mod elements_bank;
mod elements_internal;
mod feedback_metal;
mod fm_drum;
mod fm_pair;
mod frame_keyframe;
mod frame_lfo;
mod grain_pair;
mod granular;
mod hat_pair;
mod live_input;
mod modal_pair;
mod number_station;
mod particle_pair;
mod peak_function;
mod peak_pulse;
mod phase_pair;
mod quad_outputs;
mod rate_contract;
mod region;
mod release;
mod render;
mod resonant_bank;
mod rings_part;
mod shape_pair;
mod shift_pair;
mod six_op_original;
mod snare_pair;
mod spectrum_pair;
mod speech_original;
mod stage_chain;
mod stage_linked;
mod stage_segment;
mod stereo_contract;
mod stream_cv;
mod stream_dynamics;
mod string_choir;
mod string_machine_pair;
mod string_pair;
mod swarm_pair;
mod table_terrain_pair;
mod templates;
mod terrain_pair;
mod texture;
mod texture_loop;
mod texture_quality;
mod texture_spectral;
mod texture_stretch;
mod tidal_function;
mod tidal_poly;
mod ugens;
mod va_filter;
mod voice_input;
mod voice_stereo;

use std::sync::Arc;

use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::StoreKind;
use crate::dsp::bus::BusTemplate;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::{AtomicCells, Mirror};
use crate::dsp::engine::{CellStore, Engine, EngineConfig, EngineIo};
use crate::dsp::graph::{BusDef, BusId, Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::{
    AckConsumer, AckProducer, Budget, ByteInbox, Consumer, ControlSource, EventConsumer,
    EventProducer, EventRing, Garbage, NativeInstall, NativeRecord, Producer, Record, SpscRing,
};
use crate::dsp::ugen::Template;
use crate::host::caps::SampleData;
use crate::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg};
use crate::sched::slots::{CtlId, SlotId};

/// The test sample rate.
pub(super) const SR: f32 = 48_000.0;
/// The test block.
pub(super) const BLOCK: usize = 128;

/// Control ids used by the tests.
pub(super) mod ctl {
    use crate::sched::slots::CtlId;
    pub const FREQ: CtlId = CtlId::new(0);
    pub const AMP: CtlId = CtlId::new(1);
    pub const PAN: CtlId = CtlId::new(4);
    pub const SPEED: CtlId = CtlId::new(5);
    pub const BEGIN: CtlId = CtlId::new(7);
    pub const END: CtlId = CtlId::new(8);
    pub const ATTACK: CtlId = CtlId::new(9);
    pub const DECAY: CtlId = CtlId::new(10);
    pub const SUSTAIN: CtlId = CtlId::new(11);
    pub const RELEASE: CtlId = CtlId::new(12);
    pub const LOOP: CtlId = CtlId::new(25);
    pub const BANK: CtlId = CtlId::new(24);
    pub const ROOM: CtlId = CtlId::new(44);
    pub const BUS: CtlId = CtlId::new(47);
    pub const LEGATO: CtlId = CtlId::new(49);
}

/// Small tier caps so every engine stays a few megabytes.
pub(super) fn caps() -> CapabilitySet {
    CapabilitySet {
        max_voices: 8,
        max_ir_seconds: 1.0,
        max_grain_density: 100.0,
        max_grain_size: 0.25,
        max_capture_seconds: 1.0,
        ..CapabilitySet::browser()
    }
}

/// The test engine configuration.
pub(super) fn config(caps: &CapabilitySet, store: StoreKind) -> EngineConfig {
    EngineConfig {
        template_slots: 8,
        bus_slots: 4,
        bus_seconds: 1.0,
        voice_seconds: 0.1,
        orbits: 2,
        orbit_delay_seconds: 0.5,
        analysis_cells: 1024,
        event_capacity: 256,
        ..EngineConfig::new(caps, SR, BLOCK, store)
    }
}

/// The native control channel: POD records and install handovers.
pub(super) struct NativeChannel {
    pub tx: Producer<NativeRecord>,
    rx: Consumer<NativeRecord>,
}

impl ControlSource for NativeChannel {
    fn next(&mut self, budget: Budget) -> Option<Record<'_>> {
        self.rx.next(budget)
    }
}

/// An engine with its rings, cells and control source.
pub(super) struct Rig<C: CellStore, S: ControlSource> {
    pub engine: Engine,
    pub events: EventProducer,
    events_rx: EventConsumer,
    pub controls: S,
    acks_tx: AckProducer,
    pub acks_rx: AckConsumer,
    garbage_tx: Producer<Garbage>,
    pub garbage_rx: Consumer<Garbage>,
    pub cells: C,
    buf: Vec<f32>,
    next_resource: u32,
}

/// The native-tier rig.
pub(super) type NativeRig = Rig<AtomicCells, NativeChannel>;
/// The browser-tier rig.
pub(super) type BrowserRig = Rig<Mirror, ByteInbox>;

fn rig<C: CellStore, S: ControlSource>(engine: Engine, controls: S, cells: C) -> Rig<C, S> {
    let (events, events_rx) = EventRing::split(1024);
    let (acks_tx, acks_rx) = SpscRing::split(4096);
    let (garbage_tx, garbage_rx) = SpscRing::split(256);
    let block = engine.config().max_block;
    let channels = usize::from(engine.config().output_channels);
    Rig {
        engine,
        events,
        events_rx,
        controls,
        acks_tx,
        acks_rx,
        garbage_tx,
        garbage_rx,
        cells,
        buf: vec![0.0; channels * block],
        next_resource: 1,
    }
}

impl NativeRig {
    /// A native engine (`AtomicCells`, `Arc` store).
    pub(super) fn native() -> Self {
        Self::native_with(config(&caps(), StoreKind::NativeArc))
    }

    /// A native engine with a custom configuration.
    pub(super) fn native_with(cfg: EngineConfig) -> Self {
        let (tx, rx) = SpscRing::split(256);
        rig(
            Engine::with_config(cfg),
            NativeChannel { tx, rx },
            AtomicCells::new(64),
        )
    }

    /// Hands an instrument to the engine; returns its resource id.
    pub(super) fn install(&mut self, def: &InstDef) -> u32 {
        let t = Template::from_inst(def, &self.engine.build_env()).expect("template builds");
        let resource = self.fresh();
        self.post_native(NativeInstall::Inst {
            resource,
            gen: 1,
            template: t,
        });
        resource
    }

    /// Hands a bus (or master) chain to the engine.
    pub(super) fn install_bus(&mut self, def: &BusDef, master: bool) -> u32 {
        let t = Box::new(BusTemplate::from_def(def).expect("bus builds"));
        let resource = self.fresh();
        self.post_native(NativeInstall::Bus {
            resource,
            gen: 1,
            master,
            template: t,
        });
        resource
    }

    /// Hands a sample to the engine under `resource`.
    pub(super) fn sample(&mut self, resource: u32, frames: Vec<f32>, channels: u8) {
        let data = Arc::new(SampleData {
            rate: 48_000,
            channels,
            frames: frames.into_boxed_slice(),
        });
        self.post_native(NativeInstall::Sample {
            resource,
            gen: 1,
            data,
        });
    }

    pub(super) fn post_native(&mut self, n: NativeInstall) {
        assert!(self.controls.tx.push(NativeRecord::Install(n)).is_ok());
    }

    /// Posts a POD control record.
    pub(super) fn post(&mut self, m: CtlMsg) {
        assert!(self.controls.tx.push(NativeRecord::Msg(m)).is_ok());
    }
}

impl BrowserRig {
    /// A browser engine (`Mirror` cells, arena store, byte inbox).
    pub(super) fn browser(arena_bytes: usize) -> Self {
        let cfg = config(&caps(), StoreKind::Arena { bytes: arena_bytes });
        rig(Engine::with_config(cfg), ByteInbox::new(), Mirror::new(64))
    }

    pub(super) fn browser_with(cfg: EngineConfig) -> Self {
        rig(Engine::with_config(cfg), ByteInbox::new(), Mirror::new(64))
    }

    /// Queues one encoded record.
    pub(super) fn push(&mut self, rec: &[u8]) {
        assert!(self.controls.push(rec), "inbox accepts the record");
    }

    /// Queues a POD control record.
    pub(super) fn post(&mut self, m: CtlMsg) {
        let mut b = [0u8; 512];
        let n = m.encode(&mut b);
        assert!(n > 0);
        self.push(&b[..n]);
    }
}

impl<C: CellStore, S: ControlSource> Rig<C, S> {
    fn fresh(&mut self) -> u32 {
        let r = self.next_resource;
        self.next_resource += 1;
        r
    }

    /// Renders one block with the allocation probe armed; asserts zero
    /// allocations and returns the interleaved block.
    pub(super) fn step(&mut self) -> &[f32] {
        let block = self.engine.config().max_block;
        let mut io = EngineIo {
            events: &mut self.events_rx,
            controls: &mut self.controls,
            acks: &mut self.acks_tx,
            cells: &mut self.cells,
            garbage: Some(&mut self.garbage_tx),
        };
        let engine = &mut self.engine;
        let buf = &mut self.buf;
        let ((), allocs) = armed(|| {
            if engine.config().output_channels == 4 {
                engine.process_four(&mut io, buf, block);
            } else {
                engine.process(&mut io, buf, block);
            }
        });
        assert_eq!(allocs, 0, "Engine rendering allocated");
        &self.buf
    }

    /// Same allocation-probed callback with exact interleaved external L/R.
    pub(super) fn step_with_input(&mut self, input: &[f32]) -> &[f32] {
        let block = self.engine.config().max_block;
        let mut io = EngineIo {
            events: &mut self.events_rx,
            controls: &mut self.controls,
            acks: &mut self.acks_tx,
            cells: &mut self.cells,
            garbage: Some(&mut self.garbage_tx),
        };
        let engine = &mut self.engine;
        let buf = &mut self.buf;
        let ((), allocs) = armed(|| {
            if engine.config().output_channels == 4 {
                engine.process_four_with_input(&mut io, input, buf, block);
            } else {
                engine.process_with_input(&mut io, input, buf, block);
            }
        });
        assert_eq!(allocs, 0, "Engine input rendering allocated");
        &self.buf
    }

    /// Renders `blocks` blocks; returns the left and right channels.
    pub(super) fn run(&mut self, blocks: usize) -> (Vec<f32>, Vec<f32>) {
        let mut l = Vec::new();
        let mut r = Vec::new();
        for _ in 0..blocks {
            let b = self.step();
            l.extend(b.iter().step_by(2));
            r.extend(b.iter().skip(1).step_by(2));
        }
        (l, r)
    }

    /// Renders four independently addressable output lanes.
    pub(super) fn run_four(&mut self, blocks: usize) -> [Vec<f32>; 4] {
        assert_eq!(self.engine.config().output_channels, 4);
        let mut lanes = std::array::from_fn(|_| Vec::new());
        for _ in 0..blocks {
            for frame in self.step().chunks_exact(4) {
                for (lane, &sample) in lanes.iter_mut().zip(frame) {
                    lane.push(sample);
                }
            }
        }
        lanes
    }

    /// Sends an event.
    pub(super) fn send(&mut self, ev: AudioEvent) {
        assert!(self.events.push(ev).is_ok());
    }

    /// Everything the engine sent back.
    pub(super) fn acks(&mut self) -> Vec<HostMsg> {
        std::iter::from_fn(|| self.acks_rx.pop()).collect()
    }
}

/// An event on slot 1, gen 1, with constant controls.
pub(super) fn event(inst: u32, time: f64, ctls: &[(CtlId, f32)]) -> AudioEvent {
    let mut ev = AudioEvent::new(time, SlotId::new(1), 1, InstId::new(inst));
    for &(id, v) in ctls {
        ev.push_ctl(id, Ctl::Const(v)).expect("fits");
    }
    ev
}

/// A single-chain instrument: `nodes[i]` feeds port 0 of `nodes[i + 1]`.
pub(super) fn chain(id: u32, nodes: Vec<UGenSpec>) -> InstDef {
    let edges = (1..nodes.len())
        .map(|i| Edge {
            from: u16::try_from(i - 1).unwrap(),
            to: u16::try_from(i).unwrap(),
            port: 0,
        })
        .collect();
    InstDef {
        id: InstId::new(id),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges,
        node_params: Box::new([]),
    }
}

/// A bus chain.
pub(super) fn bus_def(id: u32, chain: Vec<crate::dsp::graph::EffectSpec>) -> BusDef {
    BusDef {
        id: BusId::new(id),
        chain: chain.into_boxed_slice(),
    }
}

/// Seeded noise in `[-amp, amp)`.
pub(super) fn noise(n: usize, amp: f32, seed: u32) -> Vec<f32> {
    let mut rng = crate::dsp::effects::prim::Rng::new(seed);
    (0..n).map(|_| amp * rng.bipolar()).collect()
}

/// Root-mean-square of a buffer.
pub(super) fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let n = x.len() as f32;
    (x.iter().map(|v| v * v).sum::<f32>() / n).sqrt()
}

/// The `Retired` resource ids among acks.
pub(super) fn retired(acks: &[HostMsg]) -> Vec<u32> {
    acks.iter()
        .filter_map(|m| match m {
            HostMsg::Retired { resource } => Some(*resource),
            _ => None,
        })
        .collect()
}

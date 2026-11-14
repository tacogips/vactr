//! Actual bus audio keys complete stereo tails, independent of processing order.
use super::{caps, SR};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::{decode_graph, encode_bus, FaultCode, GraphKind, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate, SlotState};
use crate::dsp::cells::{AtomicCells, CellId};
use crate::dsp::effects::{self, catalog::spec, FxCtx, FxStats, SIDECHAIN_BUS_CTL};
use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::{BusDef, BusId, EffectKind, EffectSpec};
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

const BLOCK: usize = 1024;
fn compressor(source: Option<Ctl>, mix: f32) -> EffectSpec {
    let mut effect = spec(
        EffectKind::Compressor,
        &[
            ("threshold", Ctl::Const(-20.0)),
            ("ratio", Ctl::Const(8.0)),
            ("attack", Ctl::Const(0.0005)),
            ("release", Ctl::Const(0.04)),
            ("knee", Ctl::Const(0.0)),
            ("mix", Ctl::Const(mix)),
        ],
    )
    .unwrap();
    if let Some(source) = source {
        let mut params = effect.params.into_vec();
        params.push((SIDECHAIN_BUS_CTL, source));
        effect.params = params.into_boxed_slice();
    }
    effect
}
fn template(bus: u32, chain: Vec<EffectSpec>) -> BusTemplate {
    BusTemplate::from_def(&BusDef {
        id: BusId::new(bus),
        chain: chain.into_boxed_slice(),
    })
    .unwrap()
}
struct Fixture {
    graph: BusGraph,
    cells: AtomicCells,
    store: SampleStore,
    fft: Fft,
    scratch: Vec<f32>,
    analysis: Vec<f32>,
    dry: Vec<f32>,
    stats: FxStats,
}
impl Fixture {
    fn new(reverse: bool, mix: f32) -> Self {
        let cells = AtomicCells::new(4);
        let mut graph = BusGraph::new(6, BLOCK, 12000, &cells, SR, &caps());
        let key = template(
            1,
            vec![spec(EffectKind::Gain, &[("gain", Ctl::Const(-96.0))]).unwrap()],
        );
        let receiver = template(2, vec![compressor(Some(Ctl::Const(1.0)), mix)]);
        for t in if reverse {
            [&receiver, &key]
        } else {
            [&key, &receiver]
        } {
            assert!(graph.install(t, false, t.bus.get(), 1, &cells, SR, &caps()));
        }
        Self {
            graph,
            cells,
            store: SampleStore::new(StoreKind::NativeArc),
            fft: Fft::new(FFT_SIZE),
            scratch: vec![0.0; FFT_SIZE * 4],
            analysis: vec![0.0; 1024],
            dry: vec![0.0; BLOCK * 2],
            stats: FxStats::default(),
        }
    }
    fn step(
        &mut self,
        keys: &[(usize, f32, f32)],
        frames: usize,
        bass: f32,
    ) -> (Vec<f32>, Vec<f32>) {
        self.graph.clear(frames);
        let receiver = self.graph.find(BusId::new(2)).unwrap();
        self.graph.slots[receiver].l[..frames].fill(bass);
        self.graph.slots[receiver].r[..frames].fill(bass * 0.5);
        for &(slot, l, r) in keys {
            self.graph.slots[slot].l[..frames].fill(l);
            self.graph.slots[slot].r[..frames].fill(r);
        }
        let mut l = vec![0.0; frames];
        let mut r = vec![0.0; frames];
        let capabilities = caps();
        let mut ctx = FxCtx {
            sr: SR,
            store: &self.store,
            fft: &self.fft,
            caps: &capabilities,
            scratch: &mut self.scratch,
            analysis: &mut self.analysis,
            stats: &mut self.stats,
        };
        let (_, allocations) = armed(|| {
            self.graph
                .render(frames, &self.cells, &mut self.dry, &mut ctx, &mut l, &mut r)
        });
        assert_eq!(allocations, 0);
        let (left, right) = self.graph.frames(receiver, frames);
        (left.to_vec(), right.to_vec())
    }
    fn key(&self) -> usize {
        self.graph.find(BusId::new(1)).unwrap()
    }
}
#[test]
fn same_block_stereo_peak_ducks_without_key_leakage_or_order_dependence() {
    let mut a = Fixture::new(false, 1.0);
    let mut b = Fixture::new(true, 1.0);
    let key_a = a.key();
    let key_b = b.key();
    let (l, r) = a.step(&[(key_a, 0.0, 1.0)], BLOCK, 0.4);
    let (other, _) = b.step(&[(key_b, 1.0, 0.0)], BLOCK, 0.4);
    assert_eq!(l, other);
    assert!(l[64] < 0.12, "same-block attack: {}", l[64]);
    assert!(l[BLOCK - 1] < 0.06);
    assert!(l.iter().zip(&r).all(|(l, r)| (*l - 2.0 * r).abs() < 1e-6));
    let (silence, _) = a.step(&[(key_a, 1.0, 1.0)], BLOCK, 0.0);
    assert!(silence.iter().all(|x| *x == 0.0));
}
#[test]
fn missing_key_recovers_without_self_detection_and_mix_zero_bypasses() {
    let mut f = Fixture::new(false, 1.0);
    let key = f.key();
    let (ducked, _) = f.step(&[(key, 1.0, 0.0)], BLOCK, 0.4);
    f.graph.slots[key].state = SlotState::Free;
    let mut recovered = Vec::new();
    for _ in 0..32 {
        recovered = f.step(&[], BLOCK, 0.4).0;
    }
    assert!(recovered[BLOCK - 1] > 0.39);
    assert!(ducked[BLOCK - 1] < 0.06);
    let mut bypass = Fixture::new(false, 0.0);
    let key = bypass.key();
    let (l, r) = bypass.step(&[(key, 1.0, 1.0)], BLOCK, 0.4);
    assert!(l.iter().all(|x| *x == 0.4));
    assert!(r.iter().all(|x| *x == 0.2));
}
#[test]
fn key_swaps_include_retiring_voice_audio_and_sum_before_rectification() {
    let mut f = Fixture::new(false, 1.0);
    let old = f.key();
    f.graph.slots[old].users = 1;
    let new = template(1, vec![]);
    assert!(f.graph.install(&new, false, 10, 2, &f.cells, SR, &caps()));
    let live = f.key();
    assert_eq!(f.graph.slots[old].state, SlotState::Retiring);
    let (l, _) = f.step(&[(old, 1.0, 0.0)], BLOCK, 0.4);
    assert!(l[BLOCK - 1] < 0.06, "retiring source still keys");
    // Equal opposing generations cancel as actual bus audio would.
    for _ in 0..32 {
        let _ = f.step(&[(old, 1.0, 0.0), (live, -1.0, 0.0)], BLOCK, 0.4);
    }
    let (l, _) = f.step(&[(old, 1.0, 0.0), (live, -1.0, 0.0)], BLOCK, 0.4);
    assert!(l[BLOCK - 1] > 0.39);
}
#[test]
fn fixed_detector_is_invariant_under_callback_partitioning() {
    let mut large = Fixture::new(false, 1.0);
    let mut small = Fixture::new(false, 1.0);
    let large_key = large.key();
    let small_key = small.key();
    let (expected, _) = large.step(&[(large_key, 1.0, 0.0)], BLOCK, 0.4);
    let mut actual = Vec::new();
    for frames in [64, 128, 256, 17, 31, 7, 521] {
        actual.extend(small.step(&[(small_key, 1.0, 0.0)], frames, 0.4).0);
    }
    assert_eq!(actual, expected);
}
#[test]
fn compressor_self_detector_preserves_catalog_indices() {
    for (i, name) in [
        "threshold",
        "ratio",
        "attack",
        "release",
        "makeup",
        "knee",
        "mix",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            effects::param_index(
                EffectKind::Compressor,
                effects::param_ctl(EffectKind::Compressor, name).unwrap()
            ),
            Some(i)
        );
    }
    let mut f = Fixture::new(false, 1.0);
    let t = template(2, vec![compressor(None, 1.0)]);
    assert!(f.graph.install(&t, false, 12, 1, &f.cells, SR, &caps()));
    let (l, _) = f.step(&[], BLOCK, 0.4);
    assert!(l[BLOCK - 1] < 0.13 && l[BLOCK - 1] > 0.1);
}
#[test]
fn codec_preserves_selector_and_rejects_malformed_controls() {
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    for (control, kind, receiver, duplicate, valid) in [
        (Ctl::Const(1.0), EffectKind::Compressor, 2, false, true),
        (
            Ctl::Const(16777216.0),
            EffectKind::Compressor,
            2,
            false,
            true,
        ),
        (Ctl::Const(0.0), EffectKind::Compressor, 2, false, false),
        (Ctl::Const(1.5), EffectKind::Compressor, 2, false, false),
        (
            Ctl::Const(f32::NAN),
            EffectKind::Compressor,
            2,
            false,
            false,
        ),
        (
            Ctl::Const(f32::INFINITY),
            EffectKind::Compressor,
            2,
            false,
            false,
        ),
        (
            Ctl::Const(33554432.0),
            EffectKind::Compressor,
            2,
            false,
            false,
        ),
        (
            Ctl::Cell(CellId::new(0)),
            EffectKind::Compressor,
            2,
            false,
            false,
        ),
        (Ctl::Const(2.0), EffectKind::Compressor, 2, false, false),
        (Ctl::Const(1.0), EffectKind::Gain, 2, false, false),
        (Ctl::Const(1.0), EffectKind::Compressor, 2, true, false),
    ] {
        let mut params = vec![(SIDECHAIN_BUS_CTL, control)];
        if duplicate {
            params.push((SIDECHAIN_BUS_CTL, control));
        }
        let def = BusDef {
            id: BusId::new(receiver),
            chain: vec![EffectSpec {
                kind,
                params: params.into_boxed_slice(),
            }]
            .into_boxed_slice(),
        };
        let mut bytes = Vec::new();
        encode_bus(&def, false, &mut bytes).unwrap();
        let decoded = decode_graph(&bytes, &mut raw, &mut bus);
        if valid {
            assert_eq!(decoded, Ok(GraphKind::Bus(def.id)));
            assert_eq!(bus.params[0][0], (SIDECHAIN_BUS_CTL, control));
        } else {
            assert_eq!(decoded, Err(FaultCode::BadRecord), "{control:?}/{kind:?}");
            assert!(BusTemplate::from_def(&def).is_err());
        }
    }
}

#[test]
fn resting_or_quieter_kick_changes_ducking_and_invalid_install_keeps_live_chain() {
    let mut loud = Fixture::new(false, 1.0);
    let mut quiet = Fixture::new(false, 1.0);
    let loud_key = loud.key();
    let quiet_key = quiet.key();
    let (strong, _) = loud.step(&[(loud_key, 1.0, 0.0)], BLOCK, 0.4);
    let (weak, _) = quiet.step(&[(quiet_key, 0.2, 0.0)], BLOCK, 0.4);
    assert!(weak[BLOCK - 1] > strong[BLOCK - 1] * 3.0);
    // A rest has no source voice, while its bus remains live.
    let mut rested = Vec::new();
    for _ in 0..16 {
        rested = loud.step(&[], BLOCK, 0.4).0;
    }
    assert!(rested[BLOCK - 1] > 0.39);
    let old = loud.graph.find(BusId::new(2)).unwrap();
    let mut invalid = BusTemplate::new();
    invalid.bus = BusId::new(2);
    let unit = invalid.push(EffectKind::Compressor).unwrap();
    assert!(invalid.push_param(unit, SIDECHAIN_BUS_CTL, Ctl::Const(1.5)));
    let (installed, allocations) = armed(|| {
        loud.graph
            .install(&invalid, false, 20, 2, &loud.cells, SR, &caps())
    });
    assert!(!installed);
    assert_eq!(allocations, 0);
    assert_eq!(loud.graph.find(BusId::new(2)), Some(old));
    assert_eq!(loud.graph.slots[old].state, SlotState::Live);
}

#[test]
fn instrument_codec_rejects_reserved_bus_selector() {
    use crate::dsp::arena::encode_inst;
    use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
    let effect = compressor(Some(Ctl::Const(1.0)), 1.0);
    let definition = InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::Const(0.4), UGenSpec::Effect(effect)].into_boxed_slice(),
        edges: vec![Edge {
            from: 0,
            to: 1,
            port: 0,
            output: 0,
        }]
        .into_boxed_slice(),
        node_params: Box::new([]),
    };
    let mut bytes = Vec::new();
    encode_inst(&definition, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    assert_eq!(
        decode_graph(&bytes, &mut raw, &mut bus),
        Err(FaultCode::BadRecord)
    );
}

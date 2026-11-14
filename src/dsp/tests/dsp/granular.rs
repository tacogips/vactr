//! TASK-008 criterion 6 (render half): the grain engine. Onset count and
//! timing follow the phase-accumulator model under a fixed seed; a live
//! grain never reads an overwritten or uncaptured sample; freeze holds the
//! buffer while input continues; unfreeze resumes live and short-gates only
//! violating grains; a freeze right after install spawns nothing; density,
//! size and capture depth over the caps clamp and count with the audio
//! continuing; a full pool skips with a count. (The diagnostic half is
//! BE-SCHED's `sched/tests/sched/granular.rs`.)

use super::{bus_def, caps, config, ctl, event, noise, rms, NativeRig, SR};
use crate::dsp::arena::StoreKind;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellId;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{BankRef, Edge, EffectKind, GranSrc, InstDef, InstId, UGenSpec};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

const SRC: u32 = 11;
const FREEZE: CellId = CellId::new(3);
const INPUT_GAIN: CellId = CellId::new(4);

/// A static-source granular instrument with constant node parameters.
fn grains(params: &[(&str, f32)]) -> InstDef {
    let node_params = params
        .iter()
        .map(|&(name, v)| {
            let id = crate::dsp::controls::row(name).unwrap().ctl;
            (0u16, id, Ctl::Const(v))
        })
        .collect();
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: Box::new([UGenSpec::Granular(GranSrc::Sample(BankRef::new(SRC)))]),
        edges: Box::new([]),
        node_params,
    }
}

fn static_rig(cfg: crate::dsp::engine::EngineConfig, params: &[(&str, f32)]) -> NativeRig {
    let mut rig = NativeRig::native_with(cfg);
    rig.sample(SRC, noise(48_000, 0.5, 5), 1);
    rig.install(&grains(params));
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::LEGATO, 30.0), (ctl::PAN, 0.0)]));
    rig
}

fn stats(rig: &NativeRig) -> FxStats {
    rig.engine.counters().fx
}

#[test]
fn onsets_follow_the_phase_accumulator_model() {
    let density = 60.0f32;
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.max_block = 1;
    let mut rig = static_rig(cfg, &[("density", density), ("size", 0.01)]);
    let mut model = Vec::new();
    let mut acc = 0.0f32;
    let frames = 4_800;
    for i in 0..frames {
        acc += density / SR;
        if acc >= 1.0 {
            acc -= 1.0;
            model.push(i);
        }
    }
    let mut seen = Vec::new();
    let mut before = stats(&rig).grains_spawned;
    for i in 0..frames {
        let _ = rig.step();
        let now = stats(&rig).grains_spawned;
        if now > before {
            seen.push(i);
        }
        before = now;
    }
    assert_eq!(seen, model, "onset frames");
    assert!(seen.len() >= 5, "about one onset per 800 frames");
}

#[test]
fn a_fixed_seed_renders_identically() {
    let p = [
        ("density", 80.0),
        ("size", 0.05),
        ("spray", 0.4),
        ("pitch-spray", 0.3),
        ("reverse", 0.5),
    ];
    let (a, _) = static_rig(config(&caps(), StoreKind::NativeArc), &p).run(80);
    let (b, _) = static_rig(config(&caps(), StoreKind::NativeArc), &p).run(80);
    assert_eq!(a, b);
    assert!(rms(&a) > 0.0);
}

#[test]
fn caps_clamp_and_count_with_audio_continuing() {
    let mut rig = static_rig(
        config(&caps(), StoreKind::NativeArc),
        &[("density", 5_000.0), ("size", 3.0)],
    );
    let (l, _) = rig.run(60);
    let s = stats(&rig);
    assert!(s.clamped > 0, "density and size were clamped and counted");
    assert!(s.grains_spawned > 0, "grains still spawn");
    assert!(rms(&l[l.len() / 2..]) > 0.0, "no dropout");
}

#[test]
fn a_full_pool_skips_with_a_count() {
    // 0.003 s of voice memory holds 12 slots; 100/s x 0.25 s wants 25.
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.voice_seconds = 0.003;
    let mut rig = static_rig(cfg, &[("density", 100.0), ("size", 0.25)]);
    let (l, _) = rig.run(200);
    let s = stats(&rig);
    assert!(s.grains_skipped > 0, "pool exhaustion is counted");
    assert!(s.grains_spawned > 0);
    assert!(l.iter().all(|v| v.is_finite()));
}

#[test]
fn freeze_on_a_static_source_pins_the_position() {
    let mut rig = static_rig(
        config(&caps(), StoreKind::NativeArc),
        &[
            ("density", 50.0),
            ("size", 0.02),
            ("freeze", 1.0),
            ("position", 0.3),
        ],
    );
    let (l, _) = rig.run(40);
    assert!(rms(&l) > 0.0);
}

// ---- live capture (`granulate` on a bus) -----------------------------------

/// A noise source routed through a `granulate` master; `freeze` and the
/// source level are cells.
fn live(cfg_caps: CapabilitySet, params: &[(&str, f32)]) -> NativeRig {
    let mut rig = NativeRig::native_with(config(&cfg_caps, StoreKind::NativeArc));
    let mut named: Vec<(&'static str, Ctl)> =
        vec![("freeze", Ctl::Cell(FREEZE)), ("mix", Ctl::Const(1.0))];
    for &(n, v) in params {
        let name: &'static str = crate::dsp::granular::PARAMS
            .iter()
            .find(|p| p.name == n)
            .unwrap()
            .name;
        named.push((name, Ctl::Const(v)));
    }
    let g = spec(EffectKind::Granulate, &named).unwrap();
    rig.install_bus(&bus_def(0, vec![g]), true);
    // Source: white noise times a level cell.
    let src = InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: Box::new([UGenSpec::WhiteNoise, UGenSpec::Mul]),
        edges: Box::new([Edge {
            from: 0,
            to: 1,
            port: 0,
        }]),
        node_params: Box::new([(
            1,
            CtlId::new(crate::dsp::effects::EFFECT_PARAM_BASE + 1),
            Ctl::Cell(INPUT_GAIN),
        )]),
    };
    rig.install(&src);
    rig.cells.set(INPUT_GAIN, 1.0);
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::LEGATO, 60.0)]));
    rig
}

#[test]
fn live_grains_never_read_overwritten_samples_across_wraps() {
    let mut rig = live(
        caps(),
        &[
            ("density", 100.0),
            ("size", 0.2),
            ("position", 1.0),
            ("pitch", 7.0),
            ("pitch-spray", 0.5),
            ("reverse", 0.3),
            ("spray", 1.0),
        ],
    );
    // 3 s through a 1 s capture buffer: it wraps twice.
    let (l, _) = rig.run(1_125);
    let s = stats(&rig);
    assert_eq!(s.stale_reads, 0, "no grain read a stale sample");
    assert!(s.grains_spawned > 200);
    assert!(rms(&l[l.len() / 2..]) > 0.0);
}

#[test]
fn freeze_holds_the_buffer_while_input_continues_and_unfreeze_resumes() {
    let mut rig = live(
        caps(),
        &[("density", 80.0), ("size", 0.05), ("position", 0.5)],
    );
    let _ = rig.run(400); // ~1 s: the buffer is full
    rig.cells.set(FREEZE, 1.0);
    let _ = rig.run(4);
    // The input goes silent; frozen grains still read the held audio.
    rig.cells.set(INPUT_GAIN, 0.0);
    let (frozen, _) = rig.run(400);
    assert!(
        rms(&frozen[frozen.len() / 2..]) > 0.01,
        "frozen output continues"
    );
    // Unfreeze: capture resumes (silence), grains soon read silence.
    rig.cells.set(FREEZE, 0.0);
    let (after, _) = rig.run(800);
    assert!(
        rms(&after[after.len() - 4_800..]) < 1.0e-6,
        "live again: silent input"
    );
    assert_eq!(stats(&rig).stale_reads, 0);
}

#[test]
fn unfreeze_short_gates_only_violating_grains() {
    // Reversed half-speed grains: frozen, they drift toward the oldest
    // region; live, the head chases them. From the newest region they stay
    // valid; from the oldest the resumed capture would overwrite them.
    let long = [
        ("density", 40.0),
        ("size", 0.25),
        ("reverse", 1.0),
        ("pitch", -12.0),
    ];
    let run = |position: f32| {
        let mut p = long.to_vec();
        p.push(("position", position));
        let mut rig = live(caps(), &p);
        let _ = rig.run(400);
        rig.cells.set(FREEZE, 1.0);
        let _ = rig.run(40);
        rig.cells.set(FREEZE, 0.0);
        let _ = rig.run(40);
        stats(&rig)
    };
    let newest = run(0.0);
    assert_eq!(
        newest.grains_gated, 0,
        "grains near the head pass the live rule"
    );
    let oldest = run(1.0);
    assert!(
        oldest.grains_gated > 0,
        "grains in the oldest region are gated"
    );
    assert_eq!(oldest.stale_reads, 0);
}

#[test]
fn partial_fill_freeze_exposes_only_captured_audio() {
    let mut rig = live(
        caps(),
        &[
            ("density", 100.0),
            ("size", 0.02),
            ("position", 1.0),
            ("spray", 1.0),
        ],
    );
    let _ = rig.run(40); // ~0.1 s of a 1 s buffer
    rig.cells.set(FREEZE, 1.0);
    let (l, _) = rig.run(200);
    let s = stats(&rig);
    assert_eq!(
        s.stale_reads, 0,
        "nothing beyond the captured extent is read"
    );
    assert!(s.grains_spawned > 0);
    assert!(rms(&l) > 0.0);
}

#[test]
fn freeze_right_after_install_spawns_nothing() {
    let mut rig = live(caps(), &[("density", 100.0), ("size", 0.02)]);
    rig.cells.set(FREEZE, 1.0);
    let (l, _) = rig.run(100);
    let s = stats(&rig);
    assert_eq!(s.grains_spawned, 0);
    assert!(s.grains_skipped > 0, "every onset is skipped and counted");
    assert!(
        l.iter().all(|v| *v == 0.0),
        "uncaptured storage is never heard"
    );
}

#[test]
fn capture_depth_beyond_the_cap_is_clamped_and_counted() {
    let tiny = CapabilitySet {
        max_capture_seconds: 0.05,
        ..caps()
    };
    let mut rig = live(tiny, &[("density", 50.0), ("size", 0.25)]);
    let (l, _) = rig.run(200);
    let s = stats(&rig);
    assert!(s.clamped > 0);
    assert!(s.grains_spawned > 0, "audio continues");
    assert!(rms(&l[l.len() / 2..]) > 0.0);
    assert_eq!(s.stale_reads, 0);
}

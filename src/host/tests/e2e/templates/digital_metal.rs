//! End-to-end `.vact` routing for the original programmable digital drum
//! kit's metal/cymbal and hat voices (design-music 4.1, DDRUM-001/004):
//! every declared control (every enum value and bool included) reaches
//! audio and editor metadata, and an unknown control or an out-of-domain
//! enum keyword is a diagnostic, not a silently ignored value.

use super::{all_finite, rms, E2e, HostManifest};

fn hit(template: &str, extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    let src = if extra.is_empty() {
        format!("s :{template} > note [:a3] > velocity 0.8 > once")
    } else {
        format!("s :{template} > note [:a3] > velocity 0.8 > {extra} > once")
    };
    e.eval(&src);
    let out = e.run_for(1.0);
    assert!(e.faults.is_empty(), "{template} {extra:?}: {:?}", e.faults);
    assert!(e.committed > 0, "{template} {extra:?}: event committed");
    assert!(all_finite(&out), "{template} {extra:?}: finite");
    assert!(rms(&out) > 1.0e-4, "{template} {extra:?}: audible");
    out
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

/// `(control name, baseline pattern controls, variant pattern controls)`,
/// the surface shared by `digital-metal` and `digital-hat` (DDRUM-001's
/// shared parameter table, minus the tonal-only pitch envelope and
/// `osc-mix`).
const SHARED_CHANGES: &[(&str, &str, &str)] = &[
    ("wave", "wave :sine", "wave :saw"),
    ("coarse", "coarse 0", "coarse 7"),
    ("fine", "fine 0", "fine 50"),
    ("amp-attack", "amp-attack 0.001", "amp-attack 0.2"),
    ("amp-decay", "amp-decay 0.3", "amp-decay 1.2"),
    ("amp-slope", "amp-slope 0.6", "amp-slope 0"),
    ("mod-wave", "mod-wave :sine", "mod-wave :saw"),
    ("mod-freq", "mod-freq 2", "mod-freq 5.3"),
    ("mod-level", "mod-level 0.5", "mod-level 1"),
    ("mod2-wave", "mod2-wave :sine", "mod2-wave :saw"),
    ("mod2-freq", "mod2-freq 3", "mod2-freq 6.1"),
    ("mod2-level", "mod2-level 0.3", "mod2-level 1"),
    ("fm-depth", "fm-depth 1", "fm-depth 6"),
    ("mod-decay", "mod-decay 0.15", "mod-decay 0.7"),
    (
        "transient-wave",
        "transient-wave :click",
        "transient-wave :sweep",
    ),
    (
        "transient-freq",
        "transient-freq 4000",
        "transient-freq 900",
    ),
    (
        "transient-level",
        "transient-level 0.3",
        "transient-level 0.9",
    ),
    ("filter-type", "filter-type :off", "filter-type :lp"),
    (
        "cutoff",
        "filter-type :lp > cutoff 1200",
        "filter-type :lp > cutoff 350",
    ),
    (
        "res",
        "filter-type :lp > res 0.3",
        "filter-type :lp > res 0.9",
    ),
    (
        "filter-drive",
        "filter-type :lp > filter-drive 0",
        "filter-type :lp > filter-drive 0.9",
    ),
    ("drive", "drive 0", "drive 0.85"),
    ("decimation", "decimation 0", "decimation 0.8"),
    ("volume-velocity", "volume-velocity 1", "volume-velocity 0"),
    (
        "velocity-depth",
        "velocity-target :drive > velocity-depth 0",
        "velocity-target :drive > velocity-depth 0.9",
    ),
    (
        "velocity-target",
        "velocity-depth 0.9 > velocity-target :none",
        "velocity-depth 0.9 > velocity-target :drive",
    ),
    (
        "lfo-wave",
        "lfo-target :amp > lfo-depth 0.9 > lfo-wave :sine",
        "lfo-target :amp > lfo-depth 0.9 > lfo-wave :square",
    ),
    (
        "lfo-rate",
        "lfo-target :amp > lfo-depth 0.9 > lfo-rate 4",
        "lfo-target :amp > lfo-depth 0.9 > lfo-rate 25",
    ),
    (
        "lfo-depth",
        "lfo-target :amp > lfo-depth 0",
        "lfo-target :amp > lfo-depth 0.9",
    ),
    (
        "lfo-offset",
        "lfo-target :amp > lfo-depth 0.9 > lfo-offset 0",
        "lfo-target :amp > lfo-depth 0.9 > lfo-offset 0.5",
    ),
    (
        "lfo-target",
        "lfo-depth 0.9 > lfo-target :none",
        "lfo-depth 0.9 > lfo-target :drive",
    ),
    (
        "lfo-retrigger",
        "lfo-target :amp > lfo-depth 0.9 > lfo-retrigger true",
        "lfo-target :amp > lfo-depth 0.9 > lfo-retrigger false",
    ),
    (
        "lfo-sync",
        "lfo-target :amp > lfo-depth 0.9 > lfo-rate 4 > lfo-sync false",
        "lfo-target :amp > lfo-depth 0.9 > lfo-rate 4 > lfo-sync true",
    ),
];

const METAL_CHANGES: &[(&str, &str, &str)] = &[
    ("mod-slope", "mod-slope 0.5", "mod-slope 0"),
    ("metal-spread", "metal-spread 0.3", "metal-spread 1"),
    (
        "repeat-count",
        "repeat-count 0",
        "repeat-count 2 > repeat-time 0.12",
    ),
    (
        "repeat-time",
        "repeat-count 2 > repeat-time 0.1",
        "repeat-count 2 > repeat-time 0.35",
    ),
];

const HAT_CHANGES: &[(&str, &str, &str)] = &[
    (
        "open-decay",
        "hat-open true > open-decay 0.6",
        "hat-open true > open-decay 2.5",
    ),
    (
        "closed-decay",
        "hat-open false > closed-decay 0.08",
        "hat-open false > closed-decay 0.5",
    ),
    ("hat-open", "hat-open false", "hat-open true"),
];

fn check_family(template: &str, extra_changes: &[(&str, &str, &str)]) {
    let editor = HostManifest::spec_default()
        .editor_decl(template)
        .unwrap_or_else(|| panic!("{template}: editor metadata"));
    for (name, base_extra, variant_extra) in SHARED_CHANGES.iter().chain(extra_changes) {
        assert!(
            editor.params.iter().any(|p| p.name == *name),
            "{template} {name}: editor"
        );
        let base = hit(template, base_extra);
        let variant = hit(template, variant_extra);
        let delta = difference(&base, &variant);
        assert!(delta > 1.0e-6, "{template} {name}: response {delta}");
    }
}

#[test]
fn every_declared_metal_control_reaches_audio_and_editor() {
    check_family("digital-metal", METAL_CHANGES);
}

#[test]
fn every_declared_hat_control_reaches_audio_and_editor() {
    check_family("digital-hat", HAT_CHANGES);
}

#[test]
fn unknown_control_and_out_of_domain_enum_are_diagnostics() {
    // `metal-ratio` is a real, checker-visible custom control name (it is
    // `feedback-metal-drum`'s), but `digital-metal` does not declare it: a
    // commit-time "unknown instrument control" failure, not a silently
    // ignored value. `pitch-decay` would NOT trigger this: it is a global
    // control-table row (the tonal family's), and an unwired row control
    // is silently ignored, not a diagnostic (`dsp::controls`'s module doc).
    let mut e = E2e::new();
    e.eval("s :digital-metal > note [:a3] > metal-ratio 3 > once");
    e.run_for(0.5);
    assert!(
        e.faults
            .iter()
            .any(|f| f.message.contains("unknown instrument control")),
        "{:?}",
        e.faults
    );

    let mut e2 = E2e::new();
    e2.eval("s :digital-hat > note [:a3] > filter-type :not-a-shape > once");
    e2.run_for(0.5);
    assert!(
        e2.faults
            .iter()
            .any(|f| f.message.contains("filter-type") || f.message.contains("expects")),
        "{:?}",
        e2.faults
    );
}

/// `velocity-target`'s domain past `:none` (design-music 4.1's inventory:
/// none, pitch, mod, cutoff, decay, transient, drive). Every entry routes
/// to a measurable difference: no target is a dead knob.
const VELOCITY_TARGETS: &[&str] = &["pitch", "mod", "cutoff", "decay", "transient", "drive"];

/// `lfo-target`'s domain past `:none` (design-music 4.1's inventory: none,
/// pitch, mod, cutoff, res, amp, drive, decimation, transient, decay; no
/// `pan`, since these mono cores have no stereo path to route to).
const LFO_TARGETS: &[&str] = &[
    "pitch",
    "mod",
    "cutoff",
    "res",
    "amp",
    "drive",
    "decimation",
    "transient",
    "decay",
];

#[test]
fn every_velocity_target_is_audible() {
    for template in ["digital-metal", "digital-hat"] {
        let base = hit(
            template,
            "filter-type :lp > velocity-target :none > velocity-depth 0.9",
        );
        for target in VELOCITY_TARGETS {
            let variant = hit(
                template,
                &format!("filter-type :lp > velocity-target :{target} > velocity-depth 0.9"),
            );
            let delta = difference(&base, &variant);
            assert!(
                delta > 1.0e-6,
                "{template} velocity-target :{target}: response {delta}"
            );
        }
    }
}

#[test]
fn every_lfo_target_is_audible() {
    for template in ["digital-metal", "digital-hat"] {
        let base = hit(
            template,
            "filter-type :lp > lfo-target :none > lfo-depth 0.9",
        );
        for target in LFO_TARGETS {
            let variant = hit(
                template,
                &format!("filter-type :lp > lfo-target :{target} > lfo-depth 0.9"),
            );
            let delta = difference(&base, &variant);
            assert!(
                delta > 1.0e-6,
                "{template} lfo-target :{target}: response {delta}"
            );
        }
    }
}

/// Every value differs from the default rendering of `template` with
/// `setup` applied first.
fn assert_each_value_differs_from_default(
    template: &str,
    setup: &str,
    control: &str,
    values: &[&str],
) {
    let base = hit(template, setup);
    for value in values {
        let extra = if setup.is_empty() {
            format!("{control} :{value}")
        } else {
            format!("{setup} > {control} :{value}")
        };
        let variant = hit(template, &extra);
        let delta = difference(&base, &variant);
        assert!(
            delta > 1.0e-6,
            "{template} {control} :{value}: indistinguishable from the default ({delta})"
        );
    }
}

/// Both bool states differ from each other, with `setup` applied first.
fn assert_bool_states_differ(template: &str, setup: &str, control: &str, default: bool) {
    let base = hit(template, &format!("{setup} > {control} {default}"));
    let flipped = !default;
    let variant = hit(template, &format!("{setup} > {control} {flipped}"));
    let delta = difference(&base, &variant);
    assert!(
        delta > 1.0e-6,
        "{template} {control} {flipped}: indistinguishable from the default ({delta})"
    );
}

#[test]
fn every_enum_and_bool_value_differs_from_the_default() {
    for template in ["digital-metal", "digital-hat"] {
        assert_each_value_differs_from_default(
            template,
            "",
            "wave",
            &["saw", "pulse", "square", "tri"],
        );
        assert_each_value_differs_from_default(
            template,
            "",
            "mod-wave",
            &["saw", "pulse", "square", "tri"],
        );
        assert_each_value_differs_from_default(
            template,
            "",
            "mod2-wave",
            &["saw", "pulse", "square", "tri"],
        );
        // digital-metal defaults to `:bp`, digital-hat to `:hp`: both are
        // audible non-off shapes, so `:off` and every other shape differ.
        let other_filters: &[&str] = if template == "digital-metal" {
            &["off", "lp", "hp", "notch"]
        } else {
            &["off", "lp", "bp", "notch"]
        };
        assert_each_value_differs_from_default(template, "", "filter-type", other_filters);
        assert_each_value_differs_from_default(
            template,
            "",
            "transient-wave",
            &["snap", "noise", "sweep"],
        );
        assert_each_value_differs_from_default(
            template,
            "lfo-target :amp > lfo-depth 0.9",
            "lfo-wave",
            &["tri", "saw", "ramp", "square", "random"],
        );
        assert_each_value_differs_from_default(
            template,
            "velocity-depth 0.9",
            "velocity-target",
            VELOCITY_TARGETS,
        );
        assert_each_value_differs_from_default(
            template,
            "lfo-depth 0.9",
            "lfo-target",
            LFO_TARGETS,
        );
        assert_bool_states_differ(
            template,
            "lfo-target :amp > lfo-depth 0.9",
            "lfo-retrigger",
            true,
        );
        assert_bool_states_differ(
            template,
            "lfo-target :amp > lfo-depth 0.9 > lfo-rate 4",
            "lfo-sync",
            false,
        );
    }
    assert_bool_states_differ(
        "digital-hat",
        "open-decay 3 > closed-decay 0.02",
        "hat-open",
        false,
    );
}

// Hi-hat choke (design-music 4.1: "Open/closed hat choke uses the existing
// `cut` group control. It is not a new knob."). Source inspection (2026-09-29,
// DDRUM-004) found `cut`'s group bits encoded into `AudioEvent::voice_hint`
// (`src/sched/commit.rs`) and carried across the wire (`src/host/wire.rs`)
// but never read back: `Engine::start`'s only steal path was the generic
// "pool full, steal the oldest tagged voice" fallback, unrelated to `cut`.
// DDRUM-004B (`src/dsp/engine.rs::choke_cut_group`, `src/dsp/voice.rs`)
// fixed this: a new voice with a nonzero cut group now short-gates
// (`Voice::short_gate`, the same 3 ms click-free fade the pool-exhaustion
// steal path already used) every other active voice sharing that group and
// its `voice_hint` orbit. The design documents the group but not its
// scope; same orbit and same cut group is what DDRUM-004B implements.

fn hat_source(open: bool, cut: bool) -> String {
    let tail = if open {
        "hat-open true > open-decay 3"
    } else {
        "hat-open false > closed-decay 0.02"
    };
    let cut = if cut { " > cut 1" } else { "" };
    format!("s :digital-hat > note [:a3] > velocity 0.8 > {tail}{cut} > once")
}

/// An open hat, then (after it has started ringing) a closed hat on the
/// same orbit; `cut` shares `cut 1` on both hits when `true`. Returns only
/// the audio rendered after the second hit, the open hat's would-be tail.
fn open_then_closed_tail(cut: bool) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&hat_source(true, cut));
    let _ = e.run_for(0.05);
    e.eval(&hat_source(false, cut));
    let tail = e.run_for(0.9);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(all_finite(&tail), "native tail finite");
    tail
}

/// The same two-hit sequence, replayed on a browser-tier `Engine` fed the
/// exact `digital-hat` `InstDef` a native `.vact` compile produced (the
/// `authored_browser` pattern of `src/host/tests/e2e/templates/stage_linked.rs`),
/// with the events built by hand from `src/dsp/controls.rs`'s `digital-hat`
/// rows (`freq` 0, `open-decay` 148, `closed-decay` 149, `hat-open` 150)
/// and `voice_hint` encoded exactly as `sched::commit` encodes `cut`.
fn open_then_closed_tail_browser(cut: bool) -> Vec<f32> {
    use crate::dsp::arena::{encode_inst, StoreKind};
    use crate::dsp::caps::CapabilitySet;
    use crate::dsp::cells::Mirror;
    use crate::dsp::engine::Engine;
    use crate::dsp::ring::{
        encode_graph_record, ByteInbox, EngineConfig, EngineIo, EventRing, SpscRing,
    };
    use crate::host::wire::{AudioEvent, Ctl};
    use crate::sched::slots::{CtlId, SlotId};
    use crate::value::intern::name_of_kw;

    const FREQ: CtlId = CtlId::new(0);
    const OPEN_DECAY: CtlId = CtlId::new(148);
    const CLOSED_DECAY: CtlId = CtlId::new(149);
    const HAT_OPEN: CtlId = CtlId::new(150);

    let def = E2e::new()
        .reg
        .borrow()
        .entries()
        .find(|entry| &*name_of_kw(entry.name) == "digital-hat")
        .expect("digital-hat is a prelude template")
        .def
        .clone();
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    let mut cfg = EngineConfig::new(
        &CapabilitySet::browser(),
        48_000.0,
        256,
        StoreKind::Arena { bytes: 4 << 20 },
    );
    cfg.template_slots = 8;
    cfg.bus_slots = 4;
    cfg.voice_seconds = 4.0;
    cfg.bus_seconds = 1.0;
    cfg.orbits = 2;
    cfg.orbit_delay_seconds = 0.5;
    cfg.analysis_cells = 1024;
    cfg.event_capacity = 256;
    let mut engine = Engine::with_config(cfg);
    let (mut event_tx, mut event_rx) = EventRing::split(256);
    let (mut ack_tx, _ack_rx) = SpscRing::split(256);
    let (mut garbage_tx, _garbage_rx) = SpscRing::split(32);
    let mut cells = Mirror::new(64);
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&record));
    let mut process = |engine: &mut Engine| {
        let mut buffer = [0.0; 512];
        let mut io = EngineIo {
            events: &mut event_rx,
            controls: &mut inbox,
            acks: &mut ack_tx,
            cells: &mut cells,
            garbage: Some(&mut garbage_tx),
        };
        let (_, allocations) =
            crate::dsp::alloc_probe::armed(|| engine.process(&mut io, &mut buffer, 256));
        assert_eq!(allocations, 0, "browser rendering allocated");
        buffer
    };
    for _ in 0..6 {
        let _ = process(&mut engine);
    }
    let hint = if cut { 1u32 << 8 } else { 0 };
    let mut open_ev = AudioEvent::new(engine.now(), SlotId::new(1), 1, def.id);
    open_ev.push_ctl(FREQ, Ctl::Const(4000.0)).unwrap();
    open_ev.push_ctl(HAT_OPEN, Ctl::Const(1.0)).unwrap();
    open_ev.push_ctl(OPEN_DECAY, Ctl::Const(3.0)).unwrap();
    open_ev.voice_hint = hint;
    assert!(event_tx.push(open_ev).is_ok());
    for _ in 0..10 {
        let _ = process(&mut engine);
    }
    let mut closed_ev = AudioEvent::new(engine.now(), SlotId::new(1), 1, def.id);
    closed_ev.push_ctl(FREQ, Ctl::Const(4000.0)).unwrap();
    closed_ev.push_ctl(HAT_OPEN, Ctl::Const(0.0)).unwrap();
    closed_ev.push_ctl(CLOSED_DECAY, Ctl::Const(0.02)).unwrap();
    closed_ev.voice_hint = hint;
    assert!(event_tx.push(closed_ev).is_ok());
    let mut tail = Vec::new();
    for _ in 0..170 {
        let block = process(&mut engine);
        tail.extend(block.iter().step_by(2));
    }
    tail
}

#[test]
fn cut_group_chokes_a_ringing_open_hat_and_without_cut_both_ring() {
    let choked = open_then_closed_tail(true);
    let free = open_then_closed_tail(false);
    assert!(rms(&choked) > 1.0e-4, "the closed hat itself is audible");
    assert!(
        rms(&free) > 2.0 * rms(&choked),
        "without `cut`, the still-ringing open hat keeps far more tail \
         energy than the choked version: free {} choked {}",
        rms(&free),
        rms(&choked)
    );
}

#[test]
fn cut_group_chokes_a_ringing_open_hat_on_the_browser_tier() {
    let choked = open_then_closed_tail_browser(true);
    let free = open_then_closed_tail_browser(false);
    assert!(all_finite(&choked) && all_finite(&free));
    assert!(rms(&choked) > 1.0e-4, "the closed hat itself is audible");
    assert!(
        rms(&free) > 2.0 * rms(&choked),
        "browser tier: without `cut`, the open hat keeps far more tail \
         energy than the choked version: free {} choked {}",
        rms(&free),
        rms(&choked)
    );
}

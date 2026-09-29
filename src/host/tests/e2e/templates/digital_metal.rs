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
// `cut` group control. It is not a new knob.") was investigated for a
// verifying e2e test here (a closed hat sharing a ringing open hat's `cut`
// value should stop it). Source inspection (`src/sched/commit.rs`'s
// `AudioEvent::voice_hint` construction, `src/host/wire.rs`, and every
// voice-allocation/steal path in `src/dsp/engine.rs`) found that `cut`'s
// group bits are encoded into `voice_hint` and sent across the wire but
// are never read back anywhere: `Engine::start`'s only steal path is the
// generic "pool full, steal the oldest tagged voice" fallback, unrelated
// to `cut`. Cut groups do not currently choke voices at all (any
// instrument, not just this family), so DDRUM-004 does not add a choke
// test or a choke mechanism here, per the plan's instruction to report
// this rather than inventing a replacement. `digital-hat`'s `hat-open`
// still audibly selects the tail via `open-decay`/`closed-decay`
// (`hat_open_and_closed_decays_differ_and_choose_the_base_tail`,
// `src/dsp/tests/dsp/digital_metal.rs`), so the two hat articulations are
// distinguishable even without a live choke.

//! End-to-end `.vact` routing for the original programmable digital drum
//! kit's tonal drum and snare voices (design-music 4.1, DDRUM-001/003):
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

/// `(control name, baseline pattern controls, variant pattern controls)`.
/// The baseline sets up whatever the control needs to not be dead (an
/// active filter for `cutoff`/`res`/`filter-drive`, a nonzero depth and a
/// real target for `velocity-depth`/`velocity-target`/`lfo-*`).
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
    ("fm-depth", "fm-depth 1", "fm-depth 5"),
    ("pitch-decay", "pitch-decay 0.05", "pitch-decay 0.4"),
    ("pitch-depth", "pitch-depth 12", "pitch-depth 40"),
    ("pitch-slope", "pitch-slope 0.5", "pitch-slope 0"),
    ("osc-mix", "osc-mix 0.5", "osc-mix 0.95"),
    ("mod-decay", "mod-decay 0.15", "mod-decay 0.7"),
    ("mod-slope", "mod-slope 0.5", "mod-slope 0"),
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

const SNARE_CHANGES: &[(&str, &str, &str)] = &[
    ("noise-freq", "noise-freq 2500", "noise-freq 900"),
    ("noise-mix", "noise-mix 0.3", "noise-mix 0.95"),
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
fn every_declared_drum_control_reaches_audio_and_editor() {
    check_family("digital-drum", &[]);
}

#[test]
fn every_declared_snare_control_reaches_audio_and_editor() {
    check_family("digital-snare", SNARE_CHANGES);
}

#[test]
fn unknown_control_and_out_of_domain_enum_are_diagnostics() {
    // `metal-ratio` is a real, checker-visible custom control name (it is
    // `feedback-metal-drum`'s), but `digital-drum` does not declare it: a
    // commit-time "unknown instrument control" failure, not a silently
    // ignored value.
    let mut e = E2e::new();
    e.eval("s :digital-drum > note [:a3] > metal-ratio 3 > once");
    e.run_for(0.5);
    assert!(
        e.faults
            .iter()
            .any(|f| f.message.contains("unknown instrument control")),
        "{:?}",
        e.faults
    );
}

/// `velocity-target`'s domain past `:none` (design-music 4.1's inventory:
/// none, pitch, mod, cutoff, decay, transient, drive). Every entry routes
/// to a measurable difference: no target is a dead knob.
const VELOCITY_TARGETS: &[&str] = &["pitch", "mod", "cutoff", "decay", "transient", "drive"];

/// `lfo-target`'s domain past `:none` (design-music 4.1's inventory: none,
/// pitch, mod, cutoff, res, amp, drive, decimation, transient, decay; no
/// `pan`, since this mono core has no stereo path to route to).
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
    for template in ["digital-drum", "digital-snare"] {
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
    for template in ["digital-drum", "digital-snare"] {
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
/// `setup` applied first (`setup` is empty, or a `>`-chained prefix that
/// gives the control-under-test somewhere to act: a real filter, a nonzero
/// depth, a real target).
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

/// Review follow-up: every enum and bool value of both templates must be
/// exhaustively distinguishable from the default, not merely one sampled
/// alternate (`check_family`, `every_velocity_target_is_audible`,
/// `every_lfo_target_is_audible` above sample one or iterate a target
/// list; this iterates the full remaining enum/bool domain, wave and
/// filter shapes included, and requires a genuine rate change for
/// `lfo-sync` at the default 120 bpm / 4-beat tempo (`cps = 0.5`, so
/// `lfo-rate 4` reads as 4 Hz unsynced and 2 Hz synced).
#[test]
fn every_enum_and_bool_value_differs_from_the_default() {
    for template in ["digital-drum", "digital-snare"] {
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
        // The default is `:lp` (not `:off`), so `cutoff`/`res` and their
        // velocity/LFO routes are live out of the box; test every other
        // shape, `:off` included.
        assert_each_value_differs_from_default(
            template,
            "",
            "filter-type",
            &["off", "bp", "hp", "notch"],
        );
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
}

#[test]
fn out_of_domain_enum_keyword_is_a_diagnostic() {
    let mut e2 = E2e::new();
    e2.eval("s :digital-drum > note [:a3] > filter-type :not-a-shape > once");
    e2.run_for(0.5);
    assert!(
        e2.faults
            .iter()
            .any(|f| f.message.contains("filter-type") || f.message.contains("expects")),
        "{:?}",
        e2.faults
    );
}

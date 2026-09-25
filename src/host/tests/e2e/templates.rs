//! Every prelude synthesis template, compiled from `src/prelude/templates.vact`
//! through a real `Evaluator` + `InstRegistry`, installed into a real
//! `Engine` and rendered through `Runtime` (design 12.4, 12.8.6, 12.8.12;
//! TASK-008 criterion 2), plus the `HostManifest` editor-metadata coverage
//! of design 13.5.
//!
//! Serial repair R2 fixed the BE-FINAL STOP finding (the `ports()`-vs-
//! runtime-catalog port drift in `src/dsp/build.rs`, and the unresolved
//! `bank`/`table`/`source` header default in `src/ns/insts.rs` /
//! `src/sched/commit.rs`): all seven templates now render audible output.

use std::sync::Arc;

use super::{all_finite, rms, E2e};
use crate::dsp::graph::EffectKind;
use crate::dsp::ugen::catalog::UGEN_NAMES;
use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::ns::insts::TEMPLATE_NAMES;
use crate::types::manifest::HostManifest;
use crate::value::intern::intern_kw;
use crate::vm::fail::{FailCode, Failure};

/// A deterministic, non-silent loader for the sample-, table- and
/// grain-source-backed templates (`sampler`, `wavetable`, `granular`): one
/// second of a 220 Hz sine at 48 kHz, mono, regardless of what resource is
/// asked for.
#[derive(Default)]
struct SynthLoader;

impl SampleLoader for SynthLoader {
    fn load(&mut self, _src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let n = 48_000;
        let frames: Vec<f32> = (0..n)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let t = i as f32 / 48_000.0;
                (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.8
            })
            .collect();
        Ok(Arc::new(SampleData {
            rate: 48_000,
            channels: 1,
            frames: frames.into_boxed_slice(),
        }))
    }
}

/// Each of the seven prelude templates compiles from source, registers
/// under its name, installs into the engine (a render commits at least one
/// event and reports no unexpected fault), and renders finite, non-silent,
/// zero-allocation audio. A persistent slot (not `once`) is used because
/// the resource-backed templates (`sampler`, `wavetable`, `granular`, R2b)
/// request their sample asynchronously: the first cycle or two may drop
/// with a transient `host-unavailable` ("still loading") fault while the
/// `SampleTable` gate opens (driven open here by ticking and rendering
/// through several cycles); a later cycle's event then renders for real.
#[test]
fn every_template_installs_and_renders_non_silent() {
    for name in TEMPLATE_NAMES {
        let mut e = E2e::with_loader(Box::new(SynthLoader));
        assert!(
            e.reg.borrow().id_of(intern_kw(name)).is_some(),
            "{name}: registered under its name"
        );
        e.eval(&format!("s :{name} > note [:c4] > d1"));
        // 4 cycles (8 s at the 120 bpm default): well past the gate's
        // one-render round trip, with several commit attempts after it.
        let left = e.run_for(8.0);
        for f in &e.faults {
            assert_eq!(
                f.code,
                FailCode::HostUnavailable,
                "{name}: unexpected fault {f:?}"
            );
        }
        assert!(all_finite(&left), "{name}: every sample finite");
        assert!(e.committed > 0, "{name}: installed (an event committed)");
        let level = rms(&left);
        assert!(level > 1.0e-3, "{name}: sounds (rms {level})");
    }
}

/// The mean absolute sample-to-sample difference: a cheap high-frequency
/// energy proxy (more high content moves adjacent samples further apart).
fn hf_energy(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let sum: f32 = samples.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
    #[allow(clippy::cast_precision_loss)]
    {
        sum / (samples.len() - 1) as f32
    }
}

/// `s :analog > note [:a4] > cutoff 100 > once` vs `cutoff 12000`: the
/// `inst` header parameter `cutoff` (a control, per serial repair R1's
/// `inst control` native) is an ordinary pattern control, so a low and a
/// high cutoff render measurably different output (the ladder filter
/// passes far less of `:a4`'s harmonic content at 100 Hz than at 12 kHz).
#[test]
fn a_template_parameter_is_an_ordinary_pattern_control() {
    let mut low = E2e::new();
    low.eval("s :analog > note [:a4] > cutoff 100 > once");
    let low_out = low.run_for(1.0);

    let mut high = E2e::new();
    high.eval("s :analog > note [:a4] > cutoff 12000 > once");
    let high_out = high.run_for(1.0);

    let low_hf = hf_energy(&low_out);
    let high_hf = hf_energy(&high_out);
    assert!(all_finite(&low_out) && all_finite(&high_out));
    assert!(
        rms(&low_out) > 1.0e-3 && rms(&high_out) > 1.0e-3,
        "both sound"
    );
    assert!(
        high_hf > low_hf * 3.0,
        "cutoff changes the output: low cutoff hf={low_hf}, high cutoff hf={high_hf}"
    );
}

/// `HostManifest::spec_default` declares an `EditorDecl` (design 13.5) for
/// every template, every effect and every ugen the catalog defines, and
/// `template_params` surfaces `cutoff` and `position`.
#[test]
fn every_builtin_has_editor_metadata_through_the_host_manifest() {
    let manifest = HostManifest::spec_default();
    for name in TEMPLATE_NAMES {
        assert!(manifest.editor_decl(name).is_some(), "{name}: editor decl");
    }
    for kind in EffectKind::ALL {
        let name = kind.name();
        assert!(manifest.editor_decl(name).is_some(), "{name}: editor decl");
    }
    for name in UGEN_NAMES {
        assert!(manifest.editor_decl(name).is_some(), "{name}: editor decl");
    }
    let params = manifest.template_params();
    assert!(params.contains(&"cutoff"), "template_params has `cutoff`");
    assert!(
        params.contains(&"position"),
        "template_params has `position`"
    );
}

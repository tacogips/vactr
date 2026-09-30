//! Kick-audio compression of persistent reverb tails through the real language/host.
use super::{rms, E2e, SR};

const RUMBLE: &str = include_str!("../../../../examples/rumble-kick.vact");
const INDUSTRIAL: &str = include_str!("../../../../examples/industrial-techno.vact");
const TECHNO: &str = include_str!("../../../../examples/techno-pattern.vact");

fn isolated_rumble(bypass: bool) -> Vec<f32> {
    // The key is pre-effect: muting its output preserves actual detector audio.
    let mut source = RUMBLE.replace("bus :kick:\n\tgain 0", "bus :kick:\n\tmute on: 1");
    source = source.replace("limiter ceiling: -1 release: 0.06", "gain 0");
    if bypass {
        source = source.replace("knee: 4 makeup: 0 mix: 1", "knee: 4 makeup: 0 mix: 0");
    }
    let mut rig = E2e::new();
    rig.eval(&source);
    let audio = rig.run_for(8.0);
    assert!(rig.faults.is_empty(), "{:?}", rig.faults);
    assert!(audio.iter().all(|sample| sample.is_finite()));
    audio
}
fn window_ratio(wet: &[f32], dry: &[f32], begin: f64, end: f64) -> f32 {
    let beat = 60.0 / 138.0;
    let mut wet_windows = Vec::new();
    let mut dry_windows = Vec::new();
    for kick in 8..16 {
        let onset = f64::from(kick) * beat;
        let start = ((onset + begin) * f64::from(SR)).round() as usize;
        let stop = ((onset + end) * f64::from(SR)).round() as usize;
        wet_windows.extend_from_slice(&wet[start..stop]);
        dry_windows.extend_from_slice(&dry[start..stop]);
    }
    rms(&wet_windows) / rms(&dry_windows)
}
#[test]
fn rumble_example_actual_kick_ducks_tail_and_recovers_between_beats() {
    let wet = isolated_rumble(false);
    let dry = isolated_rumble(true);
    assert!(rms(&dry) > 1e-4, "the reverb tail is audible");
    let early = window_ratio(&wet, &dry, 0.025, 0.075);
    let late = window_ratio(&wet, &dry, 0.34, 0.40);
    eprintln!(
        "rumble ducking RMS factors: onset={early:.4}, between-kicks={late:.4}, bypass-RMS={:.5}",
        rms(&dry)
    );
    assert!(early < 0.6, "kick onset should attenuate rumble: {early}");
    assert!(
        late > early * 1.3,
        "release restores rumble: {early} -> {late}"
    );
}
#[test]
fn industrial_and_preserved_techno_examples_render_finite_audible_stereo() {
    for (name, source) in [("industrial", INDUSTRIAL), ("techno", TECHNO)] {
        let mut rig = E2e::new();
        rig.eval(source);
        let (left, right) = rig.run_stereo_for(3.5);
        assert!(rig.faults.is_empty(), "{name}: {:?}", rig.faults);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()), "{name}");
        assert!(
            rms(&left) > 0.001 && rms(&right) > 0.001,
            "{name} is audible"
        );
    }
}

#[test]
fn industrial_example_pitched_bass_ducks_from_actual_kick_audio() {
    let render = |bypass| {
        let source = INDUSTRIAL
            .lines()
            .filter(|line| ![" > d2", " > d3", " > d5", " > d6", " > d7", " > d8"].iter().any(|suffix| line.ends_with(suffix)))
            .collect::<Vec<_>>()
            .join("\n")
            .replace("bus :kick:\n\tgain 0", "bus :kick:\n\tmute on: 1")
            .replace("compressor threshold: -10 ratio: 2 attack: 0.025 release: 0.12 knee: 6 makeup: 0\n\t\t> limiter ceiling: -1 release: 0.06", "gain 0");
        let source = if bypass {
            source.replace(
                "release: 0.09 knee: 4 makeup: 0 mix: 1",
                "release: 0.09 knee: 4 makeup: 0 mix: 0",
            )
        } else {
            source
        };
        let mut rig = E2e::new();
        rig.eval(&source);
        let audio = rig.run_for(8.0);
        assert!(rig.faults.is_empty(), "{:?}", rig.faults);
        audio
    };
    let wet = render(false);
    let dry = render(true);
    assert!(rms(&dry) > 0.001);
    let early = window_ratio(&wet, &dry, 0.025, 0.075);
    let late = window_ratio(&wet, &dry, 0.34, 0.40);
    eprintln!("industrial pitched-bass RMS factors: onset={early:.4}, between-kicks={late:.4}");
    assert!(early < 0.65, "pitched bass ducks at kick onset: {early}");
    assert!(
        late > early * 1.3,
        "pitched bass recovers: {early} -> {late}"
    );
}

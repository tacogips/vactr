//! Public six-stage chain controls, final parameter and editor contract.

use super::{all_finite, rms, E2e, HostManifest};

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :stage-chain-voice > note [:a3] > chain-count 6 > chain1-primary 0 > chain2-type 0 > chain2-loop 0 > chain2-primary 0.1 > chain3-primary 0 > chain4-primary 0.1 > chain5-primary 0 > chain6-type 3 > {extra} > once"));
    let audio = e.run_for(0.12);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

#[test]
fn last_segment_and_all_header_controls_are_codeable() {
    let base = render("chain6-primary 0.5 > chain6-secondary 0.1");
    let changed = render("chain6-primary 0.5 > chain6-secondary 0.9");
    assert!(rms(&base) > 1.0e-5);
    let change: f32 = base
        .iter()
        .zip(changed.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(change > 1.0e-3, "final segment secondary reaches audio");

    let mut e = E2e::new();
    let mut script = String::from(
        "s :stage-chain-voice > note [:a3] > chain-count 6 > chain-gate 1 > chain-trigger 0",
    );
    for index in 1..=6 {
        script.push_str(&format!(" > chain{index}-type 0 > chain{index}-loop 0 > chain{index}-primary 0.1 > chain{index}-secondary 0.5"));
    }
    script.push_str(" > once");
    e.eval(&script);
    let audio = e.run_for(0.1);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(all_finite(&audio));

    let editor = HostManifest::spec_default()
        .editor_decl("stage-chain-voice")
        .unwrap();
    for name in ["freq", "chain-count", "chain-gate", "chain-trigger", "amp"] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }
    for index in 1..=6 {
        for suffix in ["type", "loop", "primary", "secondary"] {
            let name = format!("chain{index}-{suffix}");
            assert!(editor.params.iter().any(|p| p.name == name), "{name}");
        }
    }
}

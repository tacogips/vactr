//! `digital-kit` (design-music 4.1, DDRUM-006): the prelude alias dict
//! (`bd`/`sd`/`cy`/`hh`) reaches the same four digital-drum-family
//! instruments as their direct template names, and `examples/digital-kit.vact`
//! keeps rendering audible, finite, allocation-free audio as the templates
//! evolve.

use super::{all_finite, rms, E2e};

/// Each alias renders bit-identical audio to `s :{template}` under the same
/// event, proving the alias reaches exactly that family's instrument (not
/// merely "some" audible instrument).
#[test]
fn each_digital_kit_alias_renders_identically_to_its_direct_template_name() {
    for (alias, template) in [
        ("bd", "digital-drum"),
        ("sd", "digital-snare"),
        ("cy", "digital-metal"),
        ("hh", "digital-hat"),
    ] {
        let mut by_alias = E2e::new();
        by_alias.eval(&format!(
            "s :{alias} kit: digital-kit > note [:a3] > velocity 0.8 > once"
        ));
        let alias_out = by_alias.run_for(1.0);
        assert!(by_alias.faults.is_empty(), "{alias}: {:?}", by_alias.faults);
        assert!(by_alias.committed > 0, "{alias}: event committed");
        assert!(all_finite(&alias_out), "{alias}: finite");
        assert!(rms(&alias_out) > 1.0e-4, "{alias}: audible");

        let mut by_name = E2e::new();
        by_name.eval(&format!("s :{template} > note [:a3] > velocity 0.8 > once"));
        let name_out = by_name.run_for(1.0);
        assert!(
            by_name.faults.is_empty(),
            "{template}: {:?}",
            by_name.faults
        );

        assert_eq!(
            alias_out, name_out,
            "`:{alias}` kit: digital-kit must render exactly `:{template}`'s family"
        );
    }
}

/// `examples/digital-kit.vact` evaluates cleanly and renders audible,
/// finite audio through every one of the four `digital-kit` aliases, a
/// pattern value, a per-event constant, a signal-driven parameter and a
/// cut-group hat choke (design-music 4.1's example usage).
#[test]
fn digital_kit_example_evaluates_and_renders_every_family() {
    let mut e = E2e::new();
    e.eval(include_str!("../../../../../examples/digital-kit.vact"));
    let out = e.run_for(4.0);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(e.committed > 0, "at least one event committed");
    assert!(all_finite(&out), "every sample finite");
    let level = rms(&out);
    assert!(level > 1.0e-4, "audible: rms {level}");
}

//! Registered Plaits templates must realize two-output graphs in the native
//! host, not merely appear in the static coverage inventory.

use super::E2e;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ported::plaits_algorithms;
use crate::dsp::ugen::{BuildEnv, Template};
use crate::value::intern::name_of_kw;

#[test]
fn reported_templates_realize_independent_main_and_aux_paths() {
    let e = E2e::new();
    let registry = e.reg.borrow();
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    for row in plaits_algorithms() {
        let Some(name) = row.vactrol_template else {
            continue;
        };
        let entry = registry
            .entries()
            .find(|entry| *name_of_kw(entry.name) == *name)
            .unwrap_or_else(|| panic!("{name}: realized prelude entry"));
        let graph = Template::from_inst(&entry.def, &env)
            .unwrap_or_else(|error| panic!("{name}: graph build {error:?}"));
        assert!(graph.has_aux, "{name}: distinct auxiliary path");
        assert_eq!((row.main_outputs, row.aux_outputs), (1, 1));
    }
}

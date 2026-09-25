//! The DSP natives in the VM (12.8.6, B2): nodes, the shared-name
//! dispatch, `osc "/addr"`, and the behavior with no instrument registry.

use crate::vm::natives::full_prelude;

use super::Sess;

fn sess() -> Sess {
    Sess::with_prelude(full_prelude())
}

#[test]
fn ugen_natives_build_nodes() {
    let mut s = sess();
    assert_eq!(s.show("sin-osc 440"), "<ugen>");
    assert_eq!(s.show("env-adsr 0.01 0.1 0.8 0.3"), "<ugen>");
    assert_eq!(s.show("* {sin-osc 440} 0.5"), "<ugen>");
    assert_eq!(s.show("- 1 {sin-osc 440}"), "<ugen>");
    assert_eq!(s.show("lpf {sin-osc 440} 800"), "<ugen>");
    assert_eq!(s.show("additive 220 partials: [1 0.5]"), "<ugen>");
    assert_eq!(s.show("+ {sin-osc 440} \"x\""), "fail: type");
    assert_eq!(s.show("sin-osc \"x\""), "fail: type");
}

#[test]
fn shared_names_keep_their_pattern_meaning() {
    let mut s = sess();
    assert_eq!(s.show("s :bd > lpf 800"), "<pattern>");
    assert_eq!(s.show("s :bd > gain 0.5 > room 0.3"), "<pattern>");
    assert_eq!(s.show("range sine 1 2"), "<pattern>");
    assert_eq!(s.show("saw"), "<signal>");
    assert_eq!(s.show("saw 440"), "fail: not-callable");
    assert_eq!(s.show("osc 20"), "<tex>");
    assert_eq!(s.show("osc \"/x\""), "(sound osc \"/x\")");
}

#[test]
fn the_body_flags_select_the_dsp_meaning() {
    let mut s = sess();
    s.vm.dsp.inst = 1;
    assert_eq!(s.show("saw 440"), "<ugen>");
    assert_eq!(s.show("lpf 800"), "<ugen>");
    assert_eq!(s.show("s :bd > lpf 800"), "<pattern>");
    s.vm.dsp.inst = 0;
    s.vm.dsp.bus = 1;
    assert_eq!(s.show("compressor ratio: 4"), "<ugen>");
    assert_eq!(s.show("room 0.3"), "<ugen>");
    assert_eq!(s.show("s :bd > room 0.3"), "<pattern>");
    s.vm.dsp.bus = 0;
    assert_eq!(s.show("compressor ratio: 4"), "fail: type");
}

#[test]
fn without_a_registry_an_inst_stays_a_closure_and_a_bus_fails() {
    let mut s = sess();
    assert_eq!(
        s.show("inst pluck freq amp = 0.5:\n\t+ amp 0\npluck 440"),
        "0.5"
    );
    assert_eq!(s.show("bus :x:\n\tplate"), "fail: host-unavailable");
    assert_eq!(s.show("master:\n\tlimiter"), "fail: host-unavailable");
}

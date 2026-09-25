//! The self-analysis surfaces from the REPL (design 14.5.9; issue item 4):
//! typed by the checker (no diagnostics) and callable through `run_repl`
//! on the recording host's synthetic taps; offline `render` on native caps,
//! `beyond-capability` on the browser tier.

use super::support::Rig;
use crate::dsp::caps::CapabilitySet;
use crate::session::repl::run_repl;
use crate::session::session::SessionConfig;

fn repl(rig: &mut Rig, input: &str) -> String {
    let clock = rig.clock.clone();
    let mut tick = || {
        clock.advance(0.01);
        clock.now()
    };
    let mut out = Vec::new();
    run_repl(&mut rig.s, input.as_bytes(), &mut out, &mut tick).expect("repl");
    String::from_utf8(out).expect("utf8")
}

/// The value printed for register `_n`.
fn register(t: &str, n: u32) -> &str {
    let tag = format!("_{n} = ");
    t.lines()
        .find_map(|l| l.strip_prefix(tag.as_str()))
        .unwrap_or_else(|| panic!("no `_{n}` in {t}"))
}

fn list_len(v: &str) -> usize {
    let inner = v.trim().trim_start_matches('[').trim_end_matches(']');
    inner.split_whitespace().count()
}

#[test]
fn scope_and_spectrum_of_the_master_tap() {
    let mut rig = Rig::new();
    let t = repl(&mut rig, "scope :master 16\nspectrum :master bins: 64\n");
    assert!(!t.contains("error[") && !t.contains("warning["), "{t}");
    assert_eq!(list_len(register(&t, 1)), 16, "{t}");
    assert_eq!(list_len(register(&t, 2)), 64, "{t}");
}

#[test]
fn render_then_rms_and_peak_print_floats_on_native_caps() {
    let mut rig = Rig::new();
    let t = repl(&mut rig, "s :analog > note [:a4] > d1\n");
    assert!(!t.contains("error["), "{t}");
    // The REPL ticked before the bind, so it activates at the next cycle
    // boundary (2 s); the render plays the ACTIVE bindings.
    rig.run_to(2.3);
    let t = repl(
        &mut rig,
        "let buf render 1\nrms buf\npeak buf\nscope buf 8\n",
    );
    assert!(!t.contains("error["), "{t}");
    for n in [3, 4] {
        let x: f64 = register(&t, n)
            .parse()
            .unwrap_or_else(|_| panic!("a float: {t}"));
        assert!(x.is_finite() && x >= 0.0, "{x}");
    }
    assert!(
        register(&t, 4).parse::<f64>().expect("peak") > 0.0,
        "the render is not silent: {t}"
    );
    assert_eq!(list_len(register(&t, 5)), 8, "{t}");
}

#[test]
fn render_on_the_browser_tier_is_beyond_capability_at_the_call() {
    let mut rig = Rig::with(SessionConfig::new(CapabilitySet::browser()));
    let t = repl(&mut rig, "render 1\n+ 1 1\n");
    assert!(t.contains("beyond-capability"), "{t}");
    assert!(t.contains("_1 = 2"), "the failed call bound nothing: {t}");
}

/// IR-S185-W5-BUSNAMES: the native host resolves a named-bus tap only when
/// it shares the session's instrument registry (`NativeAudioHost::
/// set_bus_names`, `SessionConfig::insts`). Without that wiring the same
/// tap is `host-unavailable` (the negative control below), which is what
/// production code did before `NativeHosts::open_with_bus_names` was
/// wired through `build_session_native` (14.5.9).
#[cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
#[test]
fn a_named_bus_scope_resolves_through_the_native_host_sharing_the_session_registry() {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::host::caps::Hosts;
    use crate::host::native::NativeAudioHost;
    use crate::ns::insts::InstRegistry;
    use crate::session::session::Session;

    const SR: u32 = 48_000;
    const INPUT: &str = "bus :drums:\n\tlimiter\nscope :drums 8\n";

    fn run(cfg: SessionConfig, hosts: Hosts) -> String {
        let mut s = Session::new(cfg, hosts);
        let clock = Cell::new(0.0);
        let mut tick = || {
            clock.set(clock.get() + 0.01);
            clock.get()
        };
        let mut out = Vec::new();
        run_repl(&mut s, INPUT.as_bytes(), &mut out, &mut tick).expect("repl");
        String::from_utf8(out).expect("utf8")
    }

    // Positive: the audio host's bus-names resolver is the session's own
    // instrument registry, so `scope :drums 8` resolves the named bus.
    let (mut audio, _side) = NativeAudioHost::headless(SR, CapabilitySet::native(), 64);
    let reg = InstRegistry::shared();
    audio.set_bus_names(Rc::new(Rc::clone(&reg)));
    let mut cfg = SessionConfig::new(CapabilitySet::native());
    cfg.insts = Some(reg);
    let hosts = Hosts {
        audio: Box::new(audio),
        ..Hosts::noop()
    };
    let t = run(cfg, hosts);
    assert!(!t.contains("error["), "{t}");
    // `bus :drums:` binds `_1` (the bus form's own value); `scope :drums 8`
    // binds `_2`.
    assert_eq!(list_len(register(&t, 2)), 8, "{t}");

    // Negative control: a fresh headless host with no shared registry
    // cannot resolve the named bus.
    let (audio, _side) = NativeAudioHost::headless(SR, CapabilitySet::native(), 64);
    let cfg = SessionConfig::new(CapabilitySet::native());
    let hosts = Hosts {
        audio: Box::new(audio),
        ..Hosts::noop()
    };
    let t = run(cfg, hosts);
    assert!(t.contains("host-unavailable"), "{t}");
}

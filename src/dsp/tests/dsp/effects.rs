//! Effect behaviors on the engine: `section` off bypasses its group
//! bit-identically; an effect ugen inside an instrument runs per voice;
//! convolution reads an installed impulse response; orbit delay sends.

use super::{bus_def, chain, ctl, event, rms, NativeRig};
use crate::dsp::effects::catalog::spec;
use crate::dsp::graph::{EffectKind, UGenSpec};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

fn noisy(rig: &mut NativeRig) {
    rig.install(&chain(1, vec![UGenSpec::WhiteNoise]));
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::LEGATO, 10.0), (ctl::PAN, 0.2)]));
}

#[test]
fn section_off_bypasses_its_group_bit_identically() {
    let mut plain = NativeRig::native();
    noisy(&mut plain);
    let (a, b) = plain.run(20);
    let mut rig = NativeRig::native();
    let section = spec(
        EffectKind::Section,
        &[("on", Ctl::Const(0.0)), ("count", Ctl::Const(2.0))],
    )
    .unwrap();
    let drive = spec(EffectKind::Saturate, &[("drive", Ctl::Const(24.0))]).unwrap();
    let crush = spec(EffectKind::Bitcrush, &[("bits", Ctl::Const(2.0))]).unwrap();
    rig.install_bus(&bus_def(0, vec![section, drive, crush]), true);
    noisy(&mut rig);
    let (c, d) = rig.run(20);
    assert_eq!(a, c);
    assert_eq!(b, d);
    // `on 1` processes the group.
    let mut on = NativeRig::native();
    let section = spec(EffectKind::Section, &[("on", Ctl::Const(1.0))]).unwrap();
    let crush = spec(EffectKind::Bitcrush, &[("bits", Ctl::Const(2.0))]).unwrap();
    on.install_bus(&bus_def(0, vec![section, crush]), true);
    noisy(&mut on);
    let (e, _) = on.run(20);
    assert_ne!(a, e);
}

#[test]
fn an_effect_ugen_runs_inside_the_voice() {
    let mut rig = NativeRig::native();
    let gain = spec(EffectKind::Gain, &[("gain", Ctl::Const(-6.0206))]).unwrap();
    rig.install(&chain(
        1,
        vec![UGenSpec::Const(1.0), UGenSpec::Effect(gain)],
    ));
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::PAN, 0.0), (ctl::LEGATO, 1.0)]));
    let (l, _) = rig.run(2);
    assert!((l[10] - 0.5).abs() < 1.0e-4, "{}", l[10]);
}

#[test]
fn convolution_reads_an_installed_impulse_response() {
    let mut rig = NativeRig::native();
    // A two-tap IR: identity plus an echo at 64 samples.
    let mut ir = vec![0.0; 128];
    ir[0] = 1.0;
    ir[64] = 0.5;
    rig.sample(20, ir, 1);
    let conv = spec(
        EffectKind::Convolution,
        &[("ir", Ctl::Const(20.0)), ("mix", Ctl::Const(1.0))],
    )
    .unwrap();
    rig.install_bus(&bus_def(0, vec![conv]), true);
    rig.install(&chain(1, vec![UGenSpec::Const(1.0)]));
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::PAN, 0.0), (ctl::LEGATO, 1.0)]));
    let (l, _) = rig.run(2);
    assert!((l[10] - 1.0).abs() < 1.0e-3, "direct tap {}", l[10]);
    assert!((l[100] - 1.5).abs() < 1.0e-3, "echo tap adds {}", l[100]);
}

#[test]
fn orbit_delay_sends_echo_into_master() {
    let mut rig = NativeRig::native();
    rig.install(&chain(1, vec![UGenSpec::WhiteNoise]));
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(
        1,
        t,
        &[
            (ctl::ATTACK, 0.0),
            (ctl::DECAY, 0.005),
            (ctl::RELEASE, 0.001),
            (CtlId::new(38), 0.8),
            (CtlId::new(39), 0.1),
            (CtlId::new(40), 0.3),
        ],
    ));
    let (l, _) = rig.run(80);
    // The dry burst ends after ~6 ms; the echo arrives at 100 ms.
    assert!(rms(&l[2_000..4_000]) < 1.0e-6, "silence between");
    assert!(rms(&l[4_800..5_200]) > 1.0e-3, "the echo");
}

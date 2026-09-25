//! Ugen kernels and template compilation: every `UGenSpec` renders finite
//! output in a voice; ports read voice controls by name (implicit control
//! names, B2); malformed graphs are refused with a reason; the FFT finds a
//! sine's bin.

use super::{chain, ctl, event, NativeRig};
use crate::dsp::effects::catalog::default_spec;
use crate::dsp::fft::Fft;
use crate::dsp::graph::{
    BankRef, Edge, EffectKind, GranSrc, InstDef, InstId, TableRef, UGenSpec, NODE_CAP,
};
use crate::dsp::ugen::{BuildError, Template};

fn every_spec() -> Vec<UGenSpec> {
    use UGenSpec as U;
    vec![
        U::SinOsc,
        U::Saw,
        U::Pulse,
        U::Tri,
        U::WhiteNoise,
        U::Lpf,
        U::Hpf,
        U::Bpf,
        U::Delay,
        U::Comb,
        U::EnvPerc,
        U::EnvAdsr,
        U::Line,
        U::SamplePlay(BankRef::new(1)),
        U::Mul,
        U::Add,
        U::Const(0.5),
        U::Param(ctl::AMP),
        U::Vco { unison_max: 16 },
        U::SubOsc,
        U::Ladder,
        U::Svf,
        U::FmOp,
        U::FmMod,
        U::PhaseDistortion,
        U::Additive { partials_max: 32 },
        U::Wavetable(TableRef::new(2)),
        U::Granular(GranSrc::Bus),
        U::Effect(default_spec(EffectKind::Chorus)),
    ]
}

#[test]
fn every_ugen_renders_finite_output_in_a_voice() {
    for spec in every_spec() {
        let mut rig = NativeRig::native();
        rig.sample(1, vec![0.5; 4_096], 1);
        rig.sample(2, vec![0.25; 4_096], 1);
        // Feed it from a saw so filters and effects have input.
        let node = crate::dsp::ugen::Node::from_spec(&spec);
        let def = if crate::dsp::ugen::catalog::port_count(&node) > 0 {
            chain(1, vec![UGenSpec::Saw, spec.clone()])
        } else {
            chain(1, vec![spec.clone()])
        };
        rig.install(&def);
        let _ = rig.step();
        let t = rig.engine.now();
        rig.send(event(1, t, &[(ctl::FREQ, 330.0), (ctl::LEGATO, 0.05)]));
        let (l, r) = rig.run(30);
        assert!(l.iter().chain(&r).all(|v| v.is_finite()), "{spec:?}");
    }
}

#[test]
fn unconnected_ports_read_voice_controls_by_name() {
    let env = NativeRig::native().engine.build_env();
    let t = Template::from_inst(&chain(1, vec![UGenSpec::SinOsc, UGenSpec::Lpf]), &env).unwrap();
    let ids: Vec<u16> = t.params().iter().map(|(c, _)| c.get()).collect();
    for want in [
        ctl::FREQ,
        crate::sched::slots::CtlId::new(13),
        crate::sched::slots::CtlId::new(14),
    ] {
        assert!(ids.contains(&want.get()), "freq, cutoff and res are read");
    }
    assert!(!t.reads_amp && !t.reads_pan);
}

#[test]
fn malformed_graphs_are_refused() {
    let env = NativeRig::native().engine.build_env();
    let mut cyc = chain(1, vec![UGenSpec::Add, UGenSpec::Add]);
    cyc.edges = Box::new([
        Edge {
            from: 0,
            to: 1,
            port: 0,
        },
        Edge {
            from: 1,
            to: 0,
            port: 0,
        },
    ]);
    assert_eq!(
        Template::from_inst(&cyc, &env).unwrap_err(),
        BuildError::Cycle
    );
    let mut bad = chain(1, vec![UGenSpec::SinOsc]);
    bad.edges = Box::new([Edge {
        from: 0,
        to: 5,
        port: 0,
    }]);
    assert_eq!(
        Template::from_inst(&bad, &env).unwrap_err(),
        BuildError::BadEdge
    );
    let mut port = chain(1, vec![UGenSpec::SinOsc, UGenSpec::SinOsc]);
    port.edges = Box::new([Edge {
        from: 0,
        to: 1,
        port: 7,
    }]);
    assert_eq!(
        Template::from_inst(&port, &env).unwrap_err(),
        BuildError::BadEdge
    );
    let big = InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::Const(0.0); NODE_CAP + 1].into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    };
    assert_eq!(
        Template::from_inst(&big, &env).unwrap_err(),
        BuildError::TooManyNodes
    );
    let fx = chain(1, vec![UGenSpec::Effect(default_spec(EffectKind::Gain)); 5]);
    assert_eq!(
        Template::from_inst(&fx, &env).unwrap_err(),
        BuildError::TooManyEffects
    );
    let empty = chain(1, vec![]);
    assert_eq!(
        Template::from_inst(&empty, &env).unwrap_err(),
        BuildError::Empty
    );
}

#[test]
fn the_fft_finds_a_sines_bin() {
    let fft = Fft::new(256);
    let mut re: Vec<f32> = (0..256)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let x = i as f32;
            (std::f32::consts::TAU * 8.0 * x / 256.0).sin()
        })
        .collect();
    let mut im = vec![0.0; 256];
    fft.forward(&mut re, &mut im);
    let mags: Vec<f32> = re
        .iter()
        .zip(&im)
        .map(|(a, b)| (a * a + b * b).sqrt())
        .collect();
    let peak = mags[..128]
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert_eq!(peak, 8);
    assert!((mags[8] - 128.0).abs() < 1.0e-2);
}

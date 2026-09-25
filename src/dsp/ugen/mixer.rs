//! The `*` and `+` mixers and the per-node kernel dispatch.

use crate::dsp::effects::{self};

use super::{additive, env, filter, fm, osc, sample, wavetable};
use super::{Inp, Kx, Node, NodeSpec, NodeState, MAX_PORTS};

/// Renders one non-effect node for a block (`out.len()` frames).
pub fn run(
    spec: &NodeSpec,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &mut Kx<'_>,
) {
    match spec.node {
        Node::SinOsc => osc::sine(ins, st, out, kx),
        Node::Saw => osc::saw(ins, st, out, kx),
        Node::Pulse => osc::pulse(ins, st, out, kx),
        Node::Tri => osc::tri(ins, st, out, kx),
        Node::SubOsc => osc::sub(ins, st, out, kx),
        Node::WhiteNoise => osc::noise(st, out, kx),
        Node::Vco { unison_max } => osc::vco(unison_max, ins, st, mem, out, kx),
        Node::Lpf => filter::biquad(effects::prim::Shape::Lowpass, ins, st, out, kx),
        Node::Hpf => filter::biquad(effects::prim::Shape::Highpass, ins, st, out, kx),
        Node::Bpf => filter::biquad(effects::prim::Shape::Bandpass, ins, st, out, kx),
        Node::Ladder => filter::ladder(ins, st, out, kx),
        Node::Svf => filter::svf(ins, st, out, kx),
        Node::Delay => filter::delay(ins, st, mem, out, kx),
        Node::Comb => filter::comb(ins, st, mem, out, kx),
        Node::EnvPerc => env::perc(ins, st, out, kx),
        Node::EnvAdsr => env::adsr(ins, st, out, kx),
        Node::Line => env::line(ins, st, out, kx),
        Node::SamplePlay(bank) => sample::play(bank, ins, st, out, kx),
        Node::Mul => {
            for (i, y) in out.iter_mut().enumerate() {
                *y = ins[0].at(i) * ins[1].at(i);
            }
        }
        Node::Add => {
            for (i, y) in out.iter_mut().enumerate() {
                *y = ins[0].at(i) + ins[1].at(i);
            }
        }
        Node::Const(v) => out.fill(v),
        Node::Param(_) => out.fill(ins[0].first()),
        Node::FmOp => fm::op(ins, st, out, kx),
        Node::FmMod => fm::modulate(ins, out),
        Node::PhaseDistortion => fm::phase_distortion(ins, st, out, kx),
        Node::Additive { partials_max } => additive::render(partials_max, ins, st, mem, out, kx),
        Node::Wavetable(t) => wavetable::render(t, ins, st, out, kx),
        Node::Granular(src) => crate::dsp::granular::ugen_process(src, ins, st, mem, out, kx),
        Node::Effect { .. } => out.fill(0.0),
    }
}

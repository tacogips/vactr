//! Adapts granular processing to engine resources.
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::{grain_limits, FxCtx, FxState};
use crate::dsp::graph::GranSrc;
use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};
pub use music_things_fx::granular::{GRAIN_FLOATS, PARAMS, SHORT_GATE};
use music_things_synth::KernelContext;
/// Grain slots for the tier.
pub fn pool_slots(caps: &CapabilitySet) -> usize {
    music_things_fx::granular::pool_slots(&grain_limits(caps))
}
/// Capture buffer length for the tier.
pub fn capture_len(sr: f32, caps: &CapabilitySet) -> usize {
    music_things_fx::granular::capture_len(sr, &grain_limits(caps))
}
/// Preferred ugen memory size.
pub fn ugen_mem_len(sr: f32, caps: &CapabilitySet) -> usize {
    music_things_fx::granular::ugen_mem_len(sr, &grain_limits(caps))
}
/// Preferred effect memory size.
pub fn effect_mem_len(sr: f32, caps: &CapabilitySet) -> usize {
    music_things_fx::granular::effect_mem_len(sr, &grain_limits(caps))
}
/// Initialize the effect.
pub fn effect_init(st: &mut FxState, mem: &mut [f32], sr: f32, caps: &CapabilitySet) {
    music_things_fx::granular::effect_init(st, mem, sr, &grain_limits(caps));
}
/// Process an effect block.
pub fn effect_process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    music_things_fx::granular::effect_process(p, st, mem, l, r, ctx);
}
/// Resolve and process a granular ugen block.
pub fn ugen_process(
    src: GranSrc,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &mut Kx<'_>,
) {
    let resource = match src {
        GranSrc::Sample(b) => kx.bank.unwrap_or(b.get()),
        GranSrc::Table(t) => t.get(),
        GranSrc::Bus => {
            out.fill(0.0);
            return;
        }
    };
    let Some(view) = kx.store.get(resource) else {
        out.fill(0.0);
        return;
    };
    let kc = kx.kernel_ctx();
    let limits = grain_limits(kx.caps);
    music_things_fx::granular::ugen_process(view, ins, st, mem, out, &kc, &limits, &mut *kx.stats);
}

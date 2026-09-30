//! Bass voice kernel scaffold from `design-docs/specs/design-bass-voices.md`.
//! DSP implementations are original Rust derived from published papers and
//! general knowledge; no emulation source code is consulted or translated.

pub mod diode;
pub mod fm;
pub mod ladder;
pub mod mods;
pub mod osc;

use super::{Inp, Kx, NodeState, MAX_PORTS};
use fm::{FmPair, Folder};
use ladder::Ladder;
use mods::{AmpEnv, AmpParams, FilterEnv, Glide, Lfo, LfoShape};
use osc::{Osc, Wave};

/// Number of ports in the shared bass voice contract.
pub const PORT_COUNT: usize = 32;

/// Port names and defaults. Keep this order stable for the catalog contract.
pub const PORTS: [(&str, f32); PORT_COUNT] = [
    ("freq", 55.0),
    ("mode", 0.0),
    ("cps", 0.5),
    ("onset-time", 0.0),
    ("wave", 0.0),
    ("cutoff", 800.0),
    ("res", 0.3),
    ("drive", 0.2),
    ("detune", 0.15),
    ("ratio", 1.0),
    ("index", 1.0),
    ("amp-attack", 0.002),
    ("amp-decay", 0.3),
    ("sustain", 1.0),
    ("release", 0.05),
    ("lfo-wave", 0.0),
    ("lfo-rate", 4.0),
    ("lfo-depth", 0.0),
    ("lfo-offset", 0.0),
    ("lfo-retrigger", 1.0),
    ("lfo-sync", 1.0),
    ("gate-length", 1.0),
    ("env-mod", 2.0),
    ("env-decay", 0.2),
    ("accent", 0.0),
    ("slide-from", 0.0),
    ("slide-time", 0.06),
    ("sub-level", 0.0),
    ("fm-feedback", 0.0),
    ("fold", 0.0),
    ("bit-depth", 16.0),
    ("click-level", 0.0),
];

/// Stable indices for the bass voice's named ports.
pub mod port {
    pub const FREQ: usize = 0;
    pub const MODE: usize = 1;
    pub const CPS: usize = 2;
    pub const ONSET_TIME: usize = 3;
    pub const WAVE: usize = 4;
    pub const CUTOFF: usize = 5;
    pub const RES: usize = 6;
    pub const DRIVE: usize = 7;
    pub const DETUNE: usize = 8;
    pub const RATIO: usize = 9;
    pub const INDEX: usize = 10;
    pub const AMP_ATTACK: usize = 11;
    pub const AMP_DECAY: usize = 12;
    pub const SUSTAIN: usize = 13;
    pub const RELEASE: usize = 14;
    pub const LFO_WAVE: usize = 15;
    pub const LFO_RATE: usize = 16;
    pub const LFO_DEPTH: usize = 17;
    pub const LFO_OFFSET: usize = 18;
    pub const LFO_RETRIGGER: usize = 19;
    pub const LFO_SYNC: usize = 20;
    pub const GATE_LENGTH: usize = 21;
    pub const ENV_MOD: usize = 22;
    pub const ENV_DECAY: usize = 23;
    pub const ACCENT: usize = 24;
    pub const SLIDE_FROM: usize = 25;
    pub const SLIDE_TIME: usize = 26;
    pub const SUB_LEVEL: usize = 27;
    pub const FM_FEEDBACK: usize = 28;
    pub const FOLD: usize = 29;
    pub const BIT_DEPTH: usize = 30;
    pub const CLICK_LEVEL: usize = 31;
}

/// Bass voice models selected by the `mode` port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Analog,
    Acid,
    Fm,
    Wobble,
    Sub,
    Reese,
}

impl Model {
    /// Converts the numeric mode port to a model, rounding and clamping it.
    pub fn from_port(value: f32) -> Self {
        if !value.is_finite() {
            return Self::Analog;
        }
        match value.round().clamp(0.0, 5.0) as u8 {
            0 => Self::Analog,
            1 => Self::Acid,
            2 => Self::Fm,
            3 => Self::Wobble,
            4 => Self::Sub,
            _ => Self::Reese,
        }
    }
}

/// Flushes non-finite and denormal values at the component boundary.
#[inline]
pub fn sanitize(value: f32) -> f32 {
    if !value.is_finite() || value.abs() < 1.0e-20 {
        0.0
    } else {
        value
    }
}

const FILTER: usize = 0;
const FILTER_FLOATS: usize = if ladder::Ladder::FLOATS > diode::Diode::FLOATS {
    ladder::Ladder::FLOATS
} else {
    diode::Diode::FLOATS
};
const OSC_A: usize = FILTER + FILTER_FLOATS;
const OSC_B: usize = OSC_A + osc::Osc::FLOATS;
const OSC_SUB: usize = OSC_B + osc::Osc::FLOATS;
const FM: usize = OSC_SUB + osc::Osc::FLOATS;
const FOLD: usize = FM + fm::FmPair::FLOATS;
const AMP: usize = FOLD + fm::Folder::FLOATS;
const FENV: usize = AMP + mods::AmpEnv::FLOATS;
const GLIDE: usize = FENV + mods::FilterEnv::FLOATS;
const LFO: usize = GLIDE + mods::Glide::FLOATS;
const ELAPSED: usize = LFO + mods::Lfo::FLOATS;
const GATE_LEFT: usize = ELAPSED + 1;

/// Number of floats in the complete per-voice state arena.
pub const STATE_FLOATS: usize = GATE_LEFT + 1;

/// Renders one mono bass voice using its fixed-size per-voice memory.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render_kernel(ins, st, mem, out, kx);
}
const INDEX_FLOOR: f32 = 0.35;
const MODEL_GAIN: [f32; 6] = [0.6, 0.12, 0.6, 0.6, 0.6, 0.6];
const LIMIT_KNEE: f32 = 0.8;

fn render_kernel(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if st.done() {
        out.fill(0.0);
        return;
    }
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    debug_assert!(mem.len() >= STATE_FLOATS);

    let sr = if kx.sr.is_finite() {
        kx.sr.max(1.0)
    } else {
        48_000.0
    };
    let model = Model::from_port(ins[port::MODE].first());
    let wave = Wave::from_index(ins[port::WAVE].first());
    let lfo_shape =
        LfoShape::from_index(ins[port::LFO_WAVE].first().round().clamp(0.0, 5.0) as usize);
    let cps = mods::clamp_cps(ins[port::CPS].first());
    let lfo_sync = ins[port::LFO_SYNC].first() > 0.5;
    let lfo_rate = mods::rate_hz(ins[port::LFO_RATE].first(), lfo_sync, cps);
    let seed = kx.seed ^ 0xBA55_0020;
    let accent = mods::accent(
        ins[port::ACCENT].first(),
        ins[port::RES].first(),
        ins[port::ENV_DECAY].first(),
    );
    let amp_params = AmpParams {
        attack: finite_clamp(ins[port::AMP_ATTACK].first(), 0.0, 10.0, 0.002),
        decay: finite_clamp(ins[port::AMP_DECAY].first(), 0.0, 30.0, 0.3),
        sustain: finite_clamp(ins[port::SUSTAIN].first(), 0.0, 1.0, 1.0),
        release: finite_clamp(ins[port::RELEASE].first(), 1.0e-4, 30.0, 0.05),
    };
    let slide_from = finite_clamp(ins[port::SLIDE_FROM].first(), -24.0, 24.0, 0.0);
    let legato = slide_from != 0.0;

    let mut filter_ladder = Ladder::load(&mem[FILTER..FILTER + ladder::Ladder::FLOATS]);
    let mut filter_diode = diode::Diode::load(&mem[FILTER..FILTER + diode::Diode::FLOATS]);
    let mut osc_a = Osc::load(&mem[OSC_A..OSC_A + Osc::FLOATS]);
    let mut osc_b = Osc::load(&mem[OSC_B..OSC_B + Osc::FLOATS]);
    let mut osc_sub = Osc::load(&mem[OSC_SUB..OSC_SUB + Osc::FLOATS]);
    let mut fm_pair = FmPair::load(&mem[FM..FM + FmPair::FLOATS]);
    let mut folder = Folder::load(&mem[FOLD..FOLD + Folder::FLOATS]);
    let mut amp = AmpEnv::load(&mem[AMP..AMP + AmpEnv::FLOATS]);
    let mut fenv = FilterEnv::load(&mem[FENV..FENV + FilterEnv::FLOATS]);
    let mut glide = Glide::load(&mem[GLIDE..GLIDE + Glide::FLOATS]);
    let mut lfo = Lfo::load(&mem[LFO..LFO + Lfo::FLOATS]);
    let mut elapsed = sanitize(mem[ELAPSED]).max(0.0);
    let mut gate_left = sanitize(mem[GATE_LEFT]).max(0.0);

    if st.u[1] == 0 {
        if matches!(model, Model::Wobble | Model::Reese) {
            osc_a = Osc::with_phase(seed_phase(seed, 0xA341_316C));
            osc_b = Osc::with_phase(seed_phase(seed, 0xC801_3EA4));
        }
        glide.start(slide_from);
        amp.start(legato);
        fenv.start(legato);
        let initial = mods::initial_phase(
            ins[port::LFO_RETRIGGER].first() > 0.5,
            ins[port::LFO_OFFSET].first(),
            lfo_rate,
            ins[port::ONSET_TIME].first(),
        );
        lfo.start(initial, lfo_shape, seed);
        gate_left = mods::gate_samples(ins[port::GATE_LENGTH].first(), cps, sr) as f32;
        st.s[0] = f32::from(legato);
        st.u[0] = seed;
        st.u[1] = 1;
    }
    let legato = st.s[0] > 0.5;
    let slide_time = finite_clamp(ins[port::SLIDE_TIME].first(), 1.0e-4, 10.0, 0.06);
    let cutoff = finite_clamp(ins[port::CUTOFF].first(), 20.0, 20_000.0, 800.0);
    let res = finite_clamp(ins[port::RES].first(), 0.0, 1.0, 0.3);
    let drive = finite_clamp(ins[port::DRIVE].first(), 0.0, 1.0, 0.2);
    let env_mod = finite_clamp(ins[port::ENV_MOD].first(), -8.0, 8.0, 2.0);
    let detune = finite_clamp(ins[port::DETUNE].first(), 0.0, 1.0, 0.15);
    let (r1, r2) = osc::detune_ratios(detune);
    let sub_level = finite_clamp(ins[port::SUB_LEVEL].first(), 0.0, 1.0, 0.0);
    let ratio = finite_clamp(ins[port::RATIO].first(), 0.0, 32.0, 1.0);
    let index = finite_clamp(ins[port::INDEX].first(), 0.0, 32.0, 1.0);
    let feedback = finite_clamp(ins[port::FM_FEEDBACK].first(), 0.0, 1.0, 0.0);
    let fold = finite_clamp(ins[port::FOLD].first(), 0.0, 1.0, 0.0);
    let bits = finite_clamp(ins[port::BIT_DEPTH].first(), 2.0, 16.0, 16.0);
    let click = finite_clamp(ins[port::CLICK_LEVEL].first(), 0.0, 1.0, 0.0);
    let depth = finite_clamp(ins[port::LFO_DEPTH].first(), -1.0, 1.0, 0.0);

    let mut rendered = out.len();
    for (i, sample) in out.iter_mut().enumerate() {
        if amp.done() {
            rendered = i;
            break;
        }
        if gate_left <= 0.0 {
            amp.release();
        }
        let freq = finite_clamp(ins[port::FREQ].at(i), 8.0, sr * 0.25, 55.0);
        let semis = glide.next(slide_time, sr)
            + osc::click_semis(elapsed / sr, if model == Model::Sub { click } else { 0.0 });
        let pitch = (freq * 2.0_f32.powf(semis / 12.0)).clamp(8.0, sr * 0.25);
        let inc = pitch / sr;
        let x = match model {
            Model::Analog => {
                osc_a.next(wave, inc) + sub_level * osc_sub.next(Wave::Square, inc * 0.5)
            }
            Model::Acid => osc_a.next(wave, inc),
            Model::Fm => {
                let fenv_value = fenv.value;
                let fm_index = index * (INDEX_FLOOR + (1.0 - INDEX_FLOOR) * fenv_value);
                let source = fm_pair.next(inc, ratio, fm_index, feedback);
                quantize_folder(&mut folder, source, fold, bits)
            }
            Model::Wobble | Model::Reese => {
                (osc_a.next(wave, inc * r1) + osc_b.next(wave, inc * r2))
                    * std::f32::consts::FRAC_1_SQRT_2
                    + sub_level * osc_sub.next(Wave::Square, inc * 0.5)
            }
            Model::Sub => osc_a.next(wave, inc),
        };
        let x = x * accent.gain;
        let lfo_u = lfo.next(lfo_shape, lfo_rate, seed, sr);
        let filter_env = fenv.next(accent.decay, sr);
        let fc = cutoff
            * 2.0_f32.powf(
                env_mod * filter_env + accent.oct * filter_env + mods::cutoff_octaves(depth, lfo_u),
            );
        let y = if model == Model::Acid {
            filter_diode.process(x, &diode::DiodeCoeffs::new(fc, res, drive, sr))
        } else {
            filter_ladder.process(x, &ladder::LadderCoeffs::new(fc, res, drive, sr))
        };
        let envelope = amp.next(&amp_params, legato && amp.stage == 0.0, sr);
        *sample = soft_limit(y * accent.gain * envelope * MODEL_GAIN[model_index(model)]);
        elapsed += 1.0;
        gate_left = (gate_left - 1.0).max(0.0);
    }

    if amp.done() {
        mem[..STATE_FLOATS].fill(0.0);
        out[rendered..].fill(0.0);
        st.finish();
        return;
    }
    if model == Model::Acid {
        filter_diode.flush();
        filter_diode.store(&mut mem[FILTER..FILTER + diode::Diode::FLOATS]);
    } else {
        filter_ladder.flush();
        filter_ladder.store(&mut mem[FILTER..FILTER + ladder::Ladder::FLOATS]);
    }
    osc_a.store(&mut mem[OSC_A..OSC_A + Osc::FLOATS]);
    osc_b.store(&mut mem[OSC_B..OSC_B + Osc::FLOATS]);
    osc_sub.store(&mut mem[OSC_SUB..OSC_SUB + Osc::FLOATS]);
    fm_pair.store(&mut mem[FM..FM + FmPair::FLOATS]);
    folder.store(&mut mem[FOLD..FOLD + Folder::FLOATS]);
    amp.store(&mut mem[AMP..AMP + AmpEnv::FLOATS]);
    fenv.store(&mut mem[FENV..FENV + FilterEnv::FLOATS]);
    glide.store(&mut mem[GLIDE..GLIDE + Glide::FLOATS]);
    lfo.store(&mut mem[LFO..LFO + Lfo::FLOATS]);
    mem[ELAPSED] = sanitize(elapsed);
    mem[GATE_LEFT] = sanitize(gate_left);
    for value in &mut mem[..STATE_FLOATS] {
        *value = sanitize(*value);
    }
}

#[inline]
fn quantize_folder(folder: &mut Folder, x: f32, fold: f32, bits: f32) -> f32 {
    fm::quantize(folder.process(x, fold), bits)
}

#[inline]
fn soft_limit(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    let magnitude = value.abs();
    if magnitude <= LIMIT_KNEE {
        value
    } else {
        value.signum()
            * (LIMIT_KNEE
                + (1.0 - LIMIT_KNEE) * ((magnitude - LIMIT_KNEE) / (1.0 - LIMIT_KNEE)).tanh())
    }
    .clamp(-1.0, 1.0)
}

#[inline]
fn finite_clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[inline]
fn model_index(model: Model) -> usize {
    match model {
        Model::Analog => 0,
        Model::Acid => 1,
        Model::Fm => 2,
        Model::Wobble => 3,
        Model::Sub => 4,
        Model::Reese => 5,
    }
}

#[inline]
fn seed_phase(seed: u32, salt: u32) -> f32 {
    let mut value = seed ^ salt;
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    (value >> 8) as f32 / 16_777_216.0
}

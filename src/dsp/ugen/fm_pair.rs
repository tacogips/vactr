//! Two-operator FM carrier/sub kernel with source-shaped control stages.
//!
//! The FM ratio anchors and four FIR coefficients are from the MIT-noticed
//! Plaits `resources/lookup_tables.py`; no aggregate resource or sine table
//! is imported. Analytic sine and floating-point phases remain adaptations.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

// Source's 23 scalar ratio anchors, expressed as semitones. At compile time
// each is tripled and the largest gap is bisected until the 128-step lookup
// has been filled, matching the source generator without a generated asset.
const RATIO_SEMITONES: [f32; 23] = [
    -12.0,
    -11.84,
    -6.0,
    -4.182_046_4,
    0.0,
    0.16,
    6.0,
    7.817_953_6,
    9.688_259,
    12.0,
    12.16,
    14.039_1,
    17.513_18,
    18.0,
    19.019_55,
    19.817_953,
    21.509_775,
    24.0,
    25.019_55,
    26.837_503,
    27.863_138,
    30.0,
    36.0,
];
const FIR: [f32; 4] = [0.024_424_15, 0.092_973_15, 0.167_129_38, 0.215_473_32];

const fn fm_quantizer() -> [f32; 130] {
    let mut table = [0.0; 130];
    let mut anchor = 0;
    while anchor < RATIO_SEMITONES.len() {
        let mut repeat = 0;
        while repeat < 3 {
            table[anchor * 3 + repeat] = RATIO_SEMITONES[anchor];
            repeat += 1;
        }
        anchor += 1;
    }
    let mut len = RATIO_SEMITONES.len() * 3;
    while len < 128 {
        let mut gap = 0;
        let mut widest = f32::NEG_INFINITY;
        let mut index = 0;
        while index + 1 < len {
            let width = table[index + 1] - table[index];
            if width > widest {
                widest = width;
                gap = index;
            }
            index += 1;
        }
        let mut shift = len;
        while shift > gap + 1 {
            table[shift] = table[shift - 1];
            shift -= 1;
        }
        table[gap + 1] = (table[gap] + table[gap + 2]) * 0.5;
        len += 1;
    }
    table[128] = table[127];
    table[129] = table[127];
    table
}

const QUANTIZER: [f32; 130] = fm_quantizer();

#[inline]
fn bounded(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

#[inline]
fn ratio_semitones(harmonics: f32) -> f32 {
    let index = harmonics * 128.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = index as usize;
    QUANTIZER[whole] + (QUANTIZER[whole + 1] - QUANTIZER[whole]) * index.fract()
}

#[inline]
fn fir_accumulate(head: &mut f32, tail: &mut f32, step: usize, sample: f32) {
    *head += sample * FIR[3 - step];
    *tail += sample * FIR[step];
}

/// Renders one source-shaped FM path. Port 4 chooses carrier or sub. The
/// template uses two independent node states, one for each output.
/// `s[0..8]` holds phases, prior sample and block-interpolated controls;
/// `bq[0].z1/z2` are the FIR heads and `b0/b1/b2/a1` hold four control
/// increments. `u[1]` counts down a 0.5 ms control period (24 frames at
/// 48 kHz), independent of callback partitioning.
/// No biquad operation is performed.
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    render_worker(ins, st, out, None, kx);
}

/// Renders carrier and sub outputs from one FM state advance.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    render_worker(ins, st, main, Some(aux), kx);
}

fn render_worker(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    mut aux_out: Option<&mut [f32]>,
    kx: &Kx<'_>,
) {
    if out.is_empty() {
        return;
    }
    let sr = kx.sr.max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let period = (24.0 * sr / 48_000.0).round().clamp(1.0, 192.0) as u32;
    let period_f = period as f32;
    if st.u[0] == 0 {
        st.u[0] = 1;
        // Source Init begins both oscillator increments at a0, which is
        // about 220.6 Hz after four phase advances at 48 kHz.
        st.s[4] = 220.0 * sr / 47_872.34;
        st.s[5] = st.s[4];
    }
    let mut carrier_phase = st.s[0];
    let mut mod_phase = st.s[1];
    let mut sub_phase = st.s[2];
    let mut previous = st.s[3];
    let mut carrier_hz = st.s[4];
    let mut mod_hz = st.s[5];
    let mut amount = st.s[6];
    let mut feedback = st.s[7];
    let mut carrier_head = st.bq[0].z1;
    let mut sub_head = st.bq[0].z2;
    let mut period_left = st.u[1];
    for (frame, output) in out.iter_mut().enumerate() {
        if period_left == 0 {
            let hz = bounded(ins[0].at(frame), 20.0, sr * 0.4, 220.0);
            let harmonics = bounded(ins[1].at(frame), 0.0, 1.0, 0.5);
            let timbre = bounded(ins[2].at(frame), 0.0, 1.0, 0.5);
            let morph = bounded(ins[3].at(frame), 0.0, 1.0, 0.5);
            let semitones = ratio_semitones(harmonics);
            let target_mod_hz = (hz * 2.0_f32.powf(semitones / 12.0)).min(sr * 2.0);
            let source_note = 45.0 + 12.0 * (hz / (440.0 * 48_000.0 / 47_872.34)).log2();
            let taming = (1.0 - (source_note + semitones - 72.0) * 0.025).clamp(0.0, 1.0);
            let target_amount = 2.0 * timbre * timbre * taming * taming;
            let target_feedback = morph * 2.0 - 1.0;
            st.bq[0].b0 = (hz - carrier_hz) / period_f;
            st.bq[0].b1 = (target_mod_hz - mod_hz) / period_f;
            st.bq[0].b2 = (target_amount - amount) / period_f;
            st.bq[0].a1 = (target_feedback - feedback) / period_f;
            period_left = period;
        }
        carrier_hz += st.bq[0].b0;
        mod_hz += st.bq[0].b1;
        amount += st.bq[0].b2;
        feedback += st.bq[0].a1;
        let phase_feedback = if feedback < 0.0 {
            0.5 * feedback * feedback
        } else {
            0.0
        };
        let sample_feedback = if feedback > 0.0 {
            0.25 * feedback * feedback
        } else {
            0.0
        };
        let mut carrier_tail = 0.0;
        let mut sub_tail = 0.0;
        for step in 0..4 {
            mod_phase =
                (mod_phase + mod_hz * (1.0 + previous * phase_feedback) / (sr * 4.0)).fract();
            carrier_phase = (carrier_phase + carrier_hz / (sr * 4.0)).fract();
            sub_phase = (sub_phase + carrier_hz / (sr * 8.0)).fract();
            let modulator = (TAU * (mod_phase + sample_feedback * previous)).sin();
            let carrier = (TAU * (carrier_phase + amount * modulator)).sin();
            let sub = (TAU * (sub_phase + amount * carrier * 0.25)).sin();
            previous += 0.05 * (carrier - previous);
            fir_accumulate(&mut carrier_head, &mut carrier_tail, step, carrier);
            fir_accumulate(&mut sub_head, &mut sub_tail, step, sub);
        }
        let auxiliary = ins[4].at(frame) >= 0.5;
        *output = if auxiliary { sub_head } else { carrier_head };
        if let Some(aux) = aux_out.as_deref_mut() {
            aux[frame] = if auxiliary { carrier_head } else { sub_head };
        }
        carrier_head = carrier_tail;
        sub_head = sub_tail;
        period_left -= 1;
    }
    st.s = [
        carrier_phase,
        mod_phase,
        sub_phase,
        previous,
        carrier_hz,
        mod_hz,
        amount,
        feedback,
    ];
    st.bq[0].z1 = carrier_head;
    st.bq[0].z2 = sub_head;
    st.u[1] = period_left;
}

#[cfg(test)]
mod tests {
    use super::{fir_accumulate, ratio_semitones, FIR, QUANTIZER};
    use crate::dsp::arena::SampleStore;
    use crate::dsp::caps::CapabilitySet;
    use crate::dsp::effects::FxStats;
    use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};

    fn signal(harmonics: f32, timbre: f32, morph: f32) -> [f32; 1024] {
        let store = SampleStore::default();
        let caps = CapabilitySet::native();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr: 48_000.0,
            gate: 1024,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        ins[0] = Inp::Val(440.0);
        ins[1] = Inp::Val(harmonics);
        ins[2] = Inp::Val(timbre);
        ins[3] = Inp::Val(morph);
        let mut state = NodeState::default();
        let mut out = [0.0; 1024];
        for block in out.chunks_exact_mut(64) {
            super::render(&ins, &mut state, block, &kx);
        }
        out
    }

    fn difference(a: &[f32], b: &[f32]) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let count = a.len() as f32;
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / count
    }

    fn partitioned_signal(sr: f32, block: usize, late_offset: usize, sub: bool) -> Vec<f32> {
        const FRAMES: usize = 2400;
        let store = SampleStore::default();
        let caps = CapabilitySet::native();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr,
            gate: FRAMES,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        let frequency: Vec<f32> = (0..FRAMES)
            .map(|i| if (i / 37) % 2 == 0 { 220.0 } else { 330.0 })
            .collect();
        let harmonics: Vec<f32> = (0..FRAMES)
            .map(|i| if (i / 53) % 2 == 0 { 0.1 } else { 0.9 })
            .collect();
        let timbre: Vec<f32> = (0..FRAMES)
            .map(|i| if (i / 71) % 2 == 0 { 0.3 } else { 0.8 })
            .collect();
        let morph: Vec<f32> = (0..FRAMES)
            .map(|i| if (i / 89) % 2 == 0 { 0.2 } else { 0.8 })
            .collect();
        let mut result = vec![0.0; FRAMES];
        let mut state = NodeState::default();
        let mut start = 0;
        let mut next_len = block - late_offset;
        while start < FRAMES {
            let end = (start + next_len).min(FRAMES);
            let mut inputs = [Inp::Val(0.0); MAX_PORTS];
            inputs[0] = Inp::Buf(&frequency[start..end]);
            inputs[1] = Inp::Buf(&harmonics[start..end]);
            inputs[2] = Inp::Buf(&timbre[start..end]);
            inputs[3] = Inp::Buf(&morph[start..end]);
            inputs[4] = Inp::Val(f32::from(sub));
            super::render(&inputs, &mut state, &mut result[start..end], &kx);
            start = end;
            next_len = block;
        }
        result
    }

    #[test]
    fn quantizer_has_full_source_ratio_span_and_continuous_interpolation() {
        assert_eq!(QUANTIZER.len(), 130);
        assert!((ratio_semitones(0.0) + 12.0).abs() < 1.0e-6);
        assert!((ratio_semitones(1.0) - 36.0).abs() < 1.0e-6);
        assert!((ratio_semitones(0.5) - 12.16).abs() < 1.0e-4);
        assert!(QUANTIZER.windows(2).all(|pair| pair[0] <= pair[1]));
        let midpoint = (QUANTIZER[20] + QUANTIZER[21]) * 0.5;
        assert!((ratio_semitones(20.5 / 128.0) - midpoint).abs() < 1.0e-6);
    }

    #[test]
    fn fir_carries_tail_into_the_next_host_sample() {
        let mut head = 0.0;
        let mut tail = 0.0;
        fir_accumulate(&mut head, &mut tail, 0, 1.0);
        assert_eq!(head, FIR[3]);
        assert_eq!(tail, FIR[0]);
        for step in 1..4 {
            fir_accumulate(&mut head, &mut tail, step, 0.0);
        }
        head = tail;
        assert_eq!(head, FIR[0]);
    }

    #[test]
    fn full_range_ratio_amount_and_both_feedback_polarities_change_audio() {
        let base = signal(0.5, 0.5, 0.5);
        assert!(base.iter().all(|x| x.is_finite()));
        for changed in [
            signal(0.0, 0.5, 0.5),
            signal(1.0, 0.5, 0.5),
            signal(0.5, 0.0, 0.5),
            signal(0.5, 1.0, 0.5),
            signal(0.5, 0.5, 0.0),
            signal(0.5, 0.5, 1.0),
        ] {
            assert!(changed.iter().all(|x| x.is_finite()));
            assert!(difference(&base, &changed) > 0.001);
        }
        assert!(difference(&signal(0.5, 0.5, 0.0), &signal(0.5, 0.5, 1.0)) > 0.001);
    }

    #[test]
    fn half_millisecond_control_clock_is_partition_and_late_start_invariant() {
        for (sr, period) in [(44_100.0, 22), (48_000.0, 24), (96_000.0, 48)] {
            assert_eq!((24.0_f32 * sr / 48_000.0).round() as usize, period);
            for sub in [false, true] {
                let reference = partitioned_signal(sr, period, 0, sub);
                for (block, late_offset) in [(64, 0), (256, 0), (64, 17), (256, 17), (37, 11)] {
                    let candidate = partitioned_signal(sr, block, late_offset, sub);
                    assert_eq!(
                        reference, candidate,
                        "{sr} Hz, {block}-frame block, late {late_offset}, sub {sub}"
                    );
                }
            }
        }
    }
}

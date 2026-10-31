//! Unpacked 155-parameter six-operator voice in the published DX7 voice-parameter layout.
//!
//! This module defines parameter layout and validation only; it contains no patch data.
//! MSFA cross-reference revision: `f67d41d313b7dc85f6fb99e79e515cc9d208cfff`.
//! Relevant MSFA paths: `app/src/main/jni/{dx7note.cc,dx7note.h,env.cc,env.h,exp2.cc,exp2.h,\
//! fm_core.cc,fm_core.h,fm_op_kernel.cc,fm_op_kernel.h,freqlut.cc,freqlut.h,patch.cc,patch.h,\
//! pitchenv.cc,pitchenv.h}`.

/// Number of unpacked parameters in one six-operator voice.
pub const VOICE_PARAMS: usize = 155;
/// Number of unpacked parameters for one operator.
pub const OP_PARAMS: usize = 21;
/// Maximum number of FM6 payloads in a graph node.
pub const MAX_FM6_PAYLOADS: usize = 4;
/// FM6 payload wire format version.
pub const WIRE_VERSION: u8 = 1;

/// Parameter offsets relative to an operator's base.
pub mod op {
    #[doc = "Parameter offset for r1."]
    pub const R1: usize = 0;
    #[doc = "Parameter offset for r2."]
    pub const R2: usize = 1;
    #[doc = "Parameter offset for r3."]
    pub const R3: usize = 2;
    #[doc = "Parameter offset for r4."]
    pub const R4: usize = 3;
    #[doc = "Parameter offset for l1."]
    pub const L1: usize = 4;
    #[doc = "Parameter offset for l2."]
    pub const L2: usize = 5;
    #[doc = "Parameter offset for l3."]
    pub const L3: usize = 6;
    #[doc = "Parameter offset for l4."]
    pub const L4: usize = 7;
    #[doc = "Parameter offset for break point."]
    pub const BREAK_POINT: usize = 8;
    #[doc = "Parameter offset for left depth."]
    pub const LEFT_DEPTH: usize = 9;
    #[doc = "Parameter offset for right depth."]
    pub const RIGHT_DEPTH: usize = 10;
    #[doc = "Parameter offset for left curve."]
    pub const LEFT_CURVE: usize = 11;
    #[doc = "Parameter offset for right curve."]
    pub const RIGHT_CURVE: usize = 12;
    #[doc = "Parameter offset for rate scaling."]
    pub const RATE_SCALING: usize = 13;
    #[doc = "Parameter offset for amp mod sens."]
    pub const AMP_MOD_SENS: usize = 14;
    #[doc = "Parameter offset for velocity sens."]
    pub const VELOCITY_SENS: usize = 15;
    #[doc = "Parameter offset for output level."]
    pub const OUTPUT_LEVEL: usize = 16;
    #[doc = "Parameter offset for osc mode."]
    pub const OSC_MODE: usize = 17;
    #[doc = "Parameter offset for coarse."]
    pub const COARSE: usize = 18;
    #[doc = "Parameter offset for fine."]
    pub const FINE: usize = 19;
    #[doc = "Parameter offset for detune."]
    pub const DETUNE: usize = 20;
}

/// Global parameter offsets in the unpacked voice.
pub mod global {
    #[doc = "Parameter offset for pitch r1."]
    pub const PITCH_R1: usize = 126;
    #[doc = "Parameter offset for pitch r2."]
    pub const PITCH_R2: usize = 127;
    #[doc = "Parameter offset for pitch r3."]
    pub const PITCH_R3: usize = 128;
    #[doc = "Parameter offset for pitch r4."]
    pub const PITCH_R4: usize = 129;
    #[doc = "Parameter offset for pitch l1."]
    pub const PITCH_L1: usize = 130;
    #[doc = "Parameter offset for pitch l2."]
    pub const PITCH_L2: usize = 131;
    #[doc = "Parameter offset for pitch l3."]
    pub const PITCH_L3: usize = 132;
    #[doc = "Parameter offset for pitch l4."]
    pub const PITCH_L4: usize = 133;
    #[doc = "Parameter offset for algorithm."]
    pub const ALGORITHM: usize = 134;
    #[doc = "Parameter offset for feedback."]
    pub const FEEDBACK: usize = 135;
    #[doc = "Parameter offset for osc key sync."]
    pub const OSC_KEY_SYNC: usize = 136;
    #[doc = "Parameter offset for lfo speed."]
    pub const LFO_SPEED: usize = 137;
    #[doc = "Parameter offset for lfo delay."]
    pub const LFO_DELAY: usize = 138;
    #[doc = "Parameter offset for lfo pitch depth."]
    pub const LFO_PITCH_DEPTH: usize = 139;
    #[doc = "Parameter offset for lfo amp depth."]
    pub const LFO_AMP_DEPTH: usize = 140;
    #[doc = "Parameter offset for lfo key sync."]
    pub const LFO_KEY_SYNC: usize = 141;
    #[doc = "Parameter offset for lfo wave."]
    pub const LFO_WAVE: usize = 142;
    #[doc = "Parameter offset for pitch mod sens."]
    pub const PITCH_MOD_SENS: usize = 143;
    #[doc = "Parameter offset for transpose."]
    pub const TRANSPOSE: usize = 144;
    #[doc = "Parameter offset for name."]
    pub const NAME: usize = 145;
}

/// Returns the base offset for operator numbers 1 through 6.
///
/// Operator 6 occupies the first parameter block and operator 1 the last. Invalid operator
/// numbers return `VOICE_PARAMS` so that the result cannot address a parameter.
#[must_use]
pub const fn op_base(operator: usize) -> usize {
    if operator >= 1 && operator <= 6 {
        (6 - operator) * OP_PARAMS
    } else {
        VOICE_PARAMS
    }
}

/// Returns the largest valid value for an unpacked parameter index.
///
/// Out-of-layout indexes have a maximum of zero.
#[must_use]
pub const fn max_of(index: usize) -> u8 {
    if index < 6 * OP_PARAMS {
        let field = index % OP_PARAMS;
        match field {
            op::R1..=op::R4
            | op::L1..=op::L4
            | op::BREAK_POINT
            | op::LEFT_DEPTH
            | op::RIGHT_DEPTH
            | op::OUTPUT_LEVEL
            | op::FINE => 99,
            op::LEFT_CURVE | op::RIGHT_CURVE => 3,
            op::RATE_SCALING | op::VELOCITY_SENS => 7,
            op::AMP_MOD_SENS => 3,
            op::OSC_MODE => 1,
            op::COARSE => 31,
            op::DETUNE => 14,
            _ => 0,
        }
    } else {
        match index {
            global::PITCH_R1..=global::PITCH_R4 | global::PITCH_L1..=global::PITCH_L4 => 99,
            global::ALGORITHM => 31,
            global::FEEDBACK => 7,
            global::OSC_KEY_SYNC | global::LFO_KEY_SYNC => 1,
            global::LFO_SPEED..=global::LFO_AMP_DEPTH => 99,
            global::LFO_WAVE => 5,
            global::PITCH_MOD_SENS => 7,
            global::TRANSPOSE => 48,
            global::NAME..=154 => 127,
            _ => 0,
        }
    }
}

/// A validated unpacked six-operator voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fm6Patch {
    #[doc = "Params value."]
    pub params: [u8; VOICE_PARAMS],
}

impl Fm6Patch {
    /// An all-zero patch, which is valid in the unpacked parameter format.
    pub const EMPTY: Self = Self {
        params: [0; VOICE_PARAMS],
    };

    /// Builds a patch from exactly 155 integral, finite values within each parameter's range.
    pub fn from_flat(values: &[f32]) -> Result<Self, String> {
        if values.len() != VOICE_PARAMS {
            return Err(format!(
                "patch: expected {VOICE_PARAMS} values, got {}",
                values.len()
            ));
        }

        let mut params = [0; VOICE_PARAMS];
        for (index, &value) in values.iter().enumerate() {
            let max = max_of(index);
            if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f32::from(max) {
                return Err(format!(
                    "patch: value {value} at index {index} must be an integer in 0..={max}"
                ));
            }
            params[index] = value as u8;
        }

        Ok(Self { params })
    }

    /// Returns the first parameter whose value exceeds its format-defined maximum.
    #[must_use = "handle the first invalid parameter index"]
    pub fn validate(&self) -> Result<(), usize> {
        self.params
            .iter()
            .enumerate()
            .find_map(|(index, &value)| (value > max_of(index)).then_some(index))
            .map_or(Ok(()), Err)
    }

    /// Returns the zero-based algorithm number, in the range 0 through 31.
    #[must_use]
    pub const fn algorithm(&self) -> u8 {
        self.params[global::ALGORITHM]
    }
}

#[cfg(test)]
mod tests {
    use super::{global, op, op_base, Fm6Patch, OP_PARAMS, VOICE_PARAMS};

    #[test]
    fn fm6_patch_layout_offsets() {
        assert_eq!(op_base(6), 0);
        assert_eq!(op_base(1), 105);
        assert_eq!(op_base(0), VOICE_PARAMS);
        assert_eq!(op_base(7), VOICE_PARAMS);
        assert_eq!(global::ALGORITHM, 134);
        assert_eq!(global::NAME, 145);
        assert_eq!(VOICE_PARAMS, global::NAME + 10);
        assert_eq!(OP_PARAMS, 21);
        assert_eq!(op::OUTPUT_LEVEL, 16);
    }

    #[test]
    fn fm6_patch_from_flat_accepts_valid() {
        let zeros = [0.0; VOICE_PARAMS];
        assert_eq!(Fm6Patch::from_flat(&zeros), Ok(Fm6Patch::EMPTY));

        let mut values = zeros;
        values[global::ALGORITHM] = 31.0;
        values[global::FEEDBACK] = 7.0;
        let patch = Fm6Patch::from_flat(&values);
        assert!(patch.is_ok());
        if let Ok(patch) = patch {
            assert_eq!(patch.algorithm(), 31);
            assert_eq!(patch.params[global::FEEDBACK], 7);
        }
    }

    #[test]
    fn fm6_patch_from_flat_rejects_invalid_values() {
        let short = [0.0; VOICE_PARAMS - 1];
        assert!(matches!(
            Fm6Patch::from_flat(&short),
            Err(error) if error.contains("expected 155")
        ));

        let mut values = [0.0; VOICE_PARAMS];
        values[op_base(1) + op::OUTPUT_LEVEL] = 100.0;
        assert!(matches!(
            Fm6Patch::from_flat(&values),
            Err(error) if error.contains("index 121") && error.contains("0..=99")
        ));

        values = [0.0; VOICE_PARAMS];
        values[0] = 1.5;
        assert!(Fm6Patch::from_flat(&values).is_err());
        values[0] = f32::NAN;
        assert!(Fm6Patch::from_flat(&values).is_err());
    }

    #[test]
    fn fm6_patch_validate_reports_first_bad_index() {
        let mut patch = Fm6Patch::EMPTY;
        patch.params[global::ALGORITHM] = 32;
        patch.params[global::FEEDBACK] = 8;
        assert_eq!(patch.validate(), Err(global::ALGORITHM));
    }
}

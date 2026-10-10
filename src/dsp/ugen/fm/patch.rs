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
    pub const R1: usize = 0;
    pub const R2: usize = 1;
    pub const R3: usize = 2;
    pub const R4: usize = 3;
    pub const L1: usize = 4;
    pub const L2: usize = 5;
    pub const L3: usize = 6;
    pub const L4: usize = 7;
    pub const BREAK_POINT: usize = 8;
    pub const LEFT_DEPTH: usize = 9;
    pub const RIGHT_DEPTH: usize = 10;
    pub const LEFT_CURVE: usize = 11;
    pub const RIGHT_CURVE: usize = 12;
    pub const RATE_SCALING: usize = 13;
    pub const AMP_MOD_SENS: usize = 14;
    pub const VELOCITY_SENS: usize = 15;
    pub const OUTPUT_LEVEL: usize = 16;
    pub const OSC_MODE: usize = 17;
    pub const COARSE: usize = 18;
    pub const FINE: usize = 19;
    pub const DETUNE: usize = 20;
}

/// Global parameter offsets in the unpacked voice.
pub mod global {
    pub const PITCH_R1: usize = 126;
    pub const PITCH_R2: usize = 127;
    pub const PITCH_R3: usize = 128;
    pub const PITCH_R4: usize = 129;
    pub const PITCH_L1: usize = 130;
    pub const PITCH_L2: usize = 131;
    pub const PITCH_L3: usize = 132;
    pub const PITCH_L4: usize = 133;
    pub const ALGORITHM: usize = 134;
    pub const FEEDBACK: usize = 135;
    pub const OSC_KEY_SYNC: usize = 136;
    pub const LFO_SPEED: usize = 137;
    pub const LFO_DELAY: usize = 138;
    pub const LFO_PITCH_DEPTH: usize = 139;
    pub const LFO_AMP_DEPTH: usize = 140;
    pub const LFO_KEY_SYNC: usize = 141;
    pub const LFO_WAVE: usize = 142;
    pub const PITCH_MOD_SENS: usize = 143;
    pub const TRANSPOSE: usize = 144;
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

//! Pure parser for user-supplied six-operator SysEx voice dumps.
//!
//! Single voice values are copied from the 155-byte payload and clamped to the
//! parameter ranges. Bulk voice fields are unpacked from the documented 128-byte
//! records and clamped the same way. Parsing allocates its result and belongs on
//! the evaluator side, never in an audio kernel.

use std::fmt;

use super::patch::{self, global, op, op_base, Fm6Patch};

/// Expected total size of a single-voice SysEx message.
pub const SINGLE_LEN: usize = 163;
/// Expected total size of a 32-voice bulk SysEx message.
pub const BULK_LEN: usize = 4104;
const HEADER_LEN: usize = 6;
const SINGLE_FORMAT: u8 = 0x00;
const BULK_FORMAT: u8 = 0x09;

/// A failure while validating or unpacking a six-operator SysEx dump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysexError {
    Empty,
    NotSysex,
    Manufacturer,
    SubStatus,
    Format,
    ByteCount,
    Length { expected: usize, actual: usize },
    DataByte { offset: usize, value: u8 },
    Checksum { expected: u8, actual: u8 },
    MissingEnd,
}

impl fmt::Display for SysexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Empty => f.write_str("SysEx input is empty"),
            Self::NotSysex => f.write_str("message does not start with 0xf0"),
            Self::Manufacturer => f.write_str("manufacturer byte is not 0x43"),
            Self::SubStatus => f.write_str("sub-status high nibble must be 0x0"),
            Self::Format => f.write_str("format byte must be 0x00 or 0x09"),
            Self::ByteCount => f.write_str("declared byte count does not match the format"),
            Self::Length { expected, actual } => {
                let description = if expected == BULK_LEN {
                    "a 32-voice bulk dump"
                } else {
                    "a single-voice dump"
                };
                write!(
                    f,
                    "expected {expected} bytes for {description}, found {actual}"
                )
            }
            Self::DataByte { offset, value } => write!(
                f,
                "data byte at offset {offset} is outside 7-bit range: 0x{value:02x}"
            ),
            Self::Checksum { expected, actual } => write!(
                f,
                "checksum mismatch: expected 0x{expected:02x}, found 0x{actual:02x}"
            ),
            Self::MissingEnd => f.write_str("message does not end with 0xf7"),
        }
    }
}

impl std::error::Error for SysexError {}

/// Parses a single-voice or 32-voice bulk DX7 SysEx message.
///
/// Validation stops at the first failure, in framing, header, byte count,
/// exact length, payload-byte, checksum, and end-marker order.
pub fn parse(bytes: &[u8]) -> Result<Vec<Fm6Patch>, SysexError> {
    if bytes.is_empty() {
        return Err(SysexError::Empty);
    }
    if bytes[0] != 0xf0 {
        return Err(SysexError::NotSysex);
    }
    if bytes.len() < 2 {
        return Err(length_error(SINGLE_LEN, bytes.len()));
    }
    if bytes[1] != 0x43 {
        return Err(SysexError::Manufacturer);
    }
    if bytes.len() < 3 {
        return Err(length_error(SINGLE_LEN, bytes.len()));
    }
    if bytes[2] & 0xf0 != 0 {
        return Err(SysexError::SubStatus);
    }
    if bytes.len() < 4 {
        return Err(length_error(SINGLE_LEN, bytes.len()));
    }

    let format = bytes[3];
    let (expected_len, expected_count) = match format {
        SINGLE_FORMAT => (SINGLE_LEN, (0x01, 0x1b)),
        BULK_FORMAT => (BULK_LEN, (0x20, 0x00)),
        _ => return Err(SysexError::Format),
    };
    if bytes.len() < 5 {
        return Err(length_error(expected_len, bytes.len()));
    }
    if bytes.len() < 6 {
        return Err(length_error(expected_len, bytes.len()));
    }
    if (bytes[4], bytes[5]) != expected_count {
        return Err(SysexError::ByteCount);
    }
    if bytes.len() != expected_len {
        return Err(length_error(expected_len, bytes.len()));
    }

    let payload_end = bytes.len() - 2;
    if let Some((offset, &value)) = bytes[HEADER_LEN..payload_end]
        .iter()
        .enumerate()
        .find(|(_, value)| **value > 0x7f)
    {
        return Err(SysexError::DataByte {
            offset: HEADER_LEN + offset,
            value,
        });
    }

    let expected_checksum = checksum(&bytes[HEADER_LEN..payload_end]);
    let actual_checksum = bytes[payload_end];
    if actual_checksum != expected_checksum {
        return Err(SysexError::Checksum {
            expected: expected_checksum,
            actual: actual_checksum,
        });
    }
    if bytes[bytes.len() - 1] != 0xf7 {
        return Err(SysexError::MissingEnd);
    }

    if format == SINGLE_FORMAT {
        let mut params = [0; patch::VOICE_PARAMS];
        for (index, &value) in bytes[HEADER_LEN..payload_end].iter().enumerate() {
            params[index] = value.min(patch::max_of(index));
        }
        return Ok(vec![Fm6Patch { params }]);
    }

    let mut voices = Vec::with_capacity(32);
    for voice in 0..32 {
        let record_start = HEADER_LEN + voice * 128;
        voices.push(unpack_bulk_voice(&bytes[record_start..record_start + 128]));
    }
    Ok(voices)
}

fn length_error(expected: usize, actual: usize) -> SysexError {
    SysexError::Length { expected, actual }
}

fn checksum(payload: &[u8]) -> u8 {
    let sum = payload.iter().map(|&value| u32::from(value)).sum::<u32>();
    ((128 - (sum % 128)) % 128) as u8
}

fn set_clamped(params: &mut [u8; patch::VOICE_PARAMS], index: usize, value: u8) {
    params[index] = value.min(patch::max_of(index));
}

fn unpack_bulk_voice(record: &[u8]) -> Fm6Patch {
    let mut params = [0; patch::VOICE_PARAMS];
    for packed_operator in 0..6 {
        let start = packed_operator * 17;
        let base = op_base(6 - packed_operator);
        for field in 0..11 {
            set_clamped(&mut params, base + field, record[start + field]);
        }
        let curves = record[start + 11];
        set_clamped(&mut params, base + op::LEFT_CURVE, curves & 0x03);
        set_clamped(&mut params, base + op::RIGHT_CURVE, (curves >> 2) & 0x03);
        let detune_scaling = record[start + 12];
        set_clamped(&mut params, base + op::RATE_SCALING, detune_scaling & 0x07);
        set_clamped(&mut params, base + op::DETUNE, (detune_scaling >> 3) & 0x0f);
        let sensitivity = record[start + 13];
        set_clamped(&mut params, base + op::AMP_MOD_SENS, sensitivity & 0x03);
        set_clamped(
            &mut params,
            base + op::VELOCITY_SENS,
            (sensitivity >> 2) & 0x07,
        );
        set_clamped(&mut params, base + op::OUTPUT_LEVEL, record[start + 14]);
        let mode_coarse = record[start + 15];
        set_clamped(&mut params, base + op::OSC_MODE, mode_coarse & 0x01);
        set_clamped(&mut params, base + op::COARSE, (mode_coarse >> 1) & 0x1f);
        set_clamped(&mut params, base + op::FINE, record[start + 16]);
    }

    for offset in 0..8 {
        set_clamped(&mut params, global::PITCH_R1 + offset, record[102 + offset]);
    }
    set_clamped(&mut params, global::ALGORITHM, record[110] & 0x1f);
    let feedback_sync = record[111];
    set_clamped(&mut params, global::FEEDBACK, feedback_sync & 0x07);
    set_clamped(
        &mut params,
        global::OSC_KEY_SYNC,
        (feedback_sync >> 3) & 0x01,
    );
    for offset in 0..4 {
        set_clamped(
            &mut params,
            global::LFO_SPEED + offset,
            record[112 + offset],
        );
    }
    let lfo = record[116];
    set_clamped(&mut params, global::LFO_KEY_SYNC, lfo & 0x01);
    set_clamped(&mut params, global::LFO_WAVE, (lfo >> 1) & 0x07);
    set_clamped(&mut params, global::PITCH_MOD_SENS, (lfo >> 4) & 0x07);
    for offset in 0..11 {
        set_clamped(
            &mut params,
            global::TRANSPOSE + offset,
            record[117 + offset],
        );
    }
    Fm6Patch { params }
}

#[cfg(test)]
pub(crate) fn encode_single(patch: &Fm6Patch) -> Vec<u8> {
    let payload = patch.params;
    encode_message(SINGLE_FORMAT, (0x01, 0x1b), &payload)
}

#[cfg(test)]
pub(crate) fn encode_bulk(patches: &[Fm6Patch; 32]) -> Vec<u8> {
    let mut payload = [0; 4096];
    for (voice, patch) in patches.iter().enumerate() {
        pack_bulk_voice(patch, &mut payload[voice * 128..(voice + 1) * 128]);
    }
    encode_message(BULK_FORMAT, (0x20, 0x00), &payload)
}

#[cfg(test)]
fn encode_message(format: u8, count: (u8, u8), payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(payload.len() + HEADER_LEN + 2);
    bytes.extend_from_slice(&[0xf0, 0x43, 0x00, format, count.0, count.1]);
    bytes.extend_from_slice(payload);
    bytes.push(checksum(payload));
    bytes.push(0xf7);
    bytes
}

#[cfg(test)]
fn pack_bulk_voice(patch: &Fm6Patch, record: &mut [u8]) {
    for packed_operator in 0..6 {
        let start = packed_operator * 17;
        let base = op_base(6 - packed_operator);
        record[start..start + 11].copy_from_slice(&patch.params[base..base + 11]);
        record[start + 11] =
            patch.params[base + op::LEFT_CURVE] | (patch.params[base + op::RIGHT_CURVE] << 2);
        record[start + 12] =
            patch.params[base + op::RATE_SCALING] | (patch.params[base + op::DETUNE] << 3);
        record[start + 13] =
            patch.params[base + op::AMP_MOD_SENS] | (patch.params[base + op::VELOCITY_SENS] << 2);
        record[start + 14] = patch.params[base + op::OUTPUT_LEVEL];
        record[start + 15] =
            patch.params[base + op::OSC_MODE] | (patch.params[base + op::COARSE] << 1);
        record[start + 16] = patch.params[base + op::FINE];
    }
    record[102..111].copy_from_slice(&patch.params[global::PITCH_R1..global::FEEDBACK]);
    record[111] = patch.params[global::FEEDBACK] | (patch.params[global::OSC_KEY_SYNC] << 3);
    record[112..116].copy_from_slice(&patch.params[global::LFO_SPEED..global::LFO_KEY_SYNC]);
    record[116] = patch.params[global::LFO_KEY_SYNC]
        | (patch.params[global::LFO_WAVE] << 1)
        | (patch.params[global::PITCH_MOD_SENS] << 4);
    record[117..128].copy_from_slice(&patch.params[global::TRANSPOSE..]);
}

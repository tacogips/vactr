use crate::dsp::ugen::fm::patch::{global, max_of, op, op_base, Fm6Patch, VOICE_PARAMS};
use crate::dsp::ugen::fm::sysex::{
    encode_bulk, encode_single, parse, SysexError, BULK_LEN, SINGLE_LEN,
};

fn synthetic_patch(seed: usize) -> Fm6Patch {
    let mut patch = Fm6Patch::EMPTY;
    for (index, value) in patch.params.iter_mut().enumerate() {
        *value = ((index * 7 + seed * 11) % (usize::from(max_of(index)) + 1)) as u8;
    }
    patch
}

fn synthetic_bank() -> [Fm6Patch; 32] {
    std::array::from_fn(synthetic_patch)
}

fn refresh_checksum(bytes: &mut [u8]) {
    let checksum_index = bytes.len() - 2;
    let sum = bytes[6..checksum_index]
        .iter()
        .map(|&value| u32::from(value))
        .sum::<u32>();
    bytes[checksum_index] = ((128 - (sum % 128)) % 128) as u8;
}

#[test]
fn fm6_sysex_single_round_trips() {
    let patch = synthetic_patch(4);
    let bytes = encode_single(&patch);
    assert_eq!(bytes.len(), SINGLE_LEN);
    assert_eq!(parse(&bytes), Ok(vec![patch]));
}

#[test]
fn fm6_sysex_bulk_round_trips() {
    let bank = synthetic_bank();
    let bytes = encode_bulk(&bank);
    assert_eq!(bytes.len(), BULK_LEN);
    assert_eq!(parse(&bytes), Ok(bank.to_vec()));
}

#[test]
fn fm6_sysex_device_nibble_ignored() {
    let mut bytes = encode_single(&synthetic_patch(2));
    bytes[2] = 0x05;
    assert!(parse(&bytes).is_ok());
}

#[test]
fn fm6_sysex_errors_in_order() {
    let single = encode_single(&synthetic_patch(3));
    let mut changed = single.clone();
    changed[0] = 0xf1;
    assert_eq!(parse(&changed), Err(SysexError::NotSysex));
    let mut changed = single.clone();
    changed[1] = 0x41;
    assert_eq!(parse(&changed), Err(SysexError::Manufacturer));
    let mut changed = single.clone();
    changed[2] = 0x10;
    assert_eq!(parse(&changed), Err(SysexError::SubStatus));
    let mut changed = single.clone();
    changed[3] = 0x02;
    assert_eq!(parse(&changed), Err(SysexError::Format));
    let mut changed = single.clone();
    changed[5] = 0x1c;
    assert_eq!(parse(&changed), Err(SysexError::ByteCount));
    let mut changed = single.clone();
    changed.push(0xf7);
    assert_eq!(
        parse(&changed),
        Err(SysexError::Length {
            expected: SINGLE_LEN,
            actual: SINGLE_LEN + 1,
        })
    );
    let mut changed = encode_bulk(&synthetic_bank());
    changed.pop();
    assert_eq!(
        parse(&changed),
        Err(SysexError::Length {
            expected: BULK_LEN,
            actual: BULK_LEN - 1,
        })
    );
    let mut changed = single.clone();
    changed[10] = 0x80;
    assert_eq!(
        parse(&changed),
        Err(SysexError::DataByte {
            offset: 10,
            value: 0x80,
        })
    );
    let mut changed = single.clone();
    changed[SINGLE_LEN - 2] = changed[SINGLE_LEN - 2].wrapping_add(1) & 0x7f;
    assert!(matches!(parse(&changed), Err(SysexError::Checksum { .. })));
    let mut changed = single;
    changed[SINGLE_LEN - 1] = 0x00;
    assert!(matches!(parse(&changed), Err(SysexError::MissingEnd)));
    assert_eq!(parse(&[]), Err(SysexError::Empty));
    assert_eq!(
        parse(&[0xf0]),
        Err(SysexError::Length {
            expected: SINGLE_LEN,
            actual: 1,
        })
    );
    assert_eq!(
        parse(&[0xf0, 0x43, 0x00, 0x09]),
        Err(SysexError::Length {
            expected: BULK_LEN,
            actual: 4,
        })
    );
}

#[test]
fn fm6_sysex_error_messages_are_readable() {
    let errors = [
        SysexError::Empty,
        SysexError::NotSysex,
        SysexError::Manufacturer,
        SysexError::SubStatus,
        SysexError::Format,
        SysexError::ByteCount,
        SysexError::Length {
            expected: 4104,
            actual: 4105,
        },
        SysexError::DataByte {
            offset: 10,
            value: 0x80,
        },
        SysexError::Checksum {
            expected: 0x2a,
            actual: 0x2b,
        },
        SysexError::MissingEnd,
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
    }
    assert!(errors[8].to_string().contains("checksum"));
    assert!(errors[8].to_string().contains("0x2a"));
    assert!(errors[8].to_string().contains("0x2b"));
    assert!(errors[6].to_string().contains("expected 4104 bytes"));
}

#[test]
fn fm6_sysex_bulk_clamps_out_of_range_fields() {
    let mut bytes = encode_bulk(&synthetic_bank());
    let voice_start = 6;
    let op1_record = voice_start + 5 * 17;
    bytes[op1_record + 12] = (15 << 3) | 7;
    bytes[op1_record + 14] = 120;
    bytes[voice_start + 116] = 7 << 1;
    bytes[voice_start + 117] = 127;
    refresh_checksum(&mut bytes);
    let parsed = parse(&bytes);
    assert!(parsed.is_ok());
    let Ok(parsed) = parsed else { return };
    assert_eq!(parsed[0].params[op_base(1) + op::DETUNE], 14);
    assert_eq!(parsed[0].params[op_base(1) + op::OUTPUT_LEVEL], 99);
    assert_eq!(parsed[0].params[global::LFO_WAVE], 5);
    assert_eq!(parsed[0].params[global::TRANSPOSE], 48);
}

#[test]
fn fm6_sysex_single_clamps_out_of_range() {
    let mut bytes = encode_single(&synthetic_patch(5));
    bytes[6 + op_base(1) + op::OUTPUT_LEVEL] = 120;
    refresh_checksum(&mut bytes);
    let parsed = parse(&bytes);
    assert!(parsed.is_ok());
    let Ok(parsed) = parsed else { return };
    assert_eq!(parsed[0].params[op_base(1) + op::OUTPUT_LEVEL], 99);
}

#[test]
fn fm6_sysex_bulk_layout_spot_check() {
    let mut bytes = encode_bulk(&synthetic_bank());
    let voice_start = 6 + 3 * 128;
    let op1_start = voice_start + 5 * 17;
    bytes[op1_start + 14] = 41;
    bytes[voice_start] = 37; // Operator 6 R1.
    bytes[voice_start + 110] = 0b0111_0011; // Algorithm uses the low five bits (19).
    bytes[voice_start + 111] = 0b0000_1101; // Feedback 5, key sync 1.
    bytes[voice_start + 116] = 0b0110_1111; // Key sync 1, LFO wave 7, PMS 6.
    bytes[voice_start + 117] = 32; // Transpose.
    refresh_checksum(&mut bytes);

    let parsed = parse(&bytes);
    assert!(parsed.is_ok());
    let Ok(parsed) = parsed else { return };
    let patch = &parsed[3];
    assert_eq!(patch.params[op_base(1) + op::OUTPUT_LEVEL], 41);
    assert_eq!(patch.params[op_base(6) + op::R1], 37);
    assert_eq!(patch.params[global::ALGORITHM], 19);
    assert_eq!(patch.params[global::FEEDBACK], 5);
    assert_eq!(patch.params[global::OSC_KEY_SYNC], 1);
    assert_eq!(patch.params[global::LFO_KEY_SYNC], 1);
    assert_eq!(patch.params[global::LFO_WAVE], 5);
    assert_eq!(patch.params[global::PITCH_MOD_SENS], 6);
    assert_eq!(patch.params[global::TRANSPOSE], 32);
    assert_eq!(patch.params.len(), VOICE_PARAMS);
}

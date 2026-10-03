//! Additive intent bytes only: these fixtures assert no DSP replacement success.
use vactr::host::wire::{AudioEvent, CtlMsg, WireError};
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;

fn encoded(command: SongCommand) -> Vec<u8> {
    let mut bytes = vec![0; CtlMsg::MAX_LEN];
    let n = CtlMsg::Song(command).encode(&mut bytes);
    assert_ne!(n, 0);
    bytes.truncate(n);
    bytes
}
fn replacement(epoch: u64, previous: u64, frame: u64, nonce: u64, count: u32) -> SongCommand {
    SongCommand::Replace(SongReplacement {
        activation: SongActivation {
            epoch: SnapshotEpoch(epoch),
            frame,
        },
        previous: SnapshotEpoch(previous),
        overlay_nonce: nonce,
        overlay_count: count,
    })
}
fn prefix(kind: u8, epoch: u64) -> Vec<u8> {
    let mut bytes = vec![0x1B, kind];
    bytes.extend_from_slice(&epoch.to_le_bytes());
    bytes
}
#[test]
fn replacement_and_initial_overlay_have_exact_full_width_byte_layouts() {
    for (epoch, previous) in [(0, u64::MAX), (u64::MAX, 0), (1, 2)] {
        for frame in [64, u64::from(u32::MAX) + 1, u64::MAX - 64] {
            for nonce in [1, u64::MAX] {
                for count in [0, u32::MAX] {
                    let command = replacement(epoch, previous, frame, nonce, count);
                    let mut expected = prefix(20, epoch);
                    expected.extend_from_slice(&frame.to_le_bytes());
                    expected.extend_from_slice(&previous.to_le_bytes());
                    expected.extend_from_slice(&nonce.to_le_bytes());
                    expected.extend_from_slice(&count.to_le_bytes());
                    assert_eq!(encoded(command), expected);
                    assert_eq!(expected.len(), 38);
                    assert_eq!(command.epoch(), SnapshotEpoch(epoch));
                    assert_eq!(CtlMsg::decode(&expected), Ok((CtlMsg::Song(command), 38)));
                }
            }
        }
        for muted in [false, true] {
            let command = SongCommand::PrimeMute(SongInitialMute {
                epoch: SnapshotEpoch(epoch),
                overlay_nonce: u64::MAX,
                instrument: u32::MAX,
                muted,
            });
            let mut expected = prefix(21, epoch);
            expected.extend_from_slice(&u64::MAX.to_le_bytes());
            expected.extend_from_slice(&u32::MAX.to_le_bytes());
            expected.push(u8::from(muted));
            assert_eq!(encoded(command), expected);
            assert_eq!(command.epoch(), SnapshotEpoch(epoch));
            assert_eq!(CtlMsg::decode(&expected), Ok((CtlMsg::Song(command), 23)));
        }
    }
}
#[test]
fn malformed_intents_refuse_native_and_byte_admission() {
    let mut out = vec![0; CtlMsg::MAX_LEN];
    for command in [
        replacement(1, 1, 64, 1, 0),
        replacement(1, 2, 63, 1, 0),
        replacement(1, 2, u64::MAX - 63, 1, 0),
        replacement(1, 2, 64, 0, 0),
        SongCommand::PrimeMute(SongInitialMute {
            epoch: SnapshotEpoch(1),
            overlay_nonce: 0,
            instrument: 0,
            muted: false,
        }),
    ] {
        assert!(!command.valid());
        assert_eq!(CtlMsg::Song(command).encode(&mut out), 0);
    }
    let valid = encoded(replacement(1, 2, 64, 1, 0));
    for (offset, bytes) in [
        (10, 63_u64.to_le_bytes()),
        (10, u64::MAX.to_le_bytes()),
        (18, 1_u64.to_le_bytes()),
        (26, 0_u64.to_le_bytes()),
    ] {
        let mut malformed = valid.clone();
        malformed[offset..offset + 8].copy_from_slice(&bytes);
        assert_eq!(CtlMsg::decode(&malformed), Err(WireError::BadValue));
    }
    let mut overlay = encoded(SongCommand::PrimeMute(SongInitialMute {
        epoch: SnapshotEpoch(0),
        overlay_nonce: 1,
        instrument: 0,
        muted: true,
    }));
    overlay[22] = 2;
    assert_eq!(CtlMsg::decode(&overlay), Err(WireError::BadValue));
    overlay[22] = 0;
    overlay[10..18].fill(0);
    assert_eq!(CtlMsg::decode(&overlay), Err(WireError::BadValue));
    let mut unknown = valid;
    unknown[1] = 22;
    assert_eq!(CtlMsg::decode(&unknown), Err(WireError::BadValue));
}
#[test]
fn new_records_preserve_prefix_consumption_and_refuse_all_short_buffers() {
    let commands = [
        replacement(0, 1, 64, 1, 0),
        SongCommand::PrimeMute(SongInitialMute {
            epoch: SnapshotEpoch(0),
            overlay_nonce: 1,
            instrument: 0,
            muted: false,
        }),
    ];
    for command in commands {
        let bytes = encoded(command);
        for n in 0..bytes.len() {
            assert!(CtlMsg::decode(&bytes[..n]).is_err(), "prefix length {n}");
            assert_eq!(CtlMsg::Song(command).encode(&mut vec![0; n]), 0);
        }
        let next = SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(2),
            frame: 0,
        });
        let mut stream = bytes.clone();
        stream.extend(encoded(next));
        let (first, consumed) = CtlMsg::decode(&stream).unwrap();
        assert_eq!(first, CtlMsg::Song(command));
        assert_eq!(consumed, bytes.len());
        assert_eq!(
            CtlMsg::decode(&stream[consumed..]),
            Ok((CtlMsg::Song(next), 18))
        );
    }
}
#[test]
fn original_twenty_command_tags_keep_golden_payloads() {
    // Golden payloads are independent of the song encoder. Nested AudioEvent
    // retains its own existing codec, rather than a new song-specific layout.
    let lengths = [
        0, 8, 13, 16, 24, 0, 60, 9, 43, 0, 64, 17, 13, 0, 9, 0, 11, 8, 55, 28,
    ];
    for (kind, len) in lengths.into_iter().enumerate() {
        let mut bytes = prefix(u8::try_from(kind).unwrap(), u64::MAX);
        let mut payload = vec![0; len];
        match kind {
            5 => {
                payload.resize(17, 0);
                payload[16] = 1;
                let event = AudioEvent::new(
                    0.0,
                    vactr::sched::slots::SlotId::new(0),
                    0,
                    vactr::dsp::graph::InstId::new(0),
                );
                let mut event_bytes = vec![0; AudioEvent::ENCODED_LEN];
                assert_eq!(event.encode(&mut event_bytes), event_bytes.len());
                payload.extend(event_bytes);
            }
            6 | 10 => payload[8..12].copy_from_slice(&48000_u32.to_le_bytes()),
            11 => payload[8] = 7,
            12 => payload[8] = 8,
            18 => {
                payload[4..8].copy_from_slice(&1_u32.to_le_bytes());
                payload[24] = 1; // Actual Some(private_fx), followed by its 8 bytes.
                payload[51..55].copy_from_slice(&1_u32.to_le_bytes());
            }
            19 => {
                payload[4..8].copy_from_slice(&1_u32.to_le_bytes());
                payload[8..12].copy_from_slice(&2_u32.to_le_bytes());
            }
            _ => {}
        }
        bytes.extend(payload);
        let (message, consumed) = CtlMsg::decode(&bytes).unwrap();
        assert_eq!(consumed, bytes.len(), "old tag {kind}");
        let CtlMsg::Song(command) = message else {
            panic!("song command");
        };
        assert!(!matches!(
            command,
            SongCommand::Replace(_) | SongCommand::PrimeMute(_)
        ));
        assert_eq!(command.epoch(), SnapshotEpoch(u64::MAX));
        assert_eq!(encoded(command), bytes, "old tag {kind}");
    }
}

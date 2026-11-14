//! Additive activation failure wire identity; no runtime emission is asserted.
use vactr::host::wire::{HostMsg, WireError};
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;

fn message(epoch: u64, frame: u64, reason: SongRejectCode) -> HostMsg {
    HostMsg::Song(SongHostAck::ActivationRejected {
        activation: SongActivation {
            epoch: SnapshotEpoch(epoch),
            frame,
        },
        reason,
    })
}
fn encoded(message: HostMsg) -> Vec<u8> {
    let mut bytes = vec![0; HostMsg::MAX_LEN];
    let n = message.encode(&mut bytes);
    assert_ne!(n, 0);
    bytes.truncate(n);
    bytes
}
#[test]
fn activation_rejection_roundtrips_full_requested_identity_and_reasons() {
    for epoch in [0, 1, u64::MAX] {
        for frame in [0, 1, u64::from(u32::MAX) + 1, u64::MAX] {
            for reason in [
                SongRejectCode::StaleEpoch,
                SongRejectCode::NotReady,
                SongRejectCode::Capacity,
                SongRejectCode::Malformed,
                SongRejectCode::HostFault,
            ] {
                let msg = message(epoch, frame, reason);
                let bytes = encoded(msg);
                let mut expected = vec![0x49, 14];
                expected.extend_from_slice(&epoch.to_le_bytes());
                expected.extend_from_slice(&frame.to_le_bytes());
                expected.push(reason as u8);
                assert_eq!(bytes, expected);
                assert_eq!(bytes.len(), 19);
                assert_eq!(HostMsg::decode(&bytes), Ok((msg, 19)));
                let HostMsg::Song(ack) = msg else {
                    panic!("song ACK");
                };
                assert_eq!(ack.epoch(), SnapshotEpoch(epoch));
            }
        }
    }
}

#[test]
fn old_song_ack_tags_and_payload_bytes_remain_unchanged() {
    let epoch = SnapshotEpoch(u64::MAX);
    let resource = SongResourceRef {
        id: 0,
        generation: 0,
    };
    let key = SongLeaseKey {
        epoch,
        resource,
        kind: SongResourceKind::Instrument,
    };
    let activation = SongActivation { epoch, frame: 0 };
    let request = SongClockRequest { epoch, request: 0 };
    let cases = [
        (SongHostAck::Ready(epoch), 0, 0),
        (SongHostAck::Applied(activation), 1, 8),
        (
            SongHostAck::Muted(SongMute {
                epoch,
                instrument: 0,
                muted: false,
                frame: 0,
            }),
            2,
            13,
        ),
        (
            SongHostAck::Rejected {
                epoch,
                reason: SongRejectCode::StaleEpoch,
            },
            3,
            1,
        ),
        (SongHostAck::ResourceReady { epoch, resource }, 4, 8),
        (SongHostAck::ResourceRetired { epoch, resource }, 5, 8),
        (SongHostAck::LeaseReturned(key), 6, 9),
        (SongHostAck::PreparationCancelled(epoch), 7, 0),
        (
            SongHostAck::SliceAccepted {
                lease: key,
                offset: 0,
            },
            8,
            13,
        ),
        (
            SongHostAck::CapacityReport(SongCapacityReport {
                epoch,
                serial: 0,
                available: SongHostCapacities::default(),
                analysis: SongAnalysisCapacity { slots: 0 },
            }),
            9,
            64,
        ),
        (
            SongHostAck::CapacityRejected {
                epoch,
                reason: SongRejectCode::StaleEpoch,
            },
            10,
            1,
        ),
        (
            SongHostAck::ClockReport(SongClockReport {
                request,
                clock: SongHostClock {
                    frame: 0,
                    sample_rate: 0,
                },
            }),
            11,
            20,
        ),
        (
            SongHostAck::ClockRejected(SongClockFailure {
                request,
                reason: SongRejectCode::StaleEpoch,
            }),
            12,
            9,
        ),
        (
            SongHostAck::BranchRebound(SongBranchRebound {
                epoch,
                branch: SongBranchId(0),
                generation: 0,
                frame: 0,
            }),
            13,
            16,
        ),
    ];
    for (ack, tag, payload_len) in cases {
        let msg = HostMsg::Song(ack);
        let bytes = encoded(msg);
        let mut expected = vec![0x49, tag];
        expected.extend_from_slice(&u64::MAX.to_le_bytes());
        expected.resize(10 + payload_len, 0);
        assert_eq!(bytes, expected, "old tag {tag}");
        assert_eq!(HostMsg::decode(&bytes), Ok((msg, expected.len())));
    }
}

#[test]
fn activation_codec_rejects_truncation_invalid_reason_and_short_output() {
    let msg = message(u64::MAX, u64::MAX, SongRejectCode::Capacity);
    let bytes = encoded(msg);
    for end in 0..bytes.len() {
        assert!(
            HostMsg::decode(&bytes[..end]).is_err(),
            "truncated at {end}"
        );
        let mut output = vec![0; end];
        assert_eq!(msg.encode(&mut output), 0, "short output {end}");
    }
    for invalid in [5, 14, 255] {
        let mut malformed = bytes.clone();
        malformed[18] = invalid;
        assert_eq!(HostMsg::decode(&malformed), Err(WireError::BadValue));
    }
    let mut unknown = bytes.clone();
    unknown[1] = 255;
    assert_eq!(HostMsg::decode(&unknown), Err(WireError::BadValue));
}

#[test]
fn activation_codec_preserves_exact_stream_consumption_and_trailing_records() {
    let first = message(7, u64::MAX, SongRejectCode::NotReady);
    let second = HostMsg::Song(SongHostAck::Applied(SongActivation {
        epoch: SnapshotEpoch(7),
        frame: 9,
    }));
    let mut bytes = encoded(first);
    bytes.extend_from_slice(&encoded(second));
    assert_eq!(HostMsg::decode(&bytes), Ok((first, 19)));
    assert_eq!(HostMsg::decode(&bytes[19..]), Ok((second, 18)));
    let mut trailing = encoded(first);
    trailing.push(0);
    assert_eq!(HostMsg::decode(&trailing), Ok((first, 19)));
}

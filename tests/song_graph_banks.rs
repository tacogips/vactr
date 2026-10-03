//! Graph banks retain exact leases across native and browser byte transport.
use vactr::dsp::ring::{Budget, ByteInbox, ControlSource, NativeRecord, Record, SpscRing};
use vactr::host::wire::CtlMsg;
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;

fn key(kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(u64::MAX),
        resource: SongResourceRef {
            id: u32::MAX,
            generation: u32::MAX - 1,
        },
        kind,
    }
}
fn binding(kind: SongResourceKind, controls: bool, analysis: bool) -> SongCommand {
    SongCommand::BindGraphBanks(SongGraphBanks {
        graph: key(kind),
        controls: controls.then(|| key(SongResourceKind::ControlCells)),
        analysis: analysis.then(|| key(SongResourceKind::AnalysisBank)),
    })
}
fn encoded(command: SongCommand) -> Vec<u8> {
    let mut bytes = vec![0; CtlMsg::MAX_LEN];
    let n = CtlMsg::Song(command).encode(&mut bytes);
    assert_ne!(n, 0);
    bytes.truncate(n);
    bytes
}
fn budget() -> Budget {
    Budget {
        install_bytes: usize::MAX,
        batch: true,
    }
}

#[test]
fn graph_bank_options_roundtrip_full_width_native_and_byte_records() {
    let (mut tx, mut native) = SpscRing::split(8);
    let mut inbox = ByteInbox::new();
    for kind in [
        SongResourceKind::Instrument,
        SongResourceKind::PrivateFx,
        SongResourceKind::Track,
        SongResourceKind::Master,
    ] {
        for (controls, analysis) in [(false, false), (true, false), (false, true), (true, true)] {
            let command = binding(kind, controls, analysis);
            assert!(command.valid());
            assert_eq!(command.epoch(), SnapshotEpoch(u64::MAX));
            let bytes = encoded(command);
            assert_eq!(
                bytes.len(),
                21 + 9 * (usize::from(controls) + usize::from(analysis))
            );
            assert_eq!(&bytes[..2], &[0x1B, 16]);
            assert_eq!(&bytes[2..10], &u64::MAX.to_le_bytes());
            assert_eq!(&bytes[10..14], &u32::MAX.to_le_bytes());
            assert_eq!(&bytes[14..18], &(u32::MAX - 1).to_le_bytes());
            assert_eq!(bytes[18], kind as u8);
            assert_eq!(
                CtlMsg::decode(&bytes).unwrap(),
                (CtlMsg::Song(command), bytes.len())
            );
            for end in 0..bytes.len() {
                assert!(CtlMsg::decode(&bytes[..end]).is_err(), "truncation {end}");
            }
            assert!(tx.push(NativeRecord::Msg(CtlMsg::Song(command))).is_ok());
            assert!(inbox.push(&bytes));
            match (native.next(budget()), inbox.next(budget())) {
                (Some(Record::Msg(a)), Some(Record::Msg(b))) => {
                    assert_eq!(a, CtlMsg::Song(command));
                    assert_eq!(a, b);
                }
                _ => panic!("expected native/browser graph-bank POD"),
            }
        }
    }
}

#[test]
fn graph_bank_native_epoch_and_kind_rules_reject_invalid_ownership() {
    let SongCommand::BindGraphBanks(valid) = binding(SongResourceKind::Instrument, true, true)
    else {
        unreachable!()
    };
    let mut invalid = Vec::new();
    for kind in [
        SongResourceKind::Sample,
        SongResourceKind::ControlCells,
        SongResourceKind::AnalysisBank,
    ] {
        invalid.push(SongGraphBanks {
            graph: key(kind),
            ..valid
        });
    }
    for kind in [
        SongResourceKind::Instrument,
        SongResourceKind::PrivateFx,
        SongResourceKind::Track,
        SongResourceKind::Master,
        SongResourceKind::Sample,
        SongResourceKind::AnalysisBank,
    ] {
        invalid.push(SongGraphBanks {
            controls: Some(key(kind)),
            ..valid
        });
    }
    for kind in [
        SongResourceKind::Instrument,
        SongResourceKind::PrivateFx,
        SongResourceKind::Track,
        SongResourceKind::Master,
        SongResourceKind::Sample,
        SongResourceKind::ControlCells,
    ] {
        invalid.push(SongGraphBanks {
            analysis: Some(key(kind)),
            ..valid
        });
    }
    for epoch in [SnapshotEpoch(0), SnapshotEpoch(u64::MAX - 1)] {
        invalid.push(SongGraphBanks {
            controls: Some(SongLeaseKey {
                epoch,
                ..key(SongResourceKind::ControlCells)
            }),
            ..valid
        });
        invalid.push(SongGraphBanks {
            analysis: Some(SongLeaseKey {
                epoch,
                ..key(SongResourceKind::AnalysisBank)
            }),
            ..valid
        });
    }
    for banks in invalid {
        let command = SongCommand::BindGraphBanks(banks);
        assert!(!command.valid());
        assert_eq!(CtlMsg::Song(command).encode(&mut [0; CtlMsg::MAX_LEN]), 0);
    }
}

#[test]
fn graph_bank_bytes_reject_flags_kinds_truncation_and_recover_following_record() {
    let command = binding(SongResourceKind::Master, true, true);
    let bytes = encoded(command);
    let mut malformed = Vec::new();
    for at in [19, 29] {
        for flag in [2, 255] {
            let mut bad = bytes.clone();
            bad[at] = flag;
            malformed.push(bad);
        }
    }
    for at in [18, 28, 38] {
        for kind in [5, 6, 9, 255] {
            let mut bad = bytes.clone();
            bad[at] = kind;
            malformed.push(bad);
        }
    }
    // Known kinds must also satisfy their binding role, not merely decode.
    for (at, kind) in [(18, 4), (28, 8), (38, 7)] {
        let mut bad = bytes.clone();
        bad[at] = kind;
        malformed.push(bad);
    }
    for end in 1..bytes.len() {
        malformed.push(bytes[..end].to_vec());
    }
    for bad in malformed {
        assert!(CtlMsg::decode(&bad).is_err());
        let mut inbox = ByteInbox::new();
        assert!(inbox.push(&bad));
        assert!(inbox.push(&bytes));
        assert!(inbox.next(budget()).is_none());
        assert_eq!(inbox.refused(), 1);
        match inbox.next(budget()) {
            Some(Record::Msg(msg)) => assert_eq!(msg, CtlMsg::Song(command)),
            _ => panic!("malformed frame lost following graph binding"),
        }
    }
    // Stream decode reports exact consumption; record framing rejects extras.
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(CtlMsg::decode(&trailing).unwrap().1, bytes.len());
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&trailing));
    assert!(inbox.push(&bytes));
    assert!(inbox.next(budget()).is_none());
    match inbox.next(budget()) {
        Some(Record::Msg(msg)) => assert_eq!(msg, CtlMsg::Song(command)),
        _ => panic!("trailing frame lost following graph binding"),
    }
}

#[test]
fn graph_bank_append_preserves_existing_command_tags_and_maximum_buffer() {
    for (command, tag) in [
        (SongCommand::Prepare(SnapshotEpoch(u64::MAX)), 0),
        (SongCommand::SealPreparation(SnapshotEpoch(u64::MAX)), 9),
        (SongCommand::CancelPreparation(SnapshotEpoch(u64::MAX)), 13),
        (SongCommand::CancelLease(key(SongResourceKind::Sample)), 14),
        (SongCommand::RequestCapacity(SnapshotEpoch(u64::MAX)), 15),
    ] {
        let bytes = encoded(command);
        assert_eq!(&bytes[..2], &[0x1B, tag]);
        assert_eq!(&bytes[2..10], &u64::MAX.to_le_bytes());
        assert_eq!(CtlMsg::decode(&bytes).unwrap().0, CtlMsg::Song(command));
    }
    let command = binding(SongResourceKind::Track, true, true);
    let full = encoded(command);
    let mut maximum_buffer = [0; SONG_COMMAND_MAX_LEN];
    let consumed = CtlMsg::Song(command).encode(&mut maximum_buffer);
    assert_ne!(consumed, 0);
    assert_eq!(consumed, 39);
    assert_eq!(&maximum_buffer[..consumed], full.as_slice());
    for len in 0..full.len() {
        assert_eq!(CtlMsg::Song(command).encode(&mut vec![0; len]), 0);
    }
}

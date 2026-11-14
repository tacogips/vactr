//! Actual typed activation intake failure with physical ACK pressure.
use vactr::dsp::{
    arena::StoreKind,
    caps::CapabilitySet,
    cells::AtomicCells,
    engine::{Engine, EngineConfig, SongStagingConfig},
    ring::{ByteInbox, EngineIo, EventRing, NativeRecord, SpscRing},
};
use vactr::host::wire::{CtlMsg, HostMsg};
use vactr::song::{routing::*, SnapshotEpoch};

#[test]
fn actual_native_and_byte_intake_preserve_activation_failure_under_ack_pressure() {
    for bytes in [false, true] {
        let caps = CapabilitySet::browser();
        let mut engine = Engine::with_config(EngineConfig::new(
            &caps,
            48000.,
            16,
            if bytes {
                StoreKind::Arena { bytes: 16384 }
            } else {
                StoreKind::NativeArc
            },
        ));
        let (mut acks, mut received) = SpscRing::split(2);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 1,
                    leases: 8,
                    branches: 1,
                    control_slots: 8,
                    analysis_slots: 8,
                    native_pcm_bytes: 0,
                    critical_receipts: 1,
                },
                &acks,
            )
            .unwrap();
        let (mut sender, mut controls) = SpscRing::split(8);
        let mut inbox = ByteInbox::new();
        let (_event_sender, mut events) = EventRing::split(2);
        let mut cells = AtomicCells::new(16);
        let request = SongActivation {
            epoch: SnapshotEpoch(77),
            frame: u64::MAX,
        };
        for command in [
            SongCommand::RequestCapacity(SnapshotEpoch(70)),
            SongCommand::RequestCapacity(SnapshotEpoch(71)),
            SongCommand::Activate(request),
        ] {
            if bytes {
                let mut record = [0; CtlMsg::MAX_LEN];
                let n = CtlMsg::Song(command).encode(&mut record);
                assert!(n > 0 && inbox.push(&record[..n]));
            } else {
                assert!(sender
                    .push(NativeRecord::Msg(CtlMsg::Song(command)))
                    .is_ok());
            }
        }
        let mut reports = Vec::new();
        for _ in 0..4 {
            let mut output = [0.; 32];
            if bytes {
                engine.process(
                    &mut EngineIo {
                        events: &mut events,
                        controls: &mut inbox,
                        acks: &mut acks,
                        cells: &mut cells,
                        garbage: None,
                    },
                    &mut output,
                    16,
                );
            } else {
                engine.process(
                    &mut EngineIo {
                        events: &mut events,
                        controls: &mut controls,
                        acks: &mut acks,
                        cells: &mut cells,
                        garbage: None,
                    },
                    &mut output,
                    16,
                );
            }
            assert!(output.iter().all(|v| *v == 0.));
            while let Some(report) = received.pop() {
                reports.push(report);
            }
        }
        assert_eq!(reports.len(), 3);
        for (report, epoch) in reports[..2].iter().zip([70, 71]) {
            assert!(
                matches!(report, HostMsg::Song(SongHostAck::CapacityReport(r)) if r.epoch == SnapshotEpoch(epoch))
            );
        }
        assert_eq!(
            reports[2],
            HostMsg::Song(SongHostAck::ActivationRejected {
                activation: request,
                reason: SongRejectCode::NotReady,
            })
        );
    }
}

//! Late interactive controls retain actual callback authority and exact musical scheduling.
use super::*;

fn activate(rig: &mut Rig, branches: &[u32]) {
    rig.command(SongCommand::Activate(SongActivation {
        epoch: SnapshotEpoch(11),
        frame: rig.frame,
    }));
    for &branch in branches {
        rig.event(branch, rig.frame);
    }
}
fn mute(rig: &mut Rig, epoch: SnapshotEpoch, muted: bool, frame: u64) {
    rig.command(SongCommand::Mute(SongMute {
        epoch,
        instrument: 1,
        muted,
        frame,
    }));
}
#[test]
fn late_mute_acknowledges_actual_frame_and_has_exact_pcm_ramp() {
    for bytes in [false, true] {
        let mut actual = Rig::prepared_family(bytes, 16, true);
        let mut reference = Rig::prepared_family(bytes, 16, true);
        let mut sibling = Rig::prepared_family(bytes, 16, true);
        activate(&mut actual, &[0, 1, 2]);
        activate(&mut reference, &[0]);
        activate(&mut sibling, &[0]);
        actual.process(20, true);
        reference.process(20, true);
        sibling.process(20, true);
        let frame = actual.frame;
        let requested = frame.checked_sub(16).unwrap();
        mute(&mut actual, SnapshotEpoch(11), true, requested);
        reference.event(1, frame);
        reference.event(2, frame);
        let output = actual.process(85, true);
        let expected = reference.process(85, true);
        let pcm = sibling.process(85, true);
        let mut nonzero = 0;
        for f in 0..85 {
            let gain = if f < 64 { 1. - f as f32 / 64. } else { 0. };
            for channel in 0..2 {
                let k = 2 * f + channel;
                let oracle = pcm[k] + (expected[k] - pcm[k]) * gain;
                assert!(
                    (output[k] - oracle).abs() < 0.00001,
                    "bytes={bytes} frame={f} channel={channel} actual={} oracle={oracle}",
                    output[k]
                );
                if f < 64 && (output[k] - pcm[k]).abs() > 0.0001 {
                    nonzero += 1;
                }
            }
        }
        assert!(
            nonzero > 50,
            "real reset oscillators audible through 64-frame gate"
        );
        assert_eq!(actual.engine.active_voices(), 1);
        assert!(actual
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Muted(SongMute {
                epoch: SnapshotEpoch(11),
                instrument: 1,
                muted: true,
                frame,
            }))));
        assert!(!actual
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Muted(SongMute {
                epoch: SnapshotEpoch(11),
                instrument: 1,
                muted: true,
                frame: requested,
            }))));
        assert!(!actual.receipts.iter().any(|ack| matches!(
            ack,
            HostMsg::Song(SongHostAck::Rejected {
                epoch: SnapshotEpoch(11),
                ..
            })
        )));
    }
}
#[test]
fn late_unmute_does_not_resume_old_notes_and_stale_epoch_is_refused() {
    for bytes in [false, true] {
        let mut actual = Rig::prepared(bytes, 16);
        let mut muted = Rig::prepared(bytes, 16);
        let mut reference = Rig::prepared(bytes, 16);
        for rig in [&mut actual, &mut muted, &mut reference] {
            activate(rig, &[0, 1]);
            rig.process(32, true);
            let frame = rig.frame;
            mute(rig, SnapshotEpoch(11), true, frame - 16);
            rig.process(80, true);
        }
        let unmute = actual.frame;
        mute(&mut actual, SnapshotEpoch(11), false, unmute - 16);
        mute(&mut reference, SnapshotEpoch(11), false, unmute);
        let unmuted_pcm = actual.process(64, true);
        assert_eq!(
            unmuted_pcm,
            muted.process(64, true),
            "same shared histories: late unmute cannot resume old notes"
        );
        assert_eq!(
            unmuted_pcm,
            reference.process(64, true),
            "late and on-time controls apply at the same actual callback frame"
        );
        assert!(actual
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Muted(SongMute {
                epoch: SnapshotEpoch(11),
                instrument: 1,
                muted: false,
                frame: unmute,
            }))));
        assert_eq!(actual.engine.active_voices(), 1);
        let next = actual.frame;
        actual.event(1, next);
        reference.event(1, next);
        let audible = actual.process(32, true);
        assert_eq!(audible, reference.process(32, true));
        let without_new_note = muted.process(32, true);
        assert!(
            audible
                .iter()
                .zip(&without_new_note)
                .any(|(a, b)| (*a - *b).abs() > 0.0001),
            "a genuine fresh oscillator onset is audible above the surviving PCM sibling"
        );
        assert_eq!(actual.engine.active_voices(), 2);
        assert_eq!(muted.engine.active_voices(), 1);
        let frame = actual.frame;
        mute(&mut actual, SnapshotEpoch(99), true, frame - 16);
        assert_eq!(
            actual.process(80, true),
            reference.process(80, true),
            "foreign epoch refusal leaves matched complete audio history unchanged"
        );
        assert!(actual
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Rejected {
                epoch: SnapshotEpoch(99),
                reason: SongRejectCode::StaleEpoch,
            })));
        assert_eq!(
            actual.engine.active_voices(),
            reference.engine.active_voices()
        );
    }
}

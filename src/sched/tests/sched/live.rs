//! LP-SESSION-STOP tests for scheduler output stop and telemetry behavior.

use super::{Rig, DT};
use crate::dsp::caps::CapabilitySet;
use crate::host::testing::AudioCall;
use crate::host::wire::{CtlMsg, HostMsg, OutputMode, OutputPhase, Release};
use crate::ns::stage::{SlotKey, StagedEffect};
use crate::sched::runtime::RuntimeConfig;
use crate::sched::slots::SlotId;
use crate::sched::song::SongTransportState;
use crate::song::routing::SongEndpoints;
use crate::song::SnapshotEpoch;
use crate::types::diag::DiagCode;

fn output_modes(rig: &Rig) -> Vec<OutputMode> {
    rig.audio
        .calls()
        .into_iter()
        .filter_map(|(_, call)| match call {
            AudioCall::Post(CtlMsg::OutputStop { mode }) => Some(mode),
            _ => None,
        })
        .collect()
}

fn assert_output_after_controls(rig: &Rig, mode: OutputMode, releases: &[Release]) {
    let calls = rig.audio.calls();
    let output_at = calls
        .iter()
        .position(|(_, call)| matches!(call, AudioCall::Post(CtlMsg::OutputStop { mode: m }) if *m == mode))
        .expect("output stop was posted");
    let controls: Vec<_> = calls
        .iter()
        .enumerate()
        .filter_map(|(index, (_, call))| match call {
            AudioCall::Control(control) if releases.contains(&control.release) => {
                Some((index, control.release))
            }
            _ => None,
        })
        .collect();
    assert_eq!(controls.len(), releases.len());
    assert!(controls.iter().all(|(index, _)| *index < output_at));
}

#[test]
fn stop_all_naturally_releases_every_slot_before_one_gentle_output_stop() {
    let mut rig = Rig::new();
    rig.run("s :bd > d1\ns :sd > d2");
    rig.apply(StagedEffect::StopAll);

    let controls = rig.controls();
    assert_eq!(controls.len(), 2);
    assert!(controls
        .iter()
        .all(|(_, control)| control.release == Release::Natural));
    assert_eq!(output_modes(&rig), [OutputMode::Gentle]);
    assert_output_after_controls(&rig, OutputMode::Gentle, &[Release::Natural; 2]);
    assert_eq!(rig.rt.output.cuts(), 0);
}

#[test]
fn cut_panics_every_slot_before_one_cut_output_stop() {
    let mut rig = Rig::new();
    rig.run("s :bd > d1\ns :sd > d2");
    rig.apply(StagedEffect::Cut);

    let controls = rig.controls();
    assert_eq!(controls.len(), 2);
    assert!(controls
        .iter()
        .all(|(_, control)| control.release == Release::Panic));
    assert_eq!(output_modes(&rig), [OutputMode::Cut]);
    assert_output_after_controls(&rig, OutputMode::Cut, &[Release::Panic; 2]);
    assert_eq!(rig.rt.output.cuts(), 1);
}

#[test]
fn per_slot_revoke_does_not_request_an_output_stop() {
    let mut rig = Rig::new();
    rig.run("s :bd > d1\ns :sd > d2");
    rig.apply(StagedEffect::Revoke(SlotKey::D(1)));

    assert_eq!(rig.controls().len(), 1);
    assert_eq!(rig.controls()[0].1.slot, SlotId::new(1));
    assert_eq!(rig.controls()[0].1.release, Release::Natural);
    assert!(output_modes(&rig).is_empty());
}

#[test]
fn output_state_is_absent_until_reported_and_acknowledgment_stops_retries() {
    let mut rig = Rig::new();
    assert_eq!(rig.rt.output.wire(), None);
    rig.apply(StagedEffect::StopAll);
    rig.audio.reply(HostMsg::OutputState {
        phase: OutputPhase::Draining,
        frame: 0,
    });
    rig.run_to(4.0 * DT);

    assert_eq!(rig.rt.output.wire().as_deref(), Some("draining"));
    assert_eq!(output_modes(&rig), [OutputMode::Gentle]);
}

#[test]
fn output_stop_retries_three_times_then_reports_one_transport_diagnostic() {
    let config = RuntimeConfig {
        resend_ticks: 2,
        transport_diag_ticks: 100,
        ..RuntimeConfig::default()
    };
    let mut rig = Rig::with(config, CapabilitySet::native());
    rig.apply(StagedEffect::StopAll);
    rig.run_to(8.0 * DT);

    assert_eq!(output_modes(&rig), [OutputMode::Gentle; 4]);
    let diagnostics: Vec<_> = rig
        .ticks
        .iter()
        .flat_map(|tick| &tick.diags)
        .filter(|diagnostic| diagnostic.code == DiagCode::HostTransport)
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("whole-output stop Gentle is not acknowledged"));
}

#[test]
fn output_stop_only_timeout_reports_exactly_one_transport_diagnostic() {
    let config = RuntimeConfig {
        resend_ticks: 2,
        transport_diag_ticks: 4,
        ..RuntimeConfig::default()
    };
    let mut rig = Rig::with(config, CapabilitySet::native());
    rig.apply(StagedEffect::StopAll);
    rig.run_to(10.0 * DT);

    assert_eq!(output_modes(&rig), [OutputMode::Gentle; 4]);
    let diagnostics: Vec<_> = rig
        .ticks
        .iter()
        .flat_map(|tick| &tick.diags)
        .filter(|diagnostic| diagnostic.code == DiagCode::HostTransport)
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("whole-output stop Gentle is not acknowledged"));
}

#[test]
fn slot_and_output_stop_timeout_share_one_transport_diagnostic() {
    let mut rig = Rig::new();
    rig.ack_delay = None;
    rig.run("s :bd > d1");
    rig.apply(StagedEffect::StopAll);
    rig.run_to(30.0 * DT);

    assert_eq!(output_modes(&rig), [OutputMode::Gentle; 4]);
    let diagnostics: Vec<_> = rig
        .ticks
        .iter()
        .flat_map(|tick| &tick.diags)
        .filter(|diagnostic| diagnostic.code == DiagCode::HostTransport)
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("slot d1: control"));
}

#[test]
fn pending_cut_is_not_downgraded_by_a_gentle_request() {
    let config = RuntimeConfig {
        resend_ticks: 1,
        ..RuntimeConfig::default()
    };
    let mut rig = Rig::with(config, CapabilitySet::native());
    rig.apply(StagedEffect::Cut);
    rig.apply(StagedEffect::StopAll);
    rig.run_to(DT);

    assert_eq!(output_modes(&rig), [OutputMode::Cut; 4]);
    assert_eq!(rig.rt.output.cuts(), 1);
}

#[test]
fn stopping_without_a_song_is_a_no_op() {
    let mut rig = Rig::new();
    let calls = rig.audio.calls();
    let mut report = crate::sched::runtime::DrainReport::default();
    rig.rt.stop_song(OutputMode::Gentle, &mut report);

    assert!(report.faults.is_empty());
    assert_eq!(rig.audio.calls(), calls);
}

#[test]
fn song_stop_endpoints_are_bounded_for_prepared_playing_and_draining_states() {
    let old = SongEndpoints {
        epoch: SnapshotEpoch(9),
        arrangement: 1_000,
        tail_deadline: 1_200,
    };
    let cases = [
        (
            OutputMode::Gentle,
            SongTransportState::Prepared,
            50,
            10,
            100,
            Some((100, 300)),
        ),
        (
            OutputMode::Cut,
            SongTransportState::Prepared,
            50,
            0,
            100,
            Some((100, 100)),
        ),
        (
            OutputMode::Gentle,
            SongTransportState::Playing,
            100,
            10,
            0,
            Some((110, 310)),
        ),
        (
            OutputMode::Gentle,
            SongTransportState::Playing,
            990,
            30,
            0,
            Some((1_000, 1_200)),
        ),
        (
            OutputMode::Gentle,
            SongTransportState::Draining,
            100,
            10,
            0,
            None,
        ),
        (
            OutputMode::Cut,
            SongTransportState::Playing,
            120,
            0,
            0,
            Some((120, 120)),
        ),
        (
            OutputMode::Cut,
            SongTransportState::Draining,
            1_300,
            0,
            0,
            Some((1_000, 1_200)),
        ),
        (
            OutputMode::Gentle,
            SongTransportState::Playing,
            0,
            0,
            100,
            Some((100, 300)),
        ),
    ];

    for (mode, state, now, lead, activation, expected) in cases {
        let result = crate::sched::runtime::Runtime::stop_endpoints_for_test(
            mode, state, now, lead, activation, old,
        );
        assert_eq!(
            result.map(|endpoints| (endpoints.arrangement, endpoints.tail_deadline)),
            expected,
            "{mode:?} {state:?} now={now} activation={activation}"
        );
        if let Some(endpoints) = result {
            // Match SongTransport::cutoff's epoch and endpoint validation.
            assert_eq!(endpoints.epoch, old.epoch);
            assert!(endpoints.arrangement >= activation);
            assert!(endpoints.arrangement <= old.arrangement);
            assert!(endpoints.tail_deadline >= endpoints.arrangement);
            assert!(endpoints.tail_deadline <= old.tail_deadline);
        }
    }
}

//! BE-CONTRACTS host tests: the wire codec, the monotone control merge,
//! the noop host and the test hosts and transports (design 11.3, 12.8.5).

use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::cells::{CellId, CellRead, CellState};
use crate::dsp::graph::InstId;
use crate::host::caps::{
    AudioHost, HostSigs, Hosts, InstResolver, MidiEvent, MidiHost, OscArg, OscEvent, OscHost,
    RenderHost, SampleData, SampleLoader, SampleSrc,
};
use crate::host::noop::NoopHost;
use crate::host::testing::{
    AudioCall, BrowserTransport, MockClock, NativeTransport, RecordingAudioHost, RecordingMidiHost,
    RecordingOscHost, RecordingRenderHost, RenderCall, SinkCall,
};
use crate::host::wire::{
    batch_len, decode_batch, encode_batch, AudioEvent, Ctl, CtlMsg, HostMsg, Release, SlotControl,
    SlotControlAck, VoiceTag, WireError, MAX_CTLS,
};
use crate::sched::slots::{CtlId, SlotId};
use crate::tex::texnode::OutId;
use crate::value::intern::intern_kw;
use crate::vm::fail::FailCode;

fn full_event() -> AudioEvent {
    let mut ev = AudioEvent::new(1.25, SlotId::new(3), 7, InstId::new(9));
    ev.voice_hint = 42;
    for i in 0..MAX_CTLS {
        let id = CtlId::new(u16::try_from(i).expect("small"));
        let ctl = if i % 2 == 0 {
            Ctl::Const(0.5 * i as f32)
        } else {
            Ctl::Cell(CellId::new(100 + u32::try_from(i).expect("small")))
        };
        ev.push_ctl(id, ctl).expect("room");
    }
    ev
}

fn tag() -> VoiceTag {
    VoiceTag {
        slot: SlotId::new(2),
        channel: 1,
        pitch: 60,
        seq: 77,
    }
}

fn control(gen: u32, time: f64, release: Release) -> SlotControl {
    SlotControl {
        slot: SlotId::new(1),
        new_gen: gen,
        effective_time: time,
        release,
    }
}

fn every_ctl_msg() -> Vec<CtlMsg> {
    vec![
        CtlMsg::SlotControl(control(4, 2.5, Release::Panic)),
        CtlMsg::CellInit {
            cell: CellId::new(5),
            epoch: 2,
            value: 0.75,
        },
        CtlMsg::CellBatch { seq: 9, count: 3 },
        CtlMsg::CellRetire {
            cell: CellId::new(5),
            epoch: 2,
        },
        CtlMsg::LiveNoteOn {
            tag: tag(),
            ev: full_event(),
        },
        CtlMsg::VoiceRelease { tag: tag() },
        CtlMsg::GraphInstall { id: 11, gen: 3 },
        CtlMsg::GraphRetire { id: 11 },
        CtlMsg::SampleSlice {
            resource: 6,
            offset: 65_536,
            len: 16_384,
        },
        CtlMsg::SampleRetire { resource: 6 },
    ]
}

fn every_host_msg() -> Vec<HostMsg> {
    vec![
        HostMsg::SlotControlAck(SlotControlAck {
            slot: SlotId::new(1),
            gen: 4,
        }),
        HostMsg::CellInitAck {
            cell: CellId::new(5),
            epoch: 2,
        },
        HostMsg::CellBatchAck { seq: 9 },
        HostMsg::CellRetired {
            cell: CellId::new(5),
            epoch: 2,
        },
        HostMsg::Retired { resource: 6 },
        HostMsg::SliceOk {
            resource: 6,
            offset: 65_536,
        },
        HostMsg::Installed {
            resource: 6,
            gen: 1,
        },
        HostMsg::Counters {
            late: 1,
            dropped: 2,
            stolen: 3,
            skipped: 4,
        },
        HostMsg::AnalysisCell { id: 8, value: -0.5 },
    ]
}

#[test]
fn audio_event_with_24_controls_round_trips() {
    let ev = full_event();
    assert_eq!(ev.controls().len(), MAX_CTLS);
    let mut buf = [0u8; AudioEvent::ENCODED_LEN];
    assert_eq!(ev.encode(&mut buf), AudioEvent::ENCODED_LEN);
    let (back, used) = AudioEvent::decode(&buf).expect("decodes");
    assert_eq!((back, used), (ev, AudioEvent::ENCODED_LEN));
}

#[test]
fn a_25th_control_is_too_many_controls() {
    let mut ev = full_event();
    let err = ev
        .push_ctl(CtlId::new(99), Ctl::Const(1.0))
        .expect_err("full");
    assert_eq!(err.code, FailCode::TooManyControls);
    assert_eq!(usize::from(ev.n_ctl), MAX_CTLS);
}

#[test]
fn every_ctl_msg_round_trips() {
    for msg in every_ctl_msg() {
        let mut buf = [0u8; CtlMsg::MAX_LEN];
        let n = msg.encode(&mut buf);
        assert!(n > 0, "{msg:?}");
        assert_eq!(CtlMsg::decode(&buf[..n]), Ok((msg, n)));
    }
}

#[test]
fn every_host_msg_round_trips() {
    for msg in every_host_msg() {
        let mut buf = [0u8; HostMsg::MAX_LEN];
        let n = msg.encode(&mut buf);
        assert!(n > 0, "{msg:?}");
        assert_eq!(HostMsg::decode(&buf[..n]), Ok((msg, n)));
    }
}

#[test]
fn truncated_buffers_are_wire_errors_never_panics() {
    for msg in every_ctl_msg() {
        let mut buf = [0u8; CtlMsg::MAX_LEN];
        let n = msg.encode(&mut buf);
        for cut in 0..n {
            assert_eq!(CtlMsg::decode(&buf[..cut]), Err(WireError::Truncated));
        }
    }
    for msg in every_host_msg() {
        let mut buf = [0u8; HostMsg::MAX_LEN];
        let n = msg.encode(&mut buf);
        for cut in 0..n {
            assert_eq!(HostMsg::decode(&buf[..cut]), Err(WireError::Truncated));
        }
    }
    let mut buf = [0u8; AudioEvent::ENCODED_LEN];
    let n = full_event().encode(&mut buf);
    for cut in 0..n {
        assert_eq!(AudioEvent::decode(&buf[..cut]), Err(WireError::Truncated));
    }
}

#[test]
fn bad_tags_and_fields_are_wire_errors() {
    assert_eq!(CtlMsg::decode(&[0xff]), Err(WireError::BadTag(0xff)));
    assert_eq!(HostMsg::decode(&[0x10]), Err(WireError::BadTag(0x10)));
    let mut buf = [0u8; CtlMsg::MAX_LEN];
    let n = CtlMsg::SlotControl(control(1, 0.0, Release::None)).encode(&mut buf);
    buf[n - 1] = 7; // release byte out of range
    assert_eq!(CtlMsg::decode(&buf[..n]), Err(WireError::BadValue));
    let mut ev = [0u8; AudioEvent::ENCODED_LEN];
    let n = full_event().encode(&mut ev);
    ev[1 + 24] = 25; // n_ctl above MAX_CTLS
    assert_eq!(AudioEvent::decode(&ev[..n]), Err(WireError::BadValue));
}

#[test]
fn encoding_into_a_short_buffer_writes_nothing() {
    let mut small = [0u8; 4];
    assert_eq!(full_event().encode(&mut small), 0);
    assert_eq!(CtlMsg::GraphInstall { id: 1, gen: 1 }.encode(&mut small), 0);
    assert_eq!(HostMsg::CellBatchAck { seq: 1 }.encode(&mut small), 0);
}

#[test]
fn cell_batch_entries_follow_the_header() {
    let entries = [
        (CellId::new(1), 1, 0.25),
        (CellId::new(2), 3, -1.0),
        (CellId::new(7), 1, 9.5),
    ];
    let mut buf = vec![0u8; batch_len(entries.len())];
    let n = encode_batch(12, &entries, &mut buf);
    assert_eq!(n, buf.len());
    let (view, used) = decode_batch(&buf).expect("decodes");
    assert_eq!((view.seq, view.len(), used), (12, 3, n));
    assert_eq!(view.iter().collect::<Vec<_>>(), entries);
    assert_eq!(
        CtlMsg::decode(&buf),
        Ok((CtlMsg::CellBatch { seq: 12, count: 3 }, 9))
    );
    assert_eq!(
        decode_batch(&buf[..n - 1]).err(),
        Some(WireError::Truncated)
    );
}

#[test]
fn merge_keeps_panic_through_stop_and_tempo() {
    let hush = control(5, 1.0, Release::Panic);
    let stop = control(6, 1.2, Release::Natural);
    let merged = hush.merge(stop);
    assert_eq!(merged.release, Release::Panic);
    assert_eq!((merged.new_gen, merged.effective_time), (6, 1.0));

    let tempo = control(8, 1.5, Release::None);
    let merged = hush.merge(tempo);
    assert_eq!(merged.release, Release::Panic);
    assert_eq!(merged.new_gen, 8);
}

#[test]
fn merge_is_idempotent() {
    let a = control(3, 2.0, Release::Natural);
    let b = control(5, 1.0, Release::None);
    let once = a.merge(b);
    assert_eq!(once.merge(b), once);
    assert_eq!(once.merge(once), once);
    assert_eq!(a.merge(a), a);
    assert!(Release::None < Release::Natural && Release::Natural < Release::Panic);
}

#[test]
fn noop_host_drops_sends_and_fails_reads() {
    let mut hosts = Hosts::noop();
    hosts.audio.send(full_event());
    hosts.audio.control(control(1, 0.0, Release::Panic));
    let mut out = Vec::new();
    hosts.audio.drain(&mut out);
    assert!(out.is_empty());
    assert_eq!(hosts.audio.now(), 0.0);
    assert_eq!(hosts.audio.analysis(), HostSigs::default());
    assert!(hosts.midi_in.poll().is_empty());
    let src = SampleSrc::Bank {
        kw: intern_kw("bd"),
        index: 0,
    };
    let err = hosts.samples.load(&src).expect_err("no host");
    assert_eq!(err.code, FailCode::HostUnavailable);
    let err = NoopHost
        .load(&SampleSrc::Path(crate::value::value::PathVal {
            text: Rc::from("/x.wav"),
            file: None,
        }))
        .expect_err("no host");
    assert!(err.message.contains("/x.wav"));
    let sound = crate::value::value::Sound::MidiOut(1);
    let err = NoopHost.route(&sound).expect_err("no registry");
    assert_eq!(err.code, FailCode::HostUnavailable);
    assert!(NoopHost.inst(InstId::new(0)).is_none());
    assert!(NoopHost.signal_inputs().is_empty());
    // The `ns::load` name is the same host.
    let _: crate::ns::load::NoopHost = NoopHost;
}

#[test]
fn recording_hosts_log_calls_in_order_with_arrival_time() {
    let clock = MockClock::new(0.5);
    let audio = RecordingAudioHost::new(clock.clone());
    let mut boxed: Box<dyn AudioHost> = Box::new(audio.clone());
    boxed.send(full_event());
    clock.advance(0.25);
    boxed.control(control(2, 0.75, Release::None));
    let data = Arc::new(SampleData {
        rate: 48_000,
        channels: 1,
        frames: Box::new([0.0; 4]),
    });
    boxed.install_sample(3, Arc::clone(&data));
    boxed.retire_sample(3);
    audio.reply(HostMsg::Installed {
        resource: 3,
        gen: 1,
    });
    let mut out = Vec::new();
    boxed.drain(&mut out);
    assert_eq!(
        out,
        [HostMsg::Installed {
            resource: 3,
            gen: 1
        }]
    );
    assert_eq!(boxed.now(), 0.75);
    let sigs = HostSigs {
        amp: 0.5,
        fft: [0.25; 8],
    };
    audio.set_sigs(sigs);
    assert_eq!(boxed.analysis(), sigs);
    let calls = audio.calls();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0], (0.5, AudioCall::Send(full_event())));
    assert_eq!(
        calls[1],
        (0.75, AudioCall::Control(control(2, 0.75, Release::None)))
    );
    assert_eq!(calls[2], (0.75, AudioCall::InstallSample(3, data)));
    assert_eq!(calls[3], (0.75, AudioCall::RetireSample(3)));

    let midi = RecordingMidiHost::new(clock.clone());
    let mut m: Box<dyn MidiHost> = Box::new(midi.clone());
    m.send(MidiEvent::Clock { time: 1.0 });
    m.control(control(3, 1.0, Release::Panic));
    assert_eq!(
        midi.calls(),
        [
            (0.75, SinkCall::Send(MidiEvent::Clock { time: 1.0 })),
            (0.75, SinkCall::Control(control(3, 1.0, Release::Panic)))
        ]
    );

    let osc = RecordingOscHost::new(clock.clone());
    let mut o: Box<dyn OscHost> = Box::new(osc.clone());
    let ev = OscEvent {
        time: 1.0,
        slot: SlotId::new(1),
        gen: 1,
        addr: Rc::from("/a"),
        args: vec![OscArg::F(1.0), OscArg::I(2), OscArg::S(Rc::from("x"))],
    };
    o.send(ev.clone());
    assert_eq!(osc.calls(), [(0.75, SinkCall::Send(ev))]);

    clock.set(2.0);
    let render = RecordingRenderHost::new(clock);
    let mut r: Box<dyn RenderHost> = Box::new(render.clone());
    let uniforms = crate::tex::uniforms::Uniforms {
        values: Box::new([0.5]),
    };
    r.set_uniforms(OutId::new(0), &uniforms);
    assert_eq!(
        render.calls(),
        [(2.0, RenderCall::SetUniforms(OutId::new(0), uniforms))]
    );
}

#[test]
fn native_transport_is_immediate() {
    let mut t = NativeTransport::new(4);
    assert!(t.write_cell(CellId::new(2), 0.5));
    assert!(!t.write_cell(CellId::new(9), 0.5));
    assert_eq!(t.audio_cells().get(CellId::new(2)), 0.5);
    t.post(CtlMsg::SlotControl(control(4, 0.0, Release::Natural)));
    let mut out = Vec::new();
    t.drain(&mut out);
    assert_eq!(
        out,
        [HostMsg::SlotControlAck(SlotControlAck {
            slot: SlotId::new(1),
            gen: 4
        })]
    );
    assert_eq!(t.delivered().len(), 1);
}

#[test]
fn browser_transport_delays_and_acks_through_the_fifo() {
    let mut t = BrowserTransport::new(4, 2);
    let cell = CellId::new(1);
    t.post(CtlMsg::CellInit {
        cell,
        epoch: 1,
        value: 0.25,
    });
    assert_eq!(t.in_flight(), 1);
    let mut out = Vec::new();
    t.tick();
    assert_eq!(t.mirror().state(cell), Some(CellState::Vacant));
    t.tick();
    assert_eq!(t.mirror().get(cell), 0.25);
    t.drain(&mut out);
    assert!(out.is_empty(), "the ack is still in flight");
    t.tick();
    t.tick();
    t.drain(&mut out);
    assert_eq!(out, [HostMsg::CellInitAck { cell, epoch: 1 }]);
}

#[test]
fn browser_transport_applies_one_batch_per_tick() {
    let mut t = BrowserTransport::new(4, 0);
    let cell = CellId::new(0);
    t.post(CtlMsg::CellInit {
        cell,
        epoch: 1,
        value: 0.0,
    });
    t.post_batch(1, &[(cell, 1, 0.5)]);
    t.post_batch(2, &[(cell, 1, 0.75)]);
    t.tick();
    assert_eq!(t.mirror().get(cell), 0.5);
    assert_eq!(t.in_flight(), 1);
    t.tick();
    assert_eq!(t.mirror().get(cell), 0.75);
    let mut out = Vec::new();
    t.drain(&mut out);
    assert_eq!(
        out,
        [
            HostMsg::CellInitAck { cell, epoch: 1 },
            HostMsg::CellBatchAck { seq: 1 },
            HostMsg::CellBatchAck { seq: 2 }
        ]
    );
}

#[test]
fn browser_transport_loss_and_stall() {
    let mut t = BrowserTransport::new(4, 0);
    let cell = CellId::new(3);
    t.drop_next(1);
    t.post(CtlMsg::CellInit {
        cell,
        epoch: 1,
        value: 1.0,
    });
    t.tick();
    assert_eq!(t.mirror().state(cell), Some(CellState::Vacant));
    assert_eq!(t.dropped(), (1, 0));

    // The re-send lands; its ack is lost, so a second re-send is ack-only.
    t.drop_next_acks(1);
    t.post(CtlMsg::CellInit {
        cell,
        epoch: 1,
        value: 1.0,
    });
    t.tick();
    let mut out = Vec::new();
    t.drain(&mut out);
    assert!(out.is_empty());
    assert_eq!(t.dropped(), (1, 1));

    t.stall();
    t.post(CtlMsg::CellRetire { cell, epoch: 1 });
    t.tick();
    t.tick();
    assert_eq!(t.mirror().get(cell), 1.0, "stalled: nothing applies");
    t.resume();
    t.set_delay(1);
    t.tick();
    assert_eq!(t.mirror().state(cell), Some(CellState::Vacant));
    t.tick();
    t.drain(&mut out);
    assert_eq!(out, [HostMsg::CellRetired { cell, epoch: 1 }]);
    assert!(matches!(
        t.delivered().last(),
        Some(CtlMsg::CellRetire { .. })
    ));
}

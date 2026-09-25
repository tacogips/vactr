//! The main-thread half: evaluator and scheduler (design 12.8.10, 16).
//!
//! `main_init` builds `Evaluator` + `InstRegistry` + `Runtime` on the
//! browser tier: `CapabilitySet::browser()`, cells through `WasmCellPort`,
//! `WasmAudioHost`, no-op MIDI/OSC/render/MIDI-in hosts (TASK-010 wires
//! WebMIDI and WebGL), and `WasmSamples` fed by `sample_put`. `eval` runs
//! source text and drains the runtime; `tick(now)` runs one scheduler tick
//! at the worklet's posted time; `inbox` takes the worklet's framed records.
//! Diagnostics, failures and printed lines go to the outbox as `Console`
//! records. The `harness_*` exports only encode wire records into the
//! outbox for the dev harness's fault injection (12.8.11); JS still moves
//! them over the real port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::arena::FaultCode;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellId;
use crate::dsp::controls::row;
use crate::dsp::graph::InstId;
use crate::dsp::ring::{encode_sample_begin, encode_slice};
use crate::host::caps::{HostSigs, Hosts, InstResolver, SampleData, SampleSrc};
use crate::host::noop::NoopHost;
use crate::host::wasm::abi::{for_each_record, input, push_record, TAG_FAULT, TAG_SIGS};
use crate::host::wasm::messages::{
    console, HostState, WasmAudioHost, WasmCellPort, WasmSamples, SLICE_FLOATS,
};
use crate::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg, Release, SlotControl, VoiceTag};
use crate::ns::evaluator::Evaluator;
use crate::ns::insts::InstRegistry;
use crate::ns::namespace::Prelude;
use crate::reader::span::{FileId, Span};
use crate::sched::cells::Tier;
use crate::sched::runtime::{DrainReport, Runtime, RuntimeConfig};
use crate::sched::slots::SlotId;
use crate::types::diag::Diagnostic;
use crate::value::intern::intern_kw;

/// The file id of evaluated source text.
const SOURCE: FileId = FileId::new(1);

struct Main {
    ev: Evaluator,
    rt: Runtime,
    host: Rc<RefCell<HostState>>,
    samples: WasmSamples,
    reg: Rc<RefCell<InstRegistry>>,
}

thread_local! {
    static MAIN: RefCell<Option<Main>> = const { RefCell::new(None) };
}

fn with<R: Default>(f: impl FnOnce(&mut Main) -> R) -> R {
    MAIN.with(|m| m.borrow_mut().as_mut().map(f).unwrap_or_default())
}

/// Builds the main half over a worklet arena of `arena_bytes` (0: the
/// 64 MB default) and returns 1. The prelude templates are installed
/// through the outbox here.
#[no_mangle]
pub extern "C" fn main_init(sample_rate: f32, arena_bytes: u32) -> u32 {
    let arena = if arena_bytes == 0 {
        crate::dsp::arena::DEFAULT_ARENA_BYTES
    } else {
        arena_bytes as usize
    };
    let host = Rc::new(RefCell::new(HostState::new(arena)));
    let samples = WasmSamples::default();
    let hosts = Hosts {
        audio: Box::new(WasmAudioHost(Rc::clone(&host))),
        midi: Box::new(NoopHost),
        osc: Box::new(NoopHost),
        render: Box::new(NoopHost),
        midi_in: Box::new(NoopHost),
        samples: Box::new(samples.clone()),
    };
    let caps = CapabilitySet::browser();
    let reg = InstRegistry::shared();
    reg.borrow_mut().caps = caps;
    let cfg = RuntimeConfig {
        tier: Tier::Browser(Box::new(WasmCellPort(Rc::clone(&host)))),
        ..RuntimeConfig::default()
    };
    let resolver: Rc<dyn InstResolver> = Rc::new(Rc::clone(&reg));
    let (rt, sink) = Runtime::new(hosts, resolver, caps, cfg);
    let ev = Evaluator::with_insts(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(sink),
        Rc::clone(&reg),
    );
    for e in reg.borrow().template_errors() {
        console(&format!("fault template: {e}"));
    }
    let mut m = Main {
        ev,
        rt,
        host,
        samples,
        reg,
    };
    let rep = m.rt.drain(&mut m.ev);
    report_drain(&rep);
    console(&format!("main: ready at {sample_rate} Hz"));
    MAIN.with(|s| *s.borrow_mut() = Some(m));
    1
}

fn report_drain(rep: &DrainReport) -> u32 {
    for d in &rep.diags {
        console(&format!("diag {d}"));
    }
    for f in &rep.faults {
        console(&format!("fault {f}"));
    }
    for (_, line) in &rep.dry_output {
        console(&format!("dry {line}"));
    }
    for line in &rep.console {
        console(&format!("print {line}"));
    }
    u32::try_from(rep.diags.len() + rep.faults.len()).unwrap_or(u32::MAX)
}

/// Evaluates the UTF-8 source at `ptr..ptr + len` and drains the runtime;
/// returns the number of diagnostics and failures (each also a `Console`
/// record).
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn eval(ptr: *const u8, len: u32) -> u32 {
    // SAFETY: forwarded caller contract.
    let bytes = unsafe { input(ptr, len) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        console("fault the source is not UTF-8");
        return 1;
    };
    with(|m| {
        let mut n = 0u32;
        match m.ev.eval_str(text, SOURCE) {
            Err(d) => {
                console(&format!("diag {d}"));
                n += 1;
            }
            Ok(outs) => {
                for o in outs {
                    for d in &o.diags {
                        console(&format!("diag {d}"));
                        n += 1;
                    }
                    if let Err(f) = &o.value {
                        console(&format!("fault {f}"));
                        n += 1;
                    }
                }
            }
        }
        n + report_drain(&m.rt.drain(&mut m.ev))
    })
}

/// One scheduler tick at worklet time `now` (seconds).
#[no_mangle]
pub extern "C" fn tick(now: f64) {
    with(|m| {
        m.host.borrow_mut().now = now;
        let rep = m.rt.tick(&mut m.ev, now);
        for d in &rep.diags {
            console(&format!("diag {d}"));
        }
        for f in &rep.faults {
            console(&format!("fault {f}"));
        }
        for line in &rep.console {
            console(&format!("print {line}"));
        }
    });
}

/// Takes the worklet's framed records (acks, counters, faults, signals).
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn inbox(ptr: *const u8, len: u32) {
    // SAFETY: forwarded caller contract.
    let bytes = unsafe { input(ptr, len) };
    with(|m| {
        let mut h = m.host.borrow_mut();
        for_each_record(bytes, |rec| match rec.first().copied() {
            Some(TAG_FAULT) if rec.len() >= 6 => {
                let code = fault_code(rec[1]);
                let resource = u32::from_le_bytes([rec[2], rec[3], rec[4], rec[5]]);
                let d = Diagnostic::error(
                    code.diag(),
                    Span::new(FileId::new(0), 0, 0),
                    format!("the audio side refused resource {resource} ({code:?})"),
                );
                console(&format!("diag {d}"));
                h.abort(resource);
            }
            Some(TAG_SIGS) if rec.len() >= 37 => {
                let f = |i: usize| f32::from_le_bytes([rec[i], rec[i + 1], rec[i + 2], rec[i + 3]]);
                let mut sigs = HostSigs {
                    amp: f(1),
                    ..HostSigs::default()
                };
                for (k, x) in sigs.fft.iter_mut().enumerate() {
                    *x = f(5 + 4 * k);
                }
                h.sigs = sigs;
            }
            _ => {
                if let Ok((msg, _)) = HostMsg::decode(rec) {
                    h.on_msg(msg);
                }
            }
        });
    });
}

fn fault_code(b: u8) -> FaultCode {
    match b {
        0 => FaultCode::ArenaExhausted,
        1 => FaultCode::InstallQueueOverflow,
        2 => FaultCode::GraphTooLarge,
        3 => FaultCode::BadResource,
        _ => FaultCode::BadRecord,
    }
}

/// Hands over a decoded sample: `len` interleaved little-endian `f32`
/// samples (`4 * len` bytes, any alignment) at `data`, under the key `key`
/// (`bank:index`, e.g. `bd:0`, or a path text).
///
/// # Safety
/// Both ranges must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn sample_put(
    key: *const u8,
    key_len: u32,
    data: *const u8,
    len: u32,
    rate: u32,
    channels: u32,
) -> u32 {
    // SAFETY: forwarded caller contract.
    let key = unsafe { input(key, key_len) };
    let Ok(key) = std::str::from_utf8(key) else {
        return 0;
    };
    // SAFETY: forwarded caller contract (`4 * len` bytes at `data`).
    let bytes = unsafe { input(data, len.saturating_mul(4)) };
    let frames: Box<[f32]> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    let sample = Arc::new(SampleData {
        rate,
        channels: u8::try_from(channels.max(1)).unwrap_or(2),
        frames,
    });
    with(|m| {
        m.samples.0.borrow_mut().insert(key.to_string(), sample);
        1
    })
}

/// Re-synchronizes the worklet cell mirror after the port is re-established
/// (11.3 reconnect snapshot).
#[no_mangle]
pub extern "C" fn main_resync() {
    with(|m| m.rt.resync_cells());
}

/// Main-side observations for the harness: 0 commit lead, 1 cell batches in
/// flight, 2 pending, 3 slices sent, 4 installs queued, 5 slice in flight,
/// 6 `Installed` acks, 7 `Retired` acks, 8 late events, 9 batches sent,
/// 10 batch re-sends, 11 inits sent, 12 events sent, 13 active slots.
#[no_mangle]
pub extern "C" fn main_stat(k: u32) -> f64 {
    with(|m| {
        let h = m.host.borrow();
        let (fl, pend) = m.rt.cells().outstanding();
        let st = m.rt.cells().stats();
        #[allow(clippy::cast_precision_loss)]
        let v = match k {
            0 => m.rt.commit_lead(),
            1 => fl as f64,
            2 => pend as f64,
            3 => h.slices_sent as f64,
            4 => h.installs_queued() as f64,
            5 => f64::from(u8::from(h.slice_in_flight())),
            6 => h.installed as f64,
            7 => h.retired.len() as f64,
            8 => h.late_total as f64,
            9 => f64::from(st.batches_sent),
            10 => f64::from(st.resends),
            11 => f64::from(st.inits_sent),
            12 => h.sent_total as f64,
            13 => m.rt.slots().iter().count() as f64,
            _ => f64::NAN,
        };
        v
    })
}

/// 1 when a `Retired` ack for `resource` arrived.
#[no_mangle]
pub extern "C" fn main_retired(resource: u32) -> u32 {
    with(|m| u32::from(m.host.borrow().retired.contains(&resource)))
}

/// Sets the control the sent-event log records (a negative id clears it).
#[no_mangle]
pub extern "C" fn main_probe_ctl(ctl: i32) {
    with(|m| {
        m.host.borrow_mut().probe_ctl =
            u16::try_from(ctl).ok().map(crate::sched::slots::CtlId::new);
    });
}

/// The `i`th newest sent event: `[time, slot, kind, value]` into `out`
/// (kind 0 no probe control, 1 `Const`, 2 `Cell` with the id as value).
/// Returns 0 when there is no such event.
///
/// # Safety
/// `out` must point to 4 writable `f64`s.
#[no_mangle]
pub unsafe extern "C" fn main_sent(i: u32, out: *mut f64) -> u32 {
    with(|m| {
        let h = m.host.borrow();
        let Some(e) = h.sent.iter().rev().nth(i as usize) else {
            return 0;
        };
        let (kind, value) = match e.probe {
            None => (0.0, 0.0),
            Some(Ctl::Const(v)) => (1.0, f64::from(v)),
            Some(Ctl::Cell(c)) => (2.0, f64::from(c.get())),
        };
        if !out.is_null() {
            // SAFETY: the caller provides 4 writable f64s.
            let o = unsafe { std::slice::from_raw_parts_mut(out, 4) };
            o.copy_from_slice(&[e.time, f64::from(e.slot), kind, value]);
        }
        1
    })
}

/// A UTF-8 name argument ("" when not UTF-8).
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
unsafe fn name_arg<'a>(ptr: *const u8, len: u32) -> &'a str {
    // SAFETY: forwarded caller contract.
    std::str::from_utf8(unsafe { input(ptr, len) }).unwrap_or("")
}

/// The id of instrument `name` (-1 when unknown).
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote.
#[no_mangle]
pub unsafe extern "C" fn inst_id(ptr: *const u8, len: u32) -> i32 {
    // SAFETY: forwarded caller contract.
    let name = unsafe { name_arg(ptr, len) };
    with(|m| {
        m.reg
            .borrow()
            .id_of(intern_kw(name))
            .and_then(|id| i32::try_from(id.get()).ok())
            .unwrap_or(-1)
    })
}

/// The wire id of control `name` (-1 when unknown).
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote.
#[no_mangle]
pub unsafe extern "C" fn ctl_id(ptr: *const u8, len: u32) -> i32 {
    // SAFETY: forwarded caller contract.
    row(unsafe { name_arg(ptr, len) }).map_or(-1, |r| i32::from(r.ctl.get()))
}

/// The resource id of sample `bank:index` (-1 when not requested) and its
/// state in the high byte (0 loading, 1 installed, 2 retiring, 3 failed).
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote.
#[no_mangle]
pub unsafe extern "C" fn sample_id(ptr: *const u8, len: u32) -> i64 {
    // SAFETY: forwarded caller contract.
    let key = unsafe { name_arg(ptr, len) };
    let Some((bank, index)) = key.rsplit_once(':') else {
        return -1;
    };
    let src = SampleSrc::Bank {
        kw: intern_kw(bank),
        index: index.parse().unwrap_or(0),
    };
    with(|m| {
        use crate::sched::commit::SampleState;
        match m.rt.samples().state(&src) {
            None => -1,
            Some((id, st)) => {
                let s: i64 = match st {
                    SampleState::Loading => 0,
                    SampleState::Installed => 1,
                    SampleState::Retiring => 2,
                    SampleState::Failed => 3,
                };
                i64::from(id) | (s << 32)
            }
        }
    })
}

fn post(msg: &CtlMsg) {
    let mut rec = [0u8; CtlMsg::MAX_LEN];
    let n = msg.encode(&mut rec);
    push_record(&rec[..n]);
}

/// Harness: `CellInit { cell, epoch, value }`.
#[no_mangle]
pub extern "C" fn harness_cell_init(cell: u32, epoch: u32, value: f32) {
    post(&CtlMsg::CellInit {
        cell: CellId::new(cell),
        epoch,
        value,
    });
}

/// Harness: `CellRetire { cell, epoch }`.
#[no_mangle]
pub extern "C" fn harness_cell_retire(cell: u32, epoch: u32) {
    post(&CtlMsg::CellRetire {
        cell: CellId::new(cell),
        epoch,
    });
}

fn tag(slot: u32, ch: u32, pitch: u32, seq: u32) -> VoiceTag {
    VoiceTag {
        slot: SlotId::new(slot),
        channel: u8::try_from(ch).unwrap_or(0),
        pitch: u8::try_from(pitch).unwrap_or(0),
        seq,
    }
}

/// Harness: a live note (an open voice) of instrument `inst` with `freq`,
/// the sample resource `bank` when `bank >= 0`, and control `extra` set to
/// `extra_val` when `extra >= 0`.
#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub extern "C" fn harness_live_note(
    slot: u32,
    ch: u32,
    pitch: u32,
    seq: u32,
    inst: u32,
    freq: f32,
    bank: i32,
    extra: i32,
    extra_val: f32,
) {
    let t = tag(slot, ch, pitch, seq);
    let mut ev = AudioEvent::new(0.0, t.slot, 0, InstId::new(inst));
    if let Some(r) = row("freq") {
        let _ = ev.push_ctl(r.ctl, Ctl::Const(freq));
    }
    if let (Some(r), Ok(b)) = (row("bank"), u16::try_from(bank)) {
        let _ = ev.push_ctl(r.ctl, Ctl::Const(f32::from(b)));
    }
    if let Ok(id) = u16::try_from(extra) {
        let _ = ev.push_ctl(crate::sched::slots::CtlId::new(id), Ctl::Const(extra_val));
    }
    post(&CtlMsg::LiveNoteOn { tag: t, ev });
}

/// Harness: `VoiceRelease` of a live-note tag.
#[no_mangle]
pub extern "C" fn harness_voice_release(slot: u32, ch: u32, pitch: u32, seq: u32) {
    post(&CtlMsg::VoiceRelease {
        tag: tag(slot, ch, pitch, seq),
    });
}

/// Harness: a `SlotControl` (release 0 none, 1 natural, 2 panic).
#[no_mangle]
pub extern "C" fn harness_slot_control(slot: u32, new_gen: u32, time: f64, release: u32) {
    let release = match release {
        0 => Release::None,
        1 => Release::Natural,
        _ => Release::Panic,
    };
    post(&CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(slot),
        new_gen,
        effective_time: time,
        release,
    }));
}

/// Harness: a burst that bypasses the sender window: `SampleBegin` of
/// `resource` then `slices` full slices of silence, all at once.
#[no_mangle]
pub extern "C" fn harness_slice_burst(resource: u32, slices: u32) {
    let frames = SLICE_FLOATS * slices as usize;
    let begin = encode_sample_begin(
        resource,
        1,
        u32::try_from(frames).unwrap_or(u32::MAX),
        1,
        48_000,
    );
    push_record(&begin);
    let zeros = vec![0.0f32; SLICE_FLOATS];
    let mut buf = Vec::new();
    for k in 0..slices as usize {
        let off = u32::try_from(k * SLICE_FLOATS).unwrap_or(u32::MAX);
        encode_slice(resource, off, &zeros, &mut buf);
        push_record(&buf);
    }
}

/// Harness: unloads sample `resource` through the host (16.1 retirement).
#[no_mangle]
pub extern "C" fn harness_unload(resource: u32) {
    with(|m| {
        use crate::host::caps::AudioHost;
        WasmAudioHost(Rc::clone(&m.host)).retire_sample(resource);
    });
}

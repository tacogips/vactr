//! The editor's session half (design 15.1.2 G1, `command.md` "Browser
//! transport (raw wasm ABI, TASK-010)"): `main_half`'s twin, built around a
//! `Session` instead of a bare `Evaluator` + `Runtime`. A page calls
//! `session_init` OR `main_init`, never both. This half runs on connection
//! 1 and outputs `TAG_SESSION` (0x71), `TAG_RENDER` (0x72) and `TAG_PKG`
//! (0x73) records plus the same worklet records `main_half` produces
//! (`host::wire`, `dsp::ring`). It carries its own copy of the small
//! worklet-record decoding (`TAG_FAULT`, `TAG_SIGS`, `HostMsg`), because
//! `main_half.rs` is not edited (the TASK-008 harness evidence cannot be
//! re-run headless in every sandbox).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use crate::dsp::arena::{FaultCode, DEFAULT_ARENA_BYTES};
use crate::dsp::caps::CapabilitySet;
use crate::host::caps::{HostSigs, Hosts, SampleData};
use crate::host::noop::NoopHost;
use crate::host::wasm::abi::{for_each_record, input, TAG_FAULT, TAG_PKG, TAG_SESSION, TAG_SIGS};
use crate::host::wasm::messages::{console, HostState, WasmAudioHost, WasmCellPort, WasmSamples};
use crate::host::wasm::session_hosts::{
    parse_midi, push_envelope, push_json, MidiQueue, WasmMidiIn, WasmRenderHost,
};
use crate::host::wire::HostMsg;
use crate::ns::insts::InstRegistry;
use crate::pkg::driver::{DriverReply, DriverRequest, Prefetched};
use crate::pkg::mem_cache::MemCache;
use crate::reader::span::{FileId, Span};
use crate::sched::cells::Tier;
use crate::sched::runtime::RuntimeConfig;
use crate::session::protocol::{Envelope, ErrorCode, ProtocolError, Route, ServerMsg};
use crate::session::{Dest, Outgoing, PersistenceMode, Session, SessionConfig};
use crate::types::diag::Diagnostic;

/// The connection this half answers as (design `command.md`).
const CONN: u32 = 1;
/// The editor's document (`editor/src/code/mount.ts` `DOC_FILE`).
const DOC_FILE: &str = "main.vact";

struct SessionHalf {
    session: Session,
    host: Rc<RefCell<HostState>>,
    samples: WasmSamples,
    midi: MidiQueue,
    supplied: Prefetched,
}

thread_local! {
    static SESSION: RefCell<Option<SessionHalf>> = const { RefCell::new(None) };
}

fn with<R: Default>(f: impl FnOnce(&mut SessionHalf) -> R) -> R {
    SESSION.with(|s| s.borrow_mut().as_mut().map(f).unwrap_or_default())
}

/// Delivers every outgoing message this connection should see: a reply
/// addressed to `CONN`, or a broadcast its subscription wants.
fn emit(session: &Session, out: Vec<Outgoing>) {
    for o in out {
        let deliver = match o.to {
            Dest::Conn(c) => c == CONN,
            Dest::Topic(t) => session.wants(CONN, t),
        };
        if deliver {
            push_envelope(&o.env);
        }
    }
}

/// Every console line (`print`, `at` thunks) since the last flush becomes
/// one `print` console record, like `main_half::report_drain`.
fn flush_console(session: &mut Session) {
    for line in session.take_console() {
        console(&format!("print {line}"));
    }
}

/// Builds the browser `Session` over a worklet arena of `arena_bytes` (0:
/// the 64 MB default) and returns 1.
#[no_mangle]
pub extern "C" fn session_init(sample_rate: f32, arena_bytes: u32) -> u32 {
    let arena = if arena_bytes == 0 {
        DEFAULT_ARENA_BYTES
    } else {
        arena_bytes as usize
    };
    let host = Rc::new(RefCell::new(HostState::new(arena)));
    let samples = WasmSamples::default();
    let midi = MidiQueue::default();
    let hosts = Hosts {
        audio: Box::new(WasmAudioHost(Rc::clone(&host))),
        midi: Box::new(NoopHost),
        osc: Box::new(NoopHost),
        render: Box::new(WasmRenderHost),
        midi_in: Box::new(WasmMidiIn::new(midi.clone())),
        samples: Box::new(samples.clone()),
    };
    let caps = CapabilitySet::browser();
    let reg = InstRegistry::shared();
    reg.borrow_mut().caps = caps;
    let mut cfg = SessionConfig::new(caps);
    cfg.runtime = RuntimeConfig {
        tier: Tier::Browser(Box::new(WasmCellPort(Rc::clone(&host)))),
        ..RuntimeConfig::default()
    };
    cfg.cache = Some(Box::new(MemCache::new()));
    cfg.loader = Box::new(NoopHost);
    cfg.persistence = PersistenceMode::Directive;
    cfg.insts = Some(Rc::clone(&reg));
    let mut session = Session::new(cfg, hosts);
    for e in reg.borrow().template_errors() {
        console(&format!("fault template: {e}"));
    }
    flush_console(&mut session);
    console(&format!("session: ready at {sample_rate} Hz"));
    let half = SessionHalf {
        session,
        host,
        samples,
        midi,
        supplied: Prefetched::new(),
    };
    SESSION.with(|s| *s.borrow_mut() = Some(half));
    1
}

/// Applies one client envelope (JSON text) on connection 1: every reply and
/// wanted broadcast becomes a `TAG_SESSION` record. Non-UTF-8 input becomes
/// one `protocol-error` `bad-json` envelope (seq 0), only when the session
/// exists; the session never panics on malformed input.
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn session_apply(ptr: *const u8, len: u32) {
    // SAFETY: forwarded caller contract.
    let bytes = unsafe { input(ptr, len) };
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            with(|half| {
                let out = half.session.apply_text(CONN, text);
                emit(&half.session, out);
                flush_console(&mut half.session);
            });
        }
        Err(_) => {
            SESSION.with(|s| {
                if s.borrow().is_some() {
                    let env = Envelope::new(
                        0,
                        None,
                        ServerMsg::ProtocolError(ProtocolError::new(
                            ErrorCode::BadJson,
                            "the frame is not UTF-8",
                        )),
                    );
                    push_envelope(&env);
                }
            });
        }
    }
}

/// One session tick at worklet time `now` (seconds): `HostState.now` is set
/// first, then `Session::tick_routed` and its console lines are emitted.
#[no_mangle]
pub extern "C" fn session_tick(now: f64) {
    if !now.is_finite() || now < 0.0 {
        return;
    }
    with(|half| {
        half.host.borrow_mut().now = now;
        let out = half.session.tick_routed(now);
        emit(&half.session, out);
        flush_console(&mut half.session);
    });
}

/// Resolves every output's stored uniform plan at `now` (`Session::
/// render_frame`, G5). Every message's `routing()` decides delivery, the
/// same rule `emit` uses; each is wrapped in its own envelope with seq 0,
/// because `Session::out_seq` is private here and the client does not
/// correlate these unsolicited broadcasts by seq.
#[no_mangle]
pub extern "C" fn session_frame(now: f64) {
    with(|half| {
        for m in half.session.render_frame(now) {
            let deliver = match m.routing() {
                Route::Requester => true,
                Route::Broadcast(t) => half.session.wants(CONN, t),
            };
            if deliver {
                push_envelope(&Envelope::new(0, None, m));
            }
        }
    });
}

fn fault_code(b: u8) -> FaultCode {
    match b {
        0 => FaultCode::ArenaExhausted,
        1 => FaultCode::InstallQueueOverflow,
        2 => FaultCode::GraphTooLarge,
        3 => FaultCode::BadResource,
        5 => FaultCode::OutputChannels,
        _ => FaultCode::BadRecord,
    }
}

/// Takes the worklet's framed records (acks, counters, faults, signals): a
/// local copy of `main_half::inbox` on this half's `HostState`.
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn session_inbox(ptr: *const u8, len: u32) {
    // SAFETY: forwarded caller contract.
    let bytes = unsafe { input(ptr, len) };
    with(|half| {
        let mut h = half.host.borrow_mut();
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

/// Hands over a decoded sample (the session twin of `main_half::
/// sample_put`): `len` interleaved little-endian `f32` samples at `data`,
/// under key `key` (`bank:index` or a path text). Returns 0 when the
/// session is not initialized or `key` is not UTF-8.
///
/// # Safety
/// Both ranges must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn session_sample_put(
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
    with(|half| {
        half.samples.0.borrow_mut().insert(key.to_string(), sample);
        1
    })
}

/// A `session_check` request body: `{"file"?: str, "code": str}`.
#[derive(Deserialize)]
struct CheckRequest {
    #[serde(default)]
    file: Option<String>,
    code: String,
}

/// Static analysis of the document text, never executes (`Session::check`).
/// UTF-8 input that parses as `{"file"?, "code"}` uses those; otherwise the
/// whole input text is the document text under `DOC_FILE` (the editor's
/// `WasmCore.check` passes raw text). Non-UTF-8 input checks an empty text
/// under `DOC_FILE`, so exactly one `TAG_SESSION` record `{"kind":"check",
/// "file","diagnostics"}` is always emitted (when the session exists).
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn session_check(ptr: *const u8, len: u32) {
    // SAFETY: forwarded caller contract.
    let bytes = unsafe { input(ptr, len) };
    let text = std::str::from_utf8(bytes).unwrap_or("");
    let (file, code) = match serde_json::from_str::<CheckRequest>(text) {
        Ok(req) => (req.file.unwrap_or_else(|| DOC_FILE.to_string()), req.code),
        Err(_) => (DOC_FILE.to_string(), text.to_string()),
    };
    with(|half| {
        let diagnostics = half.session.check(&file, &code);
        let rec = json!({
            "kind": "check",
            "file": file,
            "diagnostics": diagnostics,
        });
        push_json(TAG_SESSION, &rec.to_string());
    });
}

/// Raw MIDI bytes at audio-clock time `time`: decoded with [`parse_midi`]
/// and buffered for `WasmMidiIn::poll` (11.7). Undecodable bytes are
/// ignored.
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn session_midi_in(ptr: *const u8, len: u32, time: f64) {
    // SAFETY: forwarded caller contract.
    let bytes = unsafe { input(ptr, len) };
    if let Some(e) = parse_midi(bytes, time) {
        with(|half| half.midi.push(e));
    }
}

/// One package-driver step (`pkg::driver::drive` through `Session::
/// drive_packages`): non-UTF-8 input or a `DriverRequest::from_json` error
/// becomes a `TAG_PKG` `DriverReply::Error` `bad-body` record; otherwise
/// the reply of one driver step. A no-op before `session_init`.
///
/// # Safety
/// `ptr..ptr + len` must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn pkg_resolve(ptr: *const u8, len: u32) {
    // SAFETY: forwarded caller contract.
    let bytes = unsafe { input(ptr, len) };
    with(|half| {
        let reply = match std::str::from_utf8(bytes) {
            Err(_) => DriverReply::Error {
                code: "bad-body".to_string(),
                message: "the frame is not UTF-8".to_string(),
            },
            Ok(text) => match DriverRequest::from_json(text) {
                Ok(req) => half.session.drive_packages(req, &mut half.supplied),
                Err(message) => DriverReply::Error {
                    code: "bad-body".to_string(),
                    message,
                },
            },
        };
        push_json(TAG_PKG, &reply.to_json());
    });
}

/// Supplies a fetched body for `url`: `status` 200 is a successful body,
/// any other status a failed response (`404` not found, otherwise a
/// network failure). A non-UTF-8 `url` is ignored.
///
/// # Safety
/// Both ranges must be memory JS wrote (from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn pkg_supply(
    url_ptr: *const u8,
    url_len: u32,
    status: u32,
    ptr: *const u8,
    len: u32,
) {
    // SAFETY: forwarded caller contract.
    let url_bytes = unsafe { input(url_ptr, url_len) };
    let Ok(url) = std::str::from_utf8(url_bytes) else {
        return;
    };
    // SAFETY: forwarded caller contract.
    let body = unsafe { input(ptr, len) };
    with(|half| {
        if status == 200 {
            half.supplied.supply(url, body.to_vec());
        } else {
            half.supplied
                .supply_status(url, u16::try_from(status).unwrap_or(0));
        }
    });
}

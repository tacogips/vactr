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
    complete_song_banks: std::collections::BTreeMap<crate::value::intern::KwId, Vec<String>>,
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
    if !sample_rate.is_finite()
        || !(8000.0..=192000.0).contains(&sample_rate)
        || sample_rate.fract() != 0.0
    {
        console("fault song: unsupported or nonintegral browser sample rate");
        return 0;
    }

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
        sample_rate: sample_rate as u32,
        ..RuntimeConfig::default()
    };
    cfg.cache = Some(Box::new(MemCache::new()));
    cfg.loader = Box::new(NoopHost);
    cfg.persistence = PersistenceMode::Directive;
    cfg.insts = Some(Rc::clone(&reg));
    cfg.song_assets = Some(samples.song_factory(
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::new(),
    ));
    let mut session = Session::new(cfg, hosts);
    let asset_limits = crate::song::assets::SongAssetLimits {
        max_resources: 256,
        max_pcm_bytes: arena as u64,
        max_source_files: 64,
        max_source_bytes: 1_000_000,
        max_banks: 64,
        max_walk_nodes: 100_000,
        max_walk_depth: 256,
    };
    if let Err(error) = session.set_song_asset_limits(asset_limits).and_then(|()| {
        session.set_song_preparation_limits(crate::host::caps::SongPreparationLimits {
            capabilities: caps,
            song: crate::song::SongLimits::default(),
            max_resources: 256,
            max_pending_records: 4096,
            max_graph_bytes: 1_000_000,
            max_work: 8_000_000,
        })
    }) {
        console(&format!("fault song configuration: {}", error.message));
        return 0;
    }

    for e in reg.borrow().template_errors() {
        console(&format!("fault template: {e}"));
    }
    flush_console(&mut session);
    console(&format!("session: ready at {sample_rate} Hz"));
    let half = SessionHalf {
        session,
        host,
        samples,
        complete_song_banks: std::collections::BTreeMap::new(),
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
        half.session
            .set_song_asset_factory(Some(half.samples.song_factory(
                half.complete_song_banks.clone(),
                std::collections::BTreeMap::new(),
            )));
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

/// Installs a complete ordered bank catalog and refreshes future candidate assets.
/// Page upload integration calls this only after all listed PCM has arrived.
pub fn set_song_bank_catalog(
    catalog: std::collections::BTreeMap<crate::value::intern::KwId, Vec<String>>,
) -> u32 {
    with(|half| {
        half.complete_song_banks = catalog;
        half.session
            .set_song_asset_factory(Some(half.samples.song_factory(
                half.complete_song_banks.clone(),
                std::collections::BTreeMap::new(),
            )));
        1
    })
}

/// Refresh future candidate assets immediately before candidate application.
/// Already prepared candidates retain their closed inventories.
pub fn refresh_song_asset_factory() -> u32 {
    with(|half| {
        half.session
            .set_song_asset_factory(Some(half.samples.song_factory(
                half.complete_song_banks.clone(),
                std::collections::BTreeMap::new(),
            )));
        1
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SongBankCatalog {
    banks: Vec<SongBankEntry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SongBankEntry {
    name: String,
    members: Vec<String>,
}

/// Publish a complete ordered catalog of decoded sample keys for future Songs.
/// Invalid catalogs return zero and preserve the current catalog and factory.
/// # Safety
/// ptr..ptr+len is readable memory from the page; len must not exceed 1MiB.
#[no_mangle]
pub unsafe extern "C" fn session_song_bank_catalog(ptr: *const u8, len: u32) -> u32 {
    if len > 1024 * 1024 {
        console("fault song catalog: JSON exceeds 1MiB");
        return 0;
    }
    // SAFETY: the ABI caller guarantees the supplied readable range.
    let bytes = unsafe { input(ptr, len) };
    let catalog = match serde_json::from_slice::<SongBankCatalog>(bytes) {
        Ok(catalog) if catalog.banks.len() <= 64 => catalog,
        _ => {
            console("fault song catalog: invalid JSON or more than 64 banks");
            return 0;
        }
    };
    with(|half| {
        let mut complete = std::collections::BTreeMap::new();
        let mut members = 0usize;
        let decoded = half.samples.0.borrow();
        for bank in catalog.banks {
            members = match members.checked_add(bank.members.len()) {
                Some(count) if count <= 16384 => count,
                _ => {
                    console("fault song catalog: more than 16384 members");
                    return 0;
                }
            };
            if bank.name.is_empty()
                || bank.name.len() > 4096
                || bank.name.starts_with(':')
                || bank.name.chars().any(char::is_whitespace)
                || bank.name.chars().any(char::is_control)
            {
                console("fault song catalog: invalid bank name");
                return 0;
            }
            let keyword = crate::value::intern::intern_kw(&bank.name);
            if complete.contains_key(&keyword) {
                console("fault song catalog: duplicate bank name");
                return 0;
            }
            for key in &bank.members {
                if key.is_empty()
                    || key.len() > 4096
                    || key.chars().any(char::is_control)
                    || !decoded.contains_key(key)
                {
                    console("fault song catalog: invalid or missing decoded PCM key");
                    return 0;
                }
            }
            complete.insert(keyword, bank.members);
        }
        drop(decoded);
        half.complete_song_banks = complete;
        half.session
            .set_song_asset_factory(Some(half.samples.song_factory(
                half.complete_song_banks.clone(),
                std::collections::BTreeMap::new(),
            )));
        1
    })
}

/// Detach future candidate assets from the mutable page sample store.
#[no_mangle]
pub extern "C" fn session_song_refresh_assets() -> u32 {
    refresh_song_asset_factory()
}

#[cfg(test)]
mod song_asset_tests {
    use super::*;
    use crate::host::wasm::abi;
    fn take_records() -> Vec<Vec<u8>> {
        let n = abi::outbox_len();
        // SAFETY: copied while the actual outbox allocation remains unchanged.
        let bytes = unsafe { input(abi::outbox_ptr(), n) }.to_vec();
        let mut records = Vec::new();
        for_each_record(&bytes, |record| records.push(record.to_vec()));
        abi::outbox_clear();
        records
    }
    #[test]
    fn catalog_requires_complete_pcm_and_keeps_prior_catalog_on_refusal() {
        abi::fix_outbox(0);
        assert_eq!(session_init(8000.0, 1024 * 1024), 1);
        let key = b"kit:0";
        let data = [0.25f32, -0.25];
        // SAFETY: both arrays are live and exactly sized for the ABI call.
        assert_eq!(
            unsafe {
                session_sample_put(
                    key.as_ptr(),
                    key.len() as u32,
                    data.as_ptr().cast(),
                    data.len() as u32,
                    8000,
                    2,
                )
            },
            1
        );
        let good = br#"{"banks":[{"name":"kit","members":["kit:0"]}]}"#;
        // SAFETY: the JSON byte array stays live throughout the call.
        assert_eq!(
            unsafe { session_song_bank_catalog(good.as_ptr(), good.len() as u32) },
            1
        );
        let prior = with(|half| half.complete_song_banks.clone());
        for bad in [
            r#"{"banks":[{"name":"kit","members":["missing"]}]}"#,
            r#"{"banks":[{"name":"kit","members":[]},{"name":"kit","members":[]}]}"#,
            r#"{"banks":[],"unexpected":true}"#,
        ] {
            // SAFETY: borrowed JSON bytes are readable for their full length.
            assert_eq!(
                unsafe { session_song_bank_catalog(bad.as_ptr(), bad.len() as u32) },
                0
            );
            assert_eq!(with(|half| half.complete_song_banks.clone()), prior);
        }
        assert_eq!(session_init(8000.5, 1024 * 1024), 0);
        assert_eq!(with(|half| half.complete_song_banks.clone()), prior);
        // SAFETY: oversized len is rejected before any read of the null pointer.
        assert_eq!(
            unsafe { session_song_bank_catalog(std::ptr::null(), 1024 * 1024 + 1) },
            0
        );
        assert_eq!(with(|half| half.complete_song_banks.clone()), prior);
    }
    #[test]
    fn refresh_keeps_original_prepared_pcm_immutable() {
        abi::fix_outbox(0);
        assert_eq!(session_init(8000.0, 1024 * 1024), 1);
        let key = b"bd:0";
        let data = [0.25f32, -0.25];
        // SAFETY: the sample arrays are live for the ABI read.
        assert_eq!(
            unsafe {
                session_sample_put(
                    key.as_ptr(),
                    key.len() as u32,
                    data.as_ptr().cast(),
                    data.len() as u32,
                    8000,
                    2,
                )
            },
            1
        );
        let catalog = br#"{"banks":[{"name":"bd","members":["bd:0"]}]}"#;
        // SAFETY: catalog points to exactly the supplied readable bytes.
        assert_eq!(
            unsafe { session_song_bank_catalog(catalog.as_ptr(), catalog.len() as u32) },
            1
        );
        let original = with(|half| half.session.song_asset_factory()).unwrap();
        let cx = crate::session::song::CandidateBuildCtx {
            assets: original.as_ref(),
            asset_limits: crate::song::assets::SongAssetLimits {
                max_resources: 256,
                max_pcm_bytes: 1024 * 1024,
                max_source_files: 64,
                max_source_bytes: 1_000_000,
                max_banks: 64,
                max_walk_nodes: 100_000,
                max_walk_depth: 256,
            },
            lock: None,
            cache: None,
        };
        let code = "song {part [drums: {s :bd}] duration: 1} > play-song";
        let mut prepared = crate::song::prepare_song(
            crate::session::song::evaluate_song_candidate(
                code,
                "main.vact",
                1,
                crate::song::SnapshotEpoch(7),
                &cx,
            )
            .unwrap(),
        )
        .unwrap();
        let source = crate::host::caps::SampleSrc::Bank {
            kw: crate::value::intern::intern_kw("bd"),
            index: 0,
        };
        assert_eq!(prepared.sample(&source).unwrap().frames[0], 0.25);
        let new = [0.75f32, -0.75];
        // SAFETY: replacement arrays remain live while the ABI snapshots them.
        assert_eq!(
            unsafe {
                session_sample_put(
                    key.as_ptr(),
                    key.len() as u32,
                    new.as_ptr().cast(),
                    new.len() as u32,
                    8000,
                    2,
                )
            },
            1
        );
        assert_eq!(session_song_refresh_assets(), 1);
        let future = with(|half| half.session.song_asset_factory()).unwrap();
        let future_cx = crate::session::song::CandidateBuildCtx {
            assets: future.as_ref(),
            ..cx
        };
        let mut next = crate::song::prepare_song(
            crate::session::song::evaluate_song_candidate(
                code,
                "main.vact",
                2,
                crate::song::SnapshotEpoch(8),
                &future_cx,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(prepared.sample(&source).unwrap().frames[0], 0.25);
        assert_eq!(next.sample(&source).unwrap().frames[0], 0.75);
    }
    #[test]
    fn browser_session_original_song_reaches_real_worklet_applied_and_ended() {
        use crate::host::wasm::worklet_half as worklet;
        assert_eq!(worklet::worklet_init(8000.0, 1024 * 1024, 16), 1);
        assert_eq!(session_init(8000.0, 1024 * 1024), 1);
        let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > gain 0.1}] duration: 1/8} tail-seconds: 0 > play-song";
        let request = serde_json::json!({"v":1,"seq":11,"kind":"apply-song","body":{
            "file":"main.vact","code":code,"doc_revision":1,"edit_epoch":0}})
        .to_string();
        // SAFETY: request bytes are live and fully readable for this ABI call.
        unsafe {
            session_apply(request.as_ptr(), request.len() as u32);
        }
        let mut pending = std::collections::VecDeque::new();
        let mut applied = false;
        let mut audible = false;
        let mut ended = false;
        for _ in 0..512 {
            for record in take_records() {
                if record.first() == Some(&TAG_SESSION) {
                    let message: serde_json::Value = serde_json::from_slice(&record[1..]).unwrap();
                    assert_ne!(message["kind"], "song-candidate-failed", "{message}");
                    if message["kind"] == "song-candidate-applied" {
                        assert_eq!(message["re"], 11);
                        assert!(message["body"]["application_frame"].is_string());
                        applied = true;
                    }
                } else if record.first().is_some_and(|tag| *tag < TAG_FAULT) {
                    pending.push_back(record);
                }
            }
            for _ in 0..4 {
                let Some(record) = pending.front() else { break };
                assert!(record.len() <= crate::dsp::ring::INBOX_SLOT_BYTES);
                // SAFETY: genuine staging allocation has at least INBOX_SLOT_BYTES.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        record.as_ptr(),
                        worklet::staging_ptr(),
                        record.len(),
                    );
                }
                if worklet::worklet_inbox(record.len() as u32) == 0 {
                    break;
                }
                pending.pop_front();
            }
            let output = worklet::process(128);
            // SAFETY: worklet emits 128 left then128 right samples until next process.
            let pcm = unsafe { std::slice::from_raw_parts(output, 256) };
            audible |= pcm.iter().any(|value| value.abs() > 1e-6);
            let mut feedback = Vec::new();
            for record in take_records() {
                assert_ne!(
                    record.first(),
                    Some(&TAG_FAULT),
                    "real worklet fault: {record:?}"
                );
                feedback.extend_from_slice(&(record.len() as u32).to_le_bytes());
                feedback.extend_from_slice(&record);
            }
            // SAFETY: all feedback bytes remain live for the complete ABI read.
            unsafe {
                session_inbox(feedback.as_ptr(), feedback.len() as u32);
            }
            session_tick(worklet::worklet_now());
            if with(|half| half.session.runtime().song_state())
                == Some(crate::sched::song::SongTransportState::Ended)
            {
                ended = true;
                // Applied is consumed from actual TAG_SESSION on the following loop.
                if applied {
                    break;
                }
            }
        }
        assert!(
            applied && audible && ended,
            "applied={applied} audible={audible} ended={ended}"
        );
    }
}

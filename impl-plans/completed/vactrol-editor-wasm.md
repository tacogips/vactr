# Vactrol Editor: Browser Session over the Raw Wasm ABI (ED-WASM) Implementation Plan

**planId**: ED-WASM (issue #5, TASK-010, wave 3; G1: `session_*` and `pkg_*` exports, `WasmRenderHost`, `WasmMidiIn`,
the `0x71`..`0x73` records, and its own real-wasm ABI smoke test)
**Status**: Completed (implemented, gate-verified, adversarial review and integration review accepted in session 187; removed from the dispatch manifest by the session-188 amendment; source rides in the single workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 15.1.2 G1 (and the G5/G6 glue), 12.8.10, 16, 16.1,
17; design-docs/specs/command.md "Browser transport (raw wasm ABI, TASK-010)"
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-WIRE (uses `Session::check`, `Session::render_frame`, `Session::drive_packages`, `pkg::driver`,
`MemCache`), ED-SCAFFOLD (vitest, `worklet/host.js` `init: 'session'`, `test/support/zip.ts`)
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

`src/host/wasm/main_half.rs` builds `Evaluator` + `Runtime` directly (line 84), so the browser has no Session Protocol
v1 path. This plan adds a SESSION half beside it, compiled only under
`all(target_arch = "wasm32", feature = "host-wasm")` like the existing halves. The session half builds `Session::new`
with:
- `CapabilitySet::browser()`;
- `RuntimeConfig { tier: Tier::Browser(Box::new(WasmCellPort(..))), ..Default }`;
- a shared `InstRegistry` (`SessionConfig::insts`);
- `Hosts { audio: WasmAudioHost, midi: NoopHost, osc: NoopHost, render: WasmRenderHost, midi_in: WasmMidiIn,
  samples: WasmSamples }`;
- `loader: NoopHost`;
- `PersistenceMode::Directive`.

It then marshals JSON text through the outbox. A page uses `main_init` (the dev harness) OR `session_init` (the
editor), never both.

## Non-Goals

- `main_half.rs`, `worklet_half.rs`, `messages.rs`, `cells.rs` and `editor/worklet/processor.js` are NOT edited. The
  TASK-008 harness keeps its behavior.
- No protocol semantics here. Everything is delegated to `Session` and the ED-WIRE functions.
- No browser MIDI out, no browser taps (E2).

## writePaths

- `src/host/wasm/session_half.rs` (new), `src/host/wasm/session_hosts.rs` (new: `WasmRenderHost`, `WasmMidiIn`, the
  record encoders), `src/host/wasm/mod.rs`, `src/host/wasm/abi.rs` (the three tag constants only)
- `editor/test/support/wasm.ts` (new: the real-wasm loader for node tests), `editor/test/wasm/abi.test.ts` (new)
- `impl-plans/active/vactrol-editor-wasm.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **`abi.rs`.** `pub const TAG_SESSION: u8 = 0x71; TAG_RENDER: u8 = 0x72; TAG_PKG: u8 = 0x73;`, documented.
2. **`session_hosts.rs`.**
   - `WasmRenderHost` (`RenderHost`) pushes `TAG_RENDER` JSON records:
     - `set_program`: `{"op":"program","out":n,"source":..,"uniform_names":[..],"assets":[{"id":..,"text":..}]}`;
     - `set_uniforms`: `{"op":"uniforms","out":n,"values":[..]}`.

     `OutId` maps to 0..3, and `serde_json` builds the JSON.
   - `WasmMidiIn` (`MidiInHost`): a `Vec<MidiInEvent>` buffer filled by `session_midi_in`. `poll` returns the buffered
     events and clears them at the next push cycle. Decode status, data1 and data2 into the existing `MidiInEvent`
     shape (CC, note on/off, clock 0xF8, start 0xFA, stop 0xFC, continue 0xFB) with the given audio-clock time. Other
     bytes are ignored.
3. **`session_half.rs`**: `thread_local! SESSION: RefCell<Option<SessionHalf { session, host: Rc<RefCell<HostState>>,
   samples: WasmSamples, midi: Rc<RefCell<..>>, supplied: Prefetched }>>`.

   Exports (each `#[no_mangle] extern "C"`; the unsafe ones carry the `alloc` contract comment as in `main_half.rs`):
   - `session_init(sample_rate: f32, arena_bytes: u32) -> u32`: builds everything above, drains, pushes the
     console-ready line, returns 1.
   - `session_apply(ptr, len)`: UTF-8 text goes through `Session::apply_text(1, text)`, and each `Outgoing` envelope is
     encoded with the existing `session::codec` into a `TAG_SESSION` record. Non-UTF-8 input produces a
     `protocol-error bad-json` envelope record. Nothing panics.
   - `session_tick(now: f64)`: sets `HostState.now`, runs `Session::tick(now)`, then emits `TAG_SESSION` records.
     Subscription routing uses connection 1.
   - `session_frame(now: f64)`: `Session::render_frame(now)`. Render records come from the host, and any `diag` from
     the returned messages.
   - `session_inbox(ptr, len)`: the same record cases as `main_half::inbox` (`TAG_FAULT`, `TAG_SIGS`,
     `HostMsg::decode`), on this half's `HostState`. It is a local copy: `main_half.rs` is not edited.
   - `session_sample_put(key_ptr, key_len, data_ptr, len, rate, channels) -> u32`: the same contract as `sample_put`,
     into this half's `WasmSamples`.
   - `session_check(ptr, len)`: JSON `{"file": str, "code": str}` goes to `Session::check`, which emits one
     `TAG_SESSION` record `{"kind":"check","file":..,"diagnostics":[..]}`.
   - `session_midi_in(ptr, len, time: f64)`: appends to `WasmMidiIn`.
   - `pkg_resolve(ptr, len)`: parses the `{proxy, requirements}` or `{proxy, lock}` JSON into `DriverRequest`, runs
     `Session::drive_packages`, and emits one `TAG_PKG` record (`DriverReply::to_json`).
   - `pkg_supply(url_ptr, url_len, status: u32, ptr, len)`: stores the body (200), a not-found (404) or a network
     failure (other) in `supplied`.
4. **`mod.rs`.** `pub mod session_half; pub mod session_hosts;` with a module doc line.
5. **`editor/test/support/wasm.ts`.** `loadVactrolWasm()`:
   - reads `process.env.VACTROL_WASM` or `../target/wasm32-unknown-unknown/debug/vactrol.wasm` with `node:fs`;
   - instantiates it with `{}` imports;
   - THROWS (the test fails, never skips) when the file is missing or `session_init` is not exported;
   - returns helpers `call(name, ...)`, `withBytes`, `drainRecords() -> {tag, bytes}[]`, and a JSON decoder for
     `0x71`..`0x73`.
6. **`editor/test/wasm/abi.test.ts`** (`// @vitest-environment node`). Required tests:
   - `session_init` returns 1.
   - `session_apply` of `subscribe` + `eval` of `s [:bd :sd] > d1` yields an `eval-result` envelope with sites.
   - `session_tick` at increasing mock times crossing one cycle yields a `playing` envelope with `src.doc_revision`
     equal to the eval's revision.
   - `session_apply` of garbage yields `protocol-error`.
   - `session_check` of a type error yields a diagnostic and no `playing` afterwards.
   - Evaluating `osc 20 > rotate 0.5 > out o0` yields a `0x72` `program` record only after the tick crossing the next
     cycle boundary; then `session_frame` yields a `uniforms` record whose `values` length equals `uniform_names`.
   - The `pkg_resolve` / `pkg_supply` loop against in-memory proxy bodies (a fixture package zipped with
     `test/support/zip.ts`) ends in `done` with a lock line. A second run with a tampered zip ends in `error`
     `package-integrity`.
   - `session_midi_in` of CC 74 value 127 on channel 1, followed by one `session_tick`, updates the `cc` input cell.
     Assert it with the same observation the native `cc`-cell test in `src/sched/tests/midi.rs` uses: reuse its
     program text, and read the value from the `eval-result`/`playing` output. If that test observes the value only
     through Rust internals, assert instead that a pattern `s :bd > gain (cc 74)` evaluates without a diagnostic and
     keeps producing `playing` events after the MIDI input. Record the choice in `notes.md`.
   - The worklet-bound records are never tagged `0x71`..`0x73`.

## Invariants

- `main_half.rs` exports and the dev-harness behavior are unchanged (`git diff --exit-code -- src/host/wasm/main_half.rs
  src/host/wasm/worklet_half.rs src/host/wasm/messages.rs src/host/wasm/cells.rs editor/worklet/processor.js
  editor/dev-harness`).
- Every export is panic-free on malformed input, and every error becomes a record.
- `session_half.rs` and `session_hosts.rs` stay under 800 lines.
- Both wasm32 builds are green, and V9 shows no gated crate.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-WASM`. Rust goes through the rust-coding
agent, then check-and-test-after-modify. Clippy does not lint wasm32-only code on the host target, so also run X1.

## Verification (`<wave>` = `wasm`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4, E0-E5 (E1 = `npm ls --depth=0`, E4 to
`target/ed-dist/ED-WASM`), plus V1l, V2l and V9. E4 runs with
`VACTROL_REQUIRE_SESSION_ABI=1`. Plus:

| # | Command | Evidence |
|---|---------|----------|
| X1 | LOG(`clippy-wasm32`): `CARGO_TERM_QUIET=true cargo clippy --target wasm32-unknown-unknown --no-default-features --features host-wasm -- -D warnings` | `exit=0` |
| X2 | LOG(`abi`): `cd editor && VACTROL_WASM=$ROOT/target/ed-wasm/ED-WASM.wasm npx vitest run test/wasm/abi.test.ts` | `exit=0`, all abi tests passed |
| X3 | `git diff --exit-code -- src/host/wasm/main_half.rs src/host/wasm/worklet_half.rs src/host/wasm/messages.rs src/host/wasm/cells.rs editor/worklet/processor.js editor/dev-harness` | exit 0 |

## Completion Criteria

- [x] Items 1-6 implemented
- [x] `abi.test.ts` passes against the real host-wasm artifact
- [x] Common rows, V1l/V2l/V9 and X1-X3 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-WASM implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 187, ED-WASM implementer)

**Tasks Completed**: items 1-6. Evidence: `tmp/ed-editor-20260926-s186/ED-WASM/attempt-1/` (intent.md, notes.md,
pre/post/final hashes, run.sh). Logs: `target/fe-logs/ed-wasm-<check>-s187-1.log`.

- Rust (rust-coding agent, owner-reviewed): `abi.rs` +3 tag constants; `mod.rs` declares `session_half`,
  `session_hosts`; `session_hosts.rs` (159 lines: `WasmRenderHost`, `WasmMidiIn` + `MidiQueue`, local
  `parse_midi`, `push_json`/`push_envelope`); `session_half.rs` (401 lines: the ten `session_*`/`pkg_*` exports,
  connection 1, panic-free on malformed input).
- TS: `test/support/wasm.ts` (real-artifact loader, throws on a missing file or no `session_init`; worklet
  install-ack and `session_sample_put` helpers); `test/wasm/abi.test.ts` (8 tests: init and tag separation,
  eval-result with sites and `playing` `src.doc_revision`, protocol-error, check never executes, the `0x72`
  program at the cycle boundary then uniforms per frame and none after hush, pkg need/supply done + tampered
  restore `package-integrity` + 404 `package-resolve` + malformed request, MIDI CC 74 = 127 read as gain 1.0).
- Decisions (notes.md): `session_check` accepts the plan's JSON and the raw text the accepted `WasmCore.check`
  sends; the MIDI criterion uses the native test's program text and observation (the sent events' gain).
- Artifact collision (repair request for the operator and ED-FINAL): the `vactrol` bin and cdylib both uplift
  to `target/wasm32-unknown-unknown/debug/vactrol.wasm`, and V6b leaves the stub bin there
  (`ed-wasm-artifact-after-v6b-s187-1.log`: 4 exports, no `session_init`). Before V6c this plan ran
  `cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm --lib`
  (`ed-wasm-wasm32-hostwasm-lib-s187-1.log`, exit=0), which re-uplifts the cdylib; V6c then copied it
  (`ed-wasm-artifact-ed-wasm-s187-1.log`: 48 exports, `session_init`, identical to `deps/vactrol.wasm`).

**Verification** (all `exit=0`): build, build-lsp (V1l), clippy, clippy-lsp (V2l); nextest 1007 run, 1007 passed,
1 skipped; cargo test (lib 986, cli 9, directive_fixtures 2, spec_fixtures 10 + 1 ignored); fmt; wasm32;
wasm32-hostwasm; rs-lines (max 799, `src/dsp/build.rs`); tree and tree-hostwasm (V9: no cpal, midir, tungstenite,
getrandom, tokio or tower-lsp); node (v26.9.0, npm 11.19.1); npm-ls; npm-check; npm-test (52 files, 324 tests
passed); npm-build with `VACTROL_REQUIRE_SESSION_ABI=1`; dist-check; ts-lines (max 447); X1 clippy-wasm32; X2 abi
(1 file, 8 tests passed); X3 x3-invariant.

**Notes**: formal test-integrity, adversarial and integration reviews are downstream workflow steps.

### CLOSING NOTE (ED-FINAL, session 188)

Status confirmed Completed (accepted). Final-tree evidence (ED-FINAL attempt-2, `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/`): join integrity re-checked (this plan's hashes OK or explained); every row exit=0 in `target/fe-logs/ed-final-*-s188-1.log`: build, build-lsp, clippy, clippy-lsp, fmt, nextest (1007 passed, 1 skipped), cargo test, both wasm32 builds, clippy wasm32 host-wasm, npm ci/check/test (54 files, 336 tests)/build (`VACTROL_REQUIRE_SESSION_ABI=1`), real-wasm vitest (3 files, 20 tests), Tauri fetch/check/fmt, session subset, lsp_smoke, spec fixtures. Own evidence: `test/wasm/abi.test.ts` (8 tests) in target/fe-logs/ed-final-wasm-tests-verbose-s188-1.log; the artifact is the host-wasm cdylib re-uplifted by the `--lib` row (target/fe-logs/ed-final-wasm32-hostwasm-lib-s188-1.log, target/fe-logs/ed-final-v6c-copy-s188-1.log: 48 exports, `session_init`).

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-wire.md, vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-bind.md,
  vactrol-editor-visual.md
- **Next**: vactrol-editor-finalize.md

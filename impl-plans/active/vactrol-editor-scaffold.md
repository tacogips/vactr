# Vactrol Editor: Scaffold, Protocol Client, Store, Worklet Glue (ED-SCAFFOLD) Implementation Plan

**planId**: ED-SCAFFOLD (issue #5, TASK-010, wave 1; the `editor/` Vite + TypeScript + vitest app skeleton, Session
Protocol v1 client, three transports, the batch store, platform file access, `editor/worklet/host.js` options)
**Status**: Completed (implemented, gate-verified, adversarial review and integration review accepted in session 186; removed from the dispatch manifest by the session-187 amendment; source rides in the single workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 15.1.1, 15.1.3, 15.1.4, 15.1.6 ("Reactive displays",
"Persistence and saving" file access), 15.1.12; design-docs/specs/command.md "Session Protocol (v1)", "Browser transport
(raw wasm ABI, TASK-010)", "Editor files (TASK-010)"; design-docs/user-qa/pending-editor-questions.md E5
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: (none)
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

`editor/` today holds only the plain-JS TASK-008 worklet glue (`editor/worklet/host.js`, `processor.js`) and the dev
harness (`editor/dev-harness/*`). This plan creates the TypeScript app every other ED plan builds on:
- the npm project (the ONLY writer of `editor/package.json` and `editor/package-lock.json`);
- the protocol client: types, envelope, document sync, UTF-8 offsets, three transports;
- the atomic batch store;
- the `mount.ts` stub per area (seed-then-fill, design 14.5.3);
- platform file access (browser, Tauri, and an in-memory version for tests);
- the additive `host.js` options.

The protocol types include EVERY v1 shape plus the TASK-010 additions of command.md (`site.call`, `manifest.editors`,
`levels[].bands`, `levels.analyzers`, `tempo.clock`, and the `0x71`/`0x72`/`0x73` payloads), so no later plan edits
`protocol/types.ts`.

## Non-Goals

- No CodeMirror UI (ED-CODE), sliders (ED-BIND), MIDI (ED-MIDI), WebGL (ED-VISUAL), packages UI (ED-PKG), parameter
  editors (ED-PARAMS), Tauri crate (ED-TAURI), or Rust (ED-WIRE, ED-WASM).
- No UI framework, CSS framework, lint or formatting tool. Add no dependency beyond the list in item 1.
- No change to `editor/worklet/processor.js` or `editor/dev-harness/*`, and no `mise.toml` edit (E5).
- No real-wasm test. This plan's transport tests use `test/support/fake-core.ts`.

## writePaths

- `editor/package.json`, `editor/package-lock.json`, `editor/tsconfig.json`, `editor/vite.config.ts`,
  `editor/vitest.config.ts`, `editor/index.html`, `editor/.gitignore`
- `editor/src/app/main.ts`, `editor/src/app/layout.ts`, `editor/src/app/deps.ts`, `editor/src/app/apis.ts`,
  `editor/src/app/clock.ts`, `editor/src/app/app.css`
- `editor/src/protocol/types.ts`, `editor/src/protocol/envelope.ts`, `editor/src/protocol/client.ts`,
  `editor/src/protocol/document.ts`, `editor/src/protocol/utf8.ts`, `editor/src/protocol/transport.ts`,
  `editor/src/protocol/socket.ts`, `editor/src/protocol/wasm.ts`, `editor/src/protocol/store.ts`
- `editor/src/platform/files.ts`
- stubs, each filled later by its owner: `editor/src/code/mount.ts`, `editor/src/bind/mount.ts`,
  `editor/src/params/mount.ts`, `editor/src/visual/mount.ts`, `editor/src/midi/mount.ts`, `editor/src/pkg/mount.ts`
- `editor/worklet/host.js`, `editor/worklet/host.d.ts`
- `editor/test/support/recording.ts`, `editor/test/support/clock.ts`, `editor/test/support/fake-core.ts`,
  `editor/test/support/zip.ts`
- `editor/test/protocol/client.test.ts`, `editor/test/protocol/document.test.ts`, `editor/test/protocol/utf8.test.ts`,
  `editor/test/protocol/store.test.ts`, `editor/test/protocol/socket.test.ts`,
  `editor/test/protocol/wasm-transport.test.ts`, `editor/test/protocol/host-js.test.ts`,
  `editor/test/platform/files.test.ts`, `editor/test/app/main.test.ts`, `editor/test/support/zip.test.ts`
- `impl-plans/active/vactrol-editor-scaffold.md`

## sharedPaths

None. (`editor/src/app/main.ts` and `editor/src/app/layout.ts` pass to ED-FINAL sequentially. Each `mount.ts` stub
passes to its area owner sequentially. Both plans list those files.)

## File-Level Changes (behavior and signatures; no code)

1. **npm project.**
   - `package.json`: `"private": true`, `"type": "module"`, `engines.node ">=20"`.
   - Scripts: `check` = `tsc --noEmit`; `test` = `vitest run`; `build` = `vite build`; `dev` = `vite`.
   - Exact-pinned dependencies. Pin each at the highest current release that works with the others, then run
     `npm install --save-exact` from the public registry only:
     - runtime: `@codemirror/state`, `@codemirror/view`, `@codemirror/language`, `@codemirror/commands`,
       `@codemirror/lint`, `@lezer/highlight`, `@tauri-apps/api`, `@tauri-apps/plugin-dialog`,
       `@tauri-apps/plugin-fs`;
     - dev: `typescript`, `vite`, `vitest`, `jsdom`.
   - Record the chosen versions and the `npm audit` result in `notes.md`.
   - `tsconfig.json`: strict, `noEmit`, ES2022, DOM lib, `moduleResolution: bundler`, `allowJs: false`. Include
     `src`, `test`, `worklet/host.d.ts`, `vite.config.ts` and `vitest.config.ts`.
   - `vitest.config.ts`: `environment: 'jsdom'`, `include: ['test/**/*.test.ts']`.
   - `vite.config.ts`:
     - builds `index.html` to `dist/`;
     - a small inline plugin resolves the wasm artifact (`process.env.VACTROL_WASM` or
       `../target/wasm32-unknown-unknown/debug/vactrol.wasm`) and emits it as `vactrol.wasm` into the RESOLVED
       `build.outDir` (default `dist/`; the CLI `--outDir` of 15.1.12's parallel-safety rule must work), for example
       via `this.emitFile` or `writeBundle` with the resolved config;
     - it copies `worklet/processor.js` to `<outDir>/worklet/processor.js`;
     - the build FAILS with a clear message if the artifact is missing or lacks the wasm magic bytes;
     - when `VACTROL_REQUIRE_SESSION_ABI=1`, the build also fails unless the module exports `session_init`
       (`WebAssembly.Module.exports`).
   - The export check is behind that env flag because ED-WASM lands the export in wave 3. ED-WASM and ED-FINAL set the
     flag. This refines design 15.1.3 and is recorded in `notes.md`.
   - `.gitignore`: `dist/`, `src-tauri/target/`, `src-tauri/gen/`.
2. **`index.html` and the `app/` shell.**
   - `layout.ts`: builds the panes (code left, right pane, transport bar top, visual panes and analyzer area, status
     line). Each pane is a DOM element with a stable `data-pane` attribute.
   - `deps.ts`: the `EditorDeps` type `{client, store, clock, tier: 'browser' | 'native', files, core?, code?,
     midi?, visual?, bind?}`. The optional area APIs are set by the owning area's `mount`.
   - `apis.ts`: the cross-plan CONTRACT interfaces. No later plan edits them.
     - `CodeApi {view: EditorView; mapWireSpan(span, rev): {from, to} | null; currentRevision(file): number;
       selectedSiteId(): number | null; samples: SampleLibraryApi}`.
     - `SampleLibraryApi {frames(bank, index): {data: Float32Array, rate, channels} | null; openBrowser(bank?):
       void}`.
     - `MidiApi {onCc(cb: (ev: {cc, ch, value, time}) => void): () => void; learnNext(): Promise<{cc, ch}>;
       cancelLearn(): void}`.
     - `VisualApi {mountSpectrum(el: HTMLElement, source: {bus?: string}): {dispose(): void}}`.
     - `BindApi {writeSite(siteId: number, value: number): void; mode(siteId): 'overlay' | 'source-edit';
       learn(siteId): Promise<void>; siteById(id): WireSite | undefined}`.

     A missing API (for example `midi` on a tier without WebMIDI) must be handled as absent, never assumed.
   - `clock.ts`: `interface Clock { now(): number }`, and `AudioClock` over `AudioContext.currentTime`.
   - `main.ts`:
     - builds the deps from the URL: `?session=<ws-url>` means the native tier, anything else the browser tier;
     - calls each area's `mount(root, deps)` in a fixed order: code, midi, visual, bind, params, pkg. Consumers read
       `deps.<area>` at USE time, not at mount, because `deps.midi` appears only after the user grants access;
     - makes no other decision.
   - Each `mount.ts` stub exports `mount(root: HTMLElement, deps: EditorDeps): {dispose(): void}` and does nothing.
3. **`protocol/types.ts`.** TypeScript types for every envelope, client and server message, and common shape in
   command.md, including every TASK-010 field: `WireSite.call?`, `ManifestBody.editors?`, `EditorDecl`/`ParamMeta`,
   `LevelsBody.analyzers?`, `WireLevel.bands?`, `TempoBody.clock?`, the `0x72` render record, the `0x73` driver reply
   and the `session_check` record. Spans are UTF-8 byte offsets.
4. **`protocol/envelope.ts`.** Encode/decode `{v, seq, kind, body, re?}` with `v = 1`. Decoding an unknown `kind` or a
   bad shape yields a typed error value, never a throw that escapes.
5. **`protocol/transport.ts`, `socket.ts`, `wasm.ts`.**
   - `interface Transport { send(text: string): void; onText(cb): void; close(): void }`.
   - `SocketTransport(url)`: WebSocket text frames. The token stays in the URL only and is never written to storage.
   - `WasmCore` in `wasm.ts`: wraps `startHost({init: 'session', onRecord})` from `worklet/host.js`, exposes `apply`
     (`session_apply`), `check`, `frame`, `midiIn`, `samplePut`, `pkgResolve` and `pkgSupply`, and routes the
     `0x71`/`0x72`/`0x73` records to typed listeners.
   - `WasmTransport(core)`: the `Transport` over `apply` and the `0x71` records.
6. **`protocol/client.ts`.** The `Client` owns `seq` and correlates replies by `re`:
   - `request(body) -> Promise<reply>`;
   - broadcasts go to the store;
   - helpers `eval`, `hush`, `stop`, `setTweak`, `setVar`, `learn`, `subscribe` and `manifest`.

   Every write helper first calls `document.flush(file)` (14.4 rule 1) and stamps the current `edit_epoch`.
   `setTweak` is rate-limited per target: one per 16 ms, latest wins, with the trailing value always sent.
7. **`protocol/document.ts`.** Per-file `DocSync`:
   - `revision` starts at 1; `epoch` starts at 0.
   - `edit(changes: ByteChange[], dirty: ByteSpan[])` increments `epoch` and `revision` synchronously, composes the
     pending changes, and arms a 200 ms debounce.
   - `flush()` sends the pending `doc-changed {doc_revision, base_revision, changes, dirty, edit_epoch}` immediately.
   - `eval` carries the current revision and epoch.
8. **`protocol/utf8.ts`.** `Utf8Index(text)` converts UTF-16 offsets to and from byte offsets (O(log n) per lookup),
   and converts a UTF-16 change list to byte `{from, to, insert_len}` against the base text.
9. **`protocol/store.ts`.** The batch store (design 15.1.6 "Reactive displays").
   - State: sites by id, names -> `{value, form_gen, state, blocked_on?, diagnostic?}`, per-file diagnostics, runtime
     diags by slot, the latest tempo, levels, manifest and directive tables.
   - `apply(msg)` applies an entire `bindings`/`eval-result`/`diag`/`levels`/`tempo` message ATOMICALLY.
   - It then notifies each subscriber whose key set (`name:<n>`, `site:<id>`, `slot:<s>`, `levels`, `tempo`, `diag`,
     `sites`) intersects the changed keys, exactly once per message.
   - `subscribe(keys, cb) -> unsubscribe`.
   - It never exposes a partially applied message.
10. **`platform/files.ts`.** `interface FileAccess { open(): Promise<{name, text}>; save(name, text): Promise<void>;
    saveSidecar(name, text): Promise<void> }`, with three implementations:
    - `BrowserFiles`: File System Access API when present, else `<input type=file>` to open and a download to save;
    - `TauriFiles`: dynamic `import()` of `@tauri-apps/plugin-dialog` and `@tauri-apps/plugin-fs`, with `.vact` and
      `.bindings.json` dialog filters and text read/write only;
    - `MemoryFiles`: tests.
11. **`worklet/host.js`** (additive, default behavior unchanged). `startHost` options:
    - `init: 'main' | 'session'`, default `'main'`. `'session'` calls `session_init` and uses `session_tick`,
      `session_inbox` and `session_sample_put`.
    - `onRecord(tag, bytes)`: records `0x71`..`0x73` go to it and are NEVER posted to the worklet.
    - `0x70` console routing is unchanged.

    `host.d.ts` declares `startHost`, `VactrolHost` and the options.
12. **`test/support/`.**
    - `recording.ts`: `RecordingTransport` records every client envelope and replays scripted server envelopes,
      synchronously or via `deliver()`.
    - `clock.ts`: `MockClock` with `set(t)` and `advance(dt)`.
    - `fake-core.ts`: fake wasm exports for `WasmCore` and `host.js` tests.
    - `zip.ts`: an in-memory STORED-zip writer (CRC-32, local headers, central directory) for package fixtures. Zips are
      never committed; the root `.gitignore` ignores `*.zip`.

## Required Tests

- `client.test.ts`:
  - `seq` increments and `re` correlation;
  - every write is preceded by a flushed `doc-changed` when an edit is pending;
  - epoch stamping;
  - the rate limit keeps the latest value.
- `document.test.ts`:
  - the epoch increments synchronously before the debounce;
  - the debounce fires at 200 ms on `MockClock`-driven timers (vitest fake timers);
  - composition of two edits;
  - base and new revisions.
- `utf8.test.ts`: a round trip with ASCII, 2-, 3- and 4-byte characters, and a change conversion over non-ASCII text.
- `store.test.ts`:
  - one notification per message per affected subscriber;
  - an unrelated subscriber is not notified;
  - `bindings` is applied atomically: a subscriber callback observes all of `changed`, `sites` and `states` together.
- `socket.test.ts`: a fake `WebSocket` sends and receives text, and nothing is written to `localStorage` or
  `sessionStorage`.
- `wasm-transport.test.ts`: over `fake-core`, `0x71` goes to the transport, `0x72` to the render listener and `0x73` to
  the pkg listener.
- `host-js.test.ts`: with fake exports and a fake worklet port:
  - with default options, records reach the port exactly as before;
  - with `init: 'session'`, the session exports are called and the `0x71`..`0x73` records never reach the port.
- `files.test.ts`: the `MemoryFiles` round trip; `TauriFiles` against `vi.mock`ed plugin modules uses the filters and
  text calls only.
- `main.test.ts`: `main` mounts all six areas in the fixed order under jsdom, with a `RecordingTransport` injected
  through an exported `createEditor(root, deps)` entry. The test stays valid after the stubs are filled, because it
  asserts only the order and that no mount throws.
- `zip.test.ts`: the writer's output has valid CRCs and parses back with a small reader in the test.

## Invariants

- No UI component or test imports `socket.ts` or `wasm.ts` directly. Only `main.ts` chooses the transport.
- `processor.js` and `editor/dev-harness/*` are byte-identical to HEAD, and `host.js` default behavior is unchanged.
- No secret or token is persisted. `node_modules/` and `dist/` are never tracked.
- Every TS file stays under 800 lines.

## Common Contract (every ED plan)

### Edit Protocol

1. Evidence directory: `tmp/ed-editor-20260926-s186/<planId>/attempt-<n>/`, gitignored by `**/tmp`.
2. Before each edit, read the file fresh and append `shasum -a 256 <file>` to `pre-edit-hashes.txt`.
   - Before the first edit of a file this plan did not create, and of every handed-off file (a `mount.ts` stub,
     `app/main.ts`, `app/layout.ts`), write a one-line intent (file, purpose) to `intent.md`.
   - After each edit, append the post-edit hash to `post-edit-hashes.txt`.
3. Drift: if a pre-edit hash differs from this plan's last recorded post-edit hash, stop, re-read, and re-apply the
   recorded intent. Never revert another plan's hunk. Record the event in `notes.md`.
4. Edit ONLY this plan's writePaths and sharedPaths. When a check fails only because of a sibling plan's in-flight
   file, record it as sibling-caused in the progress log and let the join re-verify. Report a defect in a file outside
   every plan's ownership as a dependency blocker for the operator.
5. Never run `git reset`, `git clean`, `git stash` or `git checkout -- <path>`. Never create branches or worktrees.
   Never commit. Never run crate-wide `cargo fmt` (only `rustfmt --edition 2021` on owned `.rs` files).
6. Among plan documents, edit only this plan's own progress log and status line. Never edit the dispatch manifest.
7. Write Rust only through the rust-coding agent, then run the check-and-test-after-modify agent (CLAUDE.md).
8. Do not create a file that is not in writePaths. If one is truly needed, extend a listed file and record the
   omission for ED-FINAL.
9. At the end, write `final-hashes.txt` with the sha256 of every file this plan wrote.

### Verification (foreground; design 6.5.7 evidence rule)

- First row: `export ROOT="$(git rev-parse --show-toplevel)"; mkdir -p "$ROOT/target/fe-logs" "$ROOT/target/ed-wasm"
  "$ROOT/target/ed-dist"`. Every row runs from the repository root with `ROOT` exported.
- `LOG(<check>)` means a NEW file at the ABSOLUTE path `"$ROOT/target/fe-logs/ed-<wave>-<check>-s<S>-<n>.log"`:
  - `<wave>` is the plan's short name (`scaffold` here);
  - `<S>` is the Riela session number;
  - `<n>` counts from 1 per check within the session, and a log is never overwritten.
- Run each row as `(set -o pipefail; (CMD) 2>&1 | tee "$LOG"); echo "exit=$?" >> "$LOG"`. The inner `(CMD)` subshell
  keeps a `cd editor && ...` command from changing where `tee` writes; the absolute `$LOG` makes that doubly safe.
- The progress log cites the counting log (the last run after the final code change) and its `exit=`.
  - V3 and V3t cite their run and passed counts.
  - E3 cites the vitest file and test counts.
  - All counts must be non-zero.
- A missing log, a log without `exit=`, or a truncated log fails the row.
- The rows run in the ORDER below: V6a strictly before V6b, then V6c, then E1-E4.

| # | Command (`CMD`) | `<check>` | Evidence |
|---|-----------------|-----------|----------|
| V1 | `CARGO_TERM_QUIET=true cargo build` | `build` | `exit=0` |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` | `clippy` | `exit=0` |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run` | `nextest` | `exit=0`, run > 0, 0 failed |
| V3t | `CARGO_TERM_QUIET=true cargo test` | `cargotest` | `exit=0`, non-zero counts |
| V7 | `CARGO_TERM_QUIET=true cargo fmt --check` | `fmt` | `exit=0` |
| V6a | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown` | `wasm32` | `exit=0` |
| V6b | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` | `wasm32-hostwasm` | `exit=0` |
| V6c | `cp target/wasm32-unknown-unknown/debug/vactrol.wasm target/ed-wasm/<planId>.wasm` | - | the copy exists. E3 and E4 run with `VACTROL_WASM=$ROOT/target/ed-wasm/<planId>.wasm`, so a sibling's V6a cannot swap the artifact mid-run |
| V4 | `find src tests examples -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every `.rs` file under 800 lines |
| E0 | `node --version; npm --version` | - | recorded (node >= 20) |
| E1 | ED-SCAFFOLD (after S1) and ED-FINAL only: `cd editor && npm ci` (`npm-ci`). Every other plan: `cd editor && npm ls --depth=0` (`npm-ls`) | `npm-ci` / `npm-ls` | `exit=0`. `npm ls` is read-only and shows the installed tree matches the ED-SCAFFOLD lockfile |
| E2 | `cd editor && npm run check` | `npm-check` | `exit=0` |
| E3 | `cd editor && VACTROL_WASM=$ROOT/target/ed-wasm/<planId>.wasm npm run test` | `npm-test` | `exit=0`, test files > 0, 0 failed |
| E4 | every plan except ED-TAURI and ED-FINAL: `cd editor && VACTROL_WASM=$ROOT/target/ed-wasm/<planId>.wasm npm run build -- --outDir $ROOT/target/ed-dist/<planId> --emptyOutDir`. ED-TAURI and ED-FINAL: the same without `--outDir`/`--emptyOutDir` (default `editor/dist`) | `npm-build` | `exit=0` |
| E4c | `test -f <out>/vactrol.wasm && test -f <out>/worklet/processor.js && test -f <out>/index.html`, where `<out>` is `$ROOT/target/ed-dist/<planId>`, or `editor/dist` for ED-TAURI and ED-FINAL | - | exit 0 |
| E5 | `find editor/src editor/test -name '*.ts' -exec wc -l {} + \| sort -n \| tail -5` | - | every `.ts` file under 800 lines |

**Shared npm outputs (parallel safety).** Disjoint source ownership does NOT make the shared build outputs safe, so
they are serialized or isolated:
- `editor/node_modules` is (re)installed only by `npm ci` in ED-SCAFFOLD (wave 1, alone) and ED-FINAL (serial). No
  other plan runs `npm ci`, `npm install` or `npm update`, or deletes or modifies `node_modules`. If E1 `npm ls` fails,
  record a dependency blocker for the operator and do not repair it.
- `editor/dist` is written only by ED-TAURI (the only wave-4 writer; `generate_context!` embeds it) and ED-FINAL.
  Every other plan builds into its own `$ROOT/target/ed-dist/<planId>`.
- The whole-tree issue #5 gate (`npm ci && npm run check && npm run test && npm run build` in `editor/`) is run by
  ED-FINAL.

- Plans that touch Rust (ED-WIRE, ED-WASM, ED-FINAL) add:
  - V1l `cargo build --features lsp` (`build-lsp`);
  - V2l `cargo clippy --all-targets --features lsp -- -D warnings` (`clippy-lsp`);
  - V9 `cargo tree -e normal --target wasm32-unknown-unknown`, both with default features and with
    `--no-default-features --features host-wasm`. Neither output may contain `tungstenite`, `getrandom`, `tokio`,
    `tower-lsp`, `cpal` or `midir`.
- ED-TAURI and ED-FINAL add T1/T2 (see `vactrol-editor-tauri.md`).

Plan-specific rows (ED-SCAFFOLD):

| # | Command | Evidence |
|---|---------|----------|
| S1 | LOG(`npm-install`): `cd editor && npm install` (generates the lockfile; public npm registry only; runs BEFORE E1 `npm ci`) | `exit=0`, `package-lock.json` written |
| S2 | LOG(`npm-audit`): `cd editor && npm audit --omit=dev` | `exit=0`, or every advisory recorded with its path in `notes.md` |
| S3 | `git diff --exit-code -- editor/worklet/processor.js editor/dev-harness` | exit 0 (untouched) |

## Completion Criteria

- [x] Items 1-12 implemented; dependencies pinned exactly; the lockfile committed-ready
- [x] Required tests pass
- [x] V1-V7, V6a-V6c, V4, E0-E5 (E1 = `npm ci`, E4 to `target/ed-dist/ED-SCAFFOLD`) and S1-S3 pass with logs cited;
      `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-SCAFFOLD implementer)` entry: files, decisions, the
dependency versions, evidence log paths with `exit=` values, deviations. Edit only this log.)

### Session: 2026-09-26 (session 186, ED-SCAFFOLD implementer)

**Files**: every writePath of this plan (45 files) was written; no file outside writePaths was touched.
`editor/worklet/host.js` is the only pre-existing file edited (additive `init`/`onRecord`); `processor.js` and
`editor/dev-harness/*` are byte-identical to HEAD (S3). Evidence: `tmp/ed-editor-20260926-s186/ED-SCAFFOLD/attempt-1/`
(`intent.md`, `notes.md`, `pre-edit-hashes.txt`, `post-edit-hashes.txt`, `final-hashes.txt`, `v4-e0.txt`).

**Dependency versions** (exact, public npm registry only; every lockfile `resolved` URL is under
https://registry.npmjs.org/): @codemirror/state 6.7.6, @codemirror/view 6.43.13, @codemirror/language 6.12.4,
@codemirror/commands 6.11.1, @codemirror/lint 6.9.7, @lezer/highlight 1.2.4, @tauri-apps/api 2.11.1,
@tauri-apps/plugin-dialog 2.7.3, @tauri-apps/plugin-fs 2.5.2; dev typescript 7.0.2, vite 8.3.1, vitest 5.0.2,
jsdom 30.1.1. node v26.9.0, npm 11.19.1 (E0).

**Decisions** (details in `notes.md`):
- The vite plugin emits `vactrol.wasm` and `worklet/processor.js` via `this.emitFile`, so `--outDir` is honored;
  a missing artifact or bad magic bytes fails the build (checked: exit 1 with a clear message), and
  `VACTROL_REQUIRE_SESSION_ABI=1` adds the `session_init` export check (checked: fails against today's artifact,
  as expected before ED-WASM). No `@types/node` is pinned, so `vite.config.ts` types its node use locally.
- Sidecar name: `song.vact` -> `song.bindings.json` (`sidecarName`, platform/files.ts).
- `request()` resolves on the first reply whose `re` matches; set-tweak/set-var are fire-and-forget and their
  `stale-binding` replies reach `client.on('stale-binding')`. `setTweak` rate limit: one per 16 ms per
  `(file, id)`, latest wins, trailing value always sent; the trailing send flushes a newer pending edit first
  and keeps the epoch stamped when the value was produced.
- Store: `eval-result.sites` replaces the file's table; `bindings.sites` upserts by id and keeps a known key.
  A message applied from inside a subscriber callback is queued until the current round ends.
- host.js dispatches 0x71-0x73 to `onRecord` after `outbox_clear()` so a listener may re-enter wasm
  (tested: no double delivery); the console/worklet path is unchanged.
- `app.css` is linked from `index.html`; `main.ts` boots only when `#vactrol-app` exists.

**Verification** (all logs at `target/fe-logs/`, one run each, final source):
- V1 `ed-scaffold-build-s186-1.log` exit=0; V2 `ed-scaffold-clippy-s186-1.log` exit=0;
  V3 `ed-scaffold-nextest-s186-1.log` exit=0, 985 run / 985 passed / 1 skipped;
  V3t `ed-scaffold-cargotest-s186-1.log` exit=0 (lib 964, cli 9, directive_fixtures 2, spec_fixtures 10 + 1
  ignored; 985 passed, 0 failed); V7 `ed-scaffold-fmt-s186-1.log` exit=0.
- V6a `ed-scaffold-wasm32-s186-1.log` exit=0; V6b `ed-scaffold-wasm32-hostwasm-s186-1.log` exit=0;
  V6c `target/ed-wasm/ED-SCAFFOLD.wasm` copied (1437858 bytes). V4: largest `.rs` 799 lines
  (src/dsp/build.rs, untouched). E0: node v26.9.0, npm 11.19.1.
- S1 `ed-scaffold-npm-install-s186-1.log` exit=0 (lockfile written); S2 `ed-scaffold-npm-audit-s186-1.log`
  exit=0, 0 vulnerabilities; E1 `ed-scaffold-npm-ci-s186-1.log` exit=0 (npm 11 did not run the optional
  `fsevents` install script; informational warning only).
- E2 `ed-scaffold-npm-check-s186-1.log` exit=0; E3 `ed-scaffold-npm-test-s186-1.log` exit=0, 10 test files,
  55 tests passed, 0 failed; E4 `ed-scaffold-npm-build-s186-1.log` exit=0 to `target/ed-dist/ED-SCAFFOLD`;
  E4c exit 0 (`vactrol.wasm`, `worklet/processor.js`, `index.html` present); E5: largest `.ts` 446 lines
  (src/protocol/types.ts); S3 `git diff --exit-code -- editor/worklet/processor.js editor/dev-harness` exit 0.

**Deviations**: none. Pending downstream: test-integrity, adversarial and integration review (later workflow steps).

### CLOSING NOTE (ED-FINAL, session 188)

Status confirmed Completed (accepted). Final-tree evidence (ED-FINAL attempt-2, `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/`): join integrity re-checked (this plan's hashes OK or explained); every row exit=0 in `target/fe-logs/ed-final-*-s188-1.log`: build, build-lsp, clippy, clippy-lsp, fmt, nextest (1007 passed, 1 skipped), cargo test, both wasm32 builds, clippy wasm32 host-wasm, npm ci/check/test (54 files, 336 tests)/build (`VACTROL_REQUIRE_SESSION_ABI=1`), real-wasm vitest (3 files, 20 tests), Tauri fetch/check/fmt, session subset, lsp_smoke, spec fixtures. Own evidence: `test/protocol/*`, `test/platform/files.test.ts`, `test/app/main.test.ts` pass in target/fe-logs/ed-final-npm-test-verbose-s188-1.log; the six `mount.ts` stubs were replaced by their owners as planned.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Next**: vactrol-editor-wire.md, vactrol-editor-code.md, vactrol-editor-midi.md (wave 2), then the rest per the
  manifest

# Vactrol Editor: Tauri Shell Crate (ED-TAURI) Implementation Plan

**planId**: ED-TAURI (issue #5, TASK-010, wave 4; the standalone `editor/src-tauri/` crate wrapping the identical
`editor/dist` frontend with a minimal allowlist)
**Status**: Completed (implemented, gate-verified, adversarial review and integration review accepted in session 187; removed from the dispatch manifest by the session-188 amendment; source rides in the single workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 15.1.11, 15, 17 (Tauri allowlist), 12.8.10 (version
policy on Rust 1.83); design-docs/user-qa/pending-editor-questions.md E1
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-SCAFFOLD (`frontendDist` = `../dist` is produced by `npm run build`; `TauriFiles` in
`platform/files.ts` uses the plugins configured here)
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

The Tauri shell wraps the SAME Vite build (15, 15.1.11). It has no custom commands, and file open/save goes through
`tauri-plugin-dialog` + `tauri-plugin-fs` with dialog-granted paths only. It is a standalone crate with its own empty
`[workspace]`, so the root crate (`Cargo.toml`, no `[workspace]`) never builds it, and root `cargo build`, `clippy`,
`fmt` and `nextest` are unaffected.
- `cargo check` of this crate is an acceptance gate.
- `cargo tauri build` and running the app are MANUAL, pending user confirmation.

E1: the crates come from crates.io, and issue #5 allows only npm registry access. This plan follows the E1
recommendation:
- attempt `cargo fetch` for this crate only;
- if crates.io is unreachable or refused, OR the tree cannot build on Rust 1.83 (mise pin), record T1 as BLOCKED
  (NOT passing) in the progress log, put it in `verificationGaps` (not empty), and report a dependency blocker for the
  operator.

Never install a different toolchain.

## Non-Goals

- No shell plugin, no custom `invoke` command, no static fs scope, no updater, no native audio or MIDI (Tauri reports
  WebMIDI "not available on this host").
- No change to the frontend. `platform/files.ts` (ED-SCAFFOLD) already has `TauriFiles`.
- No root `Cargo.toml`/`Cargo.lock` change.

## writePaths

- `editor/src-tauri/Cargo.toml`, `editor/src-tauri/Cargo.lock`, `editor/src-tauri/build.rs`,
  `editor/src-tauri/src/main.rs`, `editor/src-tauri/tauri.conf.json`,
  `editor/src-tauri/capabilities/default.json`, `editor/src-tauri/icons/icon.png`
- `impl-plans/active/vactrol-editor-tauri.md`

## sharedPaths

None.

## File-Level Changes (behavior; no code)

1. **`Cargo.toml`.**
   - `[package] name = "vactrol-editor"`, `version = "0.1.0"`, `edition = "2021"`, `rust-version = "1.83"`,
     `publish = false`.
   - An empty `[workspace]` table.
   - `[build-dependencies] tauri-build = "2"`.
   - `[dependencies] tauri = "2"` (default features only), `tauri-plugin-dialog = "2"`, `tauri-plugin-fs = "2"`.
   - Version policy (12.8.10): the highest releases whose resolved tree builds on 1.83, with
     `cargo update --precise` in THIS crate's lockfile where needed. Record the versions, pins and a `cargo audit` run
     (if available offline) in `notes.md`.
2. **`build.rs`.** `tauri_build::build()`.
3. **`src/main.rs`.** `tauri::Builder::default().plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_fs::init())
   .run(tauri::generate_context!())`, with an error message on failure. `#![cfg_attr(not(debug_assertions),
   windows_subsystem = "windows")]` is optional. No commands.
4. **`tauri.conf.json`** (Tauri 2 schema).
   - `productName` "Vactrol", `identifier` "me.tacogips.vactrol".
   - `build.frontendDist: "../dist"`, `build.devUrl: "http://localhost:5173"`, and `beforeBuildCommand` /
     `beforeDevCommand` empty. The operator runs `npm run build` / `npm run dev`.
   - One window.
   - `app.security.csp`: `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'
     ws://127.0.0.1:* https:; img-src 'self' data: blob:; worker-src 'self' blob:`.
   - `bundle.active: false` for checks. `bundle.icon: ["icons/icon.png"]`.
5. **`capabilities/default.json`.**
   - `identifier` "default", windows `["main"]`.
   - Permissions: `core:default`, `dialog:allow-open`, `dialog:allow-save`, `fs:allow-read-text-file`,
     `fs:allow-write-text-file`.
   - NO `fs:scope` entry. Only dialog-picked paths are allowed.
6. **`icons/icon.png`.** A minimal valid 32x32 RGBA PNG, generated with a node one-off using the `zlib` built-in (not
   committed as a script). Record the command and the sha256 in `notes.md`. It is included only because
   `generate_context!` may require an icon.

## Required Tests

No Rust unit tests: the shell has no logic. The gates are T1/T2 plus the common rows. `TauriFiles` behavior is tested
in ED-SCAFFOLD (`files.test.ts`).

## Invariants

- The root crate and its lockfile are untouched (`git diff --exit-code -- Cargo.toml Cargo.lock`).
- `editor/src-tauri/target/` and `gen/` are ignored by `editor/.gitignore` (ED-SCAFFOLD). They are never tracked.
- The capabilities list is exactly the five permissions above.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-TAURI`. Run `rustfmt --edition 2021` on the
two owned `.rs` files.

## Verification (`<wave>` = `tauri`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5. E1 is the read-only `npm ls --depth=0` (never
`npm ci`). E4 builds to the default `editor/dist`: this plan is the only wave-4 writer of `editor/dist`. E4 MUST run
before T1, because
`generate_context!` embeds `../dist`. Plus:

| # | Command | Evidence |
|---|---------|----------|
| T0 | LOG(`tauri-fetch`): `CARGO_TERM_QUIET=true cargo fetch --manifest-path editor/src-tauri/Cargo.toml` | `exit=0`; otherwise T1 is recorded BLOCKED (E1) with this log as evidence |
| T1 | LOG(`tauri-check`): `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | `exit=0` |
| T2 | LOG(`tauri-fmt`): `cargo fmt --check --manifest-path editor/src-tauri/Cargo.toml` | `exit=0` |
| T3 | `git diff --exit-code -- Cargo.toml Cargo.lock` | exit 0 |
| T4 | `jq -e '.permissions \| length == 5' editor/src-tauri/capabilities/default.json` | prints `true` |
| T5 | `git status --short --ignored editor/src-tauri` | `target/` shown as ignored (`!!`), never untracked |

## Completion Criteria

- [x] Items 1-6 implemented
- [x] T0-T5 and the common rows pass with logs cited, or T0/T1 recorded BLOCKED under E1 with a dependency blocker
      (never claimed as passing)
- [x] The manual `cargo tauri build` and app run are recorded as PENDING USER CONFIRMATION
- [x] `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-TAURI implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 186, ED-TAURI implementer)

**Tasks Completed**: items 1-6. Evidence: `tmp/ed-editor-20260926-s186/ED-TAURI/attempt-1/` (intent.md, notes.md,
pre-/post-/final-hashes.txt). ED-SCAFFOLD was in the runtime `acceptedPlanIds`.

- Files created: `editor/src-tauri/{Cargo.toml, Cargo.lock, build.rs, src/main.rs, tauri.conf.json,
  capabilities/default.json, icons/icon.png}`. `build.rs` and `src/main.rs` were written by the rust-coding agent.
  The check-and-test-after-modify agent then ran check, fmt --check, clippy -D warnings and T3 on the shell crate; all
  exited 0.
- Resolved direct deps: tauri 2.11.6, tauri-build 2.6.3, tauri-plugin-dialog 2.7.3, tauri-plugin-fs 2.5.2. There are
  429 packages from crates.io, all from a single source.
- Rust 1.83 fit:
  - Cargo 1.83 has no MSRV-aware resolver. The already-installed cargo 1.98.1 ran
    `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile` as a resolution aid only. No toolchain
    was installed.
  - Cargo 1.83 applied three `cargo update --precise` pins:
    - dlopen2 0.8.0 and dlopen2_derive 0.4.1: the newer versions use edition 2024 manifests.
    - yoke-derive 0.8.2: 0.8.3 uses the inherent `str::from_utf8`, stable only since 1.87.
  - Every gate ran on cargo/rustc 1.83.0.
- `cargo audit --no-fetch` exited 1, which is not a plan gate. Its findings are recorded in notes.md as residual risk:
  - quick-xml 0.38.4: RUSTSEC-2026-0194 and RUSTSEC-2026-0195.
  - time 0.3.45: RUSTSEC-2026-0009.
  - Both fixed releases need Rust 1.88.
- Verification (logs under `target/fe-logs/`):
  - V1 `ed-tauri-build-s186-1.log`: exit=0
  - V2 `ed-tauri-clippy-s186-1.log`: exit=0
  - V3 `ed-tauri-nextest-s186-1.log`: exit=0, 985 run, 985 passed, 1 skipped
  - V3t `ed-tauri-cargotest-s186-1.log`: exit=0, 985 passed, 0 failed
  - V7 `ed-tauri-fmt-s186-1.log`: exit=0
  - V6a `ed-tauri-wasm32-s186-1.log`: exit=0
  - V6b `ed-tauri-wasm32-hostwasm-s186-1.log`: exit=0
  - V6c: `target/ed-wasm/ED-TAURI.wasm` copied
  - V4: max `.rs` file is 799 lines (src/dsp/build.rs)
  - E0: node v26.9.0, npm 11.19.1
  - E1 `ed-tauri-npm-ls-s186-1.log`: exit=0
  - E2 `ed-tauri-npm-check-s186-1.log`: exit=0
  - E3 `ed-tauri-npm-test-s186-1.log`: exit=0, 32 test files, 182 tests passed
  - E4 `ed-tauri-npm-build-s186-1.log`: exit=0, built to `editor/dist`
  - E4c: exit 0
  - E5: max `.ts` file is 446 lines
  - T0 `ed-tauri-tauri-fetch-s186-2.log`: exit=0
  - T1 `ed-tauri-tauri-check-s186-2.log`: exit=0. The earlier attempt `-s186-1` exited 101 on yoke-derive 0.8.3,
    which was fixed by the pin.
  - T2 `ed-tauri-tauri-fmt-s186-1.log`: exit=0
  - T3: `git diff --exit-code -- Cargo.toml Cargo.lock` exit 0
  - T4: prints `true`
  - T5: `target/` and `gen/` show as `!!`. The crate dir shows as `??` because the source tree stays uncommitted until
    the workflow commit step.
- PENDING USER CONFIRMATION: `cargo tauri build` and running the app. Automated proxies: T1 `cargo check`, which runs
  `generate_context!` over `../dist` and the capability file, plus E4/E4c.
- Siblings: no drift. The common rows ran on the shared tree while ED-PARAMS and ED-PKG were in flight, and all passed.
  ED-FINAL re-verifies the combined tree.

### Session: 2026-09-26 (session 186, ED-TAURI implementer, attempt 2: adversarial-review repair)

**Trigger**: step7 adversarial review (comm-002384) raised one mid finding on `editor/src-tauri/tauri.conf.json:22`.
- The CSP had no `style-src`.
- Tauri 2.11.6 (`src/manager/mod.rs:96-103`) injects a style nonce into `style-src`.
- That blocks every runtime `<style>` element that CodeMirror 6 / style-mod creates without a nonce, so the Tauri editor
  would render unstyled.

**Change** (evidence in `tmp/ed-editor-20260926-s186/ED-TAURI/attempt-2/`: intent.md, pre-/post-/final-hashes.txt):
- `app.security.csp` adds `style-src 'self' 'unsafe-inline'`.
- `app.security.dangerousDisableAssetCspModification = ["style-src"]`. Without it, the injected nonce makes the webview
  ignore `'unsafe-inline'`.
- `connect-src` adds `ipc: http://ipc.localhost`, which Tauri documents for its IPC custom protocol. This was a
  reviewer residual low risk and avoids a CSP violation and postMessage fallback on every dialog/fs call.
- Unchanged: `script-src 'self' 'wasm-unsafe-eval'`, the other directives, the five capabilities, and every other key.

**Design-amendment request (operator)**: design-implementation.md 15.1.11 and plan item 4 pin the CSP verbatim. The
shipped CSP deviates from them as follows:
- It adds `style-src 'self' 'unsafe-inline'`.
- It adds `ipc: http://ipc.localhost` to `connect-src`.
- It sets `dangerousDisableAssetCspModification: ["style-src"]`.

Please amend 15.1.11 to match.
- Rationale: CodeMirror injects its theme CSS at runtime.
- Alternative not taken: wire Tauri's per-load style nonce into `EditorView.cspNonce`. That touches frontend files
  outside ED-TAURI and is more fragile.
- Risk: script policy stays strict and there is no remote content, so allowing inline styles adds negligible risk.

**Verification (logs under `target/fe-logs/`)**:
- E4c: `editor/dist` artifacts present, exit 0.
- T1 `ed-tauri-tauri-check-s186-3.log`: exit=0 on cargo 1.83.0. The build script re-ran after the config edit, per
  `rerun-if-changed` on tauri.conf.json.
- T2 `ed-tauri-tauri-fmt-s186-2.log`: exit=0.
- T3: `git diff --exit-code -- Cargo.toml Cargo.lock` exits 0.
- T4: prints `true`.
- T5: `gen/` and `target/` show as `!!`.

The common rows from attempt 1 are unaffected. The change is confined to the shell crate's config, which the root
crate and the npm build do not read.

### CLOSING NOTE (ED-FINAL, session 188)

Status confirmed Completed (accepted). Final-tree evidence (ED-FINAL attempt-2, `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/`): join integrity re-checked (this plan's hashes OK or explained); every row exit=0 in `target/fe-logs/ed-final-*-s188-1.log`: build, build-lsp, clippy, clippy-lsp, fmt, nextest (1007 passed, 1 skipped), cargo test, both wasm32 builds, clippy wasm32 host-wasm, npm ci/check/test (54 files, 336 tests)/build (`VACTROL_REQUIRE_SESSION_ABI=1`), real-wasm vitest (3 files, 20 tests), Tauri fetch/check/fmt, session subset, lsp_smoke, spec fixtures. Own evidence: target/fe-logs/ed-final-tauri-fetch-s188-1.log, target/fe-logs/ed-final-tauri-check-s188-1.log, target/fe-logs/ed-final-tauri-fmt-s188-1.log, target/fe-logs/ed-final-tauri-perms-s188-1.log (`true`), target/fe-logs/ed-final-tauri-status-s188-1.log (`target/`, `gen/` ignored), target/fe-logs/ed-final-root-cargo-untouched-s188-1.log; all exit=0. `cargo tauri build` and the app run are PENDING USER CONFIRMATION.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-params.md, vactrol-editor-pkg.md
- **Next**: vactrol-editor-finalize.md

### GATE EVIDENCE NOTE (operator, 2026-09-26, after the ED-TAURI attempt-1 gate block)

- The progress gate requires at least one SUCCESSFUL behavioral test command
  with a POSITIVE test count in the step6 `verification` records. `cargo check`
  of the shell crate is not a test. Attempt 1 was blocked with "no successful
  behavioral test evidence with a positive test count". On redispatch run and
  record, with exit codes and counts: `npm run test` in editor/ (vitest, the
  shell's own frontend tests count) and a crate-wide
  `CARGO_TERM_QUIET=true cargo test` (record `testsRun`/`testsPassed`), in
  addition to `cargo check` of the shell crate. Leave `verificationGaps` empty;
  manual Tauri runs go under `residualRisks`.

### Session: 2026-09-26 (session 187, ED-TAURI implementer, attempt 3: gate-evidence rerun)

**Trigger**: the operator GATE EVIDENCE NOTE above; `reviewFeedback.findings` was empty. `dependsOn` is `[]` at rerun
(ED-SCAFFOLD accepted outside the manifest by `session187Amendment`). Evidence:
`tmp/ed-editor-20260926-s186/ED-TAURI/attempt-3/` (intent.md, pre-edit-hashes.txt, final-hashes.txt).

- No source change. The seven `editor/src-tauri` files match `attempt-2/final-hashes.txt` byte for byte; only this
  plan file changed (the operator note and this entry).
- Verification on the shared tree (logs under `target/fe-logs/`, all on the session-187 source):
  - V1 `ed-tauri-build-s187-1.log`: exit=0
  - V2 `ed-tauri-clippy-s187-1.log`: exit=0
  - V3 `ed-tauri-nextest-s187-1.log`: exit=0, 985 run, 985 passed, 1 skipped
  - V3t `ed-tauri-cargotest-s187-1.log`: exit=0, 985 passed, 0 failed, 1 ignored (lib 964, cli 9,
    directive_fixtures 2, spec_fixtures 10)
  - V7 `ed-tauri-fmt-s187-1.log`: exit=0
  - V6a `ed-tauri-wasm32-s187-1.log`: exit=0; V6b `ed-tauri-wasm32-hostwasm-s187-1.log`: exit=0
  - V6c: `target/ed-wasm/ED-TAURI.wasm` copied (exit 0)
  - V4: max `.rs` file is 799 lines (src/dsp/build.rs)
  - E0: node v26.9.0, npm 11.19.1
  - E1 `ed-tauri-npm-ls-s187-1.log`: exit=0
  - E2 `ed-tauri-npm-check-s187-1.log`: exit=0
  - E3 `ed-tauri-npm-test-s187-1.log`: exit=0, 32 test files, 182 tests passed, 0 failed
  - E4 `ed-tauri-npm-build-s187-1.log`: exit=0, built to `editor/dist`; E4c: exit 0
  - E5: max `.ts` file is 446 lines (editor/src/protocol/types.ts)
  - T0 `ed-tauri-tauri-fetch-s187-1.log`: exit=0
  - T1 `ed-tauri-tauri-check-s187-1.log`: exit=0 on cargo/rustc 1.83.0, after E4
  - T2 `ed-tauri-tauri-fmt-s187-1.log`: exit=0
  - T3: `git diff --exit-code -- Cargo.toml Cargo.lock` exit 0
  - T4: prints `true`
  - T5: `gen/` and `target/` show as `!!`; the crate dir shows as `??` (source uncommitted until the workflow commit)
- PENDING USER CONFIRMATION (unchanged): `cargo tauri build` and running the app; automated proxies T1 + E4/E4c.
- Still open for the operator: the attempt-2 design-amendment request for the 15.1.11 CSP, and the `cargo audit`
  residual risk (quick-xml 0.38.4, time 0.3.45; fixes need Rust 1.88).
- Siblings ED-WIRE and ED-BIND were in flight on the shared tree; every common row passed, so no sibling-caused
  failure is recorded. ED-FINAL re-verifies the combined tree.

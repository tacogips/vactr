# Vactrol Editor: Tauri Shell Crate (ED-TAURI) Implementation Plan

**planId**: ED-TAURI (issue #5, TASK-010, wave 4; the standalone `editor/src-tauri/` crate wrapping the identical
`editor/dist` frontend with a minimal allowlist)
**Status**: Ready
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

- [ ] Items 1-6 implemented
- [ ] T0-T5 and the common rows pass with logs cited, or T0/T1 recorded BLOCKED under E1 with a dependency blocker
      (never claimed as passing)
- [ ] The manual `cargo tauri build` and app run are recorded as PENDING USER CONFIRMATION
- [ ] `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-TAURI implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-params.md, vactrol-editor-pkg.md
- **Next**: vactrol-editor-finalize.md

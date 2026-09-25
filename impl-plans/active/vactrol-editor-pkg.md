# Vactrol Editor: Browser Package Import UI, fetch() Driver, OPFS (ED-PKG) Implementation Plan

**planId**: ED-PKG (issue #5, TASK-010, wave 4; the package import UI, the need-URL driver loop over
`pkg_resolve`/`pkg_supply` with `fetch()`, OPFS persistence and restore, and the native-tier `vactrol get` hint)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 15.1.10, 15.1.2 G6, 5.7, 14.5.1, 14.5.7, 17;
design-docs/specs/command.md `pkg_resolve`/`pkg_supply`, `0x73`, "Editor files (TASK-010)"
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-SCAFFOLD (`WasmCore.pkgResolve`/`pkgSupply`, the `0x73` listener, `test/support/zip.ts`)
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

TASK-009 moved the browser `fetch()` transport and the OPFS backend here. The Rust side validates, digests and
publishes: ED-WIRE `pkg::driver` + `MemCache`, exposed by ED-WASM. This plan is the TypeScript IO side:
1. call `pkgResolve` and receive `need <url>`;
2. `fetch(url)`;
3. `pkgSupply(url, status, bytes)`;
4. repeat until `done` or `error`.

It persists the requirements, the lock and the proxy bodies in OPFS, and restores them through the same validation.
It covers the package half of TASK-010 criterion 3: "the package UI imports the fixture package and surfaces its
diagnostics". The real-wasm end-to-end run is in ED-FINAL.

## Non-Goals

- No Rust. No package validation in TypeScript: OPFS bytes are never trusted without the Rust pipeline.
- No default proxy URL. No automatic evaluation after an import.
- On the native tier, no fetch: the UI shows the `vactrol get <path>` command.

## writePaths

- `editor/src/pkg/mount.ts` (fills the ED-SCAFFOLD stub), `editor/src/pkg/driver.ts`, `editor/src/pkg/opfs.ts`,
  `editor/src/pkg/ui.ts`, `editor/src/pkg/pkg.css`
- `editor/test/support/opfs.ts`, `editor/test/support/fetch.ts`
- `editor/test/pkg/driver.test.ts`, `editor/test/pkg/opfs.test.ts`, `editor/test/pkg/ui.test.ts`
- `impl-plans/active/vactrol-editor-pkg.md`

## sharedPaths

None.

## File-Level Changes (behavior and signatures; no code)

1. **`driver.ts`.** `importPackages(core, {proxy, requirements} | {proxy, lock}, fetchFn) -> Promise<DriverResult>`.
   - The loop: call `core.pkgResolve(req)` and await its `0x73` reply.
     - `need`: `fetchFn(url)`. A 200 supplies the body; a 404 supplies 404; any other status or a thrown error
       supplies status 0 (network). Record the body for OPFS. Loop.
     - `done`: return `{lock, resolved, bodies}`.
     - `error`: return `{code, message}`.
   - A guard of at most 256 iterations gives `error` `package-resolve` "too many fetches".
   - Only URLs under the configured proxy prefix are fetched. Any other URL is refused with `error`.
2. **`opfs.ts`.** Under `navigator.storage.getDirectory()` / `vactrol-pkg/`:
   - `requirements.json`, `vactrol.lock`, and `bodies/<sha256-of-url>`, with an `index.json` mapping url -> file;
   - `save(result)`, `load() -> {requirements, lock, bodies} | null`, `remove(url)`;
   - when OPFS is absent, a memory-only store and a hint "packages are kept for this session only".
3. **Restore on startup** (browser tier). Load OPFS, `pkgSupply` every stored body, then `pkgResolve({proxy, lock})`:
   - `done`: installed;
   - `error package-integrity`: delete the offending stored bodies and the lock from OPFS and show the diagnostic.
4. **`ui.ts`.** The package pane.
   - A proxy URL field (persisted in `localStorage`, no default).
   - A list of unresolved imports taken from static diagnostics with codes `package-not-locked` and
     `package-not-fetched` (the path is parsed from the diagnostic message/span text; record the exact rule in the
     test).
   - An "Import" button runs the driver with the current requirements plus the import's path at `""` (latest).
   - Driver `error`s show as package diagnostics with code and message.
   - After `done`, the UI shows "imported; evaluate to load". It NEVER sends `eval` itself.
   - Load diagnostics then arrive in the next user-started `eval-result` and are listed in the pane.
   - Native tier: the list plus the exact `vactrol get <path>` command text, with no fetching.
5. **`mount.ts`.** Mounts the pane, subscribes to the store diagnostics and runs the restore once.
6. **`test/support/`.** `opfs.ts` is an in-memory `FileSystemDirectoryHandle` fake. `fetch.ts` is a fake proxy that
   serves `@v/list`, `@v/<v>.toml` and `@v/<v>.zip` bodies (zips built with `test/support/zip.ts`) and records the
   requested URLs.

## Required Tests

- `driver.test.ts` (fake core scripted with `need` then `done`, and `need` then `error`):
  - fetches only the requested URLs, in order, and supplies statuses correctly (200, 404, network);
  - refuses a URL outside the proxy;
  - the iteration guard.
- `opfs.test.ts`:
  - a save/load round trip;
  - a restore supplies every stored body before `pkgResolve({lock})`;
  - a scripted `package-integrity` error deletes the stored entries;
  - memory-only fallback when OPFS is absent.
- `ui.test.ts` (criterion 3, package half):
  - a `package-not-locked` diagnostic for `github.com/test/vactrol-pads` lists the import;
  - "Import" runs the driver against the fake proxy and shows "imported; evaluate to load";
  - no `eval` is recorded on `RecordingTransport`;
  - a subsequent scripted `eval-result` with a `package-load-failed` diagnostic is shown in the pane;
  - a driver `error package-integrity` is shown;
  - the native tier shows `vactrol get github.com/test/vactrol-pads` and performs no fetch.

## Invariants

- Package content reaches the session only through `pkgSupply` + `pkgResolve` (validation, digest, staged
  publication in Rust). OPFS holds raw bodies only.
- Network requests go only to the configured proxy.
- No secrets are stored.
- Every TS file stays under 800 lines.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-PKG`.

## Verification (`<wave>` = `pkg`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5, plus:

| # | Command | Evidence |
|---|---------|----------|
| K1 | LOG(`own`): `cd editor && npx vitest run test/pkg` | `exit=0`, 3 files passed |

## Completion Criteria

- [ ] Items 1-6 implemented
- [ ] Required tests pass
- [ ] Common rows and K1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-PKG implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-params.md, vactrol-editor-tauri.md
- **Next**: vactrol-editor-finalize.md

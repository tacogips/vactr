# Vactrol Editor: Serial Reconciliation, Real-Wasm Criteria Tests, Bookkeeping (ED-FINAL) Implementation Plan

**planId**: ED-FINAL (issue #5, TASK-010, wave 5; the join integrity check, real-wasm criterion tests over G1-G6,
`app/main.ts` wiring fixes, full-tree verification, TASK-010 checkboxes with evidence, pending manual gates, plan
statuses, README updates, commit staging list and message)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 15.1.12, 6.5.7; impl-plans/active/vactrol-core.md
TASK-010; design-docs/user-qa/pending-editor-questions.md E1-E5
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-SCAFFOLD, ED-WIRE, ED-CODE, ED-MIDI, ED-WASM, ED-BIND, ED-VISUAL, ED-PARAMS, ED-PKG, ED-TAURI
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json (NEVER edited by this plan)

---

## Intent and Context

All ten ED plans have landed in one shared working tree. This plan:
1. checks join integrity against every plan's `final-hashes.txt`;
2. adds the criterion-level REAL-WASM tests, which drive the actual browser Session (G1) through
   `editor/test/support/wasm.ts` (owned by ED-WASM, read-only here);
3. fixes app wiring in `app/main.ts`/`app/layout.ts` only;
4. re-verifies the whole tree;
5. records every TASK-010 completion criterion with evidence in `vactrol-core.md`.

The manual gates are recorded as PENDING USER CONFIRMATION with automated proxies:
- hearing the worklet audio in a real browser;
- the visual pane in a real browser;
- `cargo tauri build` and the app run.

## Non-Goals

- No feature work.
- A defect in another plan's file is NOT fixed here. It is recorded as a dependency blocker, and the integration
  review decides between redispatch and repair in place.
- No archiving of plans to `impl-plans/completed/`. That is a follow-up docs commit after the workflow commit, as in
  issues #3 and #4.
- No dispatch-manifest edit.

## writePaths

- `editor/test/wasm/criteria.test.ts` (new), `editor/test/wasm/packages.test.ts` (new)
- `editor/src/app/main.ts`, `editor/src/app/layout.ts` (sequential handoff from ED-SCAFFOLD; wiring fixes only)
- `impl-plans/active/vactrol-core.md`, `impl-plans/README.md`, `README.md`
- the status line and closing note of every ED plan: `impl-plans/active/vactrol-editor-scaffold.md`,
  `vactrol-editor-wire.md`, `vactrol-editor-code.md`, `vactrol-editor-midi.md`, `vactrol-editor-wasm.md`,
  `vactrol-editor-bind.md`, `vactrol-editor-visual.md`, `vactrol-editor-params.md`, `vactrol-editor-pkg.md`,
  `vactrol-editor-tauri.md` (all under `impl-plans/active/`), and `impl-plans/active/vactrol-editor-finalize.md`

## sharedPaths

None.

## File-Level Changes (behavior; no code)

1. **Join integrity.**
   - For every ED plan, compare `tmp/ed-editor-20260926-s186/<planId>/attempt-<last>/final-hashes.txt` with the
     current `shasum -a 256` of each listed file.
   - Write `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-<n>/join-integrity.txt`. Explain every mismatch: a later
     sequential writer (a `mount.ts` stub, `app/main.ts`) or a drift. An unexplained drift is a dependency blocker.
2. **`editor/test/wasm/criteria.test.ts`** (`// @vitest-environment node`, real host-wasm artifact via
   `test/support/wasm.ts`, a mock clock driving `session_tick` times):
   - Criterion 1: `subscribe` + `eval` of `s [:bd :sd :hh :sd] > d1` at revision 1. Tick through two cycles. Each
     `playing` event has `src.doc_revision == 1` and `time` within one lookahead window of the tick `now` at which it
     was emitted. Feeding them to ED-CODE's `HighlightScheduler` with the mock clock activates the right span.
   - Criterion 2:
     - a `direct` site `set-tweak` changes the site value in the next `bindings` or site table with no `eval-result`;
     - a `reeval` site (a literal inside arithmetic) produces a `bindings` batch with a rebuilt site (new `form_gen`);
     - a `set-tweak` with an old `form_gen` gets `stale-binding stale-form-gen`;
     - a `doc-changed` touching a site followed by a `set-tweak` gets `stale-binding edit-invalidated`.
   - Criterion 3 (sites): the three origins appear for a document with a pattern literal, `let` and an `inst` default.
     The site `call` fields are present for `lpf`. A `set-tweak` on the `inst-default` site (tier `direct`) produces no
     `eval-result`, no `stale-binding` (the write is accepted), and no `bindings` batch rebuilding the `inst` form. These
     are the observables at the protocol boundary for a `direct` write. The per-voice cell behavior is already covered
     by TASK-009 `src/session/tests/tiers.rs`, and the audible part stays with the manual gate.
   - Criterion 11: evaluating `osc 20 > rotate 0.5 > out o0` yields no `0x72` program before the boundary tick and
     one after it, and ED-VISUAL's `GlRenderHost` over `RecordingGL` compiles and draws it. `text "hello" > out o1`
     yields a program record with an asset whose `text` is `hello`. A broken chain (a type error) yields a diagnostic
     and no new program, and the previous program keeps drawing.
   - `session_check` of a type error returns the diagnostic and no `playing` change.
3. **`editor/test/wasm/packages.test.ts`** (criterion 3 packages, real driver): ED-PKG's `importPackages` over the real
   core with a fake proxy (fixture package zipped by `test/support/zip.ts`):
   - `done`, then `eval` of `import github.com/test/vactrol-pads` followed by a form using it loads with no
     `package-not-locked`;
   - a tampered zip gives `error package-integrity`, shown by ED-PKG's UI.
4. **App wiring.** `app/main.ts` / `app/layout.ts` fixes needed for all six areas to mount together in the browser
   tier, if any. Record each fix in `notes.md`.
5. **Full-tree verification.** Every common row, V1l/V2l/V9, T0-T5 (ED-TAURI), X1 (ED-WASM), and E4 with
   `VACTROL_REQUIRE_SESSION_ABI=1`.
6. **`impl-plans/active/vactrol-core.md` TASK-010.**
   - Check each of the 12 completion criteria ONLY with evidence: the test file and test names plus log paths with
     `exit=0`.
   - Criterion 1: automated proxy checked; "hear audio" marked PENDING USER CONFIRMATION (manual: `npm run dev`,
     open the page, evaluate a pattern).
   - Criterion 11: automated proxy checked; the real-browser view is PENDING USER CONFIRMATION.
   - Criterion 12: `cargo check` evidence (or BLOCKED under E1, stated as not passing), plus the `npm run build`,
     `check` and `test` logs; `cargo tauri build` and the app run are PENDING USER CONFIRMATION.
   - Set the status to COMPLETED only when every automated criterion is evidenced. If T1 is BLOCKED, leave the status
     IN_PROGRESS and say why.
   - Amend the deliverable text with the 15.1.1 dispositions: browser taps and MIDI out deferred (E2); per-slot
     activity instead of levels (E3); ExternalFile editor-side (E4); the G1-G6 additions; the plan list and the
     manifest name.
   - Update the Module Status row `editor/` and the dependency rows.
7. **`impl-plans/README.md`.** Update the ED rows to Completed (or the actual state) with evidence summaries, and
   refresh the `vactrol-core.md` row.
8. **`README.md`.** An "Editor" section: `cd editor && npm ci && npm run dev`, the browser and native tiers
   (`?session=<ws-url>` with the `vactrol serve` URL), the wasm artifact build command, the Tauri shell (manual), and
   a pointer to design 15.1.
9. **Plan statuses.** Set each ED plan's status line to Completed (accepted) and append a `### CLOSING NOTE
   (ED-FINAL, session <S>)` citing its final-tree evidence.
10. **Commit record** (in this plan's progress log).
    - The explicit staging list: `editor/**` tracked sources (package.json, the lockfile, configs, `index.html`,
      `.gitignore`, `src/**`, `test/**`, `worklet/host.js`, `worklet/host.d.ts`, `src-tauri/**` excluding `target/`
      and `gen/`), the Rust files of ED-WIRE and ED-WASM, `README.md`, `impl-plans/active/vactrol-core.md`,
      `impl-plans/active/vactrol-editor-*.md` and `impl-plans/README.md`.
    - NEVER stage `node_modules/`, `dist/`, `target/` or `tmp/`.
    - The six-section commit message (CLAUDE.md sections on separate lines, no attribution or Co-Authored-By line).

## Required Tests

Items 2 and 3 above. No other new tests.

## Invariants

- The dispatch manifest is unchanged: `git diff --exit-code -- impl-plans/active/ed-editor-20260926-s186-dispatch.json`.
- No criterion is checked without a cited log with `exit=0`. BLOCKED is never reported as passing.
- `processor.js`, `editor/dev-harness/*` and `src/host/wasm/main_half.rs` are unchanged.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-FINAL`. It is serial: no sibling runs
concurrently.

## Verification (`<wave>` = `final`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5. Here E1 is `npm ci` (serial; this plan is the only
post-scaffold `npm ci`), E4 builds to the default `editor/dist` with `VACTROL_REQUIRE_SESSION_ABI=1`, and E1-E4
together are the issue #5 whole-tree gate `npm ci && npm run check && npm run test && npm run build`. Plus V1l,
V2l, V9, X1 (see `vactrol-editor-wasm.md`) and T0-T5 (see `vactrol-editor-tauri.md`), plus:

| # | Command | Evidence |
|---|---------|----------|
| F1 | LOG(`wasm-tests`): `cd editor && VACTROL_WASM=$ROOT/target/ed-wasm/ED-FINAL.wasm npx vitest run test/wasm` | `exit=0`, 3 files passed (`abi`, `criteria`, `packages`) |
| F2 | LOG(`session`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests/) \| test(/pkg::tests/) \| test(/directives::tests/) \| test(/sched::tests/) \| binary(directive_fixtures) \| binary(cli)'` | `exit=0` |
| F3 | LOG(`lsp-smoke`): `CARGO_TERM_QUIET=true cargo test --features lsp --test lsp_smoke` | `exit=0` |
| F4 | LOG(`fixtures`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `exit=0` |
| F5 | `git diff --exit-code -- impl-plans/active/ed-editor-20260926-s186-dispatch.json editor/worklet/processor.js editor/dev-harness src/host/wasm/main_half.rs` | exit 0 |
| F6 | `git status --short` and `git status --short --ignored editor` | no tracked or untracked `node_modules/`, `dist/` or `src-tauri/target/` (ignored only) |
| F7 | `jq . impl-plans/active/ed-editor-20260926-s186-dispatch.json > /dev/null` | exit 0 |
| F8 | `grep -rn "getUserMedia" editor/src \|\| echo none` | prints `none` |

## Completion Criteria

- [ ] Join integrity recorded; every mismatch explained
- [ ] `criteria.test.ts` and `packages.test.ts` pass against the real artifact
- [ ] All rows pass with logs cited (T1 may be BLOCKED only under E1, stated as a gap)
- [ ] `vactrol-core.md` TASK-010 criteria checked with evidence, manual gates PENDING USER CONFIRMATION, status set
      per item 6; Module Status updated
- [ ] `impl-plans/README.md`, `README.md` and the ED plan statuses updated; the commit staging list and message recorded

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-FINAL implementer)` entry. Edit only this log and the
bookkeeping files above.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: every other `vactrol-editor-*.md` plan

# Vactrol Editor: Serial Reconciliation, Real-Wasm Criteria Tests, Bookkeeping (ED-FINAL) Implementation Plan

**planId**: ED-FINAL (issue #5, TASK-010, wave 5; the join integrity check, real-wasm criterion tests over G1-G6,
`app/main.ts` wiring fixes, full-tree verification, TASK-010 checkboxes with evidence, pending manual gates, plan
statuses, README updates, commit staging list and message)
**Status**: Completed (session 188 rerun, attempt-2: items 5-10 done; every automated row exit=0; test-integrity, adversarial and integration review accepted with no findings; manual gates PENDING USER CONFIRMATION; archiving to completed/ in a follow-up docs commit after the workflow commit)
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

- [x] Join integrity recorded; every mismatch explained
- [x] `criteria.test.ts` and `packages.test.ts` pass against the real artifact (`target/fe-logs/ed-final-wasm-tests-s188-1.log`, 3 files, 20 tests, exit=0)
- [x] All rows pass with logs cited (T1 may be BLOCKED only under E1, stated as a gap) — T1 passed, no gap (`target/fe-logs/ed-final-*-s188-1.log`)
- [x] `vactrol-core.md` TASK-010 criteria checked with evidence, manual gates PENDING USER CONFIRMATION, status set
      per item 6; Module Status updated
- [x] `impl-plans/README.md`, `README.md` and the ED plan statuses updated; the commit staging list and message recorded

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-FINAL implementer)` entry. Edit only this log and the
bookkeeping files above.)

### Session: 2026-09-26 (session 187, ED-FINAL implementer)

**Tasks Completed**: items 1-5. Items 6-10 were deliberately NOT done because of the blocker below. Evidence:
`tmp/ed-editor-20260926-s186/ED-FINAL/attempt-1/` (join-integrity.txt, notes.md, intent.md, run.sh, pre/post/final
hashes, highlight-unit-probe.txt). Logs: `target/fe-logs/ed-final-<check>-s187-1.log`.

- Item 1: join-integrity.txt. The only mismatches are the six scaffold `mount.ts` stubs, replaced by their owners
  (each owner's hashes verify OK), and five plan files changed by checkpoint 1c02480 (`git diff HEAD` empty). There is
  no unexplained drift.
- Item 2: `editor/test/wasm/criteria.test.ts` (547 lines, 9 tests, node env, real artifact). It covers:
  - criterion 1: `doc_revision` 1 and a lookahead-bounded emission time for every event, spans through a non-ASCII
    line, and `HighlightScheduler` + `DocumentSync` on a `MockClock`;
  - criterion 2: a direct write goes out as a cell batch with no eval-result or pass, and the next site table
    carries it; a reeval write gives one `bindings` batch with a new `form_gen`, then `stale-form-gen`;
    `doc-changed` then write gives `edit-invalidated`;
  - criterion 3 sites: the three origins, the `lpf` call fields, and an inst-default write with no eval-result,
    no stale-binding and no inst rebuild;
  - criterion 11: the `osc > rotate` program only at the boundary, then compiled and drawn by `GlRenderHost` over
    `RecordingGL`; the `text "hello"` asset rasterized; the broken chain `rotate "a"` gives a form failure, no o0
    program, and the old program keeps drawing;
  - `session_check`: a type error changes nothing that plays.
- Item 3: `editor/test/wasm/packages.test.ts` (192 lines, 3 tests) runs ED-PKG over the real core:
  - the pane lists the unlocked import, import fetches list/toml/zip, and the next eval loads `pads.level = 42`;
  - a traversal zip shows `package-integrity` in the pane;
  - `restorePackages` with a tampered stored body gives `package-integrity` and deletes the lock and body.
- Item 4: no wiring fix is needed. A throwaway jsdom smoke test mounted all six real areas without errors
  (notes.md 2). `app/main.ts` and `app/layout.ts` are unchanged.
- Item 5 (serial, in order): the corrective `--lib` host-wasm row runs before V6c, per the ED-WASM precedent.
  `ed-final-artifact-after-v6b` shows the stub (4 exports); `ed-final-v6c-copy` shows ED-FINAL.wasm with 48 exports,
  `session_init`, identical to `deps/vactrol.wasm`.

**BLOCKER (dependency, outside ED-FINAL writePaths)**: `editor/src/code/highlight.ts` (ED-CODE) `durSeconds()`
treats `playing.dur` as cycles. The Rust wire sends beats (`src/session/publish.rs:297` `dur_beats`), and ED-PARAMS
`grid.ts`/`roll.ts` already read it as beats. Every highlight therefore lasts 4x its step.
- Failing test: `criteria.test.ts` "HighlightScheduler on the mock clock activates exactly the playing step"
  (`[0.62, ':bd,:sd']` vs `[0.62, ':sd']`).
- The same test passes when the scheduler's tempo is overridden to beats_per_cycle 1 (highlight-unit-probe.txt).
- Repair: `durSeconds` = beats * 60 / bpm, and the matching `editor/test/code/highlight.test.ts` expectations.
- Resume: after the repair (integration review: redispatch ED-CODE, or repair in place), re-run ED-FINAL items 5-10.

**Verification** (exit codes from the logs):
- Pass (exit=0): build, build-lsp, clippy, clippy-lsp; nextest (1007 run, 1007 passed, 1 skipped); cargotest
  (lib 986, cli 9, directive_fixtures 2, spec_fixtures 10 + 1 ignored); fmt; wasm32; wasm32-hostwasm;
  wasm32-hostwasm-lib; v6c-copy; rs-lines (max 799); tree and tree-hostwasm (clean); clippy-wasm32 (X1); node
  (v26.9.0 / npm 11.19.1); npm-ci; npm-check; npm-build (`VACTROL_REQUIRE_SESSION_ABI=1`); dist-check; ts-lines
  (max 547); tauri-fetch, tauri-check, tauri-fmt, root-cargo-untouched, tauri-perms (`true`), tauri-status
  (`target/`, `gen/` ignored); session (F2: 233 passed); lsp-smoke (F3: 1 passed); fixtures (F4: 10 passed);
  frozen (F5); git-status (F6: node_modules, dist and src-tauri/target ignored only); manifest-json (F7).
- **FAIL: npm-test** (exit=1; 54 files, 336 tests: 335 passed, 1 failed) and **wasm-tests** (F1, exit=1; 3 files,
  20 tests: 19 passed, 1 failed). The only failure is the blocker test above.
- F8 `getusermedia` prints the comment line `editor/src/visual/meters.ts:6` (a doc comment saying there is no
  `getUserMedia`, the same as reconcile-s186-2), not `none`. The code-only check `ed-final-getusermedia-code` prints
  `none` (exit=0).

**Not done (blocked)**: the TASK-010 checkboxes and status in `vactrol-core.md`, `impl-plans/README.md`, the
`README.md` Editor section, the ED plan statuses and closing notes, and the commit staging list and message.

### Session: 2026-09-26 (session 188, ED-FINAL implementer, attempt-2)

**Tasks Completed**: items 5-10 (items 1-4 stand from session 187; item 1 re-checked). Evidence:
`tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/` (base-commit.txt = ea95f1b, join-integrity.txt, intent.md, run.sh,
pre/post/final hashes, test-names.txt, staging-list.txt, commit-message.txt). Logs: `target/fe-logs/ed-final-<row>-s188-1.log`.

- Item 1 (re-check): 184 OK. The explained mismatches are:
  - the six scaffold `mount.ts` stubs (each owner's hash is OK);
  - the plan files changed by checkpoints 1c02480/ea95f1b (`git diff HEAD -- impl-plans/` is empty);
  - the operator's `highlight.ts` / `highlight.test.ts` repair (checkpoint ea95f1b).
  `criteria.test.ts` and `packages.test.ts` are unchanged from attempt-1. There is no unexplained drift.
- Items 2-4: no edits. The blocker test "HighlightScheduler on the mock clock activates exactly the playing step" now
  passes. `app/main.ts` and `app/layout.ts` are unchanged.
- Item 5 (serial; every row exit=0):
  - Rust: build, build-lsp, clippy, clippy-lsp, fmt; nextest (1007 run, 1007 passed, 1 skipped); cargotest (lib 986,
    cli 9, directive_fixtures 2, spec_fixtures 10 + 1 ignored); wasm32 and wasm32-hostwasm; clippy-wasm32 (X1);
    tree and tree-hostwasm (no native crates).
  - Artifact: `artifact-after-v6b` shows the 4-export stub. The corrective `wasm32-hostwasm-lib` row runs next. In
    `v6c-copy`, ED-FINAL.wasm has 48 exports including `session_init` and equals `deps/vactrol.wasm`.
  - Line limits: rs-lines max 799, ts-lines max 547.
  - npm: node v26.9.0 / npm 11.19.1; npm-ci; npm-check; npm-test (54 files, 336 tests); npm-build
    (`VACTROL_REQUIRE_SESSION_ABI=1`); dist-check; wasm-tests (F1: abi, criteria, packages; 3 files, 20 tests).
  - Tauri: tauri-fetch, tauri-check, tauri-fmt, root-cargo-untouched, tauri-perms (`true`), tauri-status (`gen/`,
    `target/` ignored).
  - Final rows: session (F2: 233 passed); lsp-smoke (F3: 1 passed); fixtures (F4: 10 passed); frozen (F5);
    git-status (F6: node_modules, dist, src-tauri/target and gen ignored only); manifest-json (F7).
  - F8: `getusermedia` prints only the doc comment `editor/src/visual/meters.ts:6`, as in session 187.
    `getusermedia-code` prints `none`.
  - Extra evidence rows: `wasm-tests-verbose` and `npm-test-verbose` (test names, exit=0).
- Item 6: `vactrol-core.md`:
  - TASK-010 is COMPLETED, and all 12 criteria are checked with test names and log paths.
  - PENDING USER CONFIRMATION: criterion 1 (hearing audio), criterion 11 (real-browser view) and criterion 12
    (`cargo tauri build` and the app run).
  - Also updated: the deliverable amendment (plans, manifest, G1-G6, E2/E3/E4), Module 8 status, the Module Status
    and Dependencies rows, the Verification note, a progress-log entry and a Related Plans row.
  - The project-level completion criteria are left unchanged. They still wait on the TASK-008/009/010 manual gates.
- Item 7: `impl-plans/README.md`: the ED rows are Completed (ED-FINAL "Implemented, pending formal review"), and the
  core row and manifest row are refreshed.
- Item 8: `README.md` has a new Editor section, and the Status paragraph is updated.
- Item 9: the ten ED plan status lines were already Completed (checkpoints). Each plan gets a
  `### CLOSING NOTE (ED-FINAL, session 188)` with its evidence.
- Item 10: commit record.
  - Staging list: `staging-list.txt`, 202 explicit files. The list is `git diff --name-only` plus
    `git ls-files --others --exclude-standard`, so ignored paths are excluded.
  - Top-level `editor/` files: `.gitignore`, `index.html`, `package.json`, `package-lock.json`, `tsconfig.json`,
    `vite.config.ts`, `vitest.config.ts`, `worklet/host.js`, `worklet/host.d.ts`.
  - `editor/src-tauri/`: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/main.rs`, `tauri.conf.json`,
    `capabilities/default.json`, `icons/icon.png`.
  - Editor sources and tests: 79 files under `editor/src/**` and 65 under `editor/test/**`.
  - ED-WIRE / ED-WASM Rust: `src/directives/attach.rs`, `src/directives/tests/attach.rs`,
    `src/host/wasm/{abi,mod,session_half,session_hosts}.rs`, `src/ns/insts.rs`,
    `src/pkg/{mod,driver,mem_cache}.rs`, `src/pkg/tests/{mod,driver,mem_cache}.rs`,
    `src/sched/{mod,runtime,render}.rs`, `src/sched/tests/{mod,render}.rs`,
    `src/session/{editors,eval,frontend,mod,protocol,publish,session}.rs`,
    `src/session/tests/{codec,editor_wire,mod}.rs`.
  - Docs: `README.md`, `impl-plans/README.md`, `impl-plans/active/vactrol-core.md`,
    `impl-plans/active/vactrol-editor-*.md` (11).
  - NEVER stage `node_modules/`, `dist/`, `target/`, `tmp/` or `editor/src-tauri/gen/`. The dispatch manifest is
    unchanged and not staged.
  - Commit message: `commit-message.txt`. It has six sections on separate lines and no attribution or Co-Authored-By
    line. The subject is `feat: implement the editor - browser + Tauri, visual feedback, controller binding
    (TASK-010, #5)`.

**Pending (later workflow steps)**: test-integrity, adversarial and integration review; exact-file staging and the
commit; archiving the plans to `impl-plans/completed/` in a follow-up docs commit.

### Session: 2026-09-26 (session 188, step 8 documentation refresh)

**Reviews**: step 6 test integrity was accepted with no findings. Step 7 adversarial review was accepted with no
findings and needs_revision=false. Integration review accepted ED-FINAL. Step 7b E2E was skipped because the
repository has no browser E2E suite; the proxies are 336 vitest tests (jsdom) and 20 real-wasm tests. Status is now
Completed.
**Docs**: `README.md` Editor section now warns that a build without `--lib` can overwrite the artifact with a stub
that has no `session_init`. The session-layer rows in `impl-plans/README.md` move to Completed Plans, archived in
f345e62.
**Not archived here**: the eleven ED plans and the dispatch manifest stay in `impl-plans/active/`. The staging list
(202 paths) and the manifest's plan paths refer to them there, and the accepted tradeoff archives them in a separate
docs commit after the workflow commit, as for issues #3 and #4.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: every other `vactrol-editor-*.md` plan

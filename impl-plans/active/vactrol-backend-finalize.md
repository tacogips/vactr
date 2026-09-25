# Vactrol Back End: End-to-End Tests, Reconciliation, Bookkeeping (BE-FINAL) Implementation Plan

**planId**: BE-FINAL (issue #3 serial reconciliation: headless end-to-end tests, `HostManifest` editor metadata, fixture reclassification, crate-wide checks, vactrol-core.md TASK-007/008 bookkeeping)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 12.8.1 (fixtures, session socket amendment), 12.8.3 (wiring), 12.8.12 (FINAL row, verification), 6.5.7, 7.1.7 (fixture classes); 16.1; 13.5 (`EditorDecl` through `HostManifest`)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
**dependsOn**: BE-MIDI, BE-NATIVE, BE-WASM
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json (never edited by this plan)

---

## Intent and Context

All implementation waves have joined. This plan runs alone, so it is the serial repair point. It adds the headless
end-to-end tests that need SCHED, DSP and INST together (TASK-008 criteria 2, 3, 4 and the beep proxy), carries the
`dsp::meta` table through `HostManifest` (13.5), reclassifies the four `deferred_to = "TASK-008"` fixture blocks, runs
every check on the whole tree, and does the cross-plan bookkeeping: vactrol-core.md TASK-007/008 checkboxes with
evidence and status COMPLETED, the TASK-008 deliverable amendments (session socket moved to TASK-009; raw wasm ABI
instead of wasm-bindgen), the README index, and the commit staging list with the six-section message.

## Non-Goals

- No new features. A defect found here is repaired serially with an intent snapshot, in the smallest edit, and recorded.
- Never edit the dispatch manifest. Never commit (the workflow commit step does). Never move plan files: archiving to
  `impl-plans/completed/` happens in a separate docs commit after the workflow commit, as issue #2 did in eddb5f0,
  because moving plan files during the run would break the gates that read their paths.
- No change to the manual beep check: it stays "pending user confirmation".

## writePaths (exclusive)

- `src/host/tests/e2e.rs` and new files under `src/host/tests/e2e/`
- `src/types/manifest.rs` (editor metadata carried through `HostManifest`)
- `tests/fixtures/spec/manifest.toml` (the four TASK-008 blocks, plus any multiset repin the joined tree needs)
- `impl-plans/active/vactrol-core.md`, `impl-plans/README.md`, `impl-plans/active/vactrol-backend-finalize.md`

## sharedPaths (serial; intent snapshot before every edit)

- Plan status lines and a closing note only: `impl-plans/active/vactrol-backend-{contracts,sched,dsp,inst,midi,native,wasm}.md`
- Serial repair and `rustfmt` only, when a check on the joined tree fails in that file: any `.rs` file under `src/`,
  `examples/beep.rs`, and `editor/**/*.js` (each repaired file and reason recorded)

## File-Level Changes

1. `src/host/tests/e2e/*.rs` (headless; every render inside `alloc_probe::armed` asserting 0):
   - `templates.rs` (criterion 2): each of `sampler`, `analog`, `fm`, `pd`, `additive`, `wavetable`, `granular` compiles
     from prelude source to an `InstDef` through a real `Evaluator` + `InstRegistry`, installs into a `dsp::Engine`, and
     renders non-silent, finite audio for a `s :<template> > note [..] > once` event committed by a real `Runtime` into a
     ring consumed by the engine; a template parameter sent as a pattern control changes the output.
   - `regions.rs` (criterion 3, end to end): sliced events render exactly their begin/end region; `splice` rate-fitted to
     its step; `loop-at 2` over two cycles with loop enabled; `fit`; bounds without overrun; partitioned staging
     (multi-tick lookahead, boundary splits) renders identically to the full-cycle render; overlapped staging
     ([0,3/4)+[1/4,1)) and a repeated window give each voice start exactly once while the identical two-branch stack
     gives both (multiplicity at the audio ring); the future-span `chop 2` case renders voices at 1 and 3/2 exactly once
     in both query orders; the slice-index refresh (0 -> 1) renders one voice playing [1/2,1) with no old-region output.
   - `buses.rs` (criterion 4): `bus :drums:` compiles, a slot routes into it via `bus`, `master` receives every bus, a
     bus-chain redefinition swaps under the generation + refcount lifecycle with zero callback allocation; `room` maps to
     the bus unit parameter.
   - `beep.rs`: the `examples/beep.rs` program (`s :analog > note [:a4] > once`) rendered headlessly through `Runtime` +
     `Engine`, non-silent and finite: the automated proxy for the manual criterion 10.
2. `src/types/manifest.rs`: `HostManifest` exposes the editor metadata (`dsp::meta::decl_for`) for every builtin (13.5);
   a unit assertion in `e2e/templates.rs` that every template and effect name has a declaration.
3. `tests/fixtures/spec/manifest.toml`: design-music ordinals 2, 4, 5, 6 move from `deferred` to `positive` or
   `diagnostic` with exact `check_diags`/`run_fails` (7.1.7), evaluated with the fixture `NoopHost` (file I/O still
   `host-unavailable`); each `note` says why; ordinal 4 notes B3 for the `inst drum: sampler ...:` line.
4. `impl-plans/active/vactrol-core.md`: TASK-007 and TASK-008 checkboxes checked ONLY with cited evidence (plan, test
   file, log path, `exit=`); criterion 10 of TASK-008 marked "pending user confirmation (automated proxy:
   `src/host/tests/e2e/beep.rs`)"; criteria 5 and 6 cite BOTH `src/dsp/tests/dsp/{granular,analyzer}.rs` (render half)
   and `src/sched/tests/sched/granular.rs` (diagnostic half); criteria 8 and 11 checked only from the F8 final-tree
   harness run (or the operator's headed report on the final tree per B1, explicitly labelled operator-run); statuses COMPLETED only when every other box is checked; the
   TASK-008 deliverable text amended (session socket -> TASK-009, raw `extern "C"` ABI instead of wasm-bindgen) and
   TASK-009 gains the session socket; Module 6/7 status lines, Module Status table and a progress-log entry updated.
5. `impl-plans/README.md`: rows for the eight BE plans and the dispatch manifest; vactrol-core.md row updated.
6. Commit staging list (recorded in this plan's progress log for the workflow commit step): explicit paths only
   (`Cargo.toml`, `Cargo.lock`, `src/`, `examples/`, `editor/`, `tests/fixtures/spec/manifest.toml`, `impl-plans/`,
   `design-docs/`); message with the six CLAUDE.md sections on separate lines, no AI attribution and no Co-Authored-By
   line; the operator commits and pushes if the git-commit node rejects the multi-line message.

## Join Integrity

Before any edit, compare every plan's `final-hashes.txt` with the current tree (`shasum -a 256 -c`); explain each mismatch
(a later plan's legitimate edit, or drift) in `tmp/be-backend-20260925-s181/BE-FINAL/attempt-<n>/join-integrity.txt`.

## Invariants

- A checkbox is checked only with evidence; an unmet criterion is reported, never checked.
- Crate-wide `cargo fmt --check` passes; if not, `cargo fmt` is run once and every touched file is listed.
- No `.rs` file reaches 800 lines; no native-only dependency reaches wasm32.

## Edit Protocol

Common protocol of `vactrol-backend-contracts.md` (except rule 5: this plan may run crate-wide `cargo fmt` once, recorded),
evidence under `tmp/be-backend-20260925-s181/BE-FINAL/attempt-<n>/`.

## Verification (the issue's final-tree contract; `<wave>` = `final`)

Common table rows V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8, plus:
- F1: LOG(`be-final-fmt`): `CARGO_TERM_QUIET=true cargo fmt --check` -> `exit=0`.
- F2: LOG(`be-final-example`): `CARGO_TERM_QUIET=true cargo build --example beep` -> `exit=0`.
- F3: LOG(`be-final-e2e`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/host::tests::e2e/)'` -> `exit=0`, counts cited.
- F4: `grep -c 'deferred_to = "TASK-008"' tests/fixtures/spec/manifest.toml || true` -> prints `0`.
- F5: `CARGO_TERM_QUIET=true cargo tree -e normal --target wasm32-unknown-unknown` -> no `cpal`/`midir`.
- F6: `jq . impl-plans/active/be-backend-20260925-s181-dispatch.json > /dev/null` -> exit 0 (read-only check).
- F7: `git status --short` and `git diff --stat` recorded.
- F8: ALWAYS re-run the harness on the final tree, after V6b (so the host-wasm build is current) and after every serial
  repair: LOG(`be-final-harness`): `node editor/dev-harness/run-headless.mjs`, report
  `target/fe-logs/be-wasm-harness-s<S>-<n>.json` (the runner's naming). TASK-008 criteria 8 and 11 are checked ONLY from
  this final run with `exit=0` and every check id passing, or, if it exits 2 (blocked), from an operator-run
  `--headed` report on the same final tree, explicitly labelled operator-run per B1. An earlier BE-WASM report is never
  cited as the proof.

## Completion Criteria

- [ ] Join integrity checked and explained
- [ ] TASK-008 criteria 2, 3, 4 proven end to end; beep proxy passes; TASK-007 criterion 12 and TASK-008 criteria 9, 12
      (build, nextest, wasm32 host-wasm) proven by the final rows
- [ ] `HostManifest` carries the editor metadata; the four TASK-008 fixture blocks reclassified (F4 = 0)
- [ ] V1-V8 and F1-F8 pass with logs cited
- [ ] vactrol-core.md bookkeeping done with evidence (criterion 10 pending user confirmation; 5/6 cite the DSP and SCHED
      tests; 8/11 only from the F8 final-tree harness run or a labelled operator-run report); README updated; BE plans marked Completed (archive after the workflow commit)
- [ ] Commit staging list and six-section message recorded

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-FINAL implementer)` entry. Edit only this log and the sharedPaths
status lines.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-007, TASK-008)
- **Previous**: vactrol-backend-midi.md, vactrol-backend-native.md, vactrol-backend-wasm.md

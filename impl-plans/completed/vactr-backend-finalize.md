# Vactr Back End: End-to-End Tests, Reconciliation, Bookkeeping (BE-FINAL) Implementation Plan

**planId**: BE-FINAL (issue #3 serial reconciliation: headless end-to-end tests, `HostManifest` editor metadata, fixture reclassification, crate-wide checks, vactr-core.md TASK-007/008 bookkeeping)
**Status**: Completed (serial reconciliation, e2e rigs and TASK-007/008 bookkeeping done in session 182; adversarial review accepted; the workflow's final integration review failed on output-contract slips twice, so the operator verified the reconciled tree and made the commit)
**Design Reference**: design-docs/specs/design-implementation.md 12.8.1 (fixtures, session socket amendment), 12.8.3 (wiring), 12.8.12 (FINAL row, verification), 6.5.7, 7.1.7 (fixture classes); 16.1; 13.5 (`EditorDecl` through `HostManifest`)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactr/issues/3
**dependsOn**: BE-MIDI, BE-NATIVE, BE-WASM
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json (never edited by this plan)

---

## Intent and Context

All implementation waves have joined. This plan runs alone, so it is the serial repair point. It adds the headless
end-to-end tests that need SCHED, DSP and INST together (TASK-008 criteria 2, 3, 4 and the beep proxy), carries the
`dsp::meta` table through `HostManifest` (13.5), reclassifies the four `deferred_to = "TASK-008"` fixture blocks, runs
every check on the whole tree, and does the cross-plan bookkeeping: vactr-core.md TASK-007/008 checkboxes with
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
- `impl-plans/active/vactr-core.md`, `impl-plans/README.md`, `impl-plans/active/vactr-backend-finalize.md`

## sharedPaths (serial; intent snapshot before every edit)

- Plan status lines and a closing note only: `impl-plans/active/vactr-backend-{contracts,sched,dsp,inst,midi,native,wasm}.md`
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
4. `impl-plans/active/vactr-core.md`: TASK-007 and TASK-008 checkboxes checked ONLY with cited evidence (plan, test
   file, log path, `exit=`); criterion 10 of TASK-008 marked "pending user confirmation (automated proxy:
   `src/host/tests/e2e/beep.rs`)"; criteria 5 and 6 cite BOTH `src/dsp/tests/dsp/{granular,analyzer}.rs` (render half)
   and `src/sched/tests/sched/granular.rs` (diagnostic half); criteria 8 and 11 checked only from the F8 final-tree
   harness run (or the operator's headed report on the final tree per B1, explicitly labelled operator-run); statuses COMPLETED only when every other box is checked; the
   TASK-008 deliverable text amended (session socket -> TASK-009, raw `extern "C"` ABI instead of wasm-bindgen) and
   TASK-009 gains the session socket; Module 6/7 status lines, Module Status table and a progress-log entry updated.
5. `impl-plans/README.md`: rows for the eight BE plans and the dispatch manifest; vactr-core.md row updated.
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

Common protocol of `vactr-backend-contracts.md` (except rule 5: this plan may run crate-wide `cargo fmt` once, recorded),
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

- [x] Join integrity checked and explained (`tmp/be-backend-20260925-s181/BE-FINAL/attempt-1/join-integrity.txt`)
- [x] TASK-008 criteria 2, 3, 4 proven end to end; beep proxy passes; TASK-007 criterion 12 and TASK-008 criteria 9, 12
      (build, nextest, wasm32 host-wasm) proven by the final rows
- [x] `HostManifest` carries the editor metadata; the four TASK-008 fixture blocks reclassified (F4 = 0)
- [x] V1-V8 and F1-F8 pass with logs cited
- [x] vactr-core.md bookkeeping done with evidence (criterion 10 pending user confirmation; 5/6 cite the DSP and SCHED
      tests; 8/11 only from the F8 final-tree harness run or a labelled operator-run report); README updated; BE plans marked Completed (archive after the workflow commit)
- [x] Commit staging list and six-section message recorded

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-FINAL implementer)` entry. Edit only this log and the sharedPaths
status lines.)

### Session: 2026-09-25 (session 186, BE-FINAL implementer)

**Dependency admission**: BE-MIDI, BE-NATIVE, BE-WASM (and all earlier waves) are in the runtime `acceptedPlanIds`.
**Join integrity**: `tmp/be-backend-20260925-s181/BE-FINAL/attempt-1/join-integrity.txt` (raw `join-raw.txt`, matches `join-match.txt`). BE-DSP, BE-NATIVE, BE-WASM clean; every BE-CONTRACTS/BE-SCHED/BE-INST/BE-MIDI mismatch equals a later owner's final-hashes or reconcile tree hash (or HEAD for the contracts plan document). No unexplained drift.
**Work done** (all Rust via the rust-coding agent; intents, pre/post hashes in the attempt-1 directory):
- e2e suite `src/host/tests/e2e.rs` (rig: Runtime + Evaluator over one InstRegistry, NativeAudioHost::headless, every render under `alloc_probe::armed` == 0) with `beep.rs` (1), `templates.rs` (3), `regions.rs` (11), `buses.rs` (5), `sched_gaps.rs` (7; TASK-007 sub-clauses the evidence audit found uncovered: once/at revocation, re-sent future control semantics, removed-work print, dry-run control channel/cells snapshot, stop on texture, hush on OSC).
- `src/types/manifest.rs`: `HostManifest::editor_decl`, `editor_decls`, `template_params` (13.5).
- Fixtures: design-music ordinals 2, 4, 6 -> diagnostic, 5 -> positive, exact pins and notes (ordinal 4 cites B3); F4 prints 0.
- Serial repairs (each found by a failing e2e check on the joined tree): R1 inst header parameter as a run-time pattern control (compile/compiler.rs, compile/matchc.rs, vm/natives/music.rs, types/natives_domain.rs, types/check.rs; test pin types/tests/diags.rs `loop`); R2 catalog port wiring + template resource defaults (dsp/build.rs, ns/insts.rs, sched/commit.rs, dsp/ugen/wavetable.rs; pins types/tests/inst/{implicit,templates}.rs); R3 `bus` routing control (host/caps.rs, ns/insts.rs, sched/commit.rs); R4 commit-time `speed-fit` (sched/commit.rs); R5 `loop-at` bool (pattern/combinators/region.rs; pin pattern/tests/region.rs); R6 effect-local parameter ids (dsp/build.rs; pins types/tests/inst/{buses,effects}.rs).
**Evidence** (session 186, `target/fe-logs/be-final-<check>-s186-1.log`, every log ends `exit=0`):
- V1 build; V2 clippy `--all-targets -- -D warnings`; F1 fmt `--check` (no crate-wide rewrite needed)
- V3 nextest 778 run / 778 passed / 1 skipped; V3t cargo test lib 768 + spec_fixtures 10 (1 ignored); V3f fixtures 10/10; F3 e2e 27/27
- F2 `cargo build --example beep`; V6a wasm32; V6b wasm32 host-wasm (`target/wasm32-unknown-unknown/debug/vactr.wasm` present); F5 `be-final-tree-s186-1.log`: no cpal/midir
- F8 harness (after V6b, after every repair): `be-final-harness-s186-1.log` exit=0, report `be-final-harness-s186-1.json`: HeadlessChrome 154, 18/18 PASS, memoryStable true (module `debug/deps/vactr.wasm`; no `src` file newer)
- V4: largest `.rs` `src/dsp/build.rs` 799 lines (`attempt-1/v4-wc.txt`); V5 prints `none`; V8 covered by crate-wide fmt check; F4 `grep -c 'deferred_to = "TASK-008"'` = 0; F6 `jq` exit 0 and the dispatch manifest is unchanged vs HEAD; F7 `attempt-1/f7-git-status.txt`, `f7-git-diff-stat.txt` (60 tracked files changed, 3060+/158-).
**Bookkeeping**: vactr-core.md TASK-007 12/12 and TASK-008 11/12 checked with evidence (criterion 10: pending user confirmation, proxy `e2e/beep.rs`), both COMPLETED; TASK-008 deliverable amended (session socket -> TASK-009, raw `extern "C"` ABI) and TASK-009 names the socket; Module 6/7 status, Module Status and Dependencies tables, Verification and a progress-log entry updated. README rows updated; the seven wave plans marked Completed with closing notes (archive after the workflow commit). The dispatch manifest was not edited.
**Residuals (not blocking)**: one-line `inst NAME: BODY` not a run-time definition head; `[1 0.5] > osc "/x"` run-time `type`; `effect_ports` names for `balance` and ~36 single-parameter effects (explicit lowering error); pattern-valued bus parameters `inst-failed`.

**Commit staging list** (explicit paths, for the workflow commit step): `Cargo.toml`, `Cargo.lock`, `src/`, `examples/`, `editor/`, `tests/fixtures/spec/manifest.toml`, `impl-plans/`, `design-docs/`. Never `tmp/` or `target/`.

**Commit message** (six sections on separate lines; no AI attribution, no Co-Authored-By line; the operator commits and pushes if the git-commit node rejects the multi-line message; current version at `tmp/be-backend-20260925-s181/BE-FINAL/attempt-2/commit-message.txt`, also mirrored in attempt-1):

```
feat: implement runtime back end - scheduler, slot table, hosts, DSP graph, native and wasm audio hosts (TASK-007..008, #3)

1. Primary Changes and Intent:
   Implements the Vactr runtime back end from impl-plans/active/vactr-core.md TASK-007 and TASK-008 (issue #3) through eight wave plans (BE-CONTRACTS, BE-SCHED, BE-DSP, BE-INST, BE-MIDI, BE-NATIVE, BE-WASM, BE-FINAL): the two-horizon scheduler with slot table, occurrence merge, rebind/control channel, cell transport and dry run; the DSP graph with synthesis templates, effects, buses, analyzers and granular; the native cpal/midir host and the raw-ABI wasm/AudioWorklet host with a headless-Chrome dev harness; and end-to-end headless tests from vactr source to rendered audio.

2. Key Technical Concepts:
   - Ratio-time two-horizon scheduling, occurrence records with covered extents, committed-occ ledger, SlotControl monotone merge
   - POD AudioEvent/Ctl::Cell transport: native atomic cells, browser mirror protocol (CellInit/epochs/batches)
   - Lock-free SPSC rings, preallocated voice pool, zero callback allocation (cfg(test) alloc_probe)
   - inst realization to InstDef, runtime ugen catalog ports, effect-local parameter ids, BusGraph generation+refcount swap
   - Granular engine (phase accumulator, live capture window, freeze), analyzers as f32 cells
   - host-native (cpal, midir, optional, non-wasm32 only) and host-wasm (raw extern "C", no crates), 16.1 resource lifecycle

3. Files and Code Sections:
   - src/sched/: runtime, staging, commit, ledger, control, cells, dryrun, oneshot, telemetry, midi_clock, midi_in + tests
   - src/dsp/: engine, voice, build, bus, granular, fft, arena, ring, meta, controls, cells, release, ugen/, effects/ + tests
   - src/host/: caps, wire, noop, testing, native/ (audio, midi, loader, tick), wasm/ (abi, main_half, worklet_half, messages, lifecycle) + tests incl. tests/e2e/{beep,templates,regions,buses,sched_gaps}.rs
   - src/ns/insts.rs, src/prelude/templates.vact, src/vm/natives/dsp.rs, src/types/*, src/compile/*: instruments, templates, implicit controls, overloads, inst-header controls at run time
   - editor/worklet/, editor/dev-harness/: plain JS glue and the headless harness; examples/beep.rs
   - Cargo.toml/Cargo.lock (cpal 0.16, midir 0.10 behind host-native), tests/fixtures/spec/manifest.toml, impl-plans/, design-docs/

4. Problem Solving:
   BE-FINAL found and serially repaired integration defects only visible end to end: inst header parameters failed as pattern controls at run time (R1); ugen inputs were wired to the wrong runtime ports and template resource defaults never loaded, so sampler/wavetable/granular were silent (R2); the bus routing control, speed-fit (splice/loop-at/fit) and loop-at's loop flag never reached the audio side (R3-R5); bus/master effect parameters were keyed in the wrong id space and ignored (R6); a print in a control evaluated on events that degrade-by/maybe then removed was forwarded to the console once per query window (R7).

5. Impact:
   Compiled patterns now schedule and sound: headless tests render every synthesis template, sample regions, bus chains, analyzers and granular with zero callback allocation, and the browser worklet lifecycle passes 18/18 real-worklet checks. Build, clippy -D warnings, fmt, nextest (779), cargo test and both wasm32 builds pass.

6. Unresolved TODOs:
   - [ ] examples/beep.rs: audible check on a real output device (pending user confirmation)
   - [ ] src/compile: one-line `inst NAME: BODY` is not a definition head at run time (design-music section 4)
   - [ ] src/dsp/build.rs effect_ports: balance and generic single-parameter effect names do not match their parameters
   - [ ] impl-plans: archive the BE plans to impl-plans/completed/ in a separate docs commit
```

### Session: 2026-09-25 (session 186 run 2, BE-FINAL revision after step6-test-integrity-check comm-002235)

**Feedback addressed** (intent and pre/post hashes in `tmp/be-backend-20260925-s181/BE-FINAL/attempt-2/`):
- HIGH (TASK-007 criterion 8): serial repair R7. `query_degrade` (`src/pattern/combinators/random.rs`, which also serves `maybe`) drops the print output captured while evaluating events it removes, through the new `QueryVm::put_output` (`src/pattern/eval.rs`, `src/vm/query_vm.rs`, `src/vm/vm.rs`, `src/pattern/tests/stub_vm.rs`). Structure-only output (inner query produced no events) is unchanged. The commented repro became the regression test `sched_gaps::a_filter_wrapping_the_printing_control_drops_removed_work_output`. It FAILED before R7 (320 duplicated console lines) and passes after. The sched_gaps.rs header and vactr-core.md criterion 8 now agree (closed by R7).
- MID (TASK-008 criterion 3): `regions.rs` asserts exact committed AudioEvent counts: 4 for overlapped, 2 per order for future-span, and 2 for the stack twins. It also asserts a peak upper bound: overlap and future-span equal one clean pass; twins are 2.0x, under 2.5x. Doubling mutations were caught (8 vs 4, 4 vs 2, 3 vs 2) and then reverted.
- Residual: `buses.rs` `a_bus_definition_compiles_and_installs` now ticks through `run_for` before its `faults` check. Also, the attempt-1 `intent.md` line saying "port-index pins updated" is corrected in attempt-2: only the `param:n` and effect-id pins changed.

**Evidence** (`target/fe-logs/be-final-<check>-s186-2.log`, every log ends `exit=0`; the s186-1 logs are superseded):
- fmt; build; clippy
- nextest 779/779 (1 skipped); cargo test lib 769 plus spec_fixtures 10; fixtures 10/10; e2e 28/28
- example; wasm32; wasm32-hostwasm; tree (no cpal/midir)
- F8 harness `be-final-harness-s186-2.log`, report `be-final-harness-s186-2.json`: 18/18 PASS, memoryStable true
- V4: largest file 799 lines (`src/dsp/build.rs`); V5 none; F4 0; F6 manifest unchanged vs HEAD

## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-007, TASK-008)
- **Previous**: vactr-backend-midi.md, vactr-backend-native.md, vactr-backend-wasm.md

### FINDING KEY NOTE (operator, 2026-09-25, after the BE-NATIVE and BE-WASM attempt-1 failures)

- The step6-test-integrity-check and step7-adversarial-review output contracts
  reject unknown keys INSIDE each `findings[]` item. BE-NATIVE failed with
  `$.findings[0].intentRef additional property is not allowed` and BE-WASM with
  `$.findings[0].intentIncerence ...`: both were misspellings of the accepted key
  `intentReference`. Use ONLY the keys the riela contract defines for a finding item:
  `findingId`, `severity`, `category`, `file`, `line`, `message`, `evidence`
  (confirmed against the riela binary); put anything else, such as an intent
  reference or a fix-cost note, inside the `message` or `evidence` text. `findings` must
  be present (empty array when none) and the outputs must not carry `planId`.

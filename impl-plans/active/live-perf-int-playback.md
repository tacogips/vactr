# LPI-PLAYBACK: diagnose and fix the missing playback onset after Run

**Status**: Ready
**Plan ID**: LPI-PLAYBACK (wave 2)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 2 (I5) and 5.2, plus rules D-a/D-b/D-c in section 5
**Manifest**: `impl-plans/active/live-perf-int-dispatch.json`
**Branch**: `merge/live-perf` (shared working directory; no private branch)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Intent and Context

On the merged tree, `npm run e2e:examples` check `run-plays-moderately`
fails with "no playback onset within 10 seconds" (7/8). The intake states
that it passes on main.

The recorded failure (`tmp/examples-e2e/result.json`, 2026-10-18) shows
that audio **is** produced:

- pre-sink peak -3.05 dBFS, RMS -22.26 dBFS;
- eval status `ok`, audio `running`;
- `onsetCount: 0`.

The silent sink (`editor/test/e2e/silent-sink.mjs:64-73`) counts an onset
only when a 10 ms block at >= -40 dBFS RMS follows at least five
consecutive 10 ms blocks at <= -50 dBFS RMS. So on the merged tree, from
audio start, no 50 ms quiet window comes before a loud block. Either the
output is not quiet before the first note, or the first note rises through
the -50..-40 dB band.

Static review ruled out:

- the `OutputStage` initial state (Running, gate 1.0,
  `src/dsp/engine/output.rs`);
- the `VarSlot` momentary default (`None`);
- the `RampedCells` fall-through.

The mechanism is unknown. This plan diagnoses it with real runs, then
fixes it in product code.

## Non-goals

- Never edit `editor/test/e2e/examples.mjs`, `editor/test/e2e/silent-sink.mjs`
  or any threshold (rule D-c).
- Do not change accepted live-perf semantics: gentle drain to -80 dBFS
  then idle, the 15 s cap, the 5 ms cut fade and clear, or D3 Natural
  release of scheduled voices. If the only fix would change one of them,
  stop as described in TASK-P3.
- No changes to the behavior e2e checks; LPI-BEHAVIOR owns them.
- Never change the Mac system volume. Every browser run uses the silent
  sink.

## Dependencies

- **dependsOn**: LPI-WIRE, LPI-VIM-STOP, LPI-NS-INDEX. The wasm and the
  dist must be built from the combined wave-1 tree. Start only after all
  three wave-1 plans are accepted (design section 11, session 438).
- Runs alone in wave 2. LPI-STYLE (wave 3) has a dependsOn edge on this
  plan only to serialize `editor/dist`, the e2e output roots and the
  measurement lock; it uses none of this plan's content. While this plan
  runs, it owns `editor/dist` and the lock.
- Do not touch `editor/src/bind/glide-setting.ts`, `editor/src/bind/bind.css`
  or `editor/test/style/ui-style.mjs`; LPI-STYLE owns them. If
  `npm run test:style` fails here, that is the known UF-1 baseline. It is
  not a gate of this plan.

## writePaths

Product candidates. Edit only the files the diagnosis implicates, and
record which ones in the Progress Log:

- `src/dsp/engine.rs`
- `src/dsp/engine`
- `src/dsp/ramp.rs`
- `src/dsp/voice.rs` (964 lines)
- `src/dsp/voice` (existing submodule directory; split target)
- `src/dsp/bus.rs` (999 lines)
- `src/dsp/bus` (existing submodule directory; split target)
- `src/sched/control.rs`
- `src/sched/runtime/live.rs`
- `src/sched/ramps.rs`
- `src/session/session.rs`
- `src/session/momentary.rs`
- `src/host/wasm`
- `editor/src/code/transport.ts`
- `editor/src/protocol/client.ts`

Regression tests:

- `src/dsp/tests/dsp.rs` (module registration only)
- `src/dsp/tests/dsp/playback_onset.rs` (new; use this rather than
  `output_stop.rs`, which has 981 lines)
- `src/session/tests/mod.rs` (module registration only)
- `src/session/tests/playback_onset.rs` (new, only if the root cause is
  session-side)
- `editor/test/code/playback-onset.test.ts` (new, only if the root cause is
  editor-side)

Conditional blocker record:
`design-docs/user-qa/pending-live-perf-integration-questions.md`.

Plan file: `impl-plans/active/live-perf-int-playback.md` (Progress Log
only).

Artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
`editor/node_modules`, `editor/dist`, `tmp/live-perf-int/playback`, `tmp/live-perf-int/main-ref`, `tmp/live-perf-int/main-ref-target`, `tmp/canvas-cutover`, `tmp/code-render`, `tmp/canvas`, `tmp/examples-e2e`, `tmp/ui-style`.

## sharedPaths

None. Read-only: `editor/test/e2e/examples.mjs`,
`editor/test/e2e/silent-sink.mjs`, `editor/test/e2e/serve.mjs`.

## Tasks

### TASK-P1: Fresh-build reproduction (no code change)

Run each step in the foreground and log it under
`tmp/live-perf-int/playback/`:

1. `mise run build-wasm-release` (`1-wasm-release.log`). The editor build
   embeds `target/wasm32-unknown-unknown/release/vactr.wasm`.
2. In `editor/`: `VACTR_REQUIRE_SESSION_ABI=1 npm run build`
   (`2-build.log`).
3. Under the measurement lock, run `npm run e2e:examples` in `editor/`.
   Save the log to `3-examples-repro.log` and copy
   `tmp/examples-e2e/result.json` to
   `tmp/live-perf-int/playback/3-result.json`. Lock procedure:

   ```
   until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done
   ```

   Then run the suite, then
   `rmdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`.
   Always release the lock, also on failure (use a `trap`). Never hold it
   while idle.

If this run passes 8/8, the earlier failure was a stale build:

- record that, with both result files;
- skip TASK-P2 and TASK-P3;
- go to TASK-P4, which then runs only the final confirmation (no
  regression test is needed when there is no product defect).

### TASK-P2: Onset probe and localization

1. Write a probe, `tmp/live-perf-int/playback/onset-probe.mjs`. It is an
   artifact and is not committed. It imports `installSilentSink` from
   `editor/test/e2e/silent-sink.mjs` and `startServer` from
   `editor/test/e2e/serve.mjs`, and follows examples.mjs lines 66-86:
   chromium `--mute-audio`, open `/?perf=1`, wait for the showcase, click
   `.vact-audio`, wait for `running`, click `.vact-run`.
   - Through its own `addInitScript`, it wraps `AudioNode.prototype.connect`
     the same way the sink does. It records the per-10 ms pre-sink block
     RMS dBFS and peak series from the first destination connect until 5 s
     after Run, with timestamps for the audio start and the Run click.
   - It also samples `window.__vactrPerf.transportSample()` and
     `transportOutput()` every 100 ms.
   - It writes `tmp/live-perf-int/playback/probe-merged.json`.

   Run it under the lock against the fresh dist. Run it with
   `node <path>` from the repository root, never from inside `tmp/`.
2. Classify the series in the Progress Log:
   - **(a)** blocks above -50 dBFS between audio start and Run;
   - **(b)** the first sound rises through -50..-40 dBFS over two or more
     blocks, or the first events are late or missing;
   - **(c)** another shape, with a description.
3. Localize it with one or both methods:
   - **Main reference.**
     `mkdir -p tmp/live-perf-int/main-ref && git archive main | tar -x -C tmp/live-perf-int/main-ref`.
     This is a read-only export; main's checkout and other worktrees are
     never touched. Build its release wasm with
     `cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features host-wasm --manifest-path tmp/live-perf-int/main-ref/Cargo.toml --target-dir tmp/live-perf-int/main-ref-target`.
     Build its editor with `VACTR_WASM` pointing at that wasm, into a
     separate outDir (pass `--outDir` to vite). Probe it the same way,
     with a server rooted at that dist, and diff the two series.
     - The export needs `editor/node_modules`, so symlink
       `tmp/live-perf-int/main-ref/editor/node_modules` to the worktree's
       `editor/node_modules`. Record exactly what was done.
     - If this method fails for tooling reasons, record why and use the
       bisect.
   - **Bisect on the merged tree.** Make throwaway local reverts, one at a
     time. Rebuild the release wasm and dist after each and probe. Restore
     the code after each step; never leave a revert in place. Candidates,
     in this order:
     1. the `stop_acks_full()` early break in `Engine::controls`
        (`src/dsp/engine.rs`);
     2. `OutputStage::gate_sample` and `skips_effects` in
        `src/dsp/engine/render.rs`;
     3. the `RampedCells` read path in `Engine::process` and
        `CellRamps::new(cfg.analysis_cells)`;
     4. `BusGraph::hold_retiring`;
     5. the D3 condition `v.open || v.gate_left > 0` in
        `src/dsp/voice.rs:938`;
     6. `Session::stage_now` and `Runtime::revoke_with`.

     Record each probe result.

### TASK-P3: Fix and regression test

1. Fix the root cause in the implicated writePaths file or files, with the
   smallest change that restores main's playback shape. The accepted
   live-perf semantics must stay intact. Run the existing DSP tests 8.1
   (1)-(4) (`output_stop`, `ramp`, `release`) to prove it.
2. Add a deterministic regression test that fails before the fix and
   passes after it, at the layer of the root cause:
   - DSP: `src/dsp/tests/dsp/playback_onset.rs`, registered in
     `src/dsp/tests/dsp.rs`. Use `NativeRig`/`BrowserRig` as in
     `src/dsp/tests/dsp/output_stop.rs`. Assert the property the probe
     exposed, for example: exact zeros before the first onset, and the
     first scheduled voice's block at full level within one block of its
     time.
   - Session: `src/session/tests/playback_onset.rs`.
   - Editor: `editor/test/code/playback-onset.test.ts`, with fake timers.
3. **Stop rule (design 5.2 step 4).** If the only viable fix changes an
   accepted live-perf semantic:
   - do not apply it;
   - write `design-docs/user-qa/pending-live-perf-integration-questions.md`
     with the evidence (probe files, bisect table) and the options;
   - mark this plan Blocked in its Progress Log and report blocked.

### TASK-P4: Confirmation

Rebuild with `mise run build-wasm-release` and
`VACTR_REQUIRE_SESSION_ABI=1 npm run build`, then run `npm run e2e:examples`
under the lock. It must pass 8/8.

## Pitfalls

- **File-size limit.** These files are close to 1000 lines:
  - `src/dsp/bus.rs`: 999
  - `src/dsp/voice.rs`: 964
  - `src/dsp/engine.rs`: 953
  - `src/session/session.rs`: 907

  A fix in `bus.rs` must have a net line change of 0 or less. Otherwise,
  move the touched code into a submodule under the existing `src/dsp/bus/`
  directory, following `.agents/skills/rust-coding-standards/SKILL.md`.
  Apply the same rule to `voice.rs` (`src/dsp/voice/`) and `engine.rs`
  (`src/dsp/engine/`) if they would reach 1000. Prefer putting new logic
  in the smaller module that owns it, for example
  `src/dsp/engine/output.rs` (260 lines).
- e2e serves the prebuilt `editor/dist`. Always rebuild the release wasm
  and the dist right before every e2e or probe run, or you measure stale
  code.
- Never relax the onset rule, raise the 10 s timeout or edit the fixture.
- Keep all probe and main-ref artifacts under `tmp/live-perf-int/`. Never
  commit them, and never run tools from inside the exported tree except
  through the explicit `--manifest-path` / `--target-dir` / vite `--outDir`
  commands above.
- Do not kill processes this plan did not start.

## Invariants

- design-live-performance.md 4.2-4.4 semantics are unchanged. The
  existing `output_stop`, `ramp` and `release` DSP tests pass unchanged.
- No touched source file reaches 1000 lines.

## Verification (logs under `tmp/live-perf-int/playback/`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0.
2. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` ->
   exit 0.
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib dsp::tests::dsp::output_stop dsp::tests::dsp::ramp dsp::tests::dsp::release dsp::tests::dsp::playback_onset session::tests`
   -> exit 0, failures 0. Omit `playback_onset` if no DSP test was added,
   and record why.
4. If an editor test was added: in `editor/`,
   `./node_modules/.bin/vitest run test/code/playback-onset.test.ts` ->
   exit 0.
5. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   -> exit 0.
6. `mise run build-wasm-release` -> exit 0. Then in `editor/`,
   `VACTR_REQUIRE_SESSION_ABI=1 npm run build` -> exit 0.
7. Under the lock, in `editor/`, `npm run e2e:examples` -> exit 0, with
   `testsRun 8`, `testsPassed 8`, `failureCount 0` from its stdout JSON
   (`7-examples-final.log`).
8. **Mutation evidence** (only as `mutationEvidence`). The regression test
   fails with the fix reverted, and that run is recorded.

## Completion Criteria

- [ ] The P1 reproduction is recorded: a stale-build outcome, or a
  reproduced failure.
- [ ] If reproduced: probe classification, localization evidence and the
  named root cause are in the Progress Log.
- [ ] The fix is applied within writePaths, with a failing-first
  regression test; or the plan is Blocked with a user-QA file.
- [ ] Verification 1-7 exit 0 (steps 3-4 adapted as stated).
  e2e:examples is 8/8.
- [ ] The Progress Log records pre/post sha256 for every edited file.

## Execution Rules (all plans in this manifest)

- Read each file fresh immediately before editing it. Record
  `shasum -a 256 <file>` before and after the edit in the Progress Log.
- Edit only writePaths. Report drift in any other file; never repair it.
- Run commands in the foreground only. Write each to its log with
  `cmd > log 2>&1; echo "exit=$?" >> log`.
- Do not commit, stash, branch or run any mutating git command. Read-only
  `git diff`, `git status`, `git show` and `git archive` are allowed.
- Use English only, with no emojis. Keep every touched source file under
  1000 lines.
- Verification record shape is exactly `{command, exitStatus: 0, testsRun,
  testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
  - Negative-control and mutation runs go only in `mutationEvidence`.
  - Repaired review findings go only in `addressedFeedback`, with status
    `repaired`.
- Edit only this plan's own Progress Log.

## Progress Log

### Session: 2026-10-18 (plan authored, workflow session 433 step 4)
**Tasks Completed**: none. Plan authored from the accepted design.
**Notes**: Ready for wave 2 after all wave-1 plans.

### Session: 2026-10-18 (plan revised, workflow session 438 step 4)
**Tasks Completed**: none (plan revision only).
**Notes**: Wave-1 plans must now be accepted through their R1-R6 reruns before this plan starts. LPI-STYLE (new, wave 3) follows this plan through a serialization-only edge. test:style failures are the UF-1 baseline and not a gate here. No task content changed.

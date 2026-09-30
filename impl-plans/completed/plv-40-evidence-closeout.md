# PLV-40: Evidence, Labels, Provenance and Plan Closeout (serial)

**Status**: Completed
**Plan ID**: PLV-40 (session 209, wave 3 of 3; serial finalization)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Provenance; Comparison probe; Fidelity labels; Rollout item 3)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`; handoff `impl-plans/active/modular-audio-handoff.md` priority 4
**Created**: 2026-09-30
**Last Updated**: 2026-09-30 (session 209 rewrite)

## Intent and Context

This plan runs after PLV-30 and PLV-31 land. It:

1. re-runs the comparison probe (from PLV-21) on the final tree;
2. raises the manifest `voice_layer` label only if the probe says it is
   eligible;
3. confirms that the inventory and notices are truthful now that the
   layer is wired into the 24 templates;
4. records the results in the Plaits plans and the handoff;
5. runs the full verification suite.

Current state, already committed in `f5e623b`:
- `verification/upstream_inventory.toml` already lists `plaits/dsp/voice.cc`,
  `voice.h`, `envelope.h`, `fx/low_pass_gate.h`, `engine/engine.h`,
  `dsp.h` and `stmlib/dsp/limiter.h` as `use = "source"`, and
  `plaits/user_data.h` and `stmlib/system/flash_programming.h` as
  `excluded` (probe-only).
- `THIRD_PARTY_NOTICES.md` has the section "Plaits voice-level layer
  (trigger, decay, low-pass gate, limiter)".
- Session 205's PLV-21 probe run (`tmp/plv/PLV-21/9-probe.json`) reported
  `voice_layer_label_eligible: true`. It must be re-run here; the old
  JSON is not evidence for the final tree.
- `src/dsp/ported/manifest.rs` sets `voice_layer: CoverageState::Pending`
  in `implemented()` (near line 302). `src/dsp/ported/tests.rs` expects
  Pending (lines 144 and 168) and the summary suffix
  ` Voice layer: 24 pending, 0 source-stage.` (line 267).

## Non-goals

- No DSP, kernel, template, lifetime or seed-order change, and no
  `golden_digests.txt` edit. If verification fails because of an earlier
  plan, record a blocker with file, cause and evidence; the orchestrator
  repairs it serially.
- No `SourcePort` label, and no change to any engine `coverage` value.
- Do not re-add inventory entries that exist. Do not edit
  `verification/audit_upstream.py`.
- Do not archive plans to `impl-plans/completed/`. The parent Plaits plan
  remains active.
- Do not change design rules. Only add the status subsection.

## Dependencies

- **dependsOn**: PLV-31 (and through it PLV-30; accepted: PLV-10, PLV-12, PLV-20, PLV-21)
- **Blocks**: none

## writePaths

- `src/dsp/ported/manifest.rs` (`voice_layer` value in `implemented()` only)
- `src/dsp/ported/tests.rs` (voice-layer label and summary expectations only)
- `verification/upstream_inventory.toml` (only if the audit reports an error)
- `THIRD_PARTY_NOTICES.md`
- `design-docs/specs/design-mutable-audio.md` (new status subsection only)
- `impl-plans/active/modular-plaits-engines.md`
- `impl-plans/active/modular-plaits-oscillators.md`
- `impl-plans/active/modular-plaits-resonant-noise.md`
- `impl-plans/active/modular-plaits-wave-replacements.md`
- `impl-plans/active/modular-plaits-cleared-replacements.md`
- `impl-plans/active/modular-audio-handoff.md`
- `impl-plans/README.md` (PLV and handoff/Plaits rows only)
- `impl-plans/active/plv-12-manifest-registration.md` (Status line only)
- `impl-plans/active/plv-21-voice-probe.md` (Status line only)
- `impl-plans/active/plv-40-evidence-closeout.md`
- Evidence: `tmp/plv/s209/PLV-40/hashes.txt`, `tmp/plv/s209/PLV-40/1-fmt.log`, `tmp/plv/s209/PLV-40/2-check.log`, `tmp/plv/s209/PLV-40/3-clippy.log`, `tmp/plv/s209/PLV-40/4-wasm.log`, `tmp/plv/s209/PLV-40/5-lsp.log`, `tmp/plv/s209/PLV-40/6-nextest.log`, `tmp/plv/s209/PLV-40/7-compare.json`, `tmp/plv/s209/PLV-40/7-compare.log`, `tmp/plv/s209/PLV-40/8-audit.log`, `tmp/plv/s209/PLV-40/9-lines.log`, `tmp/plv/s209/PLV-40/10-diffcheck.log`, `tmp/plv/s209/PLV-40/11-status.log`, `tmp/plv/s209/PLV-40/12-golden.log`

## sharedPaths (read-only)

- `verification/audit_upstream.py`: what the audit checks (named paths, notices, include closure, byte copies, numeric windows).
- `verification/compare_plaits_voice.py`: JSON fields `scenarios`, `meets_threshold`, `voice_layer_label_eligible`.
- `verification/plaits_voice_reference.cc`, `examples/plaits_voice_reference.rs`, `src/dsp/ugen/voice_layer.rs`, `src/dsp/ugen/vactrol_gate.rs`: every upstream path they name.
- `src/prelude/templates.vact`: wiring facts for the notices.
- `tmp/plv/s209/PLV-31/10-golden-diff.log`: the golden result to cite.

## Steps

1. **Probe.** With
   `REF=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack`,
   run command 7. If `$REF` is absent, the step is BLOCKED and the label
   stays Pending.
2. **Label.** Only if `7-compare.json` has
   `"voice_layer_label_eligible": true`:
   - change the `voice_layer` value in `implemented()` from `Pending` to
     `SourceStage`;
   - update `src/dsp/ported/tests.rs` to expect `SourceStage` at lines
     144 and 168, and the summary suffix
     ` Voice layer: 0 pending, 24 source-stage.`

   Otherwise change nothing and record every failing scenario with its
   metrics. Never write `SourcePort`.
3. **Inventory.** Run the audit (command 8).
   - If it reports 0 errors, leave `upstream_inventory.toml` untouched.
   - Otherwise fix only the entries it names, keeping the file's group
     order and format and giving an honest `use` and reason, and rerun
     until 0 errors.
   - The four files named by the intake (`plaits/dsp/voice.{h,cc}`,
     `fx/low_pass_gate.h`, `envelope.h`) must remain `use = "source"`.
4. **Notices** (`THIRD_PARTY_NOTICES.md`).
   - In the "Plaits voice-level layer" section, add a short paragraph:
     - all 24 Plaits templates now route through two `vactrol-gate` nodes;
     - the layer is opt-in through the neutral `lpg-mode`, `lpg-decay` and
       `lpg-color` controls;
     - `lpg-mode` defaults to `off`, an exact pass-through;
     - `decay-mod` is opt-in only;
     - state the measured label (SourceStage or Pending) and that the
       result is not a bit-exact source port.
   - Then update each stale sentence claiming that voice-level
     trigger/LPG behavior is "not included", "remains open" or "still
     differ[s]". `grep -n "trigger/LPG" THIRD_PARTY_NOTICES.md` finds them
     (at least lines 652, 675, 756 and 779 today).
     - Change only the voice-level clause so it points to the shared
       opt-in voice layer section.
     - Keep each sentence's engine-specific caveats unchanged.
5. **Records.**
   - `modular-plaits-engines.md`:
     - add a subtask row `PLV-001` (deliverables: the shared voice layer,
       `vactrol-gate`/`decay-mod`, the 24 wired templates,
       `compare-plaits-voice`; status: the measured result and label);
     - in each position row, replace "voice/LPG parity pending",
       "LPG/full voice parity pending" or "source parity/LPG pending" with
       "voice layer PLV-001: <label>, <probe result>", keeping any
       engine-parity remainder;
     - point the SYN-002A sentence about the separate voice-level parity
       requirement (near line 47) to PLV-001;
     - leave the "Trigger/LPG and source algorithm behavior have
       comparison evidence" criterion unchecked, because engine algorithm
       parity remains, and note that the voice-layer part is recorded;
     - add a dated Progress Log session with the probe summary and the
       JSON path.
   - `modular-plaits-oscillators.md` and `modular-plaits-resonant-noise.md`:
     replace the same pending phrases with the PLV-001 result reference.
   - `modular-plaits-wave-replacements.md` and
     `modular-plaits-cleared-replacements.md`: add one dated Progress Log
     note that their positions (5, 6, 13, 14; 2, 3, 4, 15) are wired
     through the voice layer, with the label and probe result.
   - `modular-audio-handoff.md`:
     - priority 4 cell: add the PLV-001 progress (voice layer landed and
       wired into all 24 templates, off by default; label and probe
       result; engine-specific parity and resource-safe replacements
       remain);
     - priority 7 cell: MOD-004 stereo and multi-output edges done
       2026-09-30 (commits 85a300a, b6fa077, 0a15742, 2cd547a); live
       input/device routes, rate/block and real-time bounds, and final
       coverage claims remain;
     - `**Last Updated**` becomes 2026-09-30.
   - Design: add `### Implementation status (2026-09-30)` at the end of
     the PLV-001 section, before `## References`. List the landed plans
     (PLV-10/12/20/21 in `f5e623b`; PLV-30/31/40 in this session), the
     golden result (24 graph lines, 0 render lines), the mem result (+32
     per wired template), the probe outcome and the label.
   - `impl-plans/README.md`: update the Status and date of the PLV rows
     and of the handoff and Plaits rows.
   - Set the header Status of plv-12 and plv-21 to `Completed`. plv-10
     and plv-20 already read Completed. PLV-30 and PLV-31 set their own
     Status lines when they finish.

## Verification Commands

Logs go in `tmp/plv/s209/PLV-40/`, each ending with `exit=<n>`.

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the pass count. If it fails, rerun with `--no-fail-fast` and list every failure.
7. `REF=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack sh -c 'test -d "$REF" && VACTR_MI_REFERENCE="$REF" mise run compare-plaits-voice > tmp/plv/s209/PLV-40/7-compare.json 2> tmp/plv/s209/PLV-40/7-compare.log'; echo "exit=$?" >> tmp/plv/s209/PLV-40/7-compare.log` -> exit 0 with valid JSON (`python3 -m json.tool tmp/plv/s209/PLV-40/7-compare.json`), or BLOCKED with the reason
8. `REF=<same> sh -c 'test -d "$REF" && VACTR_MI_REFERENCE="$REF" mise run audit-upstream' > tmp/plv/s209/PLV-40/8-audit.log 2>&1` -> exit 0, reporting 0 errors (or BLOCKED if `$REF` is absent)
9. `{ git diff --name-only f5e623b -- '*.rs'; git ls-files -o --exclude-standard -- '*.rs'; } | sort -u | while read -r f; do [ -f "$f" ] && wc -l "$f"; done | awk '$1 >= 1000 {bad=1} END {exit bad}'` -> exit 0
10. `git diff --check` -> exit 0
11. `test -z "$(git status --short | grep -vE '^.. (src/|examples/|verification/|design-docs/|impl-plans/|THIRD_PARTY_NOTICES.md|mise.toml)')"` -> exit 0 (tmp/ stays ignored)
12. `test "$(git diff -U0 f5e623b -- src/host/tests/e2e/templates/golden_digests.txt | grep -cE '^[-+]render')" = 0 && test "$(git diff -U0 f5e623b -- src/host/tests/e2e/templates/golden_digests.txt | grep -cE '^[-+]graph')" = 48` -> exit 0

## Completion Criteria

- [x] `7-compare.json` is recorded (or BLOCKED is recorded), and the manifest label matches `voice_layer_label_eligible`. Nothing is SourcePort.
- [x] The audit reports 0 errors, and `voice.{h,cc}`, `envelope.h` and `fx/low_pass_gate.h` stay `source`.
- [x] The notices describe the opt-in, default-off wiring and the measured label, and no stale "trigger/LPG not included" clause remains.
- [x] The engines plan and its four subplans, the handoff (priorities 4 and 7), the README and the design status carry the evidence.
- [x] Commands 1-12 pass with complete logs, or are recorded as BLOCKED with the reason.

## Execution Protocol

1. Do not change git state. The orchestrator commits afterwards, using
   the numbered 1-6 section message with no Co-Authored-By or tool
   attribution.
2. Record the pre- and post-edit `shasum -a 256` of every writePath in
   `tmp/plv/s209/PLV-40/hashes.txt`.
3. Re-read each file just before editing it. Never edit outside
   writePaths.
4. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log`. A missing
   or truncated log is not a pass.

## Progress Log

### Session: 2026-09-30, Step 6 implementation

**Tasks completed**: Fresh voice comparison, measured manifest label, upstream
provenance audit, notice and Plaits-plan evidence, handoff and design status,
verification commands 1–12, and progress tracking.

The final comparison at `tmp/plv/s209/PLV-40/7-compare.json` reports
`voice_layer_label_eligible: true`; scenarios A–E meet the thresholds. The
manifest and tests record `SourceStage` for the shared layer; engine coverage
is unchanged and nothing is labeled `SourcePort`. The audit reports 0 errors
in `tmp/plv/s209/PLV-40/8-audit.log`, and the required four upstream headers
remain `use = "source"`.

Commands 1–6 and 9–12 exited 0. Full nextest ran 1,623 tests: 1,623 passed,
2 skipped (`tmp/plv/s209/PLV-40/6-nextest.log`). Three preliminary `mise run audit-upstream` attempts are preserved in
`8-audit.log`; the task selected shell Python 3.9 (without `tomllib`) and
its nested reference-variable handoff did not work. The final direct audit run
used mise Python 3.12 and passed with 0 errors. Compare and log JSON
validation exited 0.

The implementation criteria are complete. Formal integration/adversarial
review and Git finalization are owned by downstream workflow steps.

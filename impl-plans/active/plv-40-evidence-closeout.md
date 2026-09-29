# PLV-40: Evidence, Labels, Provenance and Plan Closeout (serial)

**Status**: Ready
**Plan ID**: PLV-40 (wave 4; serial finalization)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Provenance; Comparison probe; Fidelity labels; Rollout)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`; handoff `impl-plans/active/modular-audio-handoff.md` priority 4
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan runs after all code lands. It does the following:

1. runs the comparison probe (PLV-21) against the pinned checkout;
2. raises the manifest `voice_layer` label only if the probe meets every
   threshold;
3. makes the upstream inventory and notices truthful for every newly
   named or translated file;
4. records the results in the Plaits plans and the handoff, including
   marking MOD-004 stereo edges done under priority 7;
5. runs the full verification suite.

It is the only plan that edits shared indexes (README, handoff, the
parent plans, the inventory and the notices).

## Non-goals

- No DSP, kernel, template or lifetime change. If verification fails
  because of an earlier PLV defect, record it as a blocker with file,
  cause and evidence. The orchestrator review step repairs it serially.
- No `SourcePort` label and no change to any engine `coverage` value.
- Do not archive plans to `impl-plans/completed/`: the parent Plaits
  plan remains active.
- No edits to `golden_digests.txt`.

## Dependencies

- **dependsOn**: PLV-21, PLV-30, PLV-31 (and, transitively, PLV-10, PLV-12 and PLV-20)
- **Blocks**: none

## writePaths

- `src/dsp/ported/manifest.rs` (`voice_layer` default in `implemented()` only)
- `src/dsp/ported/tests.rs` (voice-layer label and summary expectations only)
- `verification/upstream_inventory.toml`
- `THIRD_PARTY_NOTICES.md`
- `design-docs/specs/design-mutable-audio.md` (a new `### Implementation status (2026-09-30)` subsection at the end of the PLV-001 section only)
- `impl-plans/active/modular-plaits-engines.md`
- `impl-plans/active/modular-plaits-oscillators.md`
- `impl-plans/active/modular-plaits-resonant-noise.md`
- `impl-plans/active/modular-audio-handoff.md`
- `impl-plans/README.md` (the PLV rows only)
- `impl-plans/active/plv-10-voice-layer-dsp.md`, `impl-plans/active/plv-12-manifest-registration.md`, `impl-plans/active/plv-20-gate-nodes.md`, `impl-plans/active/plv-21-voice-probe.md`, `impl-plans/active/plv-30-voice-lifetime.md`, `impl-plans/active/plv-31-template-wiring.md` (header `**Status**` line only)
- `impl-plans/active/plv-40-evidence-closeout.md`
- `tmp/plv/PLV-40/1-fmt.log`, `tmp/plv/PLV-40/2-check.log`, `tmp/plv/PLV-40/3-clippy.log`, `tmp/plv/PLV-40/4-wasm.log`, `tmp/plv/PLV-40/5-lsp.log`, `tmp/plv/PLV-40/6-nextest.log`, `tmp/plv/PLV-40/7-compare.json`, `tmp/plv/PLV-40/7-compare.log`, `tmp/plv/PLV-40/8-audit.log`, `tmp/plv/PLV-40/9-lines.log`, `tmp/plv/PLV-40/10-diffcheck.log`, `tmp/plv/PLV-40/11-status.log`, `tmp/plv/PLV-40/12-golden.log`, `tmp/plv/PLV-40/hashes.txt`

## sharedPaths (read-only)

- `verification/audit_upstream.py` (what the audit checks: named paths, notices, include closure, byte copies, numeric windows)
- `verification/compare_plaits_voice.py` (JSON fields `scenarios`, `meets_threshold`, `voice_layer_label_eligible`)
- The `THIRD_PARTY_NOTICES.md` section "Plaits position 12 additive MIT source-stage translation" (format to imitate)
- `src/dsp/ugen/voice_layer.rs`, `verification/plaits_voice_reference.cc`, `design-docs/specs/design-mutable-audio.md` (every upstream path they name)

## Steps

1. **Probe**: with
   `REF=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack`,
   run `test -d "$REF" && VACTR_MI_REFERENCE="$REF" mise run compare-plaits-voice > tmp/plv/PLV-40/7-compare.json 2> tmp/plv/PLV-40/7-compare.log`.
   If `$REF` is absent, the step is BLOCKED and the labels stay Pending.
2. **Label**: only if `voice_layer_label_eligible` is `true`, change the
   `voice_layer` value set in `implemented()` from `Pending` to
   `SourceStage`, and update `src/dsp/ported/tests.rs` to expect 24
   `SourceStage` rows and the summary suffix
   ` Voice layer: 0 pending, 24 source-stage.` Otherwise change nothing
   and record every failing scenario with its metrics.
3. **Inventory** (`verification/upstream_inventory.toml`, keeping the
   file's group order and entry format):
   - `plaits/dsp/voice.cc`: set `use = "source"` and delete its `reason`.
   - Add `source` entries (license read from each pinned header, kind
     `code`): `plaits/dsp/voice.h`, `plaits/dsp/envelope.h`,
     `plaits/dsp/fx/low_pass_gate.h`, `plaits/dsp/engine/engine.h`,
     `plaits/dsp/dsp.h` and `stmlib/dsp/limiter.h`.
   - For every other upstream path that the new or changed files name but
     that is not translated, add `use = "excluded"` with a precise reason:
     `plaits/user_data.h` and `stmlib/system/flash_programming.h` (named
     in the design; firmware flash storage, not translated) and
     `stmlib/utils/buffer_allocator.h` (probe harness only). Follow the
     existing `stmlib/dsp/units.cc` reason style.
   - Do not add a path that no Vactr file names.
   - Then run the audit (step 6, command 8) and fix inventory entries
     until it reports 0 errors. Never silence the audit by editing
     `audit_upstream.py`.
4. **Notices**: add one section to `THIRD_PARTY_NOTICES.md`, "Plaits
   voice-level trigger and low-pass gate translation". It names each
   `source` file above with its pinned copyright line (read the headers;
   the design records voice/envelope/engine as 2016, low_pass_gate as
   2014 and limiter as 2015), states which Vactr files contain the
   translation (`src/dsp/ugen/voice_layer.rs`,
   `src/dsp/ugen/vactrol_gate.rs`), states that no table, resource or
   `resources.cc` data is imported, and states that the local probe
   compiles upstream only from a separate checkout.
5. **Records**:
   - `modular-plaits-engines.md`:
     - add a subtask row `PLV-001` (deliverables: the shared voice
       layer, `vactrol-gate`/`decay-mod`, the 24 wired templates,
       `compare-plaits-voice`; status: the measured result);
     - in rows SYN-002A..SYN-003A2, replace each "LPG/full voice parity
       pending", "voice/LPG parity pending" or "source parity/LPG
       pending" phrase with "voice layer PLV-001: <label and result>",
       keeping any engine-parity remainder;
     - update the SYN-002A sentence about the separate voice-level
       parity requirement to point to PLV-001;
     - leave the "Trigger/LPG and source algorithm behavior have
       comparison evidence" criterion unchecked (engine algorithm parity
       remains), with a note that the voice-layer part is recorded;
     - add a dated Progress Log session with the probe summary and the
       JSON path.
   - `modular-plaits-oscillators.md` and `modular-plaits-resonant-noise.md`:
     replace the same pending phrases with the PLV-001 result reference.
   - `modular-audio-handoff.md`:
     - priority 4 cell: add the PLV-001 progress (voice layer landed;
       label and probe result; engine-specific parity and resource-safe
       replacements remain);
     - priority 7 cell: MOD-004 stereo and multi-output edges done
       2026-09-30 (commits 85a300a, b6fa077, 0a15742, 2cd547a); live
       input/device routes, rate/block and real-time bounds and final
       coverage claims remain;
     - `**Last Updated**` becomes 2026-09-30.
   - Design: add `### Implementation status (2026-09-30)` at the end of
     the PLV-001 section (before `## References`). It lists the landed
     plans, the golden result (24 graph lines, 0 render lines), the probe
     outcome and the label. The rules above it are not changed.
   - `impl-plans/README.md`: update the PLV rows' Status and date.
     Update the PLV-10..31 header Status lines to
     `Completed (uncommitted; orchestrator commits)`.
6. **Verification** (below).

## Verification Commands (each ending with `exit=<n>` in its log)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the pass count
7. Step 1 (probe) -> exit 0 with valid JSON, or BLOCKED with the reason
8. `test -d "$REF" && VACTR_MI_REFERENCE="$REF" mise run audit-upstream > tmp/plv/PLV-40/8-audit.log 2>&1` -> exit 0, reporting 0 errors (or BLOCKED if `$REF` is absent)
9. `{ git diff --name-only 2cd547a -- '*.rs'; git ls-files -o --exclude-standard -- '*.rs'; } | sort -u | while read -r f; do [ -f "$f" ] && wc -l "$f"; done | awk '$1 >= 1000 {bad=1} END {exit bad}'` -> exit 0
10. `git diff --check` -> exit 0
11. `test -z "$(git status --short | grep -vE '^.. (src/|examples/|verification/|design-docs/|impl-plans/|THIRD_PARTY_NOTICES.md|mise.toml)')"` -> exit 0 (tmp/ stays ignored)
12. `test "$(git diff -U0 2cd547a -- src/host/tests/e2e/templates/golden_digests.txt | grep -cE '^[-+]render')" = 0` -> exit 0

## Completion Criteria

- [ ] The probe JSON is recorded (or BLOCKED is recorded), and the label matches `voice_layer_label_eligible`.
- [ ] The audit reports 0 errors, with `voice.cc` as `source` and every newly named path inventoried truthfully.
- [ ] The notices section exists and names every `source` file with its copyright.
- [ ] The engines, oscillators and resonant-noise plans, the handoff (priorities 4 and 7), the README and the design status carry the evidence.
- [ ] Commands 1-12 pass with logs, or are recorded as BLOCKED with the reason.

## Execution Protocol

Serial (no concurrent plans):

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. The orchestrator commits afterwards with the
   numbered 1-6 section message and no Co-Authored-By or tool
   attribution.
2. Record the pre- and post-edit `shasum -a 256` of every writePath in
   `tmp/plv/PLV-40/hashes.txt`.
3. Re-read each file just before editing it. Never edit outside
   writePaths.
4. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log`. A
   missing or truncated log is not a pass.

## Progress Log

(none yet)

# PLV-31: Wire the Voice Layer into the 24 Plaits Templates (transparent by default)

**Status**: Ready
**Plan ID**: PLV-31 (session 209, wave 2 of 3; serial, after PLV-30)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Controls and editor metadata; Event mapping; Template wiring and seed preservation; Real-time, capacity and invariance; `decay-mod`; Test strategy; Rollout item 2)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30 (session 209 rewrite; supersedes the session-205 version)

## Intent and Context

Every Plaits template gets three neutral controls (`lpg-mode`,
`lpg-decay`, `lpg-color`) and two `vactrol-gate` nodes: one on the aux
path before `aux-out`, and one as the new root. The `lpg-mode` default
is `:off`, which is an exact pass-through.

Because PLV-30 already landed the gate-elided seed ordinal, every seeded
kernel keeps its seed. So after this plan:

- every golden **render** line stays byte-identical;
- exactly the 24 Plaits **graph** lines change. Their reason: the design
  section "Template wiring and seed preservation".

The gates also add `2 * GATE_STATE_FLOATS` (32) floats of fixed memory
per template. That legitimately changes 14 pinned `mem_total` tests. The
root gate also becomes the last lowered node, which changes one
structural test. This plan owns all 15 of those files, with the reason
recorded here.

Session-205 lesson: the same wiring changed 8 render digests (seeds; now
fixed by PLV-30) and broke 14 unowned `mem_total` tests. Only the first
was seen, because nextest stops at the first failure. This plan fixes
that ownership.

## Non-goals

- No kernel, DSP, lifetime, seed-order, catalog, codec, controls,
  manifest, inventory or notice change. PLV-20, PLV-30 and PLV-40 own
  those.
- Do not touch non-Plaits templates, the Braids `macro-*` templates, or
  any golden `render` line.
- Do not pre-wire `decay-mod` into the 24 templates.
- Do not change the gate's memory layout to avoid the mem test updates.

## Dependencies

- **dependsOn**: PLV-30 (`Template::seed_ordinal`, the gate-elided seed
  order and the lifetime rules). Without it, 8 render lines change.
- **Blocks**: PLV-40

## writePaths

- `src/prelude/templates.vact` (the 24 Plaits templates and one comment line)
- `src/dsp/meta/templates.rs` (add `mod plaits;` and reference the 24 moved arrays)
- `src/dsp/meta/templates/plaits.rs` (new)
- `src/host/tests/e2e/templates/golden_digests.txt` (exactly the 24 Plaits `graph` lines)
- `src/host/tests/e2e/templates.rs` (one `mod voice_layer;` line)
- `src/host/tests/e2e/templates/voice_layer.rs` (new)
- `examples/voice-layer.vact` (new)
- mem_total expectations:
  - `src/host/tests/e2e/templates/analog_pair.rs`
  - `src/host/tests/e2e/templates/chip.rs`
  - `src/host/tests/e2e/templates/chord_pair.rs`
  - `src/host/tests/e2e/templates/grain_pair.rs`
  - `src/host/tests/e2e/templates/modal.rs`
  - `src/host/tests/e2e/templates/particle.rs`
  - `src/host/tests/e2e/templates/shape_pair.rs`
  - `src/host/tests/e2e/templates/six_op_original.rs`
  - `src/host/tests/e2e/templates/speech_original.rs`
  - `src/host/tests/e2e/templates/string_machine_pair.rs`
  - `src/host/tests/e2e/templates/string_voice.rs`
  - `src/host/tests/e2e/templates/table_terrain_pair.rs`
  - `src/host/tests/e2e/templates/terrain_pair.rs`
  - `src/host/tests/e2e/templates/voice_engines.rs`
- Last-node expectation: `src/types/tests/inst/templates.rs`
- Conditional (only if a test embeds prelude template text and fails because of the gates; never an old-form fixture or render comparison; record each line changed): `src/host/tests/e2e/templates/migrated_pairs.rs`, `src/host/tests/e2e/templates/select_output.rs`
- `impl-plans/active/plv-31-template-wiring.md` (Status line and Progress Log only)
- Evidence: `tmp/plv/s209/PLV-31/hashes.txt`, `tmp/plv/s209/PLV-31/golden-pre.txt`, `tmp/plv/s209/PLV-31/1-fmt.log`, `tmp/plv/s209/PLV-31/2-check.log`, `tmp/plv/s209/PLV-31/3-clippy.log`, `tmp/plv/s209/PLV-31/4-wasm.log`, `tmp/plv/s209/PLV-31/5-lsp.log`, `tmp/plv/s209/PLV-31/6-focused.log`, `tmp/plv/s209/PLV-31/7-nextest.log`, `tmp/plv/s209/PLV-31/8-lines.log`, `tmp/plv/s209/PLV-31/9-bless.log`, `tmp/plv/s209/PLV-31/10-golden-diff.log`, `tmp/plv/s209/PLV-31/11-gate-count.log`

## sharedPaths (read-only)

- `src/host/tests/e2e/templates/golden.rs`: `bless_golden_digests` (ignored; `VACTR_BLESS_GOLDEN=1`), `graph_record` (hashes the lowered `InstDef`) and `render_record`.
- `src/host/tests/e2e/templates/quad_stems.rs`: how an example `.vact` file is loaded.
- `src/host/tests/e2e/templates/digital_drum.rs`: patterning an enum keyword control and checking editor metadata.
- `src/dsp/ported/tests.rs`: the naming test must pass unchanged.
- `src/dsp/meta.rs`: `TEMPLATE_PARAMS` lookup and `row_meta` (labels, defaults and choices come from the control rows). Do not edit.
- `src/dsp/ugen/template.rs`: `Template::seed_ordinal`, `mem_total`, `nodes()` (from PLV-30).
- `src/dsp/ugen/vactrol_gate.rs`: `GATE_STATE_FLOATS`.
- `src/types/tests/inst.rs`: `kinds(def)` and the session `def(name)` helper.
- `tmp/plv-wave3-saved/wave3-tracked.patch`, `tmp/plv-wave3-saved/src/dsp/meta/templates/plaits.rs`, `tmp/plv-wave3-saved/src/host/tests/e2e/templates/voice_layer.rs`, `tmp/plv-wave3-saved/examples/voice-layer.vact`: session-205 reference. Its templates.vact hunks follow the edit rule. Do not apply its golden fixture or its plan edits.

## Template edit rule

Apply the rule to each row below; `N` is the slot.

| N | Template | N | Template | N | Template |
|---|---|---|---|---|---|
| 0 | `filter-voice` | 8 | `analog-pair-voice` | 16 | `swarm-voice` |
| 1 | `phase-pair-voice` | 9 | `shape-voice` | 17 | `clock-noise-voice` |
| 2 | `six-bank-a-voice` | 10 | `fm-pair-voice` | 18 | `particle-voice` |
| 3 | `six-bank-b-voice` | 11 | `grain-pair-voice` | 19 | `string-voice` |
| 4 | `six-bank-c-voice` | 12 | `spectrum-voice` | 20 | `modal-voice` |
| 5 | `terrain-voice` | 13 | `wave-grid-voice` | 21 | `dual-kick-voice` |
| 6 | `string-machine-voice` | 14 | `chord-layer-voice` | 22 | `dual-snare-voice` |
| 7 | `chip-voice` | 15 | `speech-voice` | 23 | `dual-hat-voice` |

1. Header: append ` lpg-mode: keyword = :off lpg-decay: float = 0.5 lpg-color: float = 0.5` before the trailing `:`.
2. Aux path: replace the body's final `> * amp > aux-out}` with
   `> * amp > vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: N lane: 1 > aux-out}`.
3. Root: add one continuation line, indented with two tabs like the
   existing `> + {...}` lines:
   `> vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: N lane: 0`.
4. For `chip-voice` only, add `clocked: chip-clocked` to both gates, before `lane:`.
5. Change nothing else: no reordering, renaming or whitespace change on
   existing lines, and no extra blank lines. Session 205 added blank
   lines; do not.
6. Add one comment line directly above `filter-voice`'s existing comment:
   `# Plaits templates end in the voice gate; lpg-mode :off is an exact pass-through.`
   Do not write the literal `vactrol-gate` in the comment, so the count
   check stays exact.

Key points:
- Gates are appended after existing nodes in lowering order. The seed
  ordinal (PLV-30) is what keeps compiled seeds. Do not try to "fix"
  seeds by reordering template text.
- In a duplicate-form template, only the aux duplicate's `* amp` gets the
  lane-1 gate. The main path is gated by the root gate.

## Editor metadata

- Create `src/dsp/meta/templates/plaits.rs` with 24
  `pub(super) const <NAME>: &[&str]` arrays, named from the template in
  SCREAMING_SNAKE (for example `FM_PAIR_VOICE`).
- Each array is the template's current list with `"lpg-mode"`,
  `"lpg-decay"` and `"lpg-color"` inserted immediately before `"amp"`.
- In `src/dsp/meta/templates.rs`, add `mod plaits;` and replace the 24
  inline arrays with `plaits::<NAME>`. Keep the order of the
  `TEMPLATE_PARAMS` table unchanged.
- Both files must end below 1000 lines. `templates.rs` is 904 now, so the
  split is mandatory.
- Labels, defaults and `lpg-mode` choices come from the PLV-20 control
  rows through `row_meta`. Add no metadata code.

## Legitimate existing-test changes

Reason for the mem changes: two `vactrol-gate` nodes add
`2 * vactrol_gate::GATE_STATE_FLOATS` (32) fixed floats (design,
"Real-time, capacity and invariance").

In each mem file:
- change the `assert_eq!(template.mem_total, X)` expectation to
  `X + 2 * crate::dsp::ugen::vactrol_gate::GATE_STATE_FLOATS`, keeping `X`
  as written;
- move the adjacent budget-minus-one `voice_mem: X - 1` `MemExceeded`
  boundary to `X + 2 * GATE_STATE_FLOATS - 1`, so it still tests the exact
  boundary.

In `voice_engines.rs`:

| Template | Line | Old | New | Boundary line |
|---|---|---|---|---|
| spectrum | 270 | 96 | 96 + 2G | |
| clock-noise | 337 | 0 | 0 + 2G | |
| dual-kick | 415 | 48 | 48 + 2G | 419 |
| dual-snare | 524 | 48 | 48 + 2G | 528 |
| dual-hat | 633 | 32 | 32 + 2G | 637 |
| swarm | 738 | 224 | 224 + 2G | 742 |

Reword the clock-noise message "node state is inline and preallocated"
to say the kernel state is inline and only the two voice gates use
memory. Keep every other assertion in these files unchanged, including
the `edge.port == 4` / `Const(1.0)` check.

`src/types/tests/inst/templates.rs`, `templates_realize_at_session_start`:
- the 24 Plaits names move out of the `"add"` group into a new group
  expecting `"vactrol-gate"`;
- every non-Plaits name keeps `"add"` or `"mul"` as today.

Record each changed line (path:line, old, new) in the Progress Log.

## Golden digests (deterministic procedure)

1. `cp src/host/tests/e2e/templates/golden_digests.txt tmp/plv/s209/PLV-31/golden-pre.txt`
2. After command 2 passes, run
   `VACTR_BLESS_GOLDEN=1 CARGO_TERM_QUIET=true cargo test -q --lib bless_golden_digests -- --ignored > tmp/plv/s209/PLV-31/9-bless.log 2>&1; echo "exit=$?" >> tmp/plv/s209/PLV-31/9-bless.log`
3. `git diff -U0 -- src/host/tests/e2e/templates/golden_digests.txt > tmp/plv/s209/PLV-31/10-golden-diff.log`. It must contain:
   - exactly 24 `-graph` and 24 `+graph` lines, naming exactly the 24 templates above;
   - zero `-render` or `+render` lines;
   - no other changes.
4. If any render line or any other graph line differs, restore the file
   from `golden-pre.txt` and stop. Record a blocker with the differing
   names. It means seed order or the edit rule is broken. Never keep a
   changed render line, and never edit render lines by hand.

## Example `examples/voice-layer.vact`

It defines one neutral instrument, `ping-fm-voice`, showing opt-in use:
- `let p {fm-pair ...}` with timbre
  `{0.3 > + {decay-mod lpg-decay: lpg-decay amount: 0.6 target: 0}}`.
  Use the pipe form; session 205 found that the infix
  `{0.3 + {...}}` fails with "a float is not callable".
- Both gates as in the edit rule, slot 10.
- Header defaults `lpg-mode: keyword = :ping`, `lpg-decay: float = 0.4`,
  `lpg-color: float = 0.6`.
- A short pattern `s :ping-fm-voice > note [:a3] > once`.
- No upstream module names.

## Test Cases (`src/host/tests/e2e/templates/voice_layer.rs`)

- Off identity: for each of the 24 templates, render
  `s :<t> > note [:a3] > once` for 0.5 s with and without
  `> lpg-mode :off`. L and R are bitwise equal.
- Pre-wiring identity (compiled level). For `clock-noise-voice`,
  `dual-snare-voice`, `swarm-voice` and `speech-voice`, define a
  test-local `inst` with the pre-wiring text, taken from
  `git show f5e623b:src/prelude/templates.vact`, under a neutral name such
  as `pre-clock-noise`.
  - Default renders of the prelude template and the copy are bitwise
    equal for 0.5 s.
  - For every compiled kernel node (not Param, Const or `vactrol-gate`)
    of the copy's `Template` at compiled index `i`, the prelude
    `Template` has a node of the same kind with `seed_ordinal == i`.
  - Use `Template::from_inst` values. Comparing `InstDef` indices is the
    session-205 mistake.
- Structure: the lowered `fm-pair-voice` last node is `vactrol-gate`, and
  a `vactrol-gate` feeds `aux-out`. `chip-voice` gates read `chip-clocked`.
- Ping shape: `fm-pair-voice` with `lpg-mode :ping` differs from off.
  Measure from the event onset: RMS over [0, 1 ms) is below RMS over
  [2 ms, 10 ms). With `lpg-decay 0.2`, RMS over [60 ms, 100 ms) is below
  the same window with `lpg-decay 0.8`.
- Level: `clock-noise-voice` with `lpg-mode :level` at velocity 0.5 has
  lower RMS over [20 ms, 80 ms) than at velocity 1.
- Enveloped bypass: `dual-kick-voice` with `lpg-mode :ping` gives
  `left == 0.8 * off_left` within `1e-6 + 1e-6*|off|` for every sample.
- Chip clocked: with `chip-clocked 1` for both renders, `lpg-mode :ping`
  gives `left == 0.5 * off_left`, same tolerance. With `chip-clocked 0`,
  ping differs from off.
- All 24 templates in `ping` and in `level` render finite, non-silent L
  and R.
- Editor: the `fm-pair-voice` editor declaration lists `lpg-mode`
  (choices `off`, `ping`, `level`; default 0), `lpg-decay` (0.5) and
  `lpg-color` (0.5).
- Example: `examples/voice-layer.vact` loads, and `ping-fm-voice` renders
  finite, non-silent audio.

## Verification Commands

Logs go in `tmp/plv/s209/PLV-31/`. Run each command as
`cmd > log 2>&1; echo "exit=$?" >> log`.

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (rustfmt only your own `.rs` files)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --no-fail-fast voice_layer golden migrated_pairs select_output ported templates_realize analog_pair chip chord_pair grain_pair modal particle shape_pair six_op_original speech_original string_machine_pair string_voice table_terrain_pair terrain_pair voice_engines` -> exit 0
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the pass count. If it fails, rerun with `--no-fail-fast` into `7-nextest-nff.log` and list every failure before any repair.
8. `wc -l src/dsp/meta/templates.rs src/dsp/meta/templates/plaits.rs src/host/tests/e2e/templates/voice_layer.rs src/host/tests/e2e/templates.rs src/host/tests/e2e/templates/voice_engines.rs src/types/tests/inst/templates.rs` -> every file under 1000 lines
9. The bless run (golden procedure step 2) -> exit 0
10. The golden diff check (golden procedure step 3) -> 24 `-graph`, 24 `+graph`, 0 render lines
11. `test "$(grep -o 'vactrol-gate' src/prelude/templates.vact | wc -l | tr -d ' ')" = 48` -> exit 0

## Completion Criteria

- [ ] All 24 templates follow the edit rule. Command 11 passes, and chip-voice's gates carry `clocked: chip-clocked`.
- [ ] The editor lists carry the three controls. `templates.rs` and `plaits.rs` are below 1000 lines.
- [ ] `golden_digests.txt` differs from `golden-pre.txt` in exactly the 24 Plaits graph lines, with 0 render lines.
- [ ] The 14 mem files and `src/types/tests/inst/templates.rs` are updated as specified, and each changed line is logged with its reason.
- [ ] Every listed test passes, and commands 1-11 pass with complete logs.
- [ ] The header `**Status**` is set to `Completed` and the Progress Log records the session.

## Execution Protocol

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff`, `git show` and `git status`
   are allowed.
2. Before the first edit, record `shasum -a 256` and a one-line intent
   per existing writePath in `tmp/plv/s209/PLV-31/hashes.txt`.
3. Re-read each file just before editing it, and record drift and the
   post-edit hash.
4. Never edit outside writePaths. If a failing test lives in an unlisted
   file, record a blocker with file, test, cause and evidence for the
   orchestrator's serial repair. Do not edit it.
5. Record every command, its exit status and its log path in the Progress
   Log. A missing or truncated log is not a pass.

## Progress Log

(none yet)

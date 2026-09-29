# PLV-31: Wire the Voice Layer into the 24 Plaits Templates

**Status**: Ready
**Plan ID**: PLV-31 (wave 3; parallel with PLV-30)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Controls and editor metadata; Event mapping; Template wiring and seed preservation; `decay-mod`; Test strategy)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

Every Plaits template gets the three neutral controls (`lpg-mode`,
`lpg-decay`, `lpg-color`) and two `vactrol-gate` nodes: one on the aux
path before `aux-out`, and one as the new root. The placement appends
all new nodes after every existing node in post-order. Kernel node
indices, and therefore per-node seeds (`voice.seed + node index`),
cannot change. `lpg-mode :off` is an exact copy, so every golden
**render** line stays identical. Only the 24 **graph** lines change.

## Non-goals

- No kernel, controls, catalog, codec, lifetime, manifest, inventory or
  notice change (PLV-20, PLV-30, PLV-12 and PLV-40 own those).
- Do not touch non-Plaits templates, Braids `macro-*` templates, or any
  golden `render` line.
- Do not pre-wire `decay-mod` into the 24 templates (design decision D7).

## Dependencies

- **dependsOn**: PLV-20 (UGen names, ports, control rows)
- **Blocks**: PLV-40

## writePaths

- `src/prelude/templates.vact` (the 24 Plaits templates only)
- `src/dsp/meta/templates.rs` (add three names to the 24 Plaits entries, and move those 24 lists into the new submodule)
- `src/dsp/meta/templates/plaits.rs` (new; the 24 Plaits parameter-name arrays as `pub(super) const`)
- `src/host/tests/e2e/templates/golden_digests.txt` (exactly the 24 Plaits `graph` lines)
- `src/host/tests/e2e/templates/voice_layer.rs` (new)
- `src/host/tests/e2e/templates.rs` (one `mod voice_layer;` line)
- `src/host/tests/e2e/templates/select_output.rs` and `src/host/tests/e2e/templates/migrated_pairs.rs`: only if a test embeds the prelude template text or graph shape and fails because of the added gates. Update only that expectation, never an old-form fixture or a render comparison, and record each line changed.
- `examples/voice-layer.vact` (new)
- `impl-plans/active/plv-31-template-wiring.md` (Progress Log only)
- `tmp/plv/PLV-31/hashes.txt`, `tmp/plv/PLV-31/golden-pre.txt`, `tmp/plv/PLV-31/1-fmt.log`, `tmp/plv/PLV-31/2-check.log`, `tmp/plv/PLV-31/3-clippy.log`, `tmp/plv/PLV-31/4-wasm.log`, `tmp/plv/PLV-31/5-lsp.log`, `tmp/plv/PLV-31/6-focused.log`, `tmp/plv/PLV-31/7-nextest.log`, `tmp/plv/PLV-31/8-lines.log`, `tmp/plv/PLV-31/9-bless.log`, `tmp/plv/PLV-31/10-golden-diff.log`

## sharedPaths (read-only)

- `src/host/tests/e2e/templates/golden.rs` (`bless_golden_digests`, `graph_record`, how an `InstDef` is obtained per template)
- `src/host/tests/e2e/templates/quad_stems.rs` (loading an example `.vact` file)
- `src/host/tests/e2e/templates/digital_drum.rs` (patterning an enum keyword control and checking editor metadata)
- `src/dsp/ported/tests.rs` (the naming test must pass unchanged)
- `src/dsp/meta.rs` (`TEMPLATE_PARAMS` lookup, `template_meta`, `row_meta`; do not edit)

## Template edit rule (apply to each row; `N` is the slot)

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

1. Header: append
   ` lpg-mode: keyword = :off lpg-decay: float = 0.5 lpg-color: float = 0.5`
   before the trailing `:`.
2. Aux path: replace the final `> * amp > aux-out}` of the body with
   `> * amp > vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: N lane: 1 > aux-out}`.
3. Root: add a continuation line, indented with two tabs like the
   existing `> + {...}` lines:
   `> vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: N lane: 0`.
4. For `chip-voice` only, add `clocked: chip-clocked` to both gates.
5. Do not change anything else in the body: no reordering, renaming or
   whitespace change on existing lines.

Pinned example (the migrated form; legacy duplicate forms get the same
three edits):

```
	{p :main} > * amp > + {{p :aux} > * amp > vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: 10 lane: 1 > aux-out}
		> vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: 10 lane: 0
```

Add one comment line above `filter-voice`:
`# Plaits templates end in vactrol-gate; lpg-mode :off is an exact pass-through.`

## Editor metadata

- In `TEMPLATE_PARAMS`, add `"lpg-mode"`, `"lpg-decay"` and `"lpg-color"`
  immediately before `"amp"` in each of the 24 entries. Labels, defaults
  and choices come from the control rows through `row_meta`; no other
  metadata code changes.
- Move the 24 Plaits arrays into `src/dsp/meta/templates/plaits.rs` as
  `pub(super) const` items (for example, `FM_PAIR_VOICE: &[&str]`), and
  reference them from the unchanged `TEMPLATE_PARAMS` table order. Add
  `mod plaits;` in `templates.rs`. Both files must stay below 1000 lines;
  `templates.rs` is 904 lines now, so the split is mandatory.

## Golden digests (deterministic procedure)

1. Copy `golden_digests.txt` to `tmp/plv/PLV-31/golden-pre.txt`.
2. Run
   `VACTR_BLESS_GOLDEN=1 CARGO_TERM_QUIET=true cargo test -q --lib bless_golden_digests -- --ignored > tmp/plv/PLV-31/9-bless.log 2>&1; echo "exit=$?" >> tmp/plv/PLV-31/9-bless.log`.
3. Build the final file from `golden-pre.txt`, replacing only the 24
   `graph <plaits-template> baseline ...` lines with their blessed
   values. If the blessed output also differs in any `render` line or in
   any other `graph` line, do NOT keep that difference: record it as a
   blocker, because it means the edit rule or a concurrent change broke
   render equivalence or seeds.
4. `git diff -U0 -- src/host/tests/e2e/templates/golden_digests.txt > tmp/plv/PLV-31/10-golden-diff.log`.
   It must contain exactly 24 `-graph` and 24 `+graph` lines, naming
   exactly the templates above, and zero `render` lines.

## Example `examples/voice-layer.vact`

It defines one neutral instrument, `ping-fm-voice`, that shows opt-in
use:
- `let p {fm-pair ...}` with
  `timbre: {0.3 + {decay-mod lpg-decay: lpg-decay amount: 0.6 target: 0}}`;
- both gates as in the edit rule, with header defaults
  `lpg-mode: keyword = :ping`, `lpg-decay: float = 0.4`,
  `lpg-color: float = 0.6`;
- a short pattern, following `examples/quad-stems.vact`.

It must use no upstream module names.

## Test Cases (`src/host/tests/e2e/templates/voice_layer.rs`)

- Off identity: for each of the 24 templates, render `s :<t> > note [:a3] > once` for 0.5 s, with and without `> lpg-mode :off`. Left and right are bitwise equal.
- Seed preservation: for `clock-noise-voice` and `dual-snare-voice`, lower the prelude template and a test-local copy of its pre-PLV text (an inline `inst` with another neutral name). The kernel nodes (`ClockNoisePair`, `SnarePair`) have identical indices in both `InstDef`s.
- Root structure: the lowered `fm-pair-voice` sink (last node) is `VactrolGate`, and a `VactrolGate` feeds `AuxOut`.
- Ping shape (all within the gate): `fm-pair-voice` with `lpg-mode :ping` differs from off. Left RMS over [0, 1 ms) is below RMS over [2 ms, 10 ms). With `lpg-decay 0.2`, RMS over [60 ms, 100 ms) is below that with `lpg-decay 0.8`.
- Level: `clock-noise-voice` with `lpg-mode :level` at velocity 0.5 has lower RMS over [20 ms, 80 ms) than at velocity 1.
- Enveloped bypass: `dual-kick-voice` with `lpg-mode :ping` gives left `== 0.8 * off-left` within `1e-6 + 1e-6 * |off|` for every sample.
- Chip clocked: `chip-voice` with `lpg-mode :ping` and `chip-clocked 1` gives left `== 0.5 * off-left` (the same tolerance). With `chip-clocked 0` it differs.
- All 24 templates in `ping` and in `level` render finite, non-silent left and right channels.
- Editor metadata: the editor declaration for `fm-pair-voice` lists `lpg-mode` (choices `off`, `ping`, `level`; default 0), `lpg-decay` (0.5) and `lpg-color` (0.5).
- Example: `examples/voice-layer.vact` loads, and `ping-fm-voice` renders finite, non-silent audio.

## Verification Commands (logs in `tmp/plv/PLV-31/`, each ending with `exit=<n>`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (fix only your `.rs` files with `rustfmt --edition 2021`)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run voice_layer golden migrated_pairs select_output ported` -> exit 0
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the count
8. `wc -l src/dsp/meta/templates.rs src/dsp/meta/templates/plaits.rs src/host/tests/e2e/templates/voice_layer.rs src/host/tests/e2e/templates.rs` -> each below 1000
9. The bless run (procedure step 2) -> exit 0
10. The golden diff check (procedure step 4) -> exactly 24 removed and 24 added `graph` lines, and 0 `render` lines

## Completion Criteria

- [ ] All 24 templates follow the edit rule (`grep -c "vactrol-gate" src/prelude/templates.vact` == 48).
- [ ] The editor lists carry the three controls; `templates.rs` and `plaits.rs` are below 1000 lines.
- [ ] `golden_digests.txt` changes in exactly the 24 Plaits graph lines; no render line changes.
- [ ] Every listed test passes; commands 1-10 pass with logs.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record `shasum -a 256` of each existing
   writePath and a one-line intent per file in `tmp/plv/PLV-31/hashes.txt`
   and in the Progress Log.
3. Re-read each file just before editing it. If it changed in a way you
   did not make, re-read it and re-apply only your own change, and record
   the drift.
4. Never edit outside writePaths. PLV-30 runs at the same time and owns
   `src/dsp/voice.rs`, `src/dsp/voice/lifetime.rs` and
   `src/dsp/ugen/template.rs`. If cargo or a render test fails only
   because of its in-progress files, wait about 60 s and retry, up to 10
   times, then record a blocker and stop. Run the bless step only after
   `cargo check -q --all-targets` passes.
5. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log`. Record
   post-edit hashes, exit statuses and log paths in the Progress Log. A
   missing or truncated log is not a pass.

## Progress Log

(none yet)

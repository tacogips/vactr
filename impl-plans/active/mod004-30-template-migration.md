# MOD004-30: Migrate the Nine Simultaneous-output Templates

**Status**: Ready
**Plan ID**: MOD004-30 (session-194 wave 1, runs alone)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Migration eligibility; list of nine; exclusion table; Implementation status)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004E)
**Baseline**: `b6fa077` (MOD004-00/10/11/12 committed in `85a300a`; MOD004-20/21/22 committed in `b6fa077`; all accepted dependencies)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29 (refined for session 194: baseline `b6fa077`, source-text origin, non-silence guard, checker test file)

## Intent and Context

Owner decision (2026-09-29): migrate only templates whose main and aux are
simultaneous outputs of one engine state, each with a render-equivalence
test. The design lists exactly nine. Each migrated template binds one
kernel node with `let`, routes output 0 (`:main`) to the main path and
output 1 (`:aux`) through the unchanged `aux-out` tap. L and R must be
bit-identical to the old two-node form, and the golden render digests must
not change.

What already exists at `b6fa077` (do not reimplement):
- Selection lowering: `src/host/tests/e2e/templates/select_output.rs:10`
  (`SEL_A`) proves the source form below parses, lowers to one node, and
  emits edges with `output` 0 and 1.
- Paired kernel path: when output 1 is consumed, the node runs its paired
  entry point (MOD004-12). `src/dsp/tests/dsp/voice_layout.rs:selected_va_filter_pair_matches_two_legacy_nodes_bitwise`
  already proves bitwise equality for a hand-built `va-filter` graph.
- Golden fixture and test: `src/host/tests/e2e/templates/golden.rs`
  (`golden_renders_match_pre_mod004_baseline`, `bless_golden_digests`)
  and `golden_digests.txt` (one `graph` line per template plus `render`
  lines).

Source form. The design writes `(p :main)` as notation only. In `.vact`
source `( )` is rejected by the lexer (`paren-form`). Write exactly:

```
	let p {<kernel args of today's main line, including the selector>}
	{p :main} > * amp > + {{p :aux} > * amp > aux-out}
```

Pre-migration text. `git diff --stat cf2ea37 b6fa077 -- src/prelude/templates.vact`
is empty, so the current (pre-edit) bodies equal the `cf2ea37` bodies.
Copy the old bodies from the working-tree `src/prelude/templates.vact`
before editing it (or with read-only `git show b6fa077:src/prelude/templates.vact`).

## Non-goals

- No other template changes. `phase-pair-voice`, `resonator-voice` and the
  other 25 excluded templates, plus `macro-filter-voice` and
  `macro-wave-grid-voice`, stay byte-for-byte unchanged.
- No runtime, codec, lowering, checker or kernel changes: no edits under
  `src/dsp/`, `src/vm/`, `src/compile/`, or non-test files under
  `src/types/`. If a migration needs one, stop and report.
- No edits to `src/dsp/ported/manifest.rs` or other `src/dsp/ported/*.rs`
  (they describe the upstream module), `src/ns/insts.rs` (template names
  are unchanged) or `src/dsp/meta.rs`.
- No re-blessing of render digests.
- No edits to other plans, the parent plan, the README or the design doc
  (MOD004-40 owns those).

## Dependencies

- **dependsOn**: none in this manifest (accepted committed dependencies: MOD004-00, -10, -11, -12, -20, -21, -22)
- **Blocks**: MOD004-40

## writePaths

- `src/prelude/templates.vact`: only the nine template bodies and the
  comment line directly above each
- `src/host/tests/e2e/templates/golden_digests.txt`: only the nine `graph`
  lines of the migrated templates
- `src/host/tests/e2e/templates/migrated_pairs.rs` (currently a one-line
  stub; fill it). If it would reach 1000 lines, split into
  `src/host/tests/e2e/templates/migrated_pairs/` submodules declared from
  `migrated_pairs.rs`.
- Structural-assertion-only updates, if and only if they fail because a
  migrated template now has one kernel node instead of two:
  `src/types/tests/inst/templates.rs`,
  `src/host/tests/e2e/templates/voice_engines.rs`, `analog_pair.rs`,
  `shape_pair.rs`, `chord_pair.rs`, `terrain_pair.rs`,
  `table_terrain_pair.rs`, `string_machine_pair.rs`, `stage_chain.rs`,
  `coverage.rs` (all under `src/host/tests/e2e/templates/`). Never weaken
  an audio assertion. List every such change with its reason in the
  Progress Log.
- This plan's Progress Log
- `tmp/mod004/MOD004-30/` (evidence; gitignored, stays untracked)

## Templates and Their Migrated Form

Each keeps its header line (name, every control and default) unchanged.
The bound node keeps today's main-line arguments verbatim, including the
selector, so output 0 is the old main and output 1 the old aux.

| Template | Kernel (`.vact` name) | Selector kept on the node | Current lines |
|---|---|---|---|
| `filter-voice` | `va-source` then `va-filter` | `mode: 0` | 103-105 |
| `fm-pair-voice` | `fm-pair` | `mode: 0` | 115-117 |
| `analog-pair-voice` | `analog-pair-core` | `mode: 0` | 176-178 |
| `chord-layer-voice` | `chord-layer-core` | `mode: 0` | 181-183 |
| `wave-grid-voice` | `wave-grid-core` | `mode: 0` | 244-246 |
| `terrain-voice` | `terrain-pair-core` | `mode: 0` | 249-251 |
| `string-machine-voice` | `string-machine-core` | `mode: 0` | 254-256 |
| `shape-voice` | `shape-pair-core` | `mode: 0` | 259-261 |
| `stage-chain-voice` | `stage-chain-core` | `chain-channel: 0` | 364-366 |

Line numbers are at `b6fa077`; re-read before editing, since edits above a
template shift them.

- `filter-voice`: the `let` binds the `va-filter` node fed by one
  `va-source`:
  `let p {va-source freq morph: morph > va-filter freq: freq timbre: timbre filter-harmonics: filter-harmonics mode: 0}`.
  The old form has two identical `va-source` nodes. Merging them is part
  of this migration; `va_filter::source` (`src/dsp/ugen/va_filter.rs:22`)
  reads no seed, so the design treats it as eligible. The bitwise test
  decides: if main or aux differs, stop and report. Do not keep two
  sources.
- Update the comment line above each migrated template to also say one
  node provides both outputs. Keep the existing descriptive text.

## Pitfalls

- Before deleting an aux line, check that its arguments equal the main
  line's apart from the selector (`mode: 1` / `chain-channel: 1`). Stop if
  not.
- `* amp` stays on both branches; `aux-out` stays the aux route. Do not
  write `{p :aux} > aux-out > * amp` or drop the `+`.
- Never write `(p :main)` in `templates.vact` or in a test string.
- The prelude is realized through the checker at session start
  (`src/types/tests/inst/templates.rs:templates_realize_at_session_start`
  asserts `template_errors()` is empty). If the name `p` triggers any new
  diagnostic (for example `shadows-prelude`), rename the binding to
  `pair` consistently across all nine templates and record it.
- `templates_realize_at_session_start` expects the last node kind `add`
  for all nine; the migrated form keeps `+` as the root, so it should
  still pass. Do not change that list unless it fails, and then only with
  a recorded reason.
- Render digests for all templates must still match. If one differs, the
  migration is wrong. Never edit `render` lines.
- Fixture update: before editing `templates.vact`, copy
  `golden_digests.txt` to `tmp/mod004/MOD004-30/golden_digests.txt.orig`.
  After migrating, take the recomputed `graph` lines for the nine
  templates from the `golden_renders_match_pre_mod004_baseline` failure
  output, or run
  `VACTR_BLESS_GOLDEN=1 CARGO_TERM_QUIET=true cargo test -q --lib bless_golden_digests -- --ignored`
  once. Then `diff -U0` against the `.orig` must show exactly nine changed
  lines, each the `graph` line of one migrated template. If any `render`
  line or any other template's line differs, restore it by hand from the
  `.orig` copy (not with `git checkout`), stop, and report.
- The equivalence tests must not be vacuous: assert both channels are
  non-silent before comparing.

## Test Cases (`src/host/tests/e2e/templates/migrated_pairs.rs`)

Imitate `select_output.rs` (`E2e::new`, `e.eval`, `entry`) and existing
render tests that use `E2e::run_stereo_for` (`src/host/tests/e2e.rs:163`).

- For each of the nine templates, eval an inline `inst old-<name>` with
  the template's header controls and its pre-migration body (a `const`
  per template). Render `s :<name> > note [:a3] > once` and
  `s :old-<name> > note [:a3] > once` with `run_stereo_for(0.5)` in
  separate `E2e` instances:
  - both renders: `rms(L) > 1e-5` and `rms(R) > 1e-5`;
  - `L new == L old` and `R new == R old`, compared with `f32::to_bits`
    (main and aux independently).
  - Repeat with one non-default control per template, for example
    `> timbre 0.9`, or `> chain1-primary 0.7` for `stage-chain-voice`.
- For each migrated template, the realized `InstDef` has exactly one node
  of its kernel `UGenSpec` (for `filter-voice`, one `VaSource` and one
  `VaFilter`), and edges from that node with `output` 0 and 1.
- `phase-pair-voice` and `resonator-voice` still have two kernel nodes,
  and every edge has `output == 0`.

## Verification Commands (logs in `tmp/mod004/MOD004-30/<n>-<name>.log`)

Each command writes full output to its log, and the Progress Log records
the command, exit status and log path. A missing or truncated log is not
a pass.

1. Before any edit: `mkdir -p tmp/mod004/MOD004-30 && cp src/host/tests/e2e/templates/golden_digests.txt tmp/mod004/MOD004-30/golden_digests.txt.orig && shasum -a 256 src/prelude/templates.vact src/host/tests/e2e/templates/golden_digests.txt src/host/tests/e2e/templates/migrated_pairs.rs` -> record the pre-hashes
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run migrated_pairs golden templates` -> after the nine `graph` lines are updated, exit 0
5. `diff -U0 tmp/mod004/MOD004-30/golden_digests.txt.orig src/host/tests/e2e/templates/golden_digests.txt` -> exit 1 (files differ, expected), exactly nine `-`/`+` pairs, each a `graph` line of a migrated template; paste into the log
6. `git diff --stat -- src/prelude/templates.vact` and `git diff -- src/prelude/templates.vact` -> only the nine bodies and their comment lines
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0
8. `rustfmt --edition 2021 --check src/host/tests/e2e/templates/migrated_pairs.rs` (plus any other `.rs` you touched) -> exit 0 (format only your own `.rs` files)
9. `VACTR_MI_REFERENCE=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack mise run audit-upstream` -> errors []
10. `wc -l src/host/tests/e2e/templates/migrated_pairs.rs` plus any other touched `.rs` -> each below 1000

## Completion Criteria

- [ ] The nine templates are migrated to one `let`-bound node; `git diff` shows no other template changed.
- [ ] Main and aux of each migrated template are bit-identical to the old two-node form at the default and one non-default control, and both channels are non-silent.
- [ ] Render digests are unchanged; the fixture diff is exactly the nine `graph` lines.
- [ ] `phase-pair-voice` and `resonator-voice` still have two kernel nodes.
- [ ] Commands 1-10 pass, with exit statuses and log paths recorded.

## Execution Protocol

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff`, `git status` and `git show` are allowed.
2. Record pre-edit hashes and an intent snapshot (the nine names and
   their selector) in the Progress Log.
3. Re-read each file just before editing it; if its hash differs from the
   recorded pre-hash without your edit, stop and report drift.
4. Never edit outside writePaths.
5. Record post-hashes, exit statuses and log paths in this Progress Log
   only; set this plan's Status to `Completed` only when every criterion
   is checked.

## Progress Log

(empty)

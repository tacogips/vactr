# MOD004-30: Migrate the Nine Simultaneous-output Templates

**Status**: Ready
**Plan ID**: MOD004-30 (wave 3, runs alone)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Migration eligibility; list of nine; exclusion table)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004E)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29

## Intent and Context

Owner decision (2026-09-29): migrate only templates whose main and aux are
simultaneous outputs of one engine state, each with a render-equivalence
test. The design lists exactly nine. Each migrated template uses one
kernel node, binds it with `let`, and routes `(p :main)` to the main path
and `(p :aux)` through the unchanged `aux-out` tap. Its L and R output
must be bit-identical to the old two-node form, and the golden render
digests must not change.

## Non-goals

- No other template changes. In particular `phase-pair-voice`,
  `resonator-voice` and the other 25 excluded templates stay byte-for-byte
  unchanged.
- No runtime, codec or lowering changes. If a migration needs one, stop
  and report.
- No re-blessing of render digests.

## Dependencies

- **dependsOn**: MOD004-20, MOD004-21, MOD004-22
- **Blocks**: MOD004-40

## writePaths

- `src/prelude/templates.vact`: only the nine template bodies listed below
- `src/host/tests/e2e/templates/golden_digests.txt`: only the `graph`
  lines of the nine templates
- `src/host/tests/e2e/templates/migrated_pairs.rs` (stub; fill it)
- Existing e2e template tests that break only because a migrated
  template's node or edge structure changed (possible files:
  `voice_engines.rs`, `analog_pair.rs`, `shape_pair.rs`, `chord_pair.rs`,
  `terrain_pair.rs`, `table_terrain_pair.rs`, `string_machine_pair.rs`,
  `stage_chain.rs` and `coverage.rs` under `src/host/tests/e2e/templates/`).
  Update a structural assertion only to reflect one shared node. Never
  weaken an audio assertion. List every change in the Progress Log.
- this plan's Progress Log

## Templates and Their Migrated Form

Each keeps its header line (the name and every control with its default)
unchanged.

| Template | Kernel (`.vact` name) | Selector kept on the node |
|---|---|---|
| `filter-voice` | `va-source` then `va-filter` | `mode: 0` |
| `fm-pair-voice` | `fm-pair` | `mode: 0` |
| `analog-pair-voice` | `analog-pair-core` | `mode: 0` |
| `chord-layer-voice` | `chord-layer-core` | `mode: 0` |
| `wave-grid-voice` | `wave-grid-core` | `mode: 0` |
| `terrain-voice` | `terrain-pair-core` | `mode: 0` |
| `string-machine-voice` | `string-machine-core` | `mode: 0` |
| `shape-voice` | `shape-pair-core` | `mode: 0` |
| `stage-chain-voice` | `stage-chain-core` | `chain-channel: 0` |

Body pattern, shown for `fm-pair-voice`. Follow `src/prelude/templates.vact`
tab indentation, keep every argument of today's main line, and drop the
duplicated aux line:

```
	let p (fm-pair freq fm-harmonics: fm-harmonics timbre: timbre morph: morph mode: 0)
	(p :main) > * amp > + {(p :aux) > * amp > aux-out}
```

- For `filter-voice`, the `let` binds the `va-filter` node, fed by a
  single `va-source`.
- Update the comment line above each migrated template to say one node
  provides both outputs.

## Pitfalls

- Copy today's main-line argument list exactly, including argument order.
  Argument order does not change node order, but copying exactly avoids
  accidental control changes.
- The old aux line's arguments equal the main line's apart from the
  selector. Verify this for each template before deleting the line, and
  stop if not.
- `* amp` must stay on both branches, and `aux-out` stays the aux route.
- The render digests for the nine templates must still match. If one
  differs, the migration is wrong. Do not edit `render` lines.
- Updating the fixture: before editing `templates.vact`, copy
  `src/host/tests/e2e/templates/golden_digests.txt` to
  `tmp/mod004/MOD004-30/golden_digests.txt.orig`. After migrating, run the
  golden test and take the recomputed `graph` lines for the nine migrated
  templates from its failure output; replace only those nine lines in the
  fixture. Alternatively, run
  `VACTR_BLESS_GOLDEN=1 CARGO_TERM_QUIET=true cargo test -q --lib bless_golden_digests -- --ignored`
  once and then check the diff. Either way,
  `diff -U0 tmp/mod004/MOD004-30/golden_digests.txt.orig src/host/tests/e2e/templates/golden_digests.txt`
  must then show exactly nine changed lines, each a `graph` line of one of
  the nine migrated templates (exit status 1 from `diff` is expected
  because the files differ; record it). If any `render` line or any other
  template's line differs, restore those lines by hand from the saved
  copy (not with `git checkout`), stop, and report the migration as
  wrong. "No re-blessing of render digests" means render lines must never
  change; the bless command may be used here only to obtain the new
  `graph` values, never to accept its output wholesale.

## Test Cases (`src/host/tests/e2e/templates/migrated_pairs.rs`)

- For each of the nine templates, define an inline `inst old-<name>` whose
  body is the template's **pre-migration** text copied from `cf2ea37`
  (`git show cf2ea37:src/prelude/templates.vact`, read-only). Render
  `s :<name> > note [:a3] > once` and `s :old-<name> > note [:a3] > once`
  with `run_stereo_for(0.5)`:
  - L new == L old, bitwise;
  - R new == R old, bitwise, which compares main and aux independently.
  - Repeat with one non-default control per template (for example
    `timbre 0.9`, or `chain1-primary 0.7` for `stage-chain-voice`).
- For each migrated template, the realized `InstDef` contains exactly one
  node of its kernel kind, with edges of `output` 0 and 1 from it.
- For `phase-pair-voice` and `resonator-voice`, the realized `InstDef`
  still contains two kernel nodes, which shows they were not migrated.

## Verification Commands (logs in `tmp/mod004/MOD004-30/`)

1. Before any edit: `mkdir -p tmp/mod004/MOD004-30 && cp src/host/tests/e2e/templates/golden_digests.txt tmp/mod004/MOD004-30/golden_digests.txt.orig`
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run migrated_pairs golden templates` -> take the recomputed `graph` lines for the nine migrated templates from the `golden` failure output (or bless once, per Pitfalls) and update only those nine fixture lines; then this command -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0
6. `diff -U0 tmp/mod004/MOD004-30/golden_digests.txt.orig src/host/tests/e2e/templates/golden_digests.txt` -> exactly nine changed lines, each a `graph` line of one of the nine migrated templates (exit status 1 from `diff` is expected because the files differ; record it; paste the output into the log); if any `render` line or any other template's line differs, restore it by hand from `tmp/mod004/MOD004-30/golden_digests.txt.orig`, stop, and report the migration as wrong
7. `git diff --stat -- src/prelude/templates.vact` -> only the nine bodies (paste the output into the log)
8. `VACTR_MI_REFERENCE=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack mise run audit-upstream` -> errors []

## Completion Criteria

- [ ] The nine templates are migrated, and the others are unchanged.
- [ ] Main and aux of each migrated template are bit-identical to the old two-node form (two control settings each).
- [ ] Golden render digests are unchanged, and only the nine `graph` lines changed.
- [ ] The full suite and the upstream audit pass, with logs recorded.

## Execution Protocol

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff`, `git status` and `git show` are
   allowed.
2. Record pre-edit hashes and an intent snapshot in the Progress Log.
3. Re-read each file just before editing it.
4. Never edit outside writePaths.
5. Format only your own `.rs` files.
6. Record post-hashes, exit statuses and log paths in this Progress Log
   only.

## Progress Log

(empty)

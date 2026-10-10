# FM1V-50: Examples for the FM Algorithms and the Six New Voices

**Status**: Ready
**Plan ID**: FM1V-50 (wave 4)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("Presets and examples")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The user needs playable examples for:

- the now-functional `fm` `algorithm` control;
- a patched `fm6-core` instrument;
- each new voice.

Every example must parse, be a formatter fixed point, render audibly for at
least two cycles, and keep the mix peak at or below 1.0.

The `fm6-core` example ships an **original, hand-written** 155-value patch
list. It is not derived from any factory, ROM or third-party patch.

## Non-goals

- No engine, kernel, registry or doc changes.
- No `.syx` files.
- Do not edit existing examples.

## Dependencies

- **dependsOn**: FM1V-40. Every template, `fm6-core` and the `fm` algorithm
  path must exist.
- **Blocks**: none. FM1V-51 runs in parallel.

## writePaths

- `examples/fm6-algorithms.vact`
- `examples/kalimba.vact`
- `examples/tonewheel-organ.vact`
- `examples/hurdy-gurdy.vact`
- `examples/experimental-oscillators.vact`
- `src/host/tests/e2e/templates/fm1_examples.rs`
- `impl-plans/active/fm1-voices-50-examples.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-50` (artifact root)

## sharedPaths

None.

## Read-only References

- `examples/bass-presets.vact` and `examples/wobble-techno.vact`: the
  header comment style, `use-bpm`, pipe style, `gain` levels and bus usage.
- `examples/resonator-lab.vact:12-20`: the `bus :name:` declaration with a
  tab-indented effect chain, routed with `> bus :name > d2`.
- `src/host/tests/e2e/templates/bass_examples.rs` and `bass_render.rs`:
  the `include_str!` example table, render, and the `check()` helper.
  Import `check` from `super::bass_render`; do not duplicate it.
- The FM1V-40 Progress Log: whether `patch:` accepts a let-bound list. If
  it does not, use a literal list in the body.

## File-level Changes

### `examples/fm6-algorithms.vact`

- A header comment. It explains that `algorithm 0` is the legacy
  two-operator `fm`, that 1..32 select the six-operator topologies, and
  that `ratio`, `index` and `velocity` shape the macro operators.
- `s :fm` patterns stepping `algorithm` through at least four values,
  including 0. Use the per-cycle alternation syntax used elsewhere in the
  examples, or one `d` slot per algorithm.
- `let epiano-patch = [ ... 155 ints ... ]`. It is original and annotated
  with comments, one line per operator (21 values) plus the global line. It
  is a two-carrier electric-piano-like patch on algorithm 5, with values in
  range.
- `inst my-epiano: fm6-core freq velocity: velocity patch: epiano-patch > * amp`,
  played as a short chord progression.
- A commented line showing user SysEx import:
  `# let bank = fm6-sysex "./my-bank.syx"` and
  `# inst from-bank: fm6-core freq patch: (bank 0) > * amp`.

### `examples/kalimba.vact`

A melody using `s :kalimba`, with:

- `kalimba-beat` variations;
- a phrase with `kalimba-buzz 0.4`;
- a `kalimba-hardness` contrast.

### `examples/tonewheel-organ.vact`

- Three registrations, written as drawbar controls:
  - 888000000: `drawbar1 8 drawbar2 8 drawbar3 8`, the rest 0;
  - 838000000;
  - 886000000.
- A phrase with `organ-perc 2`, `organ-perc-trigger` patterned `[1 0 0 0]`
  for single-trigger legato, and `organ-vibrato 6`.
- A `bus :leslie:` with `rotary speed: 6 depth: 0.7 mix: 1`. The organ
  routes to it with `> bus :leslie > d1`.
- `gate-length` set per phrase.

### `examples/hurdy-gurdy.vact`

The documented two-pattern recipe (design FV4):

- a drone pattern: `s :hurdy-gurdy > note [:g2] > gurdy-melody 0 > gate-length 32 > d1`;
- a melody pattern: drone levels at 0, `cut 1`, `gurdy-strokes 4`,
  `gurdy-buzz 0.7`.

A comment explains why drones are per voice.

### `examples/experimental-oscillators.vact`

Three short sections:

- a `vosim` formant sweep, with `vosim-formant` patterned or driven by a
  signal such as `{range sine 400 2000}`;
- `gendyn` with `gendyn-spread` 0 against 0.6;
- `scanned` with `scan-stiffness` 0.2 against 0.9.

Apply `gain` so the peak stays at or below 1.0.

### `src/host/tests/e2e/templates/fm1_examples.rs` (names contain `fm1_examples`)

- `const EXAMPLES: [(&str, u32, &str); 5]` with `(name, bpm, include_str!(..))`.
- `fm1_examples_render_audible_and_bounded`. For each example, `E2e::new()`
  evaluates the file with no errors and renders for at least two cycles at
  the file's tempo. Using `check()`: `finite`, `rms > 1e-3` and
  `peak <= 1.0`.
- `fm1_examples_use_the_new_voices`. Each file mentions its target names:
  - `:fm` and `algorithm` and `fm6-core`;
  - `:kalimba`;
  - `:tonewheel-organ` and `rotary`;
  - `:hurdy-gurdy`;
  - `:vosim`, `:gendyn` and `:scanned`.
- `fm1_examples_patch_is_in_range`. Extract the 155-value list from
  `fm6-algorithms.vact`, either by evaluating `epiano-patch` or by a simple
  bracket scan, and pass it to `Fm6Patch::from_flat` with `Ok`.

## Pitfalls

- **Formatter fixed point.**
  `CARGO_TERM_QUIET=true cargo run --quiet -- fmt --check examples/<file>.vact`
  must exit 0 for each new file. The corpus tests use `git ls-files`, so
  they only see the new files after commit. Run the direct check.
- **Use tabs** for block indentation, as the existing examples do.
- **Levels.** Lower `gain` rather than relying on clipping. `peak <= 1.0`
  is a hard check.
- **The patch must be visibly original.** Use round numbers and a comment
  saying it is hand-written for vactr. Never transcribe a known factory
  patch.

## Verification (logs under `tmp/fm1-voices/FM1V-50/`)

1. For each of the five files,
   `CARGO_TERM_QUIET=true cargo run --quiet -- fmt --check examples/<file>.vact`
   must exit 0.
2. `rustfmt --edition 2021 --check src/host/tests/e2e/templates/fm1_examples.rs`
   must exit 0.
3. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm1_examples|corpus|lsp::tests::analysis/)' > tmp/fm1-voices/FM1V-50/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 3 tests from this plan.
4. `git diff --stat` and `git status --short` show only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

FM1V-51 runs in parallel on documentation files only. You own only your
files.

## Done Criteria

- [ ] Five examples are added; each is formatted and renders audibly and
      within bounds.
- [ ] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

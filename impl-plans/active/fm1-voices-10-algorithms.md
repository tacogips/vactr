# FM1V-10: The 32 Six-Operator Algorithm Topologies and msfa Cross-check

**Status**: Ready
**Plan ID**: FM1V-10 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("Algorithm topologies", "msfa cross-check", "License boundary")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The `fm` template declares an `algorithm` control that does nothing today.
The extended engine (FM1V-20) needs a production table of the 32 classic
six-operator topologies. For each algorithm the table gives:

- which operators modulate which;
- which operators are carriers;
- the single feedback edge.

This plan authors that table from the published algorithm chart, which is
reproduced in the design. It proves the table by deriving the same graph
from msfa's bus-flag table (Apache-2.0) in a test. If the two disagree,
msfa is authoritative: fix the production table, and fix the design table in
the same change.

## Non-goals

- No rendering, EG or engine code (FM1V-11 and FM1V-20).
- No registry edits.
- Do not change `fm.rs`.
- Do not read GPL sources: Dexed beyond msfa, any FM-1 firmware, Grids,
  TB-3PO, 8W8/9W9 or CrispyZebra.

## Dependencies

- **dependsOn**: FM1V-00 (module tree, and the msfa checkout at
  `tmp/fm1-voices/msfa`)
- **Blocks**: FM1V-20

## writePaths

- `src/dsp/ugen/fm/algorithms.rs`
- `src/dsp/tests/dsp/fm_algorithms.rs`
- `design-docs/specs/design-fm1-voices.md`. Edit only the "Algorithm
  topologies" table, and only if msfa disagrees with it.
- `impl-plans/active/fm1-voices-10-algorithms.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-10` (artifact root)

## sharedPaths

None.

## Read-only References

- msfa at the SHA recorded in FM1V-00's Progress Log. The `fm_core.cc`
  `algorithms[32][6]` table and its flag constants: `OUT_BUS_ONE`,
  `OUT_BUS_TWO`, `OUT_BUS_ADD`, `IN_BUS_ONE`, `IN_BUS_TWO`, `FB_IN`,
  `FB_OUT`.
- msfa's `FmCore::render` processing loop. Note the operator order, how the
  output-bus flags decide overwrite or add, and what an output bus of 0
  means (a carrier writing to the output).
- `design-docs/specs/design-fm1-voices.md`, the algorithm table, for the
  operator-numbering convention.

## File-level Changes

### `src/dsp/ugen/fm/algorithms.rs`

Module doc:

- Says the table is authored from the published six-operator algorithm
  chart.
- Says it is cross-checked against msfa (Apache-2.0, revision `<sha>`) by
  `src/dsp/tests/dsp/fm_algorithms.rs`.
- Says operators are numbered 1..=6.

Contract. FM1V-20 depends on these signatures:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Algorithm {
    /// modulators[i] has bit j set when operator j+1 modulates operator i+1.
    pub modulators: [u8; 6],
    /// bit i set when operator i+1 is a carrier.
    pub carriers: u8,
    /// (source, destination) operator numbers 1..=6; source == destination is a self-loop.
    pub feedback: (u8, u8),
}
pub const ALGORITHMS: [Algorithm; 32];
pub const RENDER_ORDER: [u8; 6] = [6, 5, 4, 3, 2, 1];
pub fn algorithm(number: u8) -> &'static Algorithm // 1..=32; values outside clamp to 1..=32
pub const fn carrier_count(a: &Algorithm) -> u32   // carriers.count_ones()
```

`ALGORITHMS[k]` is algorithm `k + 1`. Write the table as readable literals,
one algorithm per line plus a comment that copies the design row.

### `src/dsp/tests/dsp/fm_algorithms.rs`

1. **Embed the msfa flags.** Add a `const MSFA_FLAGS: [[u8; 6]; 32]` that
   reproduces msfa's bus-flag bytes. Put an attribution comment directly
   above it:

   ```
   // Adapted from music-synthesizer-for-android (Apache-2.0),
   // fm_core.cc, revision <sha>. Copyright Google Inc.
   ```

   This is the test fixture only. Production code uses the authored table.
   Name the flag bits as local constants with the same values msfa uses.

2. **Derive the graph.** Write `fn derive(flags: &[u8; 6]) -> Algorithm`,
   which simulates msfa's bus semantics in msfa's processing order:
   - an operator whose output bus is 0 is a carrier;
   - writing to bus `b` without the add flag replaces `b`'s contributor
     set with `{op}`;
   - writing with the add flag appends `op`;
   - an operator's modulators are the contributors of its input bus at the
     moment it is processed;
   - the feedback edge goes from the operator carrying `FB_OUT` to the one
     carrying `FB_IN`.

   **Check the operator index mapping against msfa.** msfa processes its
   array index 0 first. Check in the source whether index 0 is operator 6 or
   operator 1, and document the mapping in a comment.

## Pitfalls

- **Feedback loops.**
  - Algorithms 4 and 6 have multi-operator feedback loops in the chart:
    `4->6` and `5->6` in the design table.
  - For those, check that msfa marks `FB_OUT` on the source and `FB_IN` on
    the destination. If msfa's encoding gives a different edge, msfa wins.
- **Carrier masks** must come from the flags, not from "operators without a
  modulator".
- **Do not invent a transitive closure.** "Modulates" means a direct edge.
- If msfa is recorded as unavailable in FM1V-00, do not substitute any other
  source. Mark this plan blocked in the Progress Log and stop. The design
  requires this.
- **License.** Keep the Apache-2.0 attribution comment next to the fixture.
  Never paste msfa code other than the flag bytes.

## Tests (input -> expected)

- `fm_algo_table_matches_msfa_derivation`: for k in 0..32,
  `derive(&MSFA_FLAGS[k]) == ALGORITHMS[k]`. On a mismatch the message names
  the algorithm number.
- `fm_algo_table_matches_design_examples`. Literal spot checks, which must
  still hold after any msfa correction:
  - algorithm 1: carriers {1,3}, op2 modulates op1, feedback (6,6);
  - algorithm 2: feedback (2,2);
  - algorithm 32: six carriers, no modulators, feedback (6,6);
  - algorithm 5: carriers {1,3,5}.
- `fm_algo_every_modulator_precedes_target_in_render_order`: for every
  algorithm and every edge `m -> c`, `m > c`, so `m` is rendered first in
  6..1 order.
- `fm_algo_each_algorithm_has_a_carrier_and_one_feedback_edge`.
- `fm_algo_number_clamps`: `algorithm(0)` is 1, `algorithm(40)` is 32.

## Verification (logs under `tmp/fm1-voices/FM1V-10/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/fm/algorithms.rs src/dsp/tests/dsp/fm_algorithms.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm_algo/)' > tmp/fm1-voices/FM1V-10/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 5 tests run and 0 failed.
3. `git diff --stat` shows only writePaths. If the design file changed, the
   diff touches only the algorithm table rows, and the Progress Log lists
   each corrected row with msfa evidence.

Record each result as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

- This plan owns its files alone. Fresh-read before each edit.
- Other wave-1 plans may be editing in the same tree. If a build fails in a
  file outside your writePaths, wait and re-run. Never edit that file.

## Done Criteria

- [ ] `ALGORITHMS` is implemented and equals the msfa derivation for all 32
      algorithms.
- [ ] Verification 1-3 pass and are recorded, including the msfa SHA used.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

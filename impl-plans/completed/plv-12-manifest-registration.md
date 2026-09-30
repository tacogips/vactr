# PLV-12: Plaits Manifest Voice Registration and Voice-Layer Label

**Status**: Completed
**Plan ID**: PLV-12 (wave 1; parallel with PLV-10)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Upstream behavior translated: Registration table; Modules and nodes; Fidelity labels)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

Upstream `Voice::Init` registers each engine with an
`(already_enveloped, out_gain, aux_gain)` tuple. The `vactrol-gate`
kernel (PLV-20) reads that tuple by Plaits position (`slot`), so the
constants must live once, in `src/dsp/ported/manifest.rs`. Each row also
gets a separate `voice_layer` coverage label. It starts at `Pending`, and
only PLV-40 may raise it, after the comparison evidence. The existing
engine `coverage` values must not change.

## Non-goals

- No change to any row's `coverage`, `resources`, `resource_flags`,
  `controls`, template name or output counts.
- No DSP, template or catalog change.
- No `SourcePort` anywhere.

## Dependencies

- **dependsOn**: none
- **Blocks**: PLV-20, PLV-40

## writePaths

- `src/dsp/ported/manifest.rs`
- `src/dsp/ported/mod.rs` (re-export additions in the existing `pub use manifest::{...}` only)
- `src/dsp/ported/tests.rs` (new registration tests; summary-string expectation updates only)
- `impl-plans/active/plv-12-manifest-registration.md` (Progress Log only)
- `tmp/plv/PLV-12/hashes.txt`, `tmp/plv/PLV-12/1-fmt.log`, `tmp/plv/PLV-12/2-check.log`, `tmp/plv/PLV-12/3-clippy.log`, `tmp/plv/PLV-12/4-wasm.log`, `tmp/plv/PLV-12/5-lsp.log`, `tmp/plv/PLV-12/6-focused.log`, `tmp/plv/PLV-12/7-nextest.log`, `tmp/plv/PLV-12/8-lines.log`

## sharedPaths (read-only)

- `examples/plaits_coverage.rs` (prints the summary; must still compile)
- `src/host/tests/e2e/templates/coverage.rs` (must still pass unchanged)

## Contract (pin exactly)

```rust
pub enum Enveloped { Never, Always, WhenClocked }        // Clone, Copy, Debug, PartialEq, Eq
pub struct VoiceRegistration { pub enveloped: Enveloped, pub out_gain: f32, pub aux_gain: f32 } // Clone, Copy, Debug, PartialEq
impl VoiceRegistration {
    pub fn gain(&self, lane: u8) -> f32;                  // 0 -> out_gain, otherwise aux_gain
    pub fn is_enveloped(&self, clocked: bool) -> bool;    // Always -> true, Never -> false, WhenClocked -> clocked
}
// New PortedAlgorithm fields: pub voice: VoiceRegistration, pub voice_layer: CoverageState
pub fn plaits_voice(slot: usize) -> Option<VoiceRegistration>; // None when slot >= 24
```

Re-export `Enveloped`, `VoiceRegistration` and `plaits_voice` from
`dsp::ported`.

The registration data comes from the design table, which is transcribed
from the pinned `Voice::Init`:

| Position | Enveloped | Out | Aux |
|---|---|---|---|
| 0 | Never | 1.0 | 1.0 |
| 1 | Never | 0.7 | 0.7 |
| 2, 3, 4 | Always | 1.0 | 1.0 |
| 5 | Never | 0.7 | 0.7 |
| 6 | Never | 0.8 | 0.8 |
| 7 | WhenClocked | 0.5 | 0.5 |
| 8 | Never | 0.8 | 0.8 |
| 9 | Never | 0.7 | 0.6 |
| 10 | Never | 0.6 | 0.6 |
| 11 | Never | 0.7 | 0.6 |
| 12 | Never | 0.8 | 0.8 |
| 13 | Never | 0.6 | 0.6 |
| 14 | Never | 0.8 | 0.8 |
| 15 | Never | -0.7 | 0.8 |
| 16 | Never | -3.0 | 1.0 |
| 17 | Never | -1.0 | -1.0 |
| 18 | Never | -2.0 | 1.0 |
| 19 | Always | -1.0 | 0.8 |
| 20 | Always | -1.0 | 0.8 |
| 21, 22, 23 | Always | 0.8 | 0.8 |

`voice_layer` is `CoverageState::Pending` for all 24 rows.

`plaits_coverage_summary()` appends exactly
` Voice layer: {pending} pending, {source_stage} source-stage.` to the
existing sentence. The counts come from `voice_layer`.

## Pitfalls

- `f32` fields break `Eq`. Remove `Eq` from `PortedAlgorithm`'s derive
  and keep `PartialEq`. Do not change the derives of other structs. If
  `cargo check` shows a real `Eq` requirement elsewhere, record a blocker
  rather than editing outside writePaths.
- Extend the `implemented(...)` const fn with a `voice: VoiceRegistration`
  parameter and set `voice_layer: CoverageState::Pending` inside it. Do
  not hand-write the 24 struct literals.
- Position 7 is `WhenClocked` even though upstream registers `false`,
  because the chiptune engine sets the flag at runtime. Position 15 is
  `Never` (design divergence 8). Add a one-line comment for each.
- `manifest.rs` must stay below 1000 lines (it is 397 now).

## Test Cases (`src/dsp/ported/tests.rs`)

- For every row: `plaits_voice(i) == Some(plaits_algorithms()[i].voice)`, and the tuple equals the table above (a local expected array in the test).
- `plaits_voice(24)` -> `None`.
- `gain(0)` / `gain(1)` for position 9 -> 0.7 / 0.6. For position 17, both are -1.0.
- `is_enveloped(false)` / `is_enveloped(true)`: position 7 -> false / true; position 21 -> true / true; position 0 -> false / false.
- Every row has `voice_layer == CoverageState::Pending`. No row has `coverage == SourcePort`. Engine coverage counts are unchanged (15 adaptations, 9 source-stage).
- The summary string ends with ` Voice layer: 24 pending, 0 source-stage.`

## Verification Commands (logs in `tmp/plv/PLV-12/`, each ending with `exit=<n>`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (fix with `rustfmt --edition 2021` on your files only)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run ported` -> exit 0
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the count
8. `wc -l src/dsp/ported/manifest.rs src/dsp/ported/tests.rs src/dsp/ported/mod.rs` -> each below 1000

## Completion Criteria

- [x] All 24 rows carry the registration tuple above, and the tests prove it.
- [x] Every row has `voice_layer` Pending; engine `coverage` values are unchanged (`git diff` shows no `CoverageState::` change inside rows).
- [x] Commands 1-8 pass with logs.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record `shasum -a 256` of each existing
   writePath and a one-line intent per file in `tmp/plv/PLV-12/hashes.txt`
   and in the Progress Log.
3. Re-read each file just before editing it. If it changed in a way you
   did not make, re-read it and re-apply only your own change, and record
   the drift.
4. Never edit outside writePaths. PLV-10 runs at the same time. If cargo
   fails only inside its files, wait about 60 s and retry, up to 10
   times, then record a blocker and stop.
5. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log`. Record
   post-edit hashes, exit statuses and log paths in the Progress Log. A
   missing or truncated log is not a pass.

## Progress Log

### Session: 2026-09-30 — PLV-12 implementation

**Tasks Completed**: Added the shared 24-position registration table and
`voice_layer` labels, re-exported the registration API, and added coverage
tests for the pinned tuples, helpers, summary suffix, and unchanged engine
coverage counts.

**Intent and initial source hashes**: See `tmp/plv/PLV-12/hashes.txt`.
Final hashes after Rust formatting:

- `src/dsp/ported/manifest.rs`: `ca4b107216d7059f86969cb529907358a20bab0bccc8990b66d3205dacd87a8c`
- `src/dsp/ported/mod.rs`: `cf1b833bfc26574f35aa0be0228d9b02ce53478e422b268d2abea61f372ad9`
- `src/dsp/ported/tests.rs`: `7211601131865f5cc9931719e49e1b61fab79af52539b58f57f0017ae3e46caf`

**Verification** (final runs, all exit 0):

1. `CARGO_TERM_QUIET=true cargo fmt --check` — `tmp/plv/PLV-12/1-fmt.log` (contains the initial failure and final successful retry)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` — `tmp/plv/PLV-12/2-check.log`
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` — `tmp/plv/PLV-12/3-clippy.log`
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` — `tmp/plv/PLV-12/4-wasm.log`
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` — `tmp/plv/PLV-12/5-lsp.log`
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run ported` — 23 passed; `tmp/plv/PLV-12/6-focused.log`
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` — 1,593 passed, 2 skipped; `tmp/plv/PLV-12/7-nextest.log` (contains the initial failure and final successful retry)
8. `wc -l src/dsp/ported/manifest.rs src/dsp/ported/tests.rs src/dsp/ported/mod.rs` — 602, 505, 60 lines; `tmp/plv/PLV-12/8-lines.log`

**Retry history**: The first formatter attempt found an in-progress missing
module in the concurrent voice-layer files; the next attempt found only
formatting differences in those files. After the concurrent worker completed,
the final workspace format check passed. The first full nextest attempt
failed in `dsp::ugen::voice_layer::tests::limiter_has_reference_gain_bounded_output_and_rate_consistent_attack`
(`tmp/plv/PLV-12/7-nextest.log`, exit 100; 583 passed, 1 failed before
cancellation). After the plan's shared-worker retry interval, the complete
successful retry output was appended to that declared canonical log, ending
with exit 0. The concurrent edits were preserved.

**Result**: Registration-table, pending-label, engine-coverage, formatting,
build, lint, native/wasm/LSP, focused-test, full-test, and file-length
criteria are complete. No engine `coverage` row values changed.

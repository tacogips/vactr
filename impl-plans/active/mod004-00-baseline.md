# MOD004-00: Golden Render Baseline and Test Scaffolding

**Status**: Ready
**Plan ID**: MOD004-00 (wave 0, runs alone)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Test strategy; Lowering invariant)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29

## Intent and Context

MOD-004 changes the voice buffer layout, the pan path, the effect path and
the graph codec. Every existing prelude template must still render
bit-identically, except for the nine migrated templates, which must be
bit-identical to their old two-node form. That promise can only be checked
against a record taken **before** any render-path change. This plan
captures that record at the pre-change code (the source tree equal to
commit `cf2ea37`). It also creates empty, registered test files for every
later MOD-004 plan, so that parallel workers never edit a shared module
index.

## Non-goals

- No change to any non-test Rust file, to `src/prelude/templates.vact`, or
  to any runtime behavior.
- No MOD-004 feature code. The bless command is not run again after this
  plan, except by MOD004-30, which may use it only to regenerate the nine
  migrated templates' `graph` lines under the git diff check defined in
  MOD004-30. No plan may change a `render` line.

## Dependencies

- **dependsOn**: none. Runs after the design and all MOD-004 plans are
  committed, and before every other MOD004 plan.
- **Precondition check**: `git diff --stat cf2ea37 -- src verification`
  must print nothing. That proves the code under test is the pre-change
  code. If it prints anything, stop and report.

## writePaths

- `src/host/tests/e2e/templates/golden.rs` (new)
- `src/host/tests/e2e/templates/golden_digests.txt` (new fixture)
- `src/host/tests/e2e/templates.rs`: add module lines only
- `src/dsp/tests/dsp.rs`: add module lines only
- New stub files, each containing only a `//!` doc line naming the owning plan:
  - `src/dsp/tests/dsp/shape_contract.rs` (owner MOD004-10)
  - `src/dsp/tests/dsp/kernel_pairs.rs` (owner MOD004-12)
  - `src/dsp/tests/dsp/codec_shapes.rs` (owner MOD004-21)
  - `src/dsp/tests/dsp/voice_layout.rs` (owner MOD004-22)
  - `src/dsp/tests/dsp/multi_output.rs` (owner MOD004-40)
  - `src/host/tests/e2e/templates/select_output.rs` (owner MOD004-20)
  - `src/host/tests/e2e/templates/migrated_pairs.rs` (owner MOD004-30)
- `impl-plans/active/mod004-00-baseline.md` (this file's Progress Log only)

## sharedPaths (read-only)

`src/host/tests/e2e.rs` (the `E2e` harness), `src/host/tests/e2e/regions.rs`
(the `RampLoader` pattern), `src/host/tests/e2e/templates/coverage.rs`
(registry iteration and `Template::from_inst`).

## File-level Changes

### `src/host/tests/e2e/templates/golden.rs`

Code to imitate:
- `src/host/tests/e2e/templates/coverage.rs:reported_templates_realize_independent_main_and_aux_paths`
  for registry iteration (`e.reg.borrow().entries()`, `name_of_kw`) and
  for building templates with `BuildEnv { sr: 48_000.0, caps: CapabilitySet::native(), voice_mem: 24_000 }`;
- `src/host/tests/e2e/templates/voice_engines.rs` for `e.eval("s :<name> > note [:a3] > once")`;
- `src/host/tests/e2e.rs:E2e::run_stereo_for` for L/R capture;
- `src/host/tests/e2e/regions.rs:RampLoader` for a deterministic `SampleLoader`.

Contents:
- `fn fnv64(words: impl Iterator<Item = u32>) -> u64`: FNV-1a 64 over
  little-endian bytes of `f32::to_bits`, taking all left samples, then all
  right samples.
- **Render digests.** For every prelude template name from the realized
  registry, sorted by name:
  - `center`: a fresh `E2e::new()`, then `s :<name> > note [:a3] > once`,
    then `run_stereo_for(0.5)`, then the digest of L then R.
  - `pan02`: the same with `> pan 0.2` before `> once`. Record this only
    when the template's built `Template` has `has_aux == false` and
    `has_quad == false`.
- **Graph digests.** For every template, the digest of the lowered
  `InstDef` structure: FNV of the UTF-8 bytes of `format!("{:?}", def.nodes)`,
  followed by one `"{from},{to},{port};"` record per edge in order. Never
  include any other `Edge` field, because MOD004-10 adds one.
- **Sampler.** Two extra `sampler` render records:
  - `sampler-mono-res`, using a test `SampleLoader` that returns a
    deterministic 1-channel ramp at 48 kHz;
  - `sampler-stereo-res`, using 2 channels whose left and right differ
    (for example, left is a ramp and right is its negation).
  Wait for install exactly as `regions.rs:primed` does.
- **Fixture format.** One record per line: `<kind> <name> <variant> <16-hex-digest> <frames>`,
  where `<kind>` is `render` or `graph`. Sort the lines. Load the fixture
  with `include_str!("golden_digests.txt")`.
- **Tests:**
  - `golden_renders_match_pre_mod004_baseline`: recompute every record and
    compare with the fixture. On a mismatch, the failure message must
    print, for each mismatching record, the template name, the variant,
    the expected fixture line and the recomputed line, both in the exact
    fixture format (`<kind> <name> <variant> <16-hex-digest> <frames>`).
    Also fail when a template is missing from, or added to, the fixture.
  - `golden_renders_are_deterministic`: render three templates (one mono,
    one `aux-out`, and `sampler`) twice each; the digests must be equal.
  - `#[ignore] bless_golden_digests`: when `VACTR_BLESS_GOLDEN=1`, write
    the fixture file using `std::fs` with a path built from
    `env!("CARGO_MANIFEST_DIR")`; otherwise return without doing anything.

Pitfalls:
- Faults from templates that need a resource must not abort the loop.
  Record the digest of whatever renders, even silence; the baseline is
  about bit equality, not audibility.
- Do not use `HashMap` iteration order. Sort names.
- Do not normalize floats. Hash the raw bits.
- Keep the file under 1000 lines. If it grows past 600, move the loader and
  helpers into `src/host/tests/e2e/templates/golden/`, still owned by this
  plan.

### Module registration

- `src/host/tests/e2e/templates.rs`: add `mod golden; mod migrated_pairs; mod select_output;`
  in alphabetical position.
- `src/dsp/tests/dsp.rs`: add `mod codec_shapes; mod kernel_pairs; mod multi_output; mod shape_contract; mod voice_layout;`
  in alphabetical position.
- Each stub file holds one `//! Reserved for MOD004-xx (<plan file>).` line
  and nothing else, so that it compiles with no warnings.

## Invariants

- No non-test source changes.
- The fixture is generated once, here, from the pre-change code. The
  bless command is not run again after this plan, except by MOD004-30,
  which may use it only to regenerate the nine migrated templates'
  `graph` lines under the git diff check defined in MOD004-30. No plan
  may change a `render` line.

## Test Cases

- The unmodified tree runs `golden_renders_match_pre_mod004_baseline` ->
  pass.
- Editing one float literal in a copy of a digest line -> the test fails
  and names the template. Revert this manual negative check afterwards and
  record it in the Progress Log.
- Two renders of the same template -> identical digests.

## Verification Commands (evidence required in the Progress Log)

Put logs under `tmp/mod004/MOD004-00/`.

1. `git diff --stat cf2ea37 -- src verification` -> empty output. Record it
   before blessing.
2. `VACTR_BLESS_GOLDEN=1 CARGO_TERM_QUIET=true cargo test -q --lib bless_golden_digests -- --ignored`
   -> exit 0, and the fixture exists with one render line per template.
   Run this once, in this plan, to create the fixture. It is not run
   again afterward except by MOD004-30, which may use it only to
   regenerate the nine migrated templates' `graph` lines under the git
   diff check defined in MOD004-30.
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run golden`
   -> exit 0 (two tests pass).
4. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0.
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`
   -> exit 0. Record the pass count.
6. `rustfmt --edition 2021 --check <each new or changed .rs file>` -> exit 0.

## Completion Criteria

- [ ] The precondition diff is empty and recorded.
- [ ] The fixture covers every prelude template (`render center` plus `graph`), `pan02` for every mono non-quad template, and both sampler resource variants.
- [ ] The golden and determinism tests pass; the negative check is recorded.
- [ ] All seven stub test files exist and are registered, and clippy is clean.
- [ ] The full nextest suite passes.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record in the Progress Log the `shasum -a 256`
   of every existing writePath, plus a one-paragraph intent snapshot.
3. Re-read each file just before editing it. If it changed in a way you
   did not make, re-read it and re-apply only your own change.
4. Never edit outside writePaths.
5. Format only your own files with `rustfmt --edition 2021 <files>`.
6. At the end, record post-hashes, and each command's exit status and log
   path, in this Progress Log only.

## Progress Log

(empty)

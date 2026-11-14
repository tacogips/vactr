# FM1V-00: Module Scaffold, Port Contracts, Patch Type and msfa Checkout

**Status**: Completed
**Plan ID**: FM1V-00 (wave 0; blocks every other FM1V plan)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("Architecture decision", "`fm6-core` UGen", Part 2 control tables, "Implementation partition")
**Baseline**: wf/fm1-voices at `9ac8d1f` plus the accepted design commit
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The user asked for synth ideas from the FM-1 community firmwares that vactr
does not have yet. This run covers two parts:

- Six-operator FM algorithms as an extension of the existing `fm` engine,
  with SysEx patch import.
- Six new voices: kalimba, tonewheel organ, hurdy-gurdy, VOSIM, GENDYN and
  scanned synthesis.

Ten wave-1 plans write new modules in parallel. They can only do that if the
module tree, the test module tree and the cross-plan contracts already exist.
This plan creates them, and it does four other things:

- implements the shared `Fm6Patch` type in full, because the SysEx parser,
  the engine, the native and the registry all use it;
- fetches the msfa source (Apache-2.0) once, read-only, so the msfa-based
  plans share one pinned revision;
- pins the port names, order and defaults of every new UGen, and the
  `fm-mod` algorithm-port indices;
- creates placeholder test files, with their module lines, for later plans,
  so that no later plan edits a shared test root.

## Non-goals

- No DSP. Every `render` is a stub that writes `0.0`.
- No registry edits. That means none of: `graph.rs`, the `Node` enum,
  `catalog*`, `codec.rs`, `names*`, `mixer.rs`, `build_helpers.rs`,
  `template.rs`, `voice.rs`, `meta*`, `controls.rs`, `templates.vact`,
  `commit.rs`, `ring.rs`, `natives*`.
- No change to the bodies of `fm::op`, `fm::modulate` or
  `fm::phase_distortion`.
- Do not read FM-1 firmware code (Felucca, SLOOP, sloopDX, FoMni, ChoralRoot,
  Melodee, FiMba-1, GHOULBOX, Jangada), Dexed beyond msfa, Grids, TB-3PO,
  8W8/9W9 or CrispyZebra.

## Dependencies

- **dependsOn**: none
- **Blocks**: every other FM1V plan

## writePaths

- `src/dsp/ugen/fm/patch.rs`
- `src/dsp/ugen/fm/algorithms.rs`
- `src/dsp/ugen/fm/envelope.rs`
- `src/dsp/ugen/fm/scaling.rs`
- `src/dsp/ugen/fm/engine.rs`
- `src/dsp/ugen/fm/sysex.rs`
- `src/dsp/ugen/kalimba.rs`
- `src/dsp/ugen/tonewheel.rs`
- `src/dsp/ugen/hurdy_gurdy.rs`
- `src/dsp/ugen/hurdy_gurdy/bowed.rs`
- `src/dsp/ugen/hurdy_gurdy/buzz.rs`
- `src/dsp/ugen/vosim.rs`
- `src/dsp/ugen/gendyn.rs`
- `src/dsp/ugen/scanned.rs`
- `src/dsp/tests/dsp/fm6_patch.rs`
- `src/dsp/tests/dsp/fm_algorithms.rs`
- `src/dsp/tests/dsp/fm6_envelope.rs`
- `src/dsp/tests/dsp/fm6_engine.rs`
- `src/dsp/tests/dsp/fm6_sysex.rs`
- `src/dsp/tests/dsp/fm6_registry.rs`
- `src/dsp/tests/dsp/fm1_voice_registry.rs`
- `src/dsp/tests/dsp/kalimba.rs`
- `src/dsp/tests/dsp/tonewheel.rs`
- `src/dsp/tests/dsp/hurdy_gurdy.rs`
- `src/dsp/tests/dsp/vosim.rs`
- `src/dsp/tests/dsp/gendyn.rs`
- `src/dsp/tests/dsp/scanned.rs`
- `src/host/tests/e2e/templates/fm1_voices.rs`
- `src/host/tests/e2e/templates/fm_mod_algorithm.rs`
- `src/host/tests/e2e/templates/fm1_examples.rs`
- `impl-plans/active/fm1-voices-00-scaffold.md`
- `target` (artifact root)
- `tmp/fm1-voices/msfa` (artifact root: read-only msfa clone)
- `tmp/fm1-voices/FM1V-00` (artifact root: logs)

## sharedPaths

- `src/dsp/ugen/fm.rs`
- `src/dsp/ugen/mod.rs`
- `src/dsp/tests/dsp.rs`
- `src/host/tests/e2e/templates.rs`

## sharedPathNotes

- `src/dsp/ugen/fm.rs`: add only `pub mod algorithms; pub mod engine; pub mod envelope; pub mod patch; pub mod scaling; pub mod sysex;`. Put them after the `use` lines. The existing function bodies stay byte-identical.
- `src/dsp/ugen/mod.rs`: add only `pub mod gendyn; pub mod hurdy_gurdy; pub mod kalimba; pub mod scanned; pub mod tonewheel; pub mod vosim;`, each in alphabetical position. Do not add `Node` variants.
- `src/dsp/tests/dsp.rs`: add only 13 `mod` lines, alphabetically: `fm1_voice_registry`, `fm6_engine`, `fm6_envelope`, `fm6_patch`, `fm6_registry`, `fm6_sysex`, `fm_algorithms`, `gendyn`, `hurdy_gurdy`, `kalimba`, `scanned`, `tonewheel`, `vosim`.
- `src/host/tests/e2e/templates.rs`: add only `mod fm1_examples; mod fm1_voices; mod fm_mod_algorithm;`, alphabetically.

## Read-only References

- `src/dsp/ugen/bass_voice.rs`: the `PORTS`, `port` constants and `render`
  signature pattern. Follow it exactly.
- `src/dsp/tests/dsp/bass_voice.rs`: `bass_voice_ports_are_unique_and_within_max_ports`,
  which uses a compile-time `const _: () = assert!(...)` for the port count.
- `src/dsp/ugen/frame_keyframe.rs`: `FrameData::from_flat`, `validate`,
  `MAX_PAYLOADS`, `WIRE_VERSION`.

## File-level Changes

### msfa checkout (setup, not gating)

1. `mkdir -p tmp/fm1-voices`, then check `git check-ignore -q tmp/fm1-voices/msfa`.
   If it exits non-zero, append the line `tmp/fm1-voices/` to
   `.git/info/exclude`. That file is local and never committed. Never edit
   `.gitignore`.
2. `git clone --filter=blob:none https://github.com/google/music-synthesizer-for-android tmp/fm1-voices/msfa`,
   then `git -C tmp/fm1-voices/msfa rev-parse HEAD`. Record the 40-hex SHA.
3. Find the source files with
   `git -C tmp/fm1-voices/msfa ls-files | grep -E '(fm_core|env|dx7note|patch|fm_op_kernel|exp2|freqlut)\.(cc|h)$'`.
   Record the paths in the Progress Log.
4. If the clone fails (no network), try fetching those files from
   `https://raw.githubusercontent.com/google/music-synthesizer-for-android/master/<path>`
   with `curl -fsSL`. If that also fails, record **msfa unavailable**. Then
   FM1V-10, FM1V-11 and FM1V-20 report blocked, as the design's Risks
   section requires; this plan still completes.

### `src/dsp/ugen/fm/patch.rs` (implemented in full)

Doc comment content:

- It is the unpacked 155-parameter six-operator voice. The layout is the
  published DX7 voice-parameter format.
- It contains no patch data.
- `msfa revision: <sha>`, or `msfa unavailable`.

Pin these items exactly.

**Sizes:**

- `pub const VOICE_PARAMS: usize = 155;`
- `pub const OP_PARAMS: usize = 21;`
- `pub const MAX_FM6_PAYLOADS: usize = 4;`
- `pub const WIRE_VERSION: u8 = 1;`

**Per-operator offsets:** `pub mod op { pub const R1: usize = 0; ... }`.

| Offsets | Fields |
|---------|--------|
| 0..=3 | `R1` `R2` `R3` `R4` |
| 4..=7 | `L1` `L2` `L3` `L4` |
| 8..=12 | `BREAK_POINT`, `LEFT_DEPTH`, `RIGHT_DEPTH`, `LEFT_CURVE`, `RIGHT_CURVE` |
| 13..=16 | `RATE_SCALING`, `AMP_MOD_SENS`, `VELOCITY_SENS`, `OUTPUT_LEVEL` |
| 17..=20 | `OSC_MODE`, `COARSE`, `FINE`, `DETUNE` |

**Operator base:** `pub const fn op_base(op: usize) -> usize` returns
`(6 - op) * OP_PARAMS` for operator numbers 1..=6. Operator 6 is at 0 and
operator 1 is at 105.

**Global offsets:** `pub mod global`.

| Offsets | Fields |
|---------|--------|
| 126..=129 | `PITCH_R1`..`PITCH_R4` |
| 130..=133 | `PITCH_L1`..`PITCH_L4` |
| 134..=137 | `ALGORITHM`, `FEEDBACK`, `OSC_KEY_SYNC`, `LFO_SPEED` |
| 138..=141 | `LFO_DELAY`, `LFO_PITCH_DEPTH`, `LFO_AMP_DEPTH`, `LFO_KEY_SYNC` |
| 142..=144 | `LFO_WAVE`, `PITCH_MOD_SENS`, `TRANSPOSE` |
| 145 | `NAME` (10 bytes, 145..=154) |

**Range:** `pub const fn max_of(index: usize) -> u8`. The minimum is always 0.

| Parameters | Max |
|------------|----:|
| Rates, levels, break point, depths, output level, fine, pitch R/L, LFO speed/delay/depths | 99 |
| Curves | 3 |
| Rate scaling | 7 |
| AMS | 3 |
| KVS | 7 |
| Osc mode | 1 |
| Coarse | 31 |
| Detune | 14 |
| Algorithm | 31 |
| Feedback | 7 |
| Key syncs | 1 |
| LFO wave | 5 |
| PMS | 7 |
| Transpose | 48 |
| Name bytes | 127 |
| Index >= 155 | 0 |

**Type:** `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct Fm6Patch { pub params: [u8; VOICE_PARAMS] }`,
with `pub const EMPTY` (all zeros, which is valid).

**Methods:**

- `pub fn from_flat(xs: &[f32]) -> Result<Fm6Patch, String>`. It returns an
  error when:
  - the length is not 155: `"patch: expected 155 values, got {n}"`;
  - a value is non-finite, non-integer or outside `0..=max_of(i)`:
    `"patch: value {v} at index {i} must be an integer in 0..={max}"`.
- `pub fn validate(&self) -> Result<(), usize>` returns the first
  out-of-range index.
- `pub fn algorithm(&self) -> u8` returns 0..=31.

### Kernel scaffolds

Six files: `kalimba.rs`, `tonewheel.rs`, `hurdy_gurdy.rs`, `vosim.rs`,
`gendyn.rs` and `scanned.rs`.

Each file contains:

- **Doc comment.** It cites the design section and states that the code is
  original, written from the cited papers and physics, and that no
  implementation source was consulted.
- **Ports.** `pub const PORT_COUNT: usize`,
  `pub const PORTS: [(&str, f32); PORT_COUNT]` and
  `pub mod port { SCREAMING_SNAKE = index }`.
- **State.** `pub const STATE_FLOATS: usize = 1;` is a placeholder. The
  kernel plan sets the real value.
- **Render stub.** `pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>)`
  fills `out` with 0.0.

`hurdy_gurdy.rs` also declares `pub mod bowed; pub mod buzz;`. Those two
files contain only `//! (owned by FM1V-15)`.

Pinned port tables. Order is the index. Each entry is `name default`.

| UGen | Ports |
|------|-------|
| kalimba (8) | freq 440, kalimba-beat 1.5, kalimba-hardness 0.5, kalimba-decay 2.5, kalimba-damping 0.5, kalimba-body 0.3, kalimba-buzz 0, velocity 1 |
| tonewheel (20) | freq 440, cps 0.5, onset-time 0, drawbar1 8, drawbar2 8, drawbar3 8, drawbar4 0, drawbar5 0, drawbar6 0, drawbar7 0, drawbar8 0, drawbar9 0, organ-click 0.3, organ-perc 0, organ-perc-slow 0, organ-perc-soft 0, organ-perc-trigger 1, organ-vibrato 0, gate-length 4, velocity 1 |
| hurdy_gurdy (16) | freq 440, cps 0.5, onset-time 0, gurdy-wheel 0.5, gurdy-pressure 0.5, gurdy-melody 1, gurdy-bourdon 0.6, gurdy-fifth 0.4, gurdy-trompette 0.5, gurdy-drone-key 43, gurdy-buzz 0.6, gurdy-buzz-threshold 0.5, gurdy-strokes 0, gurdy-stroke-depth 0.5, gate-length 8, velocity 1 |
| vosim (4) | freq 440, vosim-formant 900, vosim-pulses 3, vosim-decay 0.7 |
| gendyn (6) | freq 440, gendyn-points 12, gendyn-amp-step 0.2, gendyn-dur-step 0.1, gendyn-spread 0.1, gendyn-dist 1 |
| scanned (7) | freq 440, scan-stiffness 0.5, scan-damping 0.3, scan-centering 0.1, scan-hammer 0.3, scan-position 0.5, scan-update 400 |

`velocity` is last in each table, as an implicit row-bound port. Kernels use
it as a strike or onset level multiplier. It is an existing control row
(id 6).

### `src/dsp/ugen/fm/engine.rs` (scaffold)

- `pub const FM6_PORTS: [(&str, f32); 6]`: freq 440, velocity 1,
  algorithm 0, ratio 1, index 1, fm6-feedback 4. Also
  `pub mod fm6_port { FREQ=0, VELOCITY=1, ALGORITHM=2, RATIO=3, INDEX=4, FEEDBACK=5 }`.
- `pub mod fm_mod_port { IN=0, MOD=1, INDEX=2, ALGORITHM=3, FREQ=4, RATIO=5, VELOCITY=6 }`.
  This is the appended `fm-mod` port order. FM1V-40 builds the catalog from
  it.
- `pub const MACRO_FEEDBACK: u8 = 4;`
- `pub const STATE_FLOATS: usize = 1;` (placeholder)
- `pub fn fm6_render(ins: &[Inp<'_>; MAX_PORTS], patch: Option<&Fm6Patch>, st: &mut NodeState, mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>)`
  is a stub that writes 0.0.

### Other placeholders

`algorithms.rs`, `envelope.rs`, `scaling.rs` and `sysex.rs` contain only an
owner `//!` line (FM1V-10, FM1V-11, FM1V-11 and FM1V-12). Placeholder test
files contain only `//! Owned by FM1V-xx.`:

| Test file | Owner |
|-----------|-------|
| `fm_algorithms` | FM1V-10 |
| `fm6_envelope` | FM1V-11 |
| `fm6_sysex` | FM1V-12 |
| `fm6_engine` | FM1V-20 |
| `fm1_voice_registry` | FM1V-30 |
| `fm1_voices` | FM1V-30 |
| `fm6_registry` | FM1V-40 |
| `fm_mod_algorithm` | FM1V-40 |
| `fm1_examples` | FM1V-50 |

## Pitfalls

- Port tables are a cross-plan contract. Copy them exactly, and do not
  reorder or rename. Every custom name was checked against the existing
  control rows, `EXTRA_PARAMS` and the template headers, and none collide.
  `gate-length` is reused on purpose.
- The `fm.rs` edit is module declarations only. A changed `op`, `modulate`
  or `phase_distortion` body changes golden digests.
- Do not add `Node` or `UGenSpec` variants. They force registry match arms.
- Clippy must stay clean. Name unused stub parameters with a leading `_`,
  and do not use `#[allow(dead_code)]` on the public contract items.

## Tests (input -> expected)

`fm6_patch.rs`:

- `fm6_patch_layout_offsets`:
  - `op_base(6) == 0` and `op_base(1) == 105`;
  - `global::ALGORITHM == 134` and `global::NAME == 145`;
  - `VOICE_PARAMS == 145 + 10`.
- `fm6_patch_from_flat_accepts_valid`: 155 zeros produce `Ok(EMPTY)`. A list
  with algorithm 31 and feedback 7 round-trips.
- `fm6_patch_from_flat_rejects`:
  - 154 values: an error containing `"expected 155"`;
  - 100 at index `op_base(1)+op::OUTPUT_LEVEL`: an error containing
    `"index 121"` and `"0..=99"`;
  - 1.5 anywhere: an error;
  - NaN: an error.
- `fm6_patch_validate_reports_first_bad_index`.

Each kernel test file gets `<name>_ports_are_unique_and_within_max_ports`.
It checks unique names, that the `port::` constants match their names, and
`PORT_COUNT <= MAX_PORTS` with a const assert. `fm6_engine.rs` gets
`fm6_ports_contract` for `FM6_PORTS` and `fm_mod_port`.

## Verification (evidence required; logs under `tmp/fm1-voices/FM1V-00/`)

1. `rustfmt --edition 2021 --check` on every `.rs` in writePaths and
   sharedPaths. It must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6_patch|ports_are_unique|fm6_ports_contract/)' > tmp/fm1-voices/FM1V-00/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`. At least 11 tests must run, with 0 failed.
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/fm1-voices/FM1V-00/clippy.log 2>&1; echo "exit=$?"`
   must print `exit=0`.
4. `git diff -U0 -- src/dsp/ugen/fm.rs | grep -c '^[-+][^-+]'` must equal 6,
   all of them added `pub mod` lines.
5. `git diff --stat` and `git status --short` show only writePaths and
   sharedPaths, and nothing under `tmp/`.

Record each result in the form `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

- Fresh-read each shared path right before editing it.
- Record `shasum -a 256` before and after the edit.
- If the pre-hash differs from `git show HEAD:<path> | shasum -a 256`,
  re-read and apply only the stated lines. Never revert other lines.
- Run no git operations other than `diff`, `status`, `check-ignore` and the
  msfa clone under `tmp/`.

## Done Criteria

- [x] All writePaths exist, and the shared paths changed only by the stated
      lines.
- [x] The msfa SHA and file paths, or "msfa unavailable", are recorded here
      and in the `patch.rs` doc.
- [x] Verification 1-5 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none. Awaiting implementation.

### Session: 2026-10-10 (FM1V-00 implementation)
**Tasks Completed**: Module tree, kernel and engine contracts, test placeholders, `Fm6Patch`, read-only msfa checkout, and required focused verification.

- Created the six kernel scaffolds and their exact pinned port tables. All render stubs fill output with `0.0`; state size remains the one-float placeholder. Added the FM6/FM-mod port contracts and owner-only placeholders for downstream plans.
- Implemented `Fm6Patch` in `src/dsp/ugen/fm/patch.rs` with all 155 offsets/ranges, exact flat-conversion errors, validation, and algorithm access. The doc records msfa revision `f67d41d313b7dc85f6fb99e79e515cc9d208cfff` and the consulted path names: `app/src/main/jni/dx7note.cc`, `dx7note.h`, `env.cc`, `env.h`, `exp2.cc`, `exp2.h`, `fm_core.cc`, `fm_core.h`, `fm_op_kernel.cc`, `fm_op_kernel.h`, `freqlut.cc`, `freqlut.h`, `patch.cc`, `patch.h`, `pitchenv.cc`, and `pitchenv.h`. Clone is read-only at `tmp/fm1-voices/msfa`.
- Shared roots changed only as scoped: `src/dsp/ugen/fm.rs` has exactly six added `pub mod` lines; `src/dsp/ugen/mod.rs` has six new public module lines; `src/dsp/tests/dsp.rs` has thirteen test module lines; `src/host/tests/e2e/templates.rs` has three test module lines. Pre/post SHA-256 records are in `tmp/fm1-voices/FM1V-00/shared-before.sha256` and `shared-after.sha256`.
- Final formatting check passed: `rustfmt --edition 2021 --check` on every Rust write/shared path; log `tmp/fm1-voices/FM1V-00/rustfmt-final2.log`.
- Final focused behavioral verification passed: `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6_patch|ports_are_unique|fm6_ports_contract/)'`; 13 run, 13 passed, 0 failed; log `tmp/fm1-voices/FM1V-00/nextest-final.log`.
- Final strict lint passed: `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`; exit 0; log `tmp/fm1-voices/FM1V-00/clippy-final.log`.
- Resolved pre-final attempts retained: the first nextest compile found integer literals in `f32` port tables/tests (`nextest.log`); after correcting literal spelling, nextest passed 13/13 (`nextest-rerun.log`). First clippy found bare `#[must_use]` on a `Result` (`clippy.log`); after adding its diagnostic message, final clippy passed. Initial format-check attempts and formatting-only fixes are retained in `rustfmt.log`, `rustfmt-apply.log`, `rustfmt-final.log`, and `tonewheel-rustfmt.log`.
- Scope checks: `fm.rs` diff is six additions; `git diff --stat` and `git status --short` contain only declared writePaths/sharedPaths. No registry edits were made. No remaining FM1V-00 implementation tasks.

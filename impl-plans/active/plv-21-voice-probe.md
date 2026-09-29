# PLV-21: Voice-Layer Source Comparison Probe (`compare-plaits-voice`)

**Status**: Ready
**Plan ID**: PLV-21 (wave 2; parallel with PLV-20)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Comparison probe; Intentional divergences 1, 4 and 5)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan builds a local, repeatable comparison between the pinned
upstream Plaits voice layer and Vactr's `voice_layer` translation
(PLV-10). It follows the existing
`verification/compare_plaits_osc.py` + `verification/plaits_osc_reference.cc`
+ `examples/plaits_osc_reference.rs` pattern:

- a C++ harness compiled against a separate, clean, pinned checkout
  passed as `--source`;
- a Rust example driving the Vactr side;
- a Python orchestrator that prints JSON metrics.

PLV-40 runs the probe and decides the manifest label from its result.
Mismatches are measured gaps, not probe failures.

## Non-goals

- No change to Vactr runtime code, templates, manifest labels, inventory
  or notices (PLV-40 owns inventory, notices and labels).
- No copy of upstream objects, tables, `resources.cc` data or output
  samples into the repository. All build products and outputs stay in a
  temporary directory, or under `tmp/`.
- No comparison of engine kernels (the existing probes already do that).

## Dependencies

- **dependsOn**: PLV-10 (the Rust example calls `vactr::dsp::ugen::voice_layer` directly)
- **Blocks**: PLV-40

## writePaths

- `verification/plaits_voice_reference.cc` (new)
- `verification/compare_plaits_voice.py` (new)
- `examples/plaits_voice_reference.rs` (new)
- `mise.toml` (one new `[tasks.compare-plaits-voice]` table, placed right after `[tasks.compare-plaits-drums]`)
- `impl-plans/active/plv-21-voice-probe.md` (Progress Log only)
- `tmp/plv/PLV-21/hashes.txt`, `tmp/plv/PLV-21/1-fmt.log`, `tmp/plv/PLV-21/2-check.log`, `tmp/plv/PLV-21/3-clippy.log`, `tmp/plv/PLV-21/4-wasm.log`, `tmp/plv/PLV-21/5-lsp.log`, `tmp/plv/PLV-21/7-nextest.log`, `tmp/plv/PLV-21/9-probe.log`, `tmp/plv/PLV-21/9-probe.json`, `tmp/plv/PLV-21/10-pycompile.log`

## sharedPaths (read-only)

- `verification/compare_plaits_osc.py` (`run`, `revision`, `require_clean_tracked_tree`, `correlation`, the clang invocation at lines 256-264, and the JSON summary layout)
- `verification/plaits_osc_reference.cc` (harness structure and output printing)
- `examples/plaits_osc_reference.rs` (argument parsing and `BufWriter` output)
- `src/dsp/ugen/voice_layer.rs` (PLV-10 API; do not edit)
- `src/dsp/ported/manifest.rs` (`plaits_voice` for gains; do not edit)

## mise task (pin exactly)

```toml
[tasks.compare-plaits-voice]
description = "Compare the Plaits voice-level trigger and low-pass gate layer against a separate pinned Mutable Instruments checkout"
run = 'python3 verification/compare_plaits_voice.py --source "${VACTR_MI_REFERENCE:?set VACTR_MI_REFERENCE to the pinned Eurorack checkout}"'
env = { CARGO_TERM_QUIET = "true" }
```

## Program interfaces (pin exactly; text on stdout, one value per row, `%.9g` / `{:.9e}`)

Row tags: `T <block> <gain> <frequency> <hf_bleed> <decay> <start_sample>`
for one control-block trajectory row, where `start_sample` is the
block's first sample index relative to the aligned trigger (`12 * block`
upstream; the running sum of `ControlClock` lengths in Vactr); `M <v>` / `X <v>` for main/aux output
samples; `R <v>` / `Q <v>` for raw upstream engine main/aux samples.

**C++ `plaits_voice_reference`**:
- `voice <engine> <mode:off|ping|level> <decay> <color> <note> <velocity> <gate_blocks> <blocks>`
  runs the real `plaits::Voice`:
  - `Init` with a static 32768-byte `BufferAllocator`; `patch.engine = engine`;
    `patch.note = note`; harmonics, timbre and morph are 0.5; every
    modulation amount is 0.
  - `modulations.note = 0`, `engine = 0`; `trigger_patched = mode != off`;
    `level_patched = mode == level`.
  - It renders 16 warm-up blocks of 12 frames with trigger 0 and level
    0. Trigger is then 1 for `gate_blocks` blocks starting at block 16.
    Level is `velocity` for `gate_blocks` blocks starting at block 20
    (the delayed trigger edge), and 0 otherwise. The alignment is block
    20 because the upstream delay line returns the value written 4
    blocks earlier (`plaits/dsp/physical_modelling/delay_line.h`
    Write/Read semantics), so the effective trigger delay is 4 blocks.
  - From block 20 on, for `blocks` blocks, it prints one `T` row per
    block (`lpg_envelope_.gain()`, `.frequency()`, `.hf_bleed()`,
    `decay_envelope_.value()`, read after `Render`). It also prints the
    frame outputs as `M`/`X`, each converted from int16 as
    `-(s - 1) / 32767.0`, and `out_buffer_`/`aux_buffer_` as `R`/`Q`.
  - Self-check: using `-fno-access-control`, the harness reads
    `trigger_state_` after each `Render`. In ping and level modes it
    must be false after block 19 and true after block 20. Otherwise the
    harness prints a diagnostic to stderr and exits non-zero. This is a
    probe failure, not a measured gap. It keeps the alignment derived
    from observed upstream behavior rather than an assumption.
- `audio <gain> <input_path> <trajectory_path>` uses
  `plaits::LowPassGate` and `stmlib::Limiter` directly:
  - The input file has one float per line; the trajectory file has
    `<gain> <frequency> <hf_bleed>` per line.
  - For each 12-sample block: if `gain < 0`, apply
    `Limiter::Process(-gain, ...)` in place; then
    `LowPassGate::Process(post * traj_gain, freq, bleed, buf, 12)` (the
    float overload), with `post = gain < 0 ? 1 : gain`.
  - It prints `M` rows clamped to [-1, 1].
- `bypass <gain> <input_path>`: the limiter when `gain < 0`, then
  `clamp(x * post, -1, 1)`, printed as `M` rows.

**Rust `examples/plaits_voice_reference.rs`** (uses only `voice_layer` and `plaits_voice`):
- `traj <mode> <decay> <color> <freq_hz> <velocity> <gate_blocks> <blocks> <sr>`
  prints one `T` row per control block. At the voice's first block,
  `trigger()` is followed by processing, exactly as `render_gate` does.
- `audio <gain> <input_path> <trajectory_path>` uses `LowPassGate` and
  `PostLimiter` at 48 kHz with 12-sample blocks, and prints `M` rows.
- `bypass <gain> <input_path>` prints `M` rows.
- `lane <slot> <lane> <mode> <decay> <color> <freq_hz> <velocity> <gate_blocks> <input_path>`
  is the full lane path (trajectory, limiter, gate, clip) at 48 kHz on
  the given input, and prints `M` rows.
- Note to Hz: `freq_hz = 440 * 2^((note - 69) / 12) * 48000 / 47872.34`,
  the same correction as `compare-plaits-osc`.

## Scenarios, metrics and thresholds (Python)

- **A. Trajectory**: mode in {ping, level}, decay in {0.2, 0.5, 0.8},
  color in {0, 0.5, 1}, note in {48, 69}, velocity in {1.0, 0.5} (level
  only), engine 0, `gate_blocks` 400, `blocks` 1200. Compare `T` rows
  field by field: max-abs error for gain, frequency and hf_bleed.
  Threshold: <= 1e-4.
- **B. Audio path**: gain in {0.8, 0.6, -1.0, -2.0}.
  - Trajectory: A's ping, decay 0.5, color 0.5, note 69, taken from the
    upstream run.
  - Input: 14400 samples from `random.Random(1234)` uniform(-0.5, 0.5)
    plus `0.3 * sin(2 pi 220 n / 48000)`, written to a temp file.
  - Metrics: correlation, max-abs and RMS error. Threshold: correlation
    >= 0.999.
- **C. End to end**: engine in {0 (gain 1.0), 10 (gain 0.6)} and mode in
  {ping, level}, with decay 0.5, color 0.5, note 69, velocity 1. Compare
  upstream `M`/`X` with Vactr `lane` on the upstream `R`/`Q` rows, slot =
  engine, lanes 0/1. Threshold: correlation >= 0.999, with RMS reported;
  int16 quantization and the omitted 1-LSB offset are expected.
- **D. Bypass**: engine 21, mode ping: upstream `M` vs Vactr
  `bypass 0.8` on `R`. Threshold: correlation >= 0.999.
- **E. Host rate** (Vactr only): `traj` at 44100 and 96000 vs 48000,
  ping, decay 0.5, color 0.5, 440 Hz. The times in ms where the gain
  first falls below 0.5 and below 0.01 after the peak agree within
  0.25 ms plus one sample period.

The script prints one JSON object with:
- `scope`, `upstream_revision`, `stmlib_revision`;
- one `scenarios` entry per run, with its metrics and `meets_threshold`;
- `voice_layer_label_eligible`, true only if every scenario meets its
  threshold.

It exits 0 when every run completes, even with gaps, and non-zero on a
checkout, revision, compile or run failure.

## Pitfalls

- Reuse `revision` and `require_clean_tracked_tree`. Refuse a checkout
  that is not Eurorack `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` or
  stmlib `e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`, or that is dirty.
  Never write into the checkout.
- Compile with the same `clang` as `compare_plaits_osc.py`, plus
  `-std=c++11 -DTEST -O2 -fno-access-control`. The harness may read
  `Voice` private members through `-fno-access-control`; never use
  `#define private public`.
- Sources: `plaits/dsp/voice.cc`, every `.cc` under `plaits/dsp`
  (collected at run time with `sorted((source / "plaits/dsp").rglob("*.cc"))`),
  `plaits/resources.cc`, `stmlib/dsp/units.cc` and `stmlib/utils/random.cc`.
  Link into a `tempfile.TemporaryDirectory`.
- Do not hard-code the scratchpad path anywhere. It comes only from
  `--source` / `VACTR_MI_REFERENCE`.
- The upstream `T` row must be read after `Render` of that block.
  Upstream processes the trigger and envelopes before rendering, so the
  row describes the gain applied during the block, which matches
  Vactr's `begin` order.
- Upstream level mode applies the level without the trigger delay. The
  harness therefore raises the level at block 20, not 16, so it lines up
  with the delayed trigger edge and Vactr's first block. The JSON
  `scope` text must state that the alignment is at the detected
  rising-edge block 20 (trigger raised at block 16, 4-block effective
  delay).
- Name only upstream paths that PLV-40 will inventory (see PLV-40). In
  the harness, include only `plaits/dsp/voice.h`,
  `plaits/dsp/fx/low_pass_gate.h`, `plaits/dsp/envelope.h`,
  `stmlib/dsp/limiter.h` and `stmlib/utils/buffer_allocator.h`. Record
  every upstream path string in the new files in the Progress Log.
- The Rust example must not allocate per sample in hot loops beyond
  reading inputs. It runs only natively, but it still compiles under
  `cargo check --all-targets`.

## Test and Verification Commands (logs in `tmp/plv/PLV-21/`, each ending with `exit=<n>`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (fix only `examples/plaits_voice_reference.rs` with `rustfmt --edition 2021`)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0 (compiles the new example)
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0
9. `test -d "$VACTR_MI_REFERENCE" && mise run compare-plaits-voice > tmp/plv/PLV-21/9-probe.json 2> tmp/plv/PLV-21/9-probe.log` with `VACTR_MI_REFERENCE=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack` -> exit 0 and valid JSON with every scenario present. If the directory is absent, record BLOCKED (not passed). Thresholds need not be met here. The harness self-check passes (harness exit 0) for at least one ping and one level run in scenario A.
10. `python3 -m py_compile verification/compare_plaits_voice.py` -> exit 0
11. `cargo run -q --example plaits_voice_reference -- traj ping 0.5 0.5 441.17 1 400 20 48000` -> exit 0 and 20 `T` rows (smoke test; append to `2-check.log`)

## Completion Criteria

- [ ] The three new files and the `mise.toml` task exist with the pinned interfaces.
- [ ] Commands 1-5, 7, 10 and 11 pass. Command 9 completes with JSON covering scenarios A-E, or is recorded as BLOCKED with the reason. The `trigger_state_` self-check passes (harness exit 0) for at least one ping and one level run in scenario A.
- [ ] The JSON and the Progress Log record, per scenario, the metrics and whether it `meets_threshold`. Gaps are reported, not hidden.
- [ ] `git status` shows no files under `verification/` other than the two new ones, and no build products in the repository.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record `shasum -a 256` of `mise.toml` and a
   one-line intent per file in `tmp/plv/PLV-21/hashes.txt` and in the
   Progress Log.
3. Re-read `mise.toml` just before editing it. If it changed in a way
   you did not make, re-read it and re-apply only your own change, and
   record the drift.
4. Never edit outside writePaths. PLV-20 runs at the same time and owns
   the registry files. If cargo fails only inside its files, wait about
   60 s and retry, up to 10 times, then record a blocker and stop.
5. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log` (command 9
   uses its own JSON and stderr log). Record post-edit hashes, exit
   statuses and log paths in the Progress Log. A missing or truncated
   log is not a pass.

## Progress Log

(none yet)

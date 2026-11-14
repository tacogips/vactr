# MTX-S2: Plaits pair kernels, voice layer and LPG -> music-things-synth

**Status**: Completed (accepted in session 605, 2026-11-01)
**Plan ID**: MTX-S2 (wave W2; runs concurrently with MTX-F2, with no shared files)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` sections 3.2 (Plaits-derived pairs and voice layer, `lpg`), 4.1 (fn-pointer wrappers), 4.3 (rows `vactrol_gate.rs`, `voice_layer.rs`, the shim row, `ported/manifest.rs`), 4.4 (renames), 5, 6
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

This plan moves the Plaits-derived pair kernels, the shared Plaits voice
layer and the low-pass gate into the synth crate. Several of these
functions are used as higher-ranked fn pointers by
`src/dsp/tests/dsp/kernel_pairs.rs:16-20` and
`examples/plaits_drums_reference.rs:32`. vactr therefore keeps shim files
with the exact old signatures (design 4.1).

The LPG reads vactr's Plaits voice registration table. That table stays in
vactr and is injected as a function pointer.

## Non-goals

- Changing `Node::VactrolGate`, `UGenSpec::VactrolGate`, the catalog name
  `"vactrol-gate"` or `voice_ports::VACTROL_GATE`. These are vactr catalog
  items and stay.
- Changing the `VOICE_REGISTRATIONS` table or `plaits_voice`.
- Other kernels.
- Effects.

## Dependencies

- dependsOn: MTX-S1 (serial owner of the synth `lib.rs` and
  `ugen/mod.rs`; S1 provides the module layout pattern)
- Blocks: MTX-S3

## File-level changes

Moves go from `src/dsp/ugen/<f>` to `music-things/crates/music-things-synth/src/<f>`.

1. **Plain moves.** The vactr source is deleted. `ugen/mod.rs` changes
   `pub mod` to `pub use music_things_synth::<m>;`.
   - `chip_pair.rs`, `clock_noise_pair.rs`, `grain_pair.rs`,
     `modal_pair.rs`, `particle_pair.rs`, `spectrum_pair.rs`,
     `string_pair.rs`, `swarm_pair.rs`
   - `six_op_original.rs`, `speech_original.rs`, `number_station.rs`
2. **Moves whose vactr file stays as a fn-pointer shim** (protocol section
   4). The crate gets the full module, including submodules. vactr keeps
   `pub mod <m>;` pointing at the rewritten shim.
   - `fm_pair.rs`, `analog_pair.rs`, `chord_pair.rs`,
     `table_terrain_pair.rs`, `terrain_pair.rs`,
     `string_machine_pair.rs`, `shape_pair.rs`: wrappers `render` and
     `render_pair` (exact current signatures).
   - `dual_kick.rs`, with `dual_kick/analog.rs` and
     `dual_kick/synthetic.rs` moved to the crate and deleted in vactr:
     wrapper `render`.
   - `snare_pair.rs`, with `snare_pair/{analog,synthetic}.rs` moved:
     wrapper `render`.
   - `hat_pair.rs`, with `hat_pair/{ring,square}.rs` moved: wrapper
     `render`.
   - The vactr shim keeps no `mod analog;`-style declarations.
3. **`voice_layer.rs` and `voice_layer/tests.rs`.** Move them to the crate.
   - Rename `VactrolEnvelope` to `OptoEnvelope` in both files.
   - Reword the "vactrol" comments.
   - vactr `src/dsp/ugen/voice_layer.rs` becomes a shim with
     `pub use music_things_synth::voice_layer::*;` and
     `pub use music_things_synth::voice_layer::OptoEnvelope as VactrolEnvelope;`.
     `examples/plaits_voice_reference.rs:11-14` imports
     `VactrolEnvelope` and must compile unedited.
4. **`vactrol_gate.rs` becomes crate `lpg.rs`.**
   - In the crate file:
     - imports become `super::voice_layer::{…OptoEnvelope…}`;
     - constants named `VACTROL*` become `OPTO*`;
     - comments are reworded;
     - `pub const GATE_STATE_FLOATS` and `DECAY_MOD_STATE_FLOATS` are
       unchanged.
   - Add `VoiceRegistration` (with `gain`, `is_enveloped`) and `Enveloped`,
     moved verbatim from `src/dsp/ported/manifest.rs:67-103`. Add a doc
     line on each variant.
   - `initialize_gate` and the public `render_gate` gain a trailing
     argument `registry: fn(usize) -> Option<VoiceRegistration>`. Replace
     `slot.and_then(plaits_voice)` with `slot.and_then(registry)`.
   - `render_decay_mod` keeps its parameters, plus the generic context.
5. **vactr `src/dsp/ugen/vactrol_gate.rs` becomes the adapter.**
   - `pub use music_things_synth::lpg::{GATE_STATE_FLOATS, DECAY_MOD_STATE_FLOATS};`
     plus any other items vactr references (check with `grep -rn
     vactrol_gate:: src examples`).
   - `pub fn render_gate(ins, st, mem, out, kx: &Kx<'_>)` forwards with
     `crate::dsp::ported::plaits_voice`.
   - `pub fn render_decay_mod(…, kx: &Kx<'_>)` forwards. Both use exact
     current signatures.
6. **`src/dsp/ported/manifest.rs`.** Replace the `Enveloped` and
   `VoiceRegistration` definitions and impl with
   `pub use music_things_synth::lpg::{Enveloped, VoiceRegistration};`.
   The table, `plaits_voice`, every other item and `ported/mod.rs` stay
   unchanged.
7. **Imports.**
   - `crate::dsp::effects::prim::…` -> `music_things_dsp::prim::…`
   - `crate::dsp::ugen::NodeState` (in `dual_kick/*`, `snare_pair/*`) ->
     `super::super::NodeState`
   - `super::{Inp, Kx, …}` -> crate types. Generic entry points per
     protocol section 3.2.
8. **`fm_pair.rs` in-file tests (lines 227+).** Replace the
   `SampleStore`/`CapabilitySet`/`FxStats`/`Kx` construction with
   `KernelCtx { sr, gate, seed }`. Use the same values; assertions stay
   untouched.
9. **Docs** for all public items. Scrub the "Vactr" comments in
   `grain_pair.rs:5`, `speech_original.rs:5,13`, `clock_noise_pair.rs:5`
   and `string_pair.rs:3`, keeping the provenance meaning ("original
   tokens, not TI ROM words").

## Existing patterns to imitate

- Shim and adapter: protocol section 4. The `sample.rs` adapter written
  in MTX-S1.
- Re-export of moved types from a manifest: `src/dsp/ported/mod.rs:54-57`.

## Pitfalls

- **Build coupling with MTX-F2.** `music-things-fx` depends on
  `music-things-synth`, and vactr depends on both. While one of your
  modules is mid-move, every cargo command of the concurrent MTX-F2
  fails. Follow protocol section 2.1: one module (with its submodule
  files) at a time, with `cargo check -p music-things-synth --all-targets`
  and `cargo check -p vactr --all-targets` logged per module. Move the
  `voice_layer` and `lpg` pair together with the `ported/manifest.rs`
  re-export as one step. Append `EDITS-COMPLETE <timestamp>` when done,
  then run the gates. A failure located only in MTX-F2's paths follows
  protocol section 5.1.
- **Exact shim signatures.** The `render_pair` signatures differ per
  kernel: some take `mem`, some do not (see
  `src/dsp/tests/dsp/kernel_pairs.rs:15-20`, and
  `impl-plans/active/mod004-12-kernel-pairs.md` Contract). Copy each
  from the original file.
- **Registry type.** `plaits_voice` has type
  `fn(usize) -> Option<VoiceRegistration>` only after the manifest
  re-exports the crate type. Edit the manifest and lpg together, then
  compile.
- **No new banned tokens.** No identifier or string in the crate may
  contain "vactr" (V-grep is case-insensitive). The port-name strings
  are in vactr's catalog, not here. Do not change them.
- **Test assertions.** The test helpers in `voice_layer/tests.rs` (the
  `round_trip!` macro at line 206) must not use `$crate`. Rename only the
  `VactrolEnvelope` occurrences. No assertion changes.
- **Off-limits.** Do not edit `src/dsp/tests/dsp/vactrol_gate.rs`,
  `kernel_pairs.rs`, examples or `ported/tests.rs`.

## Tests

- No new tests. The moved in-file tests run in the crate binary
  (`fm_pair`, `voice_layer/tests.rs`, and any others).
- `src/dsp/tests/dsp/{vactrol_gate,kernel_pairs,dual_kick,snare_pair,hat_pair,fm_pair,analog_pair,chord_pair,*_pair}.rs`,
  `src/dsp/ported/tests.rs` and the golden template tests pass unchanged.

## Verification (protocol section 7; logs under `tmp/music-things/MTX-S2/`)

1. V-check.
2. V-fmt (all created or modified `.rs` files).
3. V-clippy: `-p music-things-synth` and `-p vactr`.
4. V-nextest.
5. V-names: `unmatched=0`. The rename map `vactrol_gate::` -> `lpg::` is
   built into the checker.
6. V-wasm, and V-crate-wasm for `music-things-synth`.
7. V-examples, especially `plaits_voice_reference`,
   `plaits_drums_reference` and `fm_pair_reference`.
8. V-grep.
9. V-protected.
10. V-gone over the deleted sources.
11. V-lines.
12. V-vitest.
13. `git diff a6e5bea -- src/dsp/ported/manifest.rs`: only the type
    definitions are replaced by one `pub use`. Log it.

## Completion Criteria

- [x] 30 source files have moved (crate `lpg.rs` replaces
      `vactrol_gate.rs`). 12 vactr shims or adapters remain with exact old
      signatures.
- [x] The LPG registry is injected, and `ported/manifest.rs` re-exports
      the moved types.
- [x] `examples/plaits_voice_reference.rs` and
      `examples/plaits_drums_reference.rs` compile unedited.
- [x] Verification 1-13 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

### Session 2026-11-01 (MTX-S2 implementation)
- Source moves, forwarding shims and LPG registry injection implemented; per-edit preimages and intentions are recorded in `tmp/music-things/MTX-S2/edit-intentions.jsonl`.
- EDITS-COMPLETE 2026-11-01T04:22:44.634781+09:00
- Final verification in progress.
- Removed a redundant FM shim glob after a compiler warning; source stable again.
- EDITS-COMPLETE 2026-11-01T04:24:06.995276+09:00
- Completed input-port documentation after author self-check.
- EDITS-COMPLETE 2026-11-01T04:25:00.238965+09:00
- Final-source checks passed: workspace check, synth and vactr clippy, test-name inventory (baseline=3108, after=3113, unmatched=0), wasm builds, examples, scoped rustfmt, grep, source deletion, line counts, manifest and protected paths. Logs: `tmp/music-things/MTX-S2/stable-*.log` and `audit-stable-*.log`.
- Editor vitest: 2399 passed, 1 skipped, exit 0 (`tmp/music-things/MTX-S2/stable-vitest-complete.log`). Full workspace nextest pending final-source rerun.
- Final-source nextest: 3113 passed, 3 skipped, exit 0; pre/post source manifests identical (`tmp/music-things/MTX-S2/stable-nextest-isolated.log`, `stable-nextest-source-{before,after}.txt`). Eight sibling-blocked per-module checks passed on retry (`mod-*-check-vactr-final-retry.log`). Formal review remains downstream.
- Verification records (Cargo commands use `CARGO_TERM_QUIET=true`; nextest additionally uses the protocol quiet environment):
  - `{command: "cargo check --workspace --all-targets", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-check.log"}`
  - `{command: "rustfmt --edition 2021 --check --config skip_children=true <scoped files>", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/audit-stable-fmt.log"}`
  - `{command: "cargo clippy -p music-things-synth --all-targets -- -D warnings", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-clippy-synth.log"}`
  - `{command: "cargo clippy -p vactr --all-targets -- -D warnings", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-clippy-vactr.log"}`
  - `{command: "cargo nextest run --workspace", exitStatus: 0, testsRun: 3113, testsPassed: 3113, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-nextest-isolated.log"}`
  - `{command: "cargo nextest list --workspace --message-format oneline; check_test_names.py", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-names.log"}`
  - `{command: "cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-wasm.log"}`
  - `{command: "cargo build synth manifest and package --target wasm32-unknown-unknown", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-crate-wasm.log"}`
  - `{command: "cargo build --examples", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-examples.log"}`
  - `{command: "forbidden-reference grep", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/audit-stable-grep.log"}`
  - `{command: "protected-path git diff", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/final-protected.log"}`
  - `{command: "deleted-source check", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/audit-stable-gone.log"}`
  - `{command: "wc -l <scoped files>", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/audit-stable-lines.log"}`
  - `{command: "git diff a6e5bea -- src/dsp/ported/manifest.rs", exitStatus: 0, testsRun: 0, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/audit-stable-manifest.log"}`
  - `{command: "host-wasm build and protocol vitest", exitStatus: 0, testsRun: 2400, testsPassed: 2399, failureCount: 0, outcome: "passed", log: "tmp/music-things/MTX-S2/stable-vitest-complete.log"}`

# MTX-F3: Cross-mod, banks, shift pair and keyframe mixer -> music-things-fx

**Status**: Ready
**Plan ID**: MTX-F3 (wave W3; runs concurrently with MTX-S3, with no shared files)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` sections 3.3 (group modules), 4.3, 4.4, 5, 6.3 (relocated alloc test)
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

This plan moves the last effect groups: the Warps-style cross-mod family
(10 files tied together with `#[path]`), the resonant and exciter banks,
the shift pair and the four-lane keyframe mixer.

`elements_bank` needs `music_things_synth::elements_ominous` and
`quad_mixer` needs `music_things_synth::frame_lfo`. Both moved in MTX-S1.

One `quad_mixer` test uses vactr's test-only allocation probe. It is
relocated unchanged into vactr's test tree.

## Non-goals

- Moving the dispatch (MTX-90).
- Synth files: MTX-S3 is concurrently editing the synth crate and
  `src/dsp/ugen/*`.
- Notices.

## Dependencies

- dependsOn: MTX-F2 (serial owner of the fx `lib.rs` and
  `effects/mod.rs`), MTX-S1 (`elements_ominous`, `frame_lfo`)
- Blocks: MTX-90

## File-level changes

1. **Moves** from `src/dsp/effects/<f>` to `music-things-fx/src/<f>`,
   deleting the source. Use the same flat names, because the `#[path]`
   attributes must keep resolving:
   - `cross_mod.rs`, `cross_mod_limiter.rs`, `cross_mod_osc.rs`,
     `cross_mod_src.rs`, `cross_mod_vocoder.rs`,
     `cross_mod_vocoder_bank.rs`, `cross_mod_vocoder_boundary.rs`,
     `cross_mod_vocoder_coeffs.rs`, `cross_mod_vocoder_fir.rs`,
     `cross_mod_vocoder_fir_coeffs.rs`
   - `resonant_bank.rs`, `elements_bank.rs`, `shift_pair.rs`,
     `quad_mixer.rs`
2. **Imports.**
   - `super::super::…` in the `cross_mod_*` files: today `super::super`
     is `effects`. In the crate it is the crate root, which re-exports
     `FxCtx`/`FxState`/`ParamDef`/`EffectKind`. For `prim`, use
     `music_things_dsp::prim`.
   - `elements_bank.rs`:
     `crate::dsp::elements_ominous::{sample, STATE_FLOATS}` ->
     `music_things_synth::elements_ominous::{sample, STATE_FLOATS}`.
   - `quad_mixer.rs`: `crate::dsp::ugen::frame_lfo` ->
     `music_things_synth::frame_lfo`.
3. **Signatures.** Apply protocol section 3.2 to every `pub` fn the vactr
   dispatch calls (`PARAMS`, `mem_len`, `init`, `process` per module).
   `quad_mixer::process(p, st, l, r, sr)` and `quad_mixer::PARAMS`
   become `pub`.
4. **Relocated test.** Cut the test fn
   `supports_rates_and_arbitrary_block_sizes_without_allocations`
   (`quad_mixer.rs`, about lines 123-141) out of the crate copy of
   `quad_mixer.rs`. Paste it unchanged as a top-level `#[test] fn` in the
   new `src/dsp/tests/dsp/quad_mixer_alloc.rs`, with imports
   `crate::dsp::effects::quad_mixer::{process, PARAMS}`,
   `crate::dsp::effects::FxState` and `crate::dsp::alloc_probe`. Add
   `mod quad_mixer_alloc;` to `src/dsp/tests/dsp.rs` in alphabetical
   position. The other `quad_mixer` tests stay in the crate copy.
5. **Scrub comments.** `cross_mod.rs:184` ("remain Vactr adaptations" ->
   "remain original adaptations"), `:374` and `:378` ("Vactr extension"
   -> "original extension", "A Vactr-generated" -> "A generated").
6. **Crate `lib.rs`.** Add `pub mod` for `cross_mod`, `resonant_bank`,
   `elements_bank`, `shift_pair` and `quad_mixer`. **vactr
   `effects/mod.rs`.** Change the matching `pub mod` lines to
   `pub use music_things_fx::<m>;`.
7. **Docs** for every public item.

## Existing patterns to imitate

- `#[path]` sub-module layout: `src/dsp/effects/cross_mod.rs:14-27` and
  `cross_mod_vocoder_bank.rs:8` (keep them verbatim).
- Relocated-test style: `src/dsp/tests/dsp/quad_mixer.rs` header and
  imports.

## Pitfalls

- **Build coupling with MTX-S3.** `music-things-fx` depends on
  `music-things-synth`, so while the concurrent MTX-S3 has a module
  mid-move, your cargo, nextest, V-names, wasm and V-grep commands may fail
  in MTX-S3's paths. Record BLOCKED-BY-SIBLING and wait under protocol
  section 5.1 (MTX-S3's `EDITS-COMPLETE`, 15-minute stability, 120-minute
  cap). Never edit MTX-S3's files. Your own moves follow protocol section
  2.1. The ten `cross_mod*` files are one step. Log
  `cargo check -p music-things-fx` and `-p vactr` per step, and append
  `EDITS-COMPLETE <timestamp>` before the gates.
- **`#[path]` resolution.** `#[path]` is relative to the directory of the
  declaring file. All 10 `cross_mod*` files must sit directly in
  `music-things-fx/src/`.
- **Test name and position.** The relocated test must keep its exact fn
  name and stay at module root, not inside `mod tests`. The test-name
  checker maps it to
  `dsp::tests::dsp::quad_mixer_alloc::supports_rates_and_arbitrary_block_sizes_without_allocations`.
- **Nothing left behind.** Do not leave the alloc test in the crate. The
  crate has no `alloc_probe`, and adding a crate allocator is out of
  scope.
- **Off-limits.** Do NOT edit `src/dsp/tests/dsp/{cross_mod,resonant_bank,elements_bank,shift_pair,quad_mixer,quad_outputs}.rs`.

## Tests

- Relocated: `quad_mixer_alloc.rs` (1 test, unchanged body).
- Moved in-file tests (`cross_mod.rs:441` module, the other `quad_mixer`
  tests, and any others) run in the crate binary.
- Every vactr test listed in Pitfalls passes unchanged.

## Verification (protocol section 7; logs under `tmp/music-things-extraction/MTX-F3/`)

1. V-check.
2. V-fmt (all created or modified `.rs` files, including
   `quad_mixer_alloc.rs` and `src/dsp/tests/dsp.rs`).
3. V-clippy: `-p music-things-fx` and `-p vactr`.
4. V-nextest.
5. V-names: `unmatched=0`. The quad_mixer special case must resolve.
6. V-wasm, and V-crate-wasm for `music-things-fx`.
7. V-examples, especially `warps_xmod_reference` and
   `elements_model_reference`.
8. V-grep.
9. V-protected, with the second command showing `+mod quad_mixer_alloc;`.
10. V-gone over the 14 deleted sources.
11. V-lines.
12. V-vitest.

## Completion Criteria

- [ ] 14 files are in the fx crate, and the vactr copies are gone.
- [ ] The alloc test is relocated byte-identical in body and registered.
- [ ] After this plan, `src/dsp/effects/` holds only `mod.rs`,
      `catalog.rs` and `fx_adapter.rs`.
- [ ] Verification 1-12 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

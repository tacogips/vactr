# MTX-S3: Braids shape kernels and voice cores -> music-things-synth

**Status**: Ready
**Plan ID**: MTX-S3 (wave W3; runs concurrently with MTX-F3, with no shared files)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` sections 3.2 (Braids, voice cores), 4.1 (fn-pointer wrappers for `digital_drum`), 4.3 (shim row), 5, 6
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

This plan moves the last synth kernels: the 14 Braids shape kernels and
the self-enveloped voice cores (bass, digital drums, kalimba, tonewheel,
hurdy-gurdy, VOSIM, GENDYN, scanned). The digital-drum entry points are
used as `fn` pointers by `src/dsp/tests/dsp/digital_drum.rs:75` and
`src/dsp/tests/dsp/digital_metal.rs:76`. vactr therefore keeps
`src/dsp/ugen/digital_drum/mod.rs` as a shim with the exact old
signatures.

After this plan, `src/dsp/ugen/` holds only these files:

- engine files: `mod.rs`, `mixer.rs`, `catalog.rs`, `catalog/*`,
  `template.rs`, `build_helpers.rs`;
- shims and adapters: `sample.rs`, `wavetable.rs`, `va_filter.rs`,
  `stage_chain.rs`, the seven pair shims, `dual_kick.rs`, `snare_pair.rs`,
  `hat_pair.rs`, `voice_layer.rs`, `vactrol_gate.rs` and
  `digital_drum/mod.rs`.

## Non-goals

- Changing `build_helpers.rs`. Its `super::{gendyn,hurdy_gurdy,kalimba,scanned,tonewheel,vosim}::…`
  references resolve through the re-exports.
- Changing the template or catalog.
- Effects.
- Notices.

## Dependencies

- dependsOn: MTX-S2 (serial owner of the synth `lib.rs` and `ugen/mod.rs`)
- Blocks: MTX-91

## File-level changes

Moves go from `src/dsp/ugen/<f>` to
`music-things/crates/music-things-synth/src/<f>`. The vactr source is
deleted, and `ugen/mod.rs` changes `pub mod` to
`pub use music_things_synth::<m>;`, unless the file is noted as a shim.

1. **Braids.** `braids_cloud.rs`, `braids_digital.rs`, `braids_filter.rs`,
   `braids_five.rs`, `braids_fm.rs`, `braids_formant.rs`,
   `braids_noise.rs`, `braids_percussion.rs`, `braids_physical.rs`,
   `braids_struck.rs`, `braids_subsync.rs`, `braids_triple.rs`,
   `braids_wave_bank.rs`, `braids_wave_line.rs`.
2. **Voice cores.**
   - `bass_voice.rs` with `bass_voice/{diode,fm,ladder,mods,osc}.rs`
   - `kalimba.rs`, `tonewheel.rs`
   - `hurdy_gurdy.rs` with `hurdy_gurdy/{bowed,buzz}.rs`
   - `vosim.rs`, `gendyn.rs`, `scanned.rs`
3. **`digital_drum`.** Move `digital_drum/mod.rs`, `digital_drum/chain.rs`,
   `digital_drum/metal.rs` and `digital_drum/tonal.rs` to
   `music-things-synth/src/digital_drum/` (mod-rs layout kept).
   - Delete vactr `chain.rs`, `metal.rs` and `tonal.rs`.
   - Rewrite vactr `src/dsp/ugen/digital_drum/mod.rs` as a shim:
     - `pub use music_things_synth::digital_drum::*;`
     - forwarding `render_drum` and `render_snare`, with today's exact
       signatures (`kx: &Kx<'_>`);
     - an inline `pub mod metal { pub use music_things_synth::digital_drum::metal::*; … }`
       with forwarding `render_metal` and `render_hat`, using exact
       signatures. Import `Inp`, `Kx`, `NodeState` and `MAX_PORTS` from
       `crate::dsp::ugen` inside the inline module (vactr side, so
       `crate::` is fine there).
   - Keep `pub mod digital_drum;` in `ugen/mod.rs`.
4. **Imports and generics** per protocol section 3.1-3.2.
   `digital_drum/{chain,metal,tonal}.rs` use `super::super::{…}`.
   Inside the crate that is the crate root, which is correct.
5. **Docs** for all public items. The voice cores already have
   `PORTS` and `port::*` tables. Braids entry docs list their ports from
   `src/dsp/ugen/catalog/voice_ports.rs`.

## Existing patterns to imitate

- Shim shape: protocol section 4 and the MTX-S2 `dual_kick.rs` shim.
- Nested-module shim: an inline `pub mod` with a glob plus wrappers.

## Pitfalls

- **Build coupling with MTX-F3.** `music-things-fx` depends on
  `music-things-synth`, and vactr depends on both. While one of your
  modules is mid-move, every cargo command of the concurrent MTX-F3
  fails. Follow protocol section 2.1: one module (with its submodule
  directory) at a time, with `cargo check -p music-things-synth --all-targets`
  and `cargo check -p vactr --all-targets` logged per module. Append
  `EDITS-COMPLETE <timestamp>` when done, then run the gates. A failure
  located only in MTX-F3's paths follows protocol section 5.1.
- **Wrapper signatures.** `render_drum` and `render_snare` take `mem`.
  Check today's signatures at `src/dsp/ugen/digital_drum/mod.rs` and
  `tonal.rs`, and copy them exactly. The test `Render` type is
  `fn(&[Inp<'_>; MAX_PORTS], &mut NodeState, &mut [f32], &mut [f32], &Kx<'_>)`.
- **Glob shadowing.** In the shim, the glob re-export includes the
  crate's `metal` module, and the local inline `pub mod metal` shadows
  it. Do not also add `pub use …::metal;`.
- **`bass_voice` nesting.** `bass_voice/mods.rs` (432 lines) and its
  siblings use `super::{rate_pole, soft_clip, svf}`. Keep the structure
  identical.
- **Off-limits.** Do not touch `src/dsp/tests/dsp/{digital_drum,digital_metal,bass_*,kalimba,braids_*}.rs`.

## Tests

- No new tests. The moved in-file tests run in the crate binary. The
  vactr tests listed above, the golden templates and the examples pass
  unchanged.

## Verification (protocol section 7; logs under `tmp/music-things-extraction/MTX-S3/`)

1. V-check.
2. V-fmt (all created or modified `.rs` files).
3. V-clippy: `-p music-things-synth` and `-p vactr`.
4. V-nextest.
5. V-names: `unmatched=0`.
6. V-wasm, and V-crate-wasm for `music-things-synth`.
7. V-examples, especially `braids_shapes_reference`,
   `peaks_drums_reference` and `plaits_physical_reference`.
8. V-grep.
9. V-protected.
10. V-gone over the deleted sources.
11. V-lines.
12. V-vitest.
13. `ls src/dsp/ugen src/dsp/ugen/*/`: equals the file list in Intent and
    Context above. Log it.

## Completion Criteria

- [ ] 32 files have moved (14 Braids, 6 bass, 4 digital_drum, 3
      hurdy_gurdy, kalimba, tonewheel, vosim, gendyn, scanned). The vactr
      `digital_drum/mod.rs` shim keeps exact signatures.
- [ ] `digital_drum.rs` and `digital_metal.rs` tests compile and pass
      unedited.
- [ ] Verification 1-13 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

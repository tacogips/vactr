# MTX-S3: Braids shape kernels and voice cores -> music-things-synth

**Status**: Completed (accepted in session 605, 2026-11-01)
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

## Verification (protocol section 7; logs under `tmp/music-things/MTX-S3/`)

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

- [x] 32 files have moved (14 Braids, 6 bass, 4 digital_drum, 3
      hurdy_gurdy, kalimba, tonewheel, vosim, gendyn, scanned). The vactr
      `digital_drum/mod.rs` shim keeps exact signatures.
- [x] `digital_drum.rs` and `digital_metal.rs` tests compile and pass
      unedited.
- [x] Verification 1-13 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

### Session 2026-11-01 (MTX-S3 implementation)
- Moved 32 files into music-things-synth, preserving the digital drum signature shims. Detailed per-edit preimages and intentions: `tmp/music-things/MTX-S3/edit-intentions.jsonl`.
- Edits: `music-things/crates/music-things-synth/src/braids_cloud.rs` pre=missing post=bb4d692678fd4ffd7b55f229f522e0ccc6dba00f4252a0eb065058d4de47cf18 intent=Copy kernel; adapt context, exported helpers, and port docs without changing DSP
- Edits: `music-things/crates/music-things-synth/src/lib.rs` pre=4afe1b0e988543475b5d2358997ebeccf818e19c1b1262a5effb822020fe532d post=8bae63668bac429f958ae2add1a0894c5231741dc2f8e5e2c81fbeac4dcd7bb0 intent=Expose braids_cloud
- Edits: `src/dsp/ugen/mod.rs` pre=f09140b518f99d61734009a1e0e3585b76a705c5e2f132fb6f7d8ede5148bae8 post=cbf9af93b4ed1b817169a523bc4fe38691254e82284ca8b23a60ed7617b62664 intent=Re-export braids_cloud
- Edits: `src/dsp/ugen/braids_cloud.rs` pre=980bcba39305a04f4542846804b312d0bae5dd676fe70d60a43c51a5528d5667 post=missing intent=Remove moved braids_cloud implementation
- Edits: `music-things/crates/music-things-synth/src/braids_digital.rs` pre=missing post=c36ba25e95729f42f8031f54ceb6817de6b25f811ca2193dba0365ee640bcc0a intent=Copy kernel; adapt context, exported helpers, and port docs without changing DSP
- Edits: `src/dsp/ugen/braids_digital.rs` pre=87a286cdfa6ca62ebca8763720ce426acfd95f94b5ecf8e13cc4b2c1985ef5bf post=missing intent=Remove moved braids_digital implementation
- Edits: `music-things/crates/music-things-synth/src/braids_filter.rs` pre=missing post=09d49db45d85591d7f16d1f7d9d42ac278d411d7d7be3889504b4289ea44b628 intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_filter.rs` pre=6ca0796033a0907bd3b9e85fc5c5f099e21934e8c13aedbe50c91689b4868c7e post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_five.rs` pre=missing post=a25e6a9373cfc0bc85434ea8fa9904fe2aa27ea936d02ab9d78510171bd2b71c intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_five.rs` pre=5951294e0f190d4b1b6c98993e589e4858241196976a283bfcdafba19f95703c post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_fm.rs` pre=missing post=21af5c3872c2741571a62d4a52c39d28678dc36518a32eab4a77f39755559347 intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_fm.rs` pre=25da0d57b851f70e980cd0f53d3717b014e8489c67884210af8042ac4722d2c4 post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_formant.rs` pre=missing post=eb3aa75462832112154c818bbb8d3980e00433dd835a9b9935e5fd8d863f53fc intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_formant.rs` pre=3d9f4ac0f1f11397f8830d41b2faacccf2fb597e990af75f5833cec9c5848453 post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_noise.rs` pre=missing post=66760d6f3cfb3eafb6323f6aba33dcd84be03b1806fa2cdcb9c046571c37c093 intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_noise.rs` pre=6a49351624359d58c337ec43a1b922f1ced1fa1ef7cd88e0464e5a2e0102feef post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_percussion.rs` pre=missing post=6276f706b9030e72d540c62281c12668611a26f04756710771c5dcf00fa8fca8 intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_percussion.rs` pre=cfec13bd3c85d069f5850c15e08d6b1c937d8f5c7579d3cdf6eed33b3f99750e post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_physical.rs` pre=missing post=d0b506cb8c9a681260e55f31831122e8af0956c5c214060650c09603eb3e5d79 intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_physical.rs` pre=809c305c98d42aa2302c41247c9505d7a70cddb830d58f1834da025a273605c9 post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_struck.rs` pre=missing post=74f04f585221be21f16836db8d924d755624c1b8dc300dc570eb131041e2c0ac intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_struck.rs` pre=ff9353a53fa57504a322d5bc3fad4a3816f538431d387e0e2838b2d12ac0c282 post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_subsync.rs` pre=missing post=4e6dd1b72d579be6800cbf30a0e34109bb7672181137da641e6b530b7fb5c3ec intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_subsync.rs` pre=b526ccc4cbc5d2685b3e19cfc77cd2911459f9cb692c895e951e2c9127c8cecc post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_triple.rs` pre=missing post=3b6ef003ef8fd48061d18176f60b13f714cb3aab88629d28dc0a037247ef8a3b intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_triple.rs` pre=7b4ac269415f71a703f85b8e065386514c2f9f11c661d56b1d1887d3519f1820 post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_wave_bank.rs` pre=missing post=21060b83a1f112ed7baa6c366822cf1d62cc0a0a788a8253ec3ff03d7ed800ff intent=Move kernel with generic context, helper visibility and port docs
- Edits: `src/dsp/ugen/braids_wave_bank.rs` pre=adaae7b0587087c2fe0fc988f30be4a47a474cd7e48a957d2f34d8034ce29433 post=missing intent=Delete moved source
- Edits: `music-things/crates/music-things-synth/src/braids_wave_line.rs` pre=missing post=91b4d2d2449766a4c8a72ec241ad2e601e4e2cb2dc5b082c7916ba1d389ed982 intent=Move with context, visibility and port docs
- Edits: `src/dsp/ugen/braids_wave_line.rs` pre=e2d811e7c9c9e77ac4d8d5acda0cdc44fbff6f106ce901e88a563f8d7028ac74 post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/bass_voice.rs` pre=missing post=6289e9d7c60ac3df6a3acb206c1f2705eaf9b433ece1c485d847192e143654ee intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `music-things/crates/music-things-synth/src/bass_voice/fm.rs` pre=missing post=5910ef3c7b339c0f471138a591154b441b69ddacf87584755d31e0f5ec1f7124 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `music-things/crates/music-things-synth/src/bass_voice/osc.rs` pre=missing post=9862866f908c8b69b26e1d1ca120ec96c01abdf8e8d7562f9ef59cdd438a92e5 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `music-things/crates/music-things-synth/src/bass_voice/ladder.rs` pre=missing post=6ad306a26b662f5e1f791c2066f57d95fb9b0f0ee17c0e1bf3dd302b9b14c0d3 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `music-things/crates/music-things-synth/src/bass_voice/mods.rs` pre=missing post=0004d1f8712d879cff6410881b617787f2aad4abeb744ebf36a6b2f312e8efe1 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `music-things/crates/music-things-synth/src/bass_voice/diode.rs` pre=missing post=fa4525d4830987b30195bd99f2b42c85fa25bf59267d15dc7357b2b85cacd8ce intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `src/dsp/ugen/bass_voice.rs` pre=5b5f121e6b75707b9ec9aa93f448070a18b76a80e45ca4df0b4205989cf84ce2 post=missing intent=Delete moved implementation
- Edits: `src/dsp/ugen/bass_voice/fm.rs` pre=5910ef3c7b339c0f471138a591154b441b69ddacf87584755d31e0f5ec1f7124 post=missing intent=Delete moved implementation
- Edits: `src/dsp/ugen/bass_voice/osc.rs` pre=73eb4fb905ea8c2ea18b2bf1e5619d0550c8f6ef56cde6eaef4cae0703430646 post=missing intent=Delete moved implementation
- Edits: `src/dsp/ugen/bass_voice/ladder.rs` pre=6ad306a26b662f5e1f791c2066f57d95fb9b0f0ee17c0e1bf3dd302b9b14c0d3 post=missing intent=Delete moved implementation
- Edits: `src/dsp/ugen/bass_voice/mods.rs` pre=2702a49d3c3112897858ceddefc1e15e0bc1caabba2213caf78b978a8d19942e post=missing intent=Delete moved implementation
- Edits: `src/dsp/ugen/bass_voice/diode.rs` pre=fa4525d4830987b30195bd99f2b42c85fa25bf59267d15dc7357b2b85cacd8ce post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/kalimba.rs` pre=missing post=ff285ab2d38a2dc6f3dc6d83843a7e651f9894319d60605cd77bea3c73175ec5 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `src/dsp/ugen/kalimba.rs` pre=f5d1900fcd4861a592bcec4f87a4227a2ea7b309ef73d8df5f0c0d888e51b66c post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/tonewheel.rs` pre=missing post=056fb6998420e0228994dab8c46b70f0884f9bb831374c8b32de26e5a7d9c182 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `src/dsp/ugen/tonewheel.rs` pre=65c16cfdc4592dab0a3ab455a7d20bf24cf1892f2a885e2daeb1e587d59fa065 post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/hurdy_gurdy.rs` pre=missing post=e8d2fd9322fa56fb24837fcce3bfdcf62b3af1d311bc0b5e69636d29831b7d28 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `music-things/crates/music-things-synth/src/hurdy_gurdy/buzz.rs` pre=missing post=6b021d37635d403946bf161f3526ca629dcb64a3f30d204159efdab01001ad08 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `music-things/crates/music-things-synth/src/hurdy_gurdy/bowed.rs` pre=missing post=02d78f34b8eb8489159eb1333381b68d4a264306d41af3625f36fede53565438 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `src/dsp/ugen/hurdy_gurdy.rs` pre=cfd6b3622f957c1ba3d0a4f7978c503e3b33a50ca261b5bfa1964380c6bc1979 post=missing intent=Delete moved implementation
- Edits: `src/dsp/ugen/hurdy_gurdy/buzz.rs` pre=6b021d37635d403946bf161f3526ca629dcb64a3f30d204159efdab01001ad08 post=missing intent=Delete moved implementation
- Edits: `src/dsp/ugen/hurdy_gurdy/bowed.rs` pre=02d78f34b8eb8489159eb1333381b68d4a264306d41af3625f36fede53565438 post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/vosim.rs` pre=missing post=dcb5e0ab342c3478f1d167894c6d0d3ea8afddbb79db9470ab2323e7af623dcf intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `src/dsp/ugen/vosim.rs` pre=6783b9e49397b910832b5f96fc1843d2723e378f1378f30118caa864fd7dfac7 post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/gendyn.rs` pre=missing post=1391c103ef9fe3d77f165b05780fb006af4c41eb97cc60f7ee26ec5021609bda intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `src/dsp/ugen/gendyn.rs` pre=4094e913fa34bfd59bd9f251d835cb173673d00822451327507a40df1b4da03f post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/scanned.rs` pre=missing post=989c1478952d73e2dc9f1677ea7a2921537d441bd72a371d5a38a25e88e72c29 intent=Move source; adapt imports, context and public test support without altering DSP
- Edits: `src/dsp/ugen/scanned.rs` pre=fb566658c7f3ef5bb5fd8a91bc63bba08cf6d4c01349d0b5a6625966310ebd70 post=missing intent=Delete moved implementation
- Edits: `music-things/crates/music-things-synth/src/digital_drum/mod.rs` pre=missing post=823320c5420fa05303472cca4e969ae552e280cf0a68d38e09f9ab4e7c3d2e7c intent=Move digital drum source, adapt imports and context without changing DSP
- Edits: `music-things/crates/music-things-synth/src/digital_drum/chain.rs` pre=missing post=513c3e2dba2656b279fadd729773de48fd4303a2b6399e7a3086856367db930f intent=Move digital drum source, adapt imports and context without changing DSP
- Edits: `music-things/crates/music-things-synth/src/digital_drum/metal.rs` pre=missing post=34dc48cc96004a91d6838063972eb47a690a4d75bb7e74a2fed1a62a75c50d34 intent=Move digital drum source, adapt imports and context without changing DSP
- Edits: `music-things/crates/music-things-synth/src/digital_drum/tonal.rs` pre=missing post=20c4001861366ceb2991cc3c34ebc63b1f2930bb0c724404ef657e4cb644df68 intent=Move digital drum source, adapt imports and context without changing DSP
- Edits: `src/dsp/ugen/digital_drum/mod.rs` pre=823320c5420fa05303472cca4e969ae552e280cf0a68d38e09f9ab4e7c3d2e7c post=82558928f4e571c666f0584639abab1616d4f3a6d2aa17b3c3327e051d37ae07 intent=Replace implementation with four exact function-pointer forwarding shims
- Edits: `src/dsp/ugen/digital_drum/chain.rs` pre=09dc4b529ce6fa440081ab884564618bfd09d5c2edd3c2bfe75f4c8da9f44ae5 post=missing intent=Delete moved digital drum implementation
- Edits: `src/dsp/ugen/digital_drum/metal.rs` pre=bd73229f56f1dfcb1e1c1f27be0bb65369c61d536679bc9872801ff3675b5122 post=missing intent=Delete moved digital drum implementation
- Edits: `src/dsp/ugen/digital_drum/tonal.rs` pre=fcd75c07efc92b430070bea657a6abe8e341c9a44232c4be7919f995883f3cd5 post=missing intent=Delete moved digital drum implementation
- BLOCKED-BY-SIBLING attempts are recorded in `tmp/music-things/MTX-S3/sibling-blockers.log`; final checks require retries.
- EDITS-COMPLETE 2026-11-01T05:44:37+09:00
- Verification: V-check exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-check.log`.
- Verification: V-fmt exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-fmt.log`.
- Verification: V-clippy synth exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-clippy-synth.log`.
- Verification: V-clippy vactr exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-clippy-vactr.log`.
- Verification: V-names (baseline=3108, after=3113, unmatched=0) exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-names.log`.
- Verification: V-wasm exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-wasm.log`.
- Verification: V-crate-wasm manifest exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-crate-wasm-manifest.log`.
- Verification: V-crate-wasm package exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-crate-wasm-package.log`.
- Verification: V-examples exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-examples.log`.
- Verification: V-grep exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-grep.log`.
- Verification: V-protected exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-protected.log`.
- Verification: V-gone exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-gone.log`.
- Verification: V-lines exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-lines.log`.
- Verification: V-list exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-ls.log`.
- Verification: V-vitest (2399 passed, 1 skipped) exitStatus=0, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-vitest.log`.
- V-nextest remains in progress; do not treat it as passed until the terminal summary and exit are recorded.
- Verification: `CARGO_TARGET_DIR=target/MTX-S3 CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --workspace`, exitStatus=0, testsRun=3113, testsPassed=3113, failureCount=0, outcome=passed, log=`tmp/music-things/MTX-S3/final-nextest.log`, notes=3 baseline skips.
- The previous in-progress nextest note above is superseded by the terminal result. Final source inventory matched (`tmp/music-things/MTX-S3/source-drift-final.log`: 1643 files, drift=0). The blocked vactr check retry passed (`tmp/music-things/MTX-S3/sibling-vactr-retry.log`). Formal integrity, adversarial and integration reviews remain downstream.

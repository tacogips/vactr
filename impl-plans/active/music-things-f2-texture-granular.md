# MTX-F2: Texture, granular, stream and analyzer groups -> music-things-fx

**Status**: Ready
**Plan ID**: MTX-F2 (wave W2; runs concurrently with MTX-S2, with no shared files)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` sections 3.3 (`granular`, `texture_quality`, `GrainLimits`), 4.3 (`src/dsp/granular.rs` row, `effects/mod.rs` row), 4.4 (`StreamOpto`, `opto`), 5, 6
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

This plan moves the four Clouds-style texture adaptations with their
shared quality conversion, the granular engine (serving both the
`granulate` effect and the `granular` ugen), the Streams-role
stream-control groups and the analyzers into the fx crate.

`src/dsp/granular.rs` stays at its path as a vactr adapter. It keeps the
`&CapabilitySet` signatures used by `ugen/build_helpers.rs`,
`ugen/mixer.rs:141` and the `effects/mod.rs` dispatch, and it resolves
`GranSrc` resources through the `SampleStore`.

## Non-goals

- Moving the dispatch (MTX-90).
- Changing `FxUnit`'s logic. The only change is to how `texture_quality`
  is imported.
- The other groups.
- Synth files: MTX-S2 is concurrently editing the synth crate and
  `src/dsp/ugen/*`.

## Dependencies

- dependsOn: MTX-F1 (serial owner of the fx `lib.rs` and
  `effects/mod.rs`)
- Blocks: MTX-F3

## File-level changes

1. **Moves** from `src/dsp/effects/<f>` to `music-things-fx/src/<f>`,
   deleting the source:
   - `texture.rs`, `texture_quality.rs`, `texture_loop.rs`,
     `texture_stretch.rs`, `texture_spectral.rs`
   - `dynamic_control.rs`, `stream_dynamics.rs`, `stream_cv.rs`,
     `analyzer.rs`
2. **`src/dsp/granular.rs` -> `music-things-fx/src/granular.rs`.** In the
   crate file:
   - `&CapabilitySet` becomes `&GrainLimits` in `pool_slots`,
     `capture_len`, `ugen_mem_len`, `effect_mem_len`, `split` and
     `effect_init`.
   - `Ctl::read(…, caps, stats)` takes `&GrainLimits`, and the field
     names are unchanged.
   - `effect_process` is generic over `FxContext`, reads `ctx.limits`
     and `ctx.stats`, and is otherwise verbatim.
   - New `pub fn ugen_process<C: KernelContext + ?Sized>(view: SampleView<'_>, ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, mem: &mut [f32], out: &mut [f32], kx: &C, limits: &GrainLimits, stats: &mut FxStats)`.
     Its body is today's `ugen_process` from
     `let mut c = Ctl::read(|i| ins[i].first(), …)` (line 530) to the end,
     with `kx.caps` -> `limits` and `kx.stats` -> `stats`.
   - Imports: `music_things_dsp::{prim::{pan_gains, Rng}, SampleView}`,
     `music_things_synth::{Inp, KernelContext, KernelCtx, NodeState, MAX_PORTS}`,
     and `super::{FxCtx, FxContext, FxState, FxStats, GrainLimits, ParamDef}`.
3. **vactr `src/dsp/granular.rs` adapter** (under 80 lines, no DSP).
   - `pub use music_things_fx::granular::{PARAMS, GRAIN_FLOATS, SHORT_GATE};`
     plus any other `pub const` vactr uses.
   - Wrappers with today's exact signatures: `pool_slots(caps)`,
     `capture_len(sr, caps)`, `ugen_mem_len(sr, caps)`,
     `effect_mem_len(sr, caps)` and `effect_init(st, mem, sr, caps)`.
     Each calls the crate with `&grain_limits(caps)` (from
     `crate::dsp::effects::grain_limits`).
   - `effect_process(p, st, mem, l, r, ctx: &mut FxCtx<'_>)` forwards.
   - `ugen_process(src: GranSrc, ins, st, mem, out, kx: &mut Kx<'_>)`
     follows today's order exactly:
     1. Match `src`: `Sample(b)` -> `kx.bank.unwrap_or(b.get())`;
        `Table(t)` -> `t.get()`; `Bus` -> `out.fill(0.0); return`.
     2. `let Some(view) = kx.store.get(resource) else { out.fill(0.0); return; }`
     3. `let kc = kx.kernel_ctx();` and
        `let limits = grain_limits(kx.caps);`
     4. Call the crate `ugen_process(view, ins, st, mem, out, &kc, &limits, &mut *kx.stats)`.
4. **`texture_quality.rs`.**
   - Make the module `pub`, and `pub fn mono(kind: impl Into<EffectKind>, vals: &[f32]) -> bool`
     (keep today's exact parameters, with the kind prologue added).
   - In vactr `effects/mod.rs`, change `mod texture_quality;` to
     `use music_things_fx::texture_quality;`. `FxUnit::run_with_key` at
     line 749 stays textually unchanged.
   - Scrub the "Vactr's low-fidelity path" comment (line 4) to "This
     low-fidelity path".
5. **`dynamic_control.rs`.**
   - Match arms `EffectKind::StreamVactr` become `EffectKind::StreamOpto`.
   - `fn vactr(` becomes `fn opto(`, at its definition (line 89) and its
     call site (line 137).
6. **`analyzer.rs`.**
   - `crate::dsp::fft::{Fft, FFT_SIZE}` -> `music_things_dsp::fft::{…}`.
   - `crate::dsp::graph::{AnalyzerKind, EffectKind}` -> `super::{AnalyzerKind, EffectKind}`.
   - In-file tests that build `FxCtx` use the crate `FxCtx` with
     `samples: &NoSamples`, a `GrainLimits` built from the same
     `CapabilitySet::browser()` values the test used before (copy the
     three numbers from `src/dsp/caps.rs` `browser()`), and a crate
     `FxStats`. No assertion change.
7. **`texture_spectral.rs`.** `crate::dsp::fft::FFT_SIZE` ->
   `music_things_dsp::fft::FFT_SIZE`.
8. **Scrub `texture_loop.rs:3`** ("Vactr retains") per protocol section
   3.5.
9. **Crate `lib.rs`.** Add `pub mod` for the 10 modules. **vactr
   `effects/mod.rs`.** `pub mod <m>;` -> `pub use music_things_fx::<m>;`
   for the 9 effects modules. `src/dsp/mod.rs` keeps `pub mod granular;`.
10. **Docs** for every public item.

## Existing patterns to imitate

- Adapter shape: `src/dsp/ugen/sample.rs`, as rewritten by MTX-S1.
- Kind mirror conversions: `src/dsp/effects/fx_adapter.rs` (MTX-00).

## Pitfalls

- **Build coupling with MTX-S2.** `music-things-fx` depends on
  `music-things-synth`, so while the concurrent MTX-S2 has a module
  mid-move, your cargo, nextest, V-names, wasm and V-grep commands may fail
  in MTX-S2's paths. Record BLOCKED-BY-SIBLING and wait under protocol
  section 5.1 (MTX-S2's `EDITS-COMPLETE`, 15-minute stability, 120-minute
  cap). Never edit MTX-S2's files. Your own moves follow protocol section
  2.1. Move the five `texture*` files as one step, because they import
  each other. Move `granular` together with its `src/dsp/granular.rs`
  adapter. Log `cargo check -p music-things-fx` and `-p vactr` per step,
  and append `EDITS-COMPLETE <timestamp>` before the gates.
- **Borrow conflict.** Passing `&*kx` and `&mut *kx.stats` together does
  not compile. Build `kc` by value first, as in step 3.3.
- **Stats increment order.** Do not move `Ctl::read` before the resource
  lookup. Today a missing resource skips `Ctl::read` and so does not bump
  `stats.clamped`. That must stay true.
- **Caps conversion.** `grain_limits(caps)` must copy the exact `f32`
  fields. No arithmetic.
- **Existing test.** `src/dsp/tests/dsp/dynamic_control.rs` calls
  `dynamic_control::process(kind, …)` with vactr
  `EffectKind::StreamVactr`. That works through `From` (-> `StreamOpto`)
  and must not be edited.
- **Off-limits.** `src/dsp/tests/dsp/{texture*,dynamic_control,stream_*,analyzer,shift_pair}.rs`.

## Tests

- No new tests. The moved in-file tests run in the crate binary (analyzer
  and any others).
- `src/dsp/tests/dsp/{texture,texture_loop,texture_quality,texture_spectral,texture_stretch,dynamic_control,stream_cv,stream_dynamics,analyzer}.rs`
  and the granular template tests pass unchanged.

## Verification (protocol section 7; logs under `tmp/music-things/MTX-F2/`)

1. V-check.
2. V-fmt (all created or modified `.rs` files).
3. V-clippy: `-p music-things-fx` and `-p vactr`.
4. V-nextest.
5. V-names: `unmatched=0`.
6. V-wasm, and V-crate-wasm for `music-things-fx`.
7. V-examples, especially `clouds_texture_reference`.
8. V-grep.
9. V-protected.
10. V-gone over the 9 deleted effects files.
11. V-lines: `src/dsp/granular.rs` is under 80 lines.
12. V-vitest.

## Completion Criteria

- [ ] 10 modules are in the fx crate. `src/dsp/granular.rs` is an
      adapter with no DSP loops.
- [ ] `StreamOpto`/`opto` renames are done. No banned tokens.
- [ ] `FxUnit::run_with_key` is textually unchanged.
- [ ] Verification 1-12 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

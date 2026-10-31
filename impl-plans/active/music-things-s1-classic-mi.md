# MTX-S1: Classic, FM and Mutable Instruments role kernels -> music-things-synth

**Status**: Ready
**Plan ID**: MTX-S1 (wave W1; runs concurrently with MTX-F1, with no shared files)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` sections 3.2 (classic and FM list, other Mutable Instruments roles list, `sample`/`wavetable`), 4.1 (fn-pointer wrappers for `va_filter` and `stage_chain`), 4.3, 4.4, 5, 6
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

This plan moves 27 synth kernel modules (34 files, ~6000 lines) into
`music-things/crates/music-things-synth/src/`, together with their in-file
tests. vactr reaches them through `pub use` re-exports, or through thin
shims and adapters where design 4.1 and 4.3 require one. Audio must stay
bit-identical.

MTX-00 already provides `Inp`, `NodeState`, `KernelCtx`, `KernelContext`
and `MAX_PORTS` in the crate root, and `impl KernelContext for Kx` in vactr.

## Non-goals

- Other kernels: the pairs and LPG are MTX-S2; Braids and voice cores are
  MTX-S3.
- Effects. Any change to `catalog.rs`, `catalog/`, `template.rs` or
  `build_helpers.rs`.
- Notices (MTX-91).
- Refactoring, renaming (no banned tokens occur in these files except in
  comments) and documentation beyond `missing_docs`.

## Dependencies

- dependsOn: MTX-00
- Blocks: MTX-S2 (serial owner of `music-things-synth/src/lib.rs` and
  `src/dsp/ugen/mod.rs`), MTX-F3 (needs `frame_lfo` and `elements_ominous`)

## File-level changes

Moves go from `src/dsp/ugen/<f>` to
`music-things/crates/music-things-synth/src/<f>`, keeping the same relative
path.

1. **Plain moves.** The vactr source is deleted. `src/dsp/ugen/mod.rs`
   changes `pub mod <m>;` to `pub use music_things_synth::<m>;`.
   - `osc.rs`, `filter.rs`, `env.rs`, `additive.rs`
   - `fm.rs` with `fm/algorithms.rs`, `fm/engine.rs`, `fm/envelope.rs`,
     `fm/patch.rs`, `fm/scaling.rs`, `fm/sysex.rs`
   - `musicdsp_synth.rs` with `musicdsp_synth/chaos.rs`
   - `phase_pair.rs`, `analog_percussion.rs`, `feedback_metal.rs`,
     `fm_drum.rs`, `fusion_drum.rs`
   - `rings_part.rs`, `string_choir.rs`, `elements_internal.rs`,
     `tidal_function.rs`, `tidal_poly.rs`, `peak_function.rs`,
     `peak_pulse.rs`, `stage_segment.rs`, `stage_linked.rs`,
     `frame_lfo.rs`, `frame_keyframe.rs`
2. **`src/dsp/elements_ominous.rs`.** Move it to
   `music-things-synth/src/elements_ominous.rs`. In `src/dsp/mod.rs`,
   change `pub mod elements_ominous;` to
   `pub use music_things_synth::elements_ominous;`.
   `elements_internal.rs` then imports `super::elements_ominous::…`.
3. **Fn-pointer shims.** These vactr files stay and are rewritten as
   shims per protocol section 4. The crate gets the moved full module.
   - `src/dsp/ugen/va_filter.rs`: wrappers `filter` and `filter_pair`,
     with signatures exactly as today:
     `filter(ins, st, out, kx: &Kx<'_>)` and
     `filter_pair(ins, st, main, aux, kx: &Kx<'_>)`.
   - `src/dsp/ugen/stage_chain.rs`: wrappers `render` and `render_pair`,
     with today's signatures.
   - Re-read each original signature before writing the wrapper.
     `src/dsp/tests/dsp/kernel_pairs.rs:16-20` coerces them to
     `for<'a> fn(..., &Kx<'a>)`.
4. **Resource adapters (design 3.2 and 4.3).**
   - Crate `sample.rs`:
     - `frame_region` and `region`, verbatim;
     - `pub fn play_view<C: KernelContext + ?Sized>(view: Option<SampleView<'_>>, ins, st, out, kx: &C)`;
     - `pub fn play_stereo_view<C>(view, ins, st, mono, left, right, kx: &C)`;
     - both wrap a private `play_worker_view`. That is today's
       `play_worker` (`src/dsp/ugen/sample.rs:78-…`) from
       `let Some(view) = …` onward, where the `None` arm is exactly the
       current else-branch (zero `out`, zero `left`/`right`, then
       `st.finish()`).
   - vactr `src/dsp/ugen/sample.rs` becomes an adapter:
     - re-export `frame_region` and `region`;
     - keep `pub fn play(bank: BankRef, ins, st, out, kx: &Kx<'_>)` and
       `play_stereo(bank, ins, st, mono, left, right, kx: &Kx<'_>)`;
     - each computes `kx.store.get(kx.bank.unwrap_or(bank.get()))` and
       calls the crate `_view` fn.
   - Crate `wavetable.rs`: `pub const FRAME` and
     `pub fn render_view<C>(view: Option<SampleView<'_>>, ins, st, out, kx: &C)`.
   - vactr `src/dsp/ugen/wavetable.rs` keeps
     `render(table: TableRef, ins, st, out, kx: &Kx<'_>)`, resolving with
     `kx.bank.unwrap_or(table.get())`, and re-exports `FRAME`.
   - Resolution order must stay: compute the resource id, look it up,
     then render. A missing resource produces exactly the outputs and
     state it produces today.
5. **Import rewrites inside moved files.**
   - `use crate::dsp::effects::prim::X` -> `use music_things_dsp::prim::X`
   - `use super::{Inp, Kx, NodeState, MAX_PORTS}` ->
     `use super::{Inp, KernelContext, KernelCtx, NodeState, MAX_PORTS}`
     (drop the names you do not use)
   - In `fm/*.rs`, `super::super::{Inp, Kx, NodeState, MAX_PORTS}` gets
     the same treatment.
6. **Generic entry points.** Apply this to every `pub fn` that takes
   `kx: &Kx<'_>` or `&mut Kx<'_>`, including:
   - `osc::{sine,saw,pulse,tri,sub,noise,vco}`, `filter::{biquad,ladder,svf,delay,comb}`, `env::{perc,adsr,line}`
   - `fm::{op,modulate_with_algorithm,phase_distortion}`, `fm::engine::fm6_render`
   - `stage_linked::render`, `musicdsp_synth::{dsf,chebyshev,gaussian,lorenz,rossler,am_formant}`
   - each `render` of the moved modules, and `va_filter::{source, filter, filter_pair}`
   - `frame_keyframe::render` takes no `kx`. It is unchanged.
7. **`src/dsp/ugen/mod.rs`.** Make only the `pub mod` -> `pub use` changes
   for the modules above. The kept files `va_filter`, `stage_chain`,
   `sample` and `wavetable` keep `pub mod`.
8. **`src/dsp/ugen/mixer.rs` and `src/dsp/voice.rs`.** Change these only
   if a call `x::f(…, kx)`, with `kx: &mut Kx`, fails to infer `C`. Then
   change that argument to `&*kx` and record each changed line. No other
   edit.
9. **Docs.** Document every public item of the moved modules. The docs
   of `osc`, `filter` and `env` entry points list their port indices,
   taken from `src/dsp/ugen/catalog.rs` and `catalog/voice_ports.rs`.
   Scrub the comments that mention "Vactr" (protocol section 3.5); in
   this plan that is `elements_ominous.rs:4` and `fm_drum.rs:5`.

## Existing patterns to imitate

- Generic prologue: the design 3.2 rule. Kernel style:
  `src/dsp/ugen/kalimba.rs:render` (unchanged body).
- Shim shape: protocol section 4. Re-export style: `src/dsp/ported/mod.rs:18-57`.

## Pitfalls

- **Build coupling with MTX-F1.** `music-things-fx` depends on
  `music-things-synth`, and vactr depends on both. While one of your
  modules is mid-move, every cargo command of the concurrent MTX-F1
  fails. Follow protocol section 2.1 strictly: one module at a time,
  `cargo check -p music-things-synth --all-targets` and
  `cargo check -p vactr --all-targets` logged per module, and never a
  copied-but-unrewritten file left under `music-things/`. Append
  `EDITS-COMPLETE <timestamp>` to your Progress Log when the last module
  is done, and only then start the gate phase. If your own commands fail
  only in MTX-F1's paths, apply protocol section 5.1 (wait for MTX-F1's
  EDITS-COMPLETE; 120-minute overall cap).
- **In-file tests.** `elements_ominous.rs` has an in-file `mod tests`
  using `super::*`. It moves as-is. No moved test may be weakened.
- **Shim globs.** A shim must not also `pub use` a same-named fn through
  the glob into a conflicting position; the local fn shadows the glob.
  Do not add a `pub mod` in a shim for a crate submodule that is private.
- **Shim callers.** `mixer.rs` calls `va_filter::filter(...)`, which now
  resolves to the shim. That is intended.
- **Shared tables.** `fm/sysex.rs` and `fm/patch.rs` allocate at load
  time, not on the process path. Keep them as they are.
- **Inter-module paths.** `stage_chain` and `stage_segment` may reference
  each other or `frame_lfo` via `super::`. Within the crate, `super::`
  is the crate root, so `super::stage_segment` still works.
- **Visibility.** If vactr code (`voice.rs`, `build.rs`, `graph.rs`,
  `catalog`) or a test needs a `pub(crate)` item, make it `pub` and
  document it.
- **Off-limits files.** Do NOT touch `src/dsp/tests/**`, `examples/**`,
  `ugen/catalog*`, `template.rs`, `build_helpers.rs`, or any effects
  file.

## Tests

- No new tests. Moved in-file tests run in the `music-things-synth`
  binary.
- `src/dsp/tests/dsp/{osc*,filter*,fm*,fm6_*,elements_internal,rings_part,stage_*,frame_*,tidal_*,peak_*,sample*,kernel_pairs}.rs`
  and the golden template tests must pass unchanged. They are the
  bit-identity proof.

## Verification (protocol section 7; logs under `tmp/music-things/MTX-S1/`)

1. V-check.
2. V-fmt on every created or modified `.rs` path in writePaths.
3. V-clippy: `-p music-things-synth` and `-p vactr`.
4. V-nextest: exit 0.
5. V-names: `unmatched=0`.
6. V-wasm, and V-crate-wasm for `music-things-synth`.
7. V-examples: `fm_pair_reference`, `rings_part_reference`,
   `tides_poly_reference` and `plaits_osc_reference` must compile
   unedited.
8. V-grep.
9. V-protected.
10. V-gone over every deleted source in writePaths. The four shim and
    adapter files are excluded.
11. V-lines.
12. V-vitest.
13. `grep -nE 'for |while |\* |\+ ' src/dsp/ugen/{va_filter,stage_chain,sample,wavetable}.rs`:
    review that only resolution logic remains and no DSP loop. Record
    the output.

## Completion Criteria

- [ ] All 27 modules (34 files, including `elements_ominous`) live in the synth crate, and
      the vactr copies are gone, except the 4 shims and adapters.
- [ ] Every `pub` kernel entry is generic over `KernelContext`, with the
      one-line prologue. The bodies are otherwise unchanged.
- [ ] The `va_filter` and `stage_chain` shims keep the exact old
      signatures, and `kernel_pairs.rs` compiles unedited.
- [ ] The `sample` and `wavetable` adapters preserve resolution order and
      the missing-resource behavior.
- [ ] Verification 1-13 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

### Session 2026-11-01 (MTX-S1 partial)
- Moved 23 modules including FM; per-edit intentions and hashes: `tmp/music-things/MTX-S1/edit-intentions.jsonl`. Per-module checks are under `tmp/music-things/MTX-S1/mod-*-check-*.log`.
- BLOCKED: `CARGO_TERM_QUIET=true cargo check -p vactr --all-targets` exits 101 (`tmp/music-things/MTX-S1/mod-fm-check-vactr.log`): unchanged vactr tests require FM `silence_op`, `encode_single`, and `encode_bulk`, which are `#[cfg(test)]` in a dependency compiled without test configuration. Protocol section 3 forbids removing existing attributes. An explicit protocol amendment or compatible in-scope solution is required before continuing the sequential module moves.
- Remaining: va_filter and stage_chain shims, sample and wavetable adapters, public documentation, formatting, sibling check retries, and all final verification gates. No completion criteria are asserted.


## Operator resume note (2026-11-01, after session 595 wave 2)

MTX-S1's session-595 attempt is committed as WIP on `wf/music-things-extract`.
It blocked only because `#[cfg(test)]` FM helpers were configured out of the
synth dependency. The operator added the `test-support` features (protocol
section 7, "test-only helpers") and re-gated `silence_op`, `encode_single`,
`encode_bulk`, `encode_message` and `pack_bulk_voice`. `cargo check
--workspace --all-targets` now passes on that tree. Continue from it, apply the
same rule to any further moved test-only helpers, finish the remaining moves,
and re-run every gate with fresh logs.

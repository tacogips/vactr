# MTX-90: Effect kind dispatch -> music-things-fx

**Status**: Ready
**Plan ID**: MTX-90 (wave W4, alone)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` section 3.3 ("Kind dispatch"), 3.5, 4.3 (`effects/mod.rs` row)
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

After MTX-F1 to F3, every effect group lives in the fx crate, but the
kind-to-group dispatch is still in vactr's `src/dsp/effects/mod.rs`. Those
lines are `Group`, `group`, `params`, `mem_len`, `requires_full_memory`,
`init`, `process` and `mix_index`, at `effects/mod.rs:189-569` as of
baseline. Line numbers shift after MTX-00; locate them by symbol.

This plan moves the dispatch verbatim into the crate, so a host can run
any effect by `EffectKind`. vactr keeps thin wrappers with today's exact
signatures, because tests and engine code call
`crate::dsp::effects::{params, mem_len, init, process, mix_index, requires_full_memory}`
with `graph::EffectKind` and `&CapabilitySet`.

## Non-goals

- Changing `FxUnit`, `FxCtx`, `param_ctl`, `param_index`, `ir_is`,
  `sidechain_bus`, `SIDECHAIN_BUS_CTL`, `MAX_SIDECHAIN_BUS`,
  `EFFECT_PARAM_BASE` or `MAX_FX_PARAMS`. They all stay in vactr.
- A crate-side unit wrapper or parameter smoothing.
- Notices.

## Dependencies

- dependsOn: MTX-F3
- Blocks: MTX-91

## File-level changes

1. **New `music-things/crates/music-things-fx/src/dispatch.rs`.**
   - `enum Group` and `fn group(kind: EffectKind) -> Group`, verbatim.
   - `pub fn params(kind: EffectKind) -> &'static [ParamDef]`
   - `pub fn mem_len(kind: EffectKind, sr: f32, limits: &GrainLimits) -> usize`
   - `pub fn requires_full_memory(kind: EffectKind) -> bool`
   - `pub fn init(kind: EffectKind, st: &mut FxState, mem: &mut [f32], sr: f32, limits: &GrainLimits)`
   - `pub fn process<C: FxContext + ?Sized>(kind: EffectKind, p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], ctx: &mut C)`,
     with prologue `let ctx = &mut ctx.fx_ctx();`
   - `pub fn mix_index(kind: EffectKind) -> Option<usize>`
   - The bodies are verbatim, with three changes:
     - `crate::dsp::granular::X` -> `super::granular::X`;
     - `caps` -> `limits` (`granular::effect_mem_len(sr, limits)` and
       `effect_init(st, mem, sr, limits)`);
     - the `KeyframeMixer` arm keeps `quad_mixer::process(p, st, l, r, ctx.sr)`.
   - These take the concrete crate `EffectKind`, not `impl Into`, because
     vactr's wrappers convert.
2. **Crate `lib.rs`.** Add `mod dispatch;` and
   `pub use dispatch::{init, mem_len, mix_index, params, process, requires_full_memory};`.
   Add doc comments.
3. **vactr `src/dsp/effects/mod.rs`.**
   - Delete `Group`, `group` and the six dispatch bodies.
   - Add wrappers with today's exact signatures:
     - `params(kind: EffectKind) -> &'static [ParamDef]` ->
       `music_things_fx::params(kind.into())`
     - `mem_len(kind, sr, caps: &CapabilitySet)` -> `…(kind.into(), sr, &grain_limits(caps))`
     - `requires_full_memory(kind)`
     - `init(kind, st, mem, sr, caps)`
     - `process(kind, p, st, mem, l, r, ctx: &mut FxCtx<'_>)` ->
       `music_things_fx::process(kind.into(), p, st, mem, l, r, ctx)`
     - `mix_index(kind)`
   - Remove the `pub use`/`use` lines for group modules that vactr no
     longer references directly. Keep the re-exports that tests, engine
     code or `FxUnit` use: `dynamics` (for `compress`), `texture_quality`,
     and every group module path a test imports (`grep -rn
     'effects::[a-z_]*::' src examples`). When in doubt, keep the
     re-export.
   - `FxUnit` methods call the vactr wrappers unchanged.
4. **Docs** for the dispatch items.

## Existing patterns to imitate

- The current dispatch in `src/dsp/effects/mod.rs` (moved verbatim).
- Wrapper style: `src/dsp/granular.rs` adapter (MTX-F2).

## Pitfalls

- **Match arm order.** Keep the arm order and group mapping exactly. The
  `StreamOpto` arm replaces `StreamVactr` (crate enum).
- **Allowed changes.** Do not convert `group` or the arms. Only paths and
  `caps` -> `limits` change.
- **`process` prologue.** The crate `process` calls generic group fns with
  `ctx` (now `&mut FxCtx`). That compiles because `FxCtx: FxContext`. Do
  not call `fx_ctx()` twice.
- **`effects/mod.rs` size.** It must shrink, and stay under 1000 lines.
- **Off-limits.** Do NOT edit any test, example, or `effects/catalog.rs`.

## Tests

- No new tests. Every vactr test that calls `effects::{params, mem_len, init, process}`
  (for example `src/dsp/tests/dsp/{catalog,cross_mod,lofi_composite,music_filters,effects}.rs`)
  passes unchanged, and so does the full golden suite.

## Verification (protocol section 7; logs under `tmp/music-things/MTX-90/`)

1. V-check.
2. V-fmt (`dispatch.rs`, fx `lib.rs`, `src/dsp/effects/mod.rs`).
3. V-clippy: `-p music-things-fx` and `-p vactr`.
4. V-nextest.
5. V-names: `unmatched=0`.
6. V-wasm, and V-crate-wasm for `music-things-fx`.
7. V-examples.
8. V-grep.
9. V-protected.
10. V-lines: record `src/dsp/effects/mod.rs`.
11. V-vitest.
12. `grep -nE 'enum Group|fn group' src/dsp/effects/mod.rs`: exit 1 (none
    left).

## Completion Criteria

- [ ] The dispatch lives in `music-things-fx/src/dispatch.rs`, re-exported
      at the crate root.
- [ ] vactr `effects/mod.rs` holds only wrappers, `FxUnit`, `FxCtx`,
      sidechain and param-id helpers, and re-exports.
- [ ] Verification 1-12 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

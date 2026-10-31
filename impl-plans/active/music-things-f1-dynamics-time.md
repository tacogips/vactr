# MTX-F1: Dynamics, EQ, filters, delay, reverb, modulation, spatial and lo-fi groups -> music-things-fx

**Status**: Ready
**Plan ID**: MTX-F1 (wave W1; runs concurrently with MTX-S1, with no shared files)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` sections 3.3 (group modules, signature rules), 4.3 (`effects/mod.rs` row), 4.4, 5, 6
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

This plan moves 19 effect-group files (~4800 lines) into
`music-things/crates/music-things-fx/src/`. The kind dispatch
(`group`, `params`, `mem_len`, `init`, `process`), `FxUnit` and `FxCtx`
stay in vactr's `src/dsp/effects/mod.rs` until MTX-90. During W1-W3 the
vactr dispatch calls the crate group functions with
`graph::EffectKind` (converted through `impl Into<EffectKind>`) and with
vactr's `FxCtx` (through `FxContext`). Both come from MTX-00.

## Non-goals

- Moving the dispatch (MTX-90).
- Changing `FxUnit`, `FxCtx`, `param_ctl`, `SIDECHAIN_*` or
  `effects/catalog.rs`.
- Other effect groups (MTX-F2, MTX-F3).
- Synth files.
- Notices.

## Dependencies

- dependsOn: MTX-00
- Blocks: MTX-F2 (serial owner of `music-things-fx/src/lib.rs` and
  `src/dsp/effects/mod.rs`)

## File-level changes

Moves go from `src/dsp/effects/<f>` to
`music-things/crates/music-things-fx/src/<f>`. The vactr source is
deleted.

1. **Files.**
   - `dynamics.rs`, `eq.rs`, `music_filters.rs`, `musicdsp.rs`,
     `resonator.rs`, `resonator_extra.rs`, `restoration.rs`,
     `utility.rs`, `saturation.rs`
   - `delay.rs`, `reverb.rs`, `creative_reverb.rs` with
     `creative_reverb/common.rs` and `creative_reverb/diffuse.rs`,
     `creative_delay.rs`
   - `modulation.rs`, `spatial.rs`, `lofi.rs`, `composite_lofi.rs`
2. **Crate `lib.rs`.** Add `pub mod <m>;` for each moved top-level module.
   `composite_lofi` is `pub` too.
3. **vactr `src/dsp/effects/mod.rs`.**
   - Each `pub mod <m>;` becomes `pub use music_things_fx::<m>;`.
   - `mod composite_lofi;` becomes `use music_things_fx::composite_lofi;`.
   - No other change. The dispatch match arms stay textually identical
     and now resolve to crate functions.
4. **Imports in moved files.**
   - `use crate::dsp::graph::EffectKind;` -> `use super::EffectKind;`
   - `use super::{FxCtx, FxState, ParamDef}` -> add `FxContext` where
     needed
   - `super::prim::…` -> `music_things_dsp::prim::…`
   - In `creative_reverb/{common,diffuse}.rs`, `super::` keeps pointing at
     `creative_reverb`.
5. **Signatures** per design 3.3 and protocol section 3.2:
   - Every `params`, `mem_len`, `init` and `process` that the vactr
     dispatch calls becomes `pub`.
   - `kind: EffectKind` becomes `kind: impl Into<EffectKind>`, with the
     prologue first.
   - `ctx: &mut FxCtx<'_>` becomes `<C: FxContext + ?Sized>`, with the
     prologue.
   - `dynamics::compress` becomes `pub`. Its signature is unchanged, as
     it takes no ctx or kind.
   - Make helpers `pub` only if vactr uses them.
6. **Convolution (`reverb.rs:290`).** `ctx.store.get(resource)` becomes
   `ctx.samples.sample(resource)`. Nothing else changes in that function.
7. **Docs** for every public item. The parameter tables are already
   self-describing; document each `pub fn` and each `pub const`
   parameter table.

## Existing patterns to imitate

- Group file layout: `src/dsp/effects/delay.rs` (header doc, param
  tables, `params`/`mem_len`/`init`/`process`).
- Prologue: protocol section 3.2.

## Pitfalls

- **Build coupling with MTX-S1.** `music-things-fx` depends on
  `music-things-synth`, so while the concurrent MTX-S1 has a module
  mid-move, your `cargo check`, `clippy -p music-things-fx`, nextest,
  V-names and wasm builds fail inside `music-things-synth` or
  `src/dsp/ugen`. V-grep may also match MTX-S1's in-progress copies. That
  is BLOCKED-BY-SIBLING: never edit MTX-S1's files. Retry under protocol
  section 5.1: wait while MTX-S1 lacks `EDITS-COMPLETE` or its files
  changed within 15 minutes, with a 120-minute overall cap. Your own
  moves follow protocol section 2.1 (one group module at a time;
  `cargo check -p music-things-fx --all-targets` and
  `cargo check -p vactr --all-targets` logged per module). Append
  `EDITS-COMPLETE <timestamp>` before the gate phase.
- **Kind prologue position.** Functions that `match kind { … }` directly
  in the signature's first expression still need
  `let kind: EffectKind = kind.into();` before the match.
- **Match arms.** Do not change `use EffectKind as K` match arms. The
  variant names are identical in the crate enum. None of these files
  reference `StreamVactr`.
- **Generic calls inside the crate.** Inside the crate, calling a sibling
  group's generic fn with an existing `ctx: &mut FxCtx` works because
  `FxCtx: FxContext`. For example, `creative_delay` calls
  `creative_reverb` helpers.
- **Dispatch calls.** `src/dsp/effects/mod.rs` dispatch calls such as
  `dynamics::process(kind, p, st, mem, l, r, ctx)` pass vactr types. They
  must compile without edits. If one does not compile, fix the crate
  signature, never the call.
- **Missing IR.** The convolution `Option` handling must stay
  byte-identical. A missing IR returns early.
- **Off-limits.** Do NOT touch `src/dsp/tests/dsp/{music_filters,musicdsp,creative_reverb,lofi_composite,effects,catalog,sidechain}.rs`
  or any synth file. MTX-S1 is concurrently editing the synth crate and
  `src/dsp/ugen/*`.

## Tests

- No new tests. Moved in-file tests (if any) run in the
  `music-things-fx` binary.
- `src/dsp/tests/dsp/{effects,catalog,music_filters,musicdsp,creative_reverb,lofi_composite,sidechain}.rs`
  and the bus and offline golden tests pass unchanged.

## Verification (protocol section 7; logs under `tmp/music-things-extraction/MTX-F1/`)

1. V-check.
2. V-fmt (all created or modified `.rs` files).
3. V-clippy: `-p music-things-fx` and `-p vactr`.
4. V-nextest.
5. V-names: `unmatched=0`.
6. V-wasm, and V-crate-wasm for `music-things-fx`.
7. V-examples.
8. V-grep.
9. V-protected.
10. V-gone over the 19 deleted sources.
11. V-lines.
12. V-vitest.

## Completion Criteria

- [ ] 19 files are in `music-things-fx/src`, and the vactr copies are
      deleted.
- [ ] `effects/mod.rs` changed only module lines.
- [ ] The group entry points are `pub`, generic over `FxContext` and
      `impl Into<EffectKind>`, with bodies otherwise identical.
- [ ] Verification 1-12 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

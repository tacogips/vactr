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

## Verification (protocol section 7; logs under `tmp/music-things/MTX-F1/`)

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

- [x] 19 files are in `music-things-fx/src`, and the vactr copies are
      deleted.
- [x] `effects/mod.rs` changed only module lines.
- [ ] The group entry points are `pub`, generic over `FxContext` and
      `impl Into<EffectKind>`, with bodies otherwise identical.
- [ ] Verification 1-12 pass, with logs. The Progress Log is updated.

## Progress Log

(append sessions here per protocol section 8)

### Session 2026-11-01 (MTX-F1)
- EDITS-COMPLETE 2026-10-31T15:56:06.476167+00:00
- Per-edit intentions and hashes: `tmp/music-things/MTX-F1/edit-intentions.jsonl`.
- Final corrections: module-use ordering and crate-private creative reverb helpers; per-edit pre/post hashes in `tmp/music-things/MTX-F1/edit-intentions.jsonl`.
- Verification: `rustfmt --edition 2021 --check` on owned Rust files, V-grep, V-protected, V-gone and V-lines passed (exit 0; logs `tmp/music-things/MTX-F1/final-{fmt,grep,protected,gone,lines}.log`; no tests run).
- BLOCKED-BY-SIBLING: `CARGO_TERM_QUIET=true cargo check --workspace --all-targets` exit 101, `tmp/music-things/MTX-F1/gate-check-sibling-blocked.log`: MTX-S1 FM test helpers are configured out; see `impl-plans/active/music-things-s1-classic-mi.md`. Sibling has declared its edit phase blocked. Resume final gates after MTX-S1 resolves this and finishes its edits. Clippy, nextest, names, wasm builds, examples and vitest remain unverified on the final combined tree. Earlier moving-tree attempts are retained under `tmp/music-things/MTX-F1/` and are not passes.


## Operator resume note (2026-11-01, after session 595 wave 2)

MTX-F1's session-595 attempt is committed as WIP on `wf/music-things-extract`.
It blocked only because `#[cfg(test)]` FM helpers were configured out of the
synth dependency. The operator added the `test-support` features (protocol
section 7, "test-only helpers") and re-gated `silence_op`, `encode_single`,
`encode_bulk`, `encode_message` and `pack_bulk_voice`. `cargo check
--workspace --all-targets` now passes on that tree. Continue from it, apply the
same rule to any further moved test-only helpers, finish the remaining moves,
and re-run every gate with fresh logs.

### Session 2026-11-01 (MTX-F1 resume)
- Existing committed extraction inspected; no Rust edits made in this session.
- Moving-tree checks: workspace check and owned-file rustfmt passed (`602-check.log`, `602-fmt.log`); both Clippy gates failed in sibling synth documentation (`602-clippy-fx.log`, `602-clippy-vactr.log`). The protected-path baseline diff passed (`602-protected-independent.log`).
- V-grep failed (`602-grep-independent.log`): test-support comments in the dsp, fx and synth Cargo manifests contain a prohibited token. These manifests are outside MTX-F1 writePaths; an authorized owner must repair them before rerunning gates.
- Workspace nextest encountered shared-target executable replacement while sibling builds ran; see `602-nextest.log` for its complete terminal outcome. Rerun on a settled tree with isolated build output. Remaining verification criteria are pending.
- Nextest terminal result: exit 100, 1820 run, 1818 passed, 2 spawn failures, 1293 not run; complete log `tmp/music-things/MTX-F1/602-nextest.log`.

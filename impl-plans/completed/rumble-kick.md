# Kick-keyed rumble implementation plan

**Status**: Completed
**Design Reference**: [RUM-001](../../design-docs/specs/design-music.md#kick-keyed-rumble-compression-rum-001)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

Implement an actual kick-audio detector for the existing compressor and
use it for industrial rumble/bass. Reuse primitives and graph transport;
no new dependencies or full bus feedback graph are needed.

## Modules and deliverables

| Task | Deliverables | Dependencies | Status | Parallelizable |
|------|--------------|--------------|--------|----------------|
| RUM-001 | Compressor selector parsing/lowering and constant transport | None | Completed | No |
| RUM-002 | Preallocated bus key snapshots and linked compressor detection | RUM-001 | Completed | No |
| RUM-003 | DSP/routing/codec/e2e/allocation verification | RUM-002 | Completed | No |
| RUM-004 | Preserved techno pattern, minimal rumble example, heavier industrial loop | RUM-002 | Completed | Yes |

Existing public signatures to retain:

```rust
pub fn param_ctl(kind: EffectKind, name: &str) -> Option<CtlId>;
pub fn lower_bus(id: BusId, root: &Rc<UGenNode>, lw: Lowering<'_>) -> Result<(BusDef, Extras), LowerError>;
```

Add a scoped compressor processing entry point or optional detector input
without changing ordinary effect behavior; expose internal declarations
next to their modules as implementation clarifies the borrow layout.

## Completion criteria

- [x] Symbolic sidechain selector is validated and preserved by transport.
- [x] Actual same-block kick audio ducks complete rumble/bass tails.
- [x] Stereo, source silence/swaps, partitioning and callback allocations verified.
- [x] Existing compressor and graph tests pass.
- [x] Examples evaluate and render finite, audible audio with measured ducking.
- [x] Native/browser builds, formatting and clippy pass.
- [x] Revised industrial example played; last-played techno pattern preserved.

## Progress log

### Session: 2026-09-30
**Tasks Completed**: Research and routing design; last-played loop copied to techno-pattern.vact.
**Tasks In Progress**: RUM-001 through RUM-004.
**Blockers**: None.
**Notes**: Compressor already exists; cross-bus detection is missing. Use its
existing feed-forward gain computer, adding an actual kick audio key.

### Session: 2026-09-30 (implementation)
**Tasks Completed**: Selector lowering, typed internal key, bus snapshots,
compressor detector plumbing; minimal and industrial examples authored.
**Tasks In Progress**: Quantitative DSP/end-to-end checks and playback.
**Blockers**: None.
**Notes**: The reserved selector travels as a validated constant graph
parameter and is held separately from smoothed audio parameters. Missing key
audio supplies silence. The heavier loop has four non-master buses plus
master, within default six-slot capacity.

## User goal extension (2026-09-30)

The user explicitly expanded this active goal: audit
https://github.com/bdejong/musicdsp/tree/master/source and add useful effects
that do not duplicate existing capabilities; investigate resonators and
parametric resonators; expand comb and other filters; add more delays and
reverbs, including spring, a spacious Blackhole-pedal-inspired reverb, and
pitch-shifted-feedback shimmer reverb. Follow
`musicdsp-effects.md` for this added scope. Do not mark the overall goal
complete until that plan is verified as well as this rumble work.

### Session: 2026-09-30 (rumble verification)
**Tasks Completed**: 11 sidechain tests; 3 musical tests; 1,635 native
tests passed (2 ignored), fmt, clippy, native build, host-wasm build.
Actual isolated rumble onset/bypass RMS=0.2275; between-kick=1.0000.
Pitched-bass onset/bypass RMS=0.2924; between-kick=1.0000.
**Notes**: Native industrial playback completed for 16 bars with no audio
failures; host reported only unavailable MIDI ports. Overall goal extended.

Further user extension: [MusicDSP synths](musicdsp-synths.md). The managed goal remains active until this plan and the effects plan are verified.

### Session: 2026-09-30 — completion
**Completed**: Full final verification: 1,679 passed, zero failed, two existing
ignored tests. Formatting, all-target Clippy with warnings denied, native
build, default WebAssembly build and host-wasm library build pass. Editor
TypeScript check passes. All 34 sample-free example files evaluate without
errors or rejected graph installs. Native auditions completed for revised
industrial (eight cycles), spring/shimmer (four each), synth sketch (eight)
and AM formant (four), all exit 0. Only unavailable MIDI-port diagnostics.
**Evidence**: `/tmp/vactr-musicdsp-final-tests.log`,
`/tmp/vactr-musicdsp-noop-final.log`; all 42 touched Rust files below 1000
lines, largest 835; `git diff --check` clean. Existing techno pattern retained.
**Remaining**: None within this plan. MusicDSP algorithms recorded as deferred
in the audit are outside this selected implementation batch. Original
industrial example commit is `2793ce8`; subsequent work remains uncommitted.

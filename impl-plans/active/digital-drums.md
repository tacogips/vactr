# Programmable Digital Drums Implementation Plan

**Status**: In Progress (DDRUM-001..004 complete 2026-09-29; 004B and 006 open)
**Design Reference**: `design-docs/specs/design-music.md#41-programmable-digital-drums-author-2026-09-27`
**Created**: 2026-09-27
**Last Updated**: 2026-09-29

## Design Document Reference

Implement Vactr's original digital drum kit and make all of its declared
sound parameters addressable in `.vact`. `digital-drum`, `digital-snare`,
`digital-metal` and `digital-hat` are prelude templates, each built on one
original core UGen. DaisySP code may be ported only with its MIT notices.
No third-party wave table, sample, preset, lookup data or asset-derived
trace is used. All DSP, defaults, ratios and names are original.

## DDRUM-001 inventory (authoritative)

### Constraints found in the current code (2026-09-29)

| Limit | Where | Consequence for this plan |
|---|---|---|
| `MAX_PORTS = 30` named inputs per UGen node | `src/dsp/ugen/catalog.rs` | Raise it to `MAX_PARAMS` (48), so one core node carries a whole family |
| `MAX_PARAMS = 48` distinct controls per template | `src/dsp/ugen/mod.rs` | Every family stays at 45 or fewer, including implicit `freq`/`amp`/`pan`/`velocity` |
| `MAX_CTLS = 32` controls per event | `src/host/wire.rs` | Unchanged; overflow is already an event-local `too-many-controls` failure |
| 128 instrument-default cells | `src/ns/insts.rs` `alloc_cell` | Exhaustion currently falls back to a constant silently; it must become a diagnostic |
| Custom `keyword` headers are unsupported | `src/compile/matchc.rs`, `src/dsp/controls.rs` | Closed domains are global `CtlDomain::Enum` rows, like `wave` |
| `ParamMeta` has no default, label or enum choices | `src/dsp/meta.rs`, `src/session/protocol.rs` | Extend the metadata and the wire (DDRUM-006) |

### Parameter surface

Existing global rows are reused: `freq`, `amp`, `pan`, `velocity`, `wave`
(osc waveform: saw/pulse/square/tri/sine), `cutoff`, `res` and `drive`.
New global `InstParam` rows are added for every other name. Enum rows and
their domains:

| Row | Domain (index order) |
|---|---|
| `filter-type` | off, lp, bp, hp, notch |
| `mod-wave`, `mod2-wave` | the `wave` domain |
| `lfo-wave` | sine, tri, saw, ramp, square, random |
| `transient-wave` | click, snap, noise, sweep |
| `velocity-target` | none, pitch, mod, cutoff, decay, transient, drive |
| `lfo-target` | none, pitch, mod, cutoff, res, amp, drive, decimation, transient, decay |

Every family implements every target in both target domains, so no value is
a dead knob. Pan is not an LFO target: the cores are mono, so modulating
pan per voice would be a fake knob. Pan per event uses `pan` patterns or
signals. `lfo-retrigger` and `lfo-sync` are `bool` headers.

| Name | Unit/range | Meaning |
|---|---|---|
| `coarse`, `fine` | semitones -24..24, cents -100..100 | Tuning offset of the carrier/bank |
| `amp-attack`, `amp-decay` | s 0..2, s 0.005..8 | Amplitude envelope |
| `amp-slope` | 0..1 | 0 linear, 1 exponential decay curve |
| `filter-drive` | 0..1 | Pre-filter gain into a saturating SVF |
| `decimation` | 0..1 | Sample-and-hold rate reduction (0 = off) |
| `volume-velocity` | 0..1 | How much `velocity` scales output level |
| `velocity-depth` | -1..1 | Velocity amount routed to `velocity-target` |
| `transient-freq`, `transient-level` | Hz 20..12000, 0..1 | Onset transient generator |
| `lfo-rate`, `lfo-depth`, `lfo-offset` | Hz 0.01..50 (cycles per pattern cycle when synced), -1..1, phase 0..1 | Per-voice LFO |
| `lfo-retrigger` | bool | Reset LFO phase at each hit; otherwise phase follows host time |
| `lfo-sync` | bool | Rate follows the event's cycles-per-second tempo |

Family additions:

| Family (template) | Additional controls |
|---|---|
| Tonal (`digital-drum`) | `mod-wave`, `mod-freq` (ratio), `mod-level`, `fm-depth`, `pitch-decay`, `pitch-depth` (semitones), `pitch-slope`, `osc-mix`, `mod-decay`, `mod-slope`, `repeat-count` (int 0..8), `repeat-time` (s) |
| Snare (`digital-snare`) | Tonal additions plus `noise-freq`, `noise-mix` |
| Metal/cymbal (`digital-metal`) | `mod-wave`, `mod-freq`, `mod-level`, `mod2-wave`, `mod2-freq`, `mod2-level`, `fm-depth`, `mod-decay`, `mod-slope`, `metal-spread`, `repeat-count`, `repeat-time` |
| Hat (`digital-hat`) | `mod-wave`, `mod-freq`, `mod-level`, `mod2-wave`, `mod2-freq`, `mod2-level`, `fm-depth`, `mod-decay`, `open-decay`, `closed-decay`, `hat-open` (bool) |

Open/closed hat choke uses the existing `cut` group control. It is not a
new knob. As of 2026-09-29, the engine never reads the cut bits in
`voice_hint`, so `cut` chokes nothing for any instrument. DDRUM-004B fixes
this in the engine.

### Non-DSP controls (codeable elsewhere)

| Original concept | Vactr construct |
|---|---|
| Audio output selection | `bus`/`orbit` routing and `aux-out` |
| MIDI note assignment | MIDI input mapping (`vactr-editor-midi`) |
| Step volume / probability / note | pattern `amp`/`velocity`, `degrade`/probability combinators, `note` |
| Euclidean length and steps, pattern selection | Euclidean and pattern-selection combinators |
| Shuffle, tempo, automation | swing/nudge combinators, `cps`, signals and live cells |

### Kit

`digital-kit` is a prelude dict of pure aliases: `bd` to `digital-drum`,
`sd` to `digital-snare`, `cy` to `digital-metal`, `hh` to `digital-hat`.
Use it as `s :bd kit: digital-kit`. A kit value cannot carry preset control
values today; presets remain pattern values or separate `inst` definitions.

## Subtasks

| Task | Deliverable | Depends on | Status |
|------|-------------|------------|--------|
| DDRUM-001 | Inventory above | none | Completed 2026-09-29 |
| DDRUM-002 | Typed arbitrary declared controls (MOD-003) | DDRUM-001 | Completed |
| DDRUM-002A | Enum and scalar rows; `MAX_PORTS` 48; cell-exhaustion diagnostic | DDRUM-001 | Completed 2026-09-29 |
| DDRUM-003 | `digital-drum` and `digital-snare` cores, templates and tests | DDRUM-002A | Completed 2026-09-29 |
| DDRUM-004 | `digital-metal` and `digital-hat` cores, templates and tests | DDRUM-003 | Completed 2026-09-29 |
| DDRUM-005 | Shared LFO, velocity targets, transients and repeat inside every core | DDRUM-003, DDRUM-004 | Delivered with 003/004 |
| DDRUM-004B | Engine choke: make `cut` groups gate same-group voices (currently encoded but never read) | DDRUM-004 | Not started |
| DDRUM-006 | `digital-kit`, editor default/label/choices metadata, examples, native/browser e2e | DDRUM-004 | Not started |

### Shared core contract (DDRUM-003/004)

- Each family has one core UGen, `digital-{drum,snare,metal,hat}-core`,
  with named ports for every family control. Its template wires every
  header to it and applies `* amp`, while `pan` stays with the voice.
- Signal chain: sources, then transient, then filter (`filter-type`,
  `cutoff`, `res`, `filter-drive`), then `drive`, then `decimation`, then
  the amplitude envelope, then the velocity level.
- The LFO and the velocity route modulate their typed target at audio rate.
- `repeat-count`/`repeat-time` retrigger the envelopes inside one voice.
- State is fixed and preallocated, with no callback allocation.
- Host-rate timing is correct at 44.1, 48 and 96 kHz.
- A random stream is deterministic per voice.
- All DSP is original. Tonal and snare use a carrier with a pitch envelope,
  plus an FM modulator with its own envelope; the snare adds band-passed
  noise. Metal and hat use an inharmonic original oscillator bank, FM'd by
  two modulators, with high-passed noise.

## Completion Criteria

- [x] Every original sound-control category has a documented Vactr mapping (DDRUM-001).
- [ ] Any ported DaisySP code retains applicable MIT notices; no third-party wave table, sample, preset, or lookup data enters the implementation (`mise run audit-upstream` stays clean).
- [ ] Every exposed voice knob accepts `.vact` pattern and live-cell values.
- [ ] Tonal drum, snare, cymbal, and hat render finite, audible audio.
- [ ] Every exposed parameter produces a measurable sound difference (per-parameter test).
- [ ] Unknown controls, enum values outside the domain, port/param capacity and cell exhaustion produce diagnostics.
- [ ] Editor metadata covers every declared parameter, with default, range, unit, label and enum choices.
- [ ] Quiet Cargo check, strict Clippy, rustfmt, wasm check and full tests pass.

## Progress Log

### Session: 2026-09-29, DDRUM-004

**Tasks Completed**:
- The shared chain moved out of `src/dsp/ugen/digital_drum.rs` into
  `src/dsp/ugen/digital_drum/{mod,chain,tonal,metal}.rs`. The tonal
  output is numerically unchanged.
- `digital-metal-core` and `digital-hat-core` (codec tags 94/95) run an
  original inharmonic bank of square roots of small primes. `metal-spread`
  widens it, `wave` shapes every partial, and two FM modulators and
  high-passed noise feed the shared chain.
- Hat decay: `hat-open` chooses `open-decay` or `closed-decay` as the base
  tail, and `amp-decay` scales it. All four controls are audible.
- `src/dsp/ugen/mod.rs` and `src/dsp/build/names.rs` were split below the
  1000-line limit.

**Tests**: Twelve new kernel and end-to-end tests cover:
- the codec and the rate/block matrix;
- determinism and voice end;
- every control reaching audio and the editor metadata;
- every enum value and bool state being audible;
- every target;
- the diagnostics;
- zero callback allocation.

Independent check-and-test review confirmed:
- nextest: 1,511 passed, 1 skipped;
- quiet check, strict Clippy, rustfmt, the wasm32 and `lsp` checks, the
  diff check and the upstream audit: clean.

**Finding**: `cut` groups do not choke voices. `voice_hint` carries the
cut bits, but `Engine::start` never reads them. DDRUM-004B was added.

### Session: 2026-09-29, DDRUM-002A and DDRUM-003

**Tasks Completed**:
- Control rows:
  - Global `InstParam` rows 109–152 cover the whole parameter surface,
    including the metal and hat rows reserved for DDRUM-004.
  - The enum domains follow the table above. `lfo-target` has no `pan`.
  - Two hidden implicit rows, `cps` and `onset-time`, are never template
    headers.
- `MAX_PORTS` is raised from 30 to 48. Every port array uses the constant,
  and `NAMED_PORT` (64) stays above it.
- Cell exhaustion now reports `DiagCode::CellCapacity` for header defaults
  and signal inputs, instead of silently falling back to a constant.
- `src/dsp/ugen/digital_drum.rs` provides the `digital-drum-core` (41
  ports) and `digital-snare-core` (43 ports) kernels, with codec tags
  92/93. The `digital-drum` and `digital-snare` templates wire every
  header to the kernel. All DSP is original; no DaisySP or upstream code
  is used.
- Tempo and phase:
  - Commit delivers the event tempo (`cps`) and host onset time as
    implicit controls to these cores.
  - `lfo-sync` reads `lfo-rate` as cycles per pattern cycle.
  - With `lfo-retrigger false`, the LFO phase derives from host time, so
    separate hits share phase.
- The exhaustive value test found one dead knob. Cutoff/res modulation did
  nothing while `filter-type` defaulted to `off`, so both templates now
  default to `:lp`.

**Tests**:
- Kernel tests cover the codec, the native/browser rate and block matrix,
  determinism, and repeat/voice end.
- End-to-end tests cover:
  - every declared control reaching audio and the editor metadata;
  - every value of every enum and both states of each bool differing
    audibly from the default;
  - every velocity and LFO target;
  - unknown-control and out-of-domain diagnostics.
- Two cell-capacity tests.
- Independent check-and-test review confirmed: nextest 1,499 passed,
  1 skipped; quiet check, strict Clippy, rustfmt, the wasm32 check, the
  diff check and the upstream audit (zero errors) are clean.

**Remaining**:
- DDRUM-004: metal and hat.
- DDRUM-006: kit, editor default/label/choices metadata, examples.
- The cores are mono, so per-voice stereo modulation is not provided.

### Session: 2026-09-27

**Tasks Completed**: Design gap identified and implementation plan created.
**Tasks In Progress**: Original parameter inventory and independent DSP specification.
**Blockers**: None.
**Notes**: Existing seven templates provide generic digital synthesis, but event commit currently skips custom controls without a built-in row. Preserve unrelated working-tree changes.

### Session: 2026-09-28

Typed custom instrument controls now route through the scheduler, and several
separate original drum templates have `.vact`, editor and native/browser tests.
`feedback-metal-drum` adds a coupled-FM metallic voice based only on general
percussion synthesis research. This plan's unified kit inventory, common
velocity/LFO assignment, family aliases and kit-level editor remain open.
The 2026-09-27 statement about custom controls being silently skipped is
historical and was resolved by MOD-003; no imported LXR, Elektron or other
uncleared waveform or preset assets are part of this progress.

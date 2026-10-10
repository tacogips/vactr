# Six-Operator FM Algorithms and New Instrument Voices Design

This document covers two DSP-layer changes:

- Extending the existing FM engine with the 32 classic six-operator
  algorithm topologies, plus user-supplied six-operator SysEx patch import.
- Adding six new instrument voices: kalimba, drawbar tonewheel organ,
  hurdy-gurdy, VOSIM, GENDYN and scanned synthesis.

It is linked from
[`design-music.md` section 4.4](design-music.md#44-six-operator-fm-algorithms-and-new-voices-author-2026-10-10).

Status: accepted design for the wf/fm1-voices workflow (2026-10-10, session
347; reconciled in session 352). The user decisions in
[`../user-qa/pending-fm1-voices-questions.md`](../user-qa/pending-fm1-voices-questions.md)
are all decided: the owner accepted FV1 (a), FV2 (a), FV3, FV4 (a), FV5 (a),
FV6 and FV7 on 2026-10-10.

This run owns every UGen, template and codec registry change. A sibling run
(wf/fm1-tuning) covers microtonal tuning and strum/harp chords and only
appends pattern and natives functions; this document does not cover that
work.

## Overview

The user asked for synth ideas from the FM-1 community firmwares that vactr
does not have yet. Those firmwares are used as concept descriptions only (see
"License boundary"). Two problems are solved here, each checked against the
code at `9ac8d1f`:

| Gap | Evidence |
|-----|----------|
| The `fm` template's `algorithm` header does nothing | `src/prelude/templates.vact:23-27` declares `algorithm: int = 5`, but the body is a fixed two-operator stack (`fm-op` > `fm-mod {fm-op}`). No UGen reads control row 21 (`src/dsp/controls.rs:255`). |
| No six-operator algorithm routing, operator feedback, operator EGs or patch import | `src/dsp/ugen/fm.rs` (71 lines) has only the stateless `fm-op`, `fm-mod` and `phase-distortion`. Graphs reject cycles, so `fm-op` nodes cannot express feedback. `six-op-original` (`src/dsp/ugen/six_op_original.rs`) has three fixed original routings and arithmetic configurations. Its header and `THIRD_PARTY_NOTICES.md` (around line 679) state that patch data and user-data loading are not implemented. |
| No kalimba, tonewheel organ, hurdy-gurdy, VOSIM, GENDYN or scanned voice | No organ, drawbar, tonewheel, kalimba, mbira, hurdy, gendyn or scanned code exists. The only VOSIM is the Braids-position approximation inside `macro-formant-voice` (`src/dsp/ugen/braids_formant.rs:75-80`). No bow friction model exists that can be reused: `elements_internal.rs:219` is a free-running `tanh` sine, and the only delay-coupled bow is private to `braids_physical.rs:85-89`. |

## Scope

In scope:

- An FM engine core under `src/dsp/ugen/fm/`. It provides the 32 algorithm
  topologies, the feedback operator, ratio and fixed frequency with detune,
  output level, the 4-rate/4-level EG, keyboard rate and level scaling,
  velocity sensitivity and transpose.
- Making the `fm` template's `algorithm` functional through the existing
  `fm-mod` node, with the existing `fm` graph and render digests unchanged.
- One new UGen, `fm6-core`, that exposes the full engine with an optional
  patch payload.
- A SysEx parser for single-voice (163-byte) and 32-voice bulk (4104-byte)
  dumps, with checksum validation.
- One language native, `fm6-sysex path`, that reads a user file and returns
  its voices as patch lists.
- Six new kernels and six new prelude templates: `kalimba`,
  `tonewheel-organ`, `hurdy-gurdy`, `vosim`, `gendyn` and `scanned`.
- Metadata, completion, `lang-reference.md` and `design-music.md` entries,
  examples, tests, appended golden digest lines and a `THIRD_PARTY_NOTICES.md`
  msfa section.

Out of scope, and not changed:

- Any existing template's graph or render output.
- `six-op-original` and the `six-bank-*` templates (see "Relation to
  six-op-original").
- These engine features: the global LFO, the pitch EG, amplitude-modulation
  sensitivity, pitch-modulation sensitivity, oscillator key sync and patch
  names (see "Intentional simplifications").
- Browser-side file loading for `fm6-sysex`. The browser host reports
  `host-unavailable`; see user question FV6.
- Microtonal tuning, strum and harp chords (sibling run).
- The scheduler, the voice pool, the canvas editor and the rotary effect.

## License boundary

vactr is MIT. Rules for this design:

- **FM-1 firmwares are concept descriptions only.** Felucca, SLOOP, sloopDX,
  FoMni, ChoralRoot, Melodee, FiMba-1, GHOULBOX, Jangada and their forks are
  GPL-3.0. Their code, tables and patches are not read, copied, translated or
  paraphrased. Only their published feature lists informed the voice
  choice: six-operator FM, kalimba, organ, hurdy-gurdy and experimental
  oscillators.
- **msfa is the only permitted code and table reference.** This is
  `music-synthesizer-for-android` (Google, Apache-2.0). It may be adapted
  for:
  - the algorithm bus-flag table (test fixture only, see "Algorithm
    topologies");
  - the EG rate and level formulas;
  - the keyboard level and rate scaling curves;
  - velocity scaling;
  - the operator frequency formulas (ratio, fixed, detune);
  - the feedback scaling;
  - the bulk-dump unpack layout.

  Every adapted item is listed in a new `THIRD_PARTY_NOTICES.md` section. That
  section gives the pinned msfa commit SHA, the files consulted, what was
  adapted, a modification statement, and the Apache-2.0 license text. The
  implementing plan records the SHA.
- **Forbidden:** Dexed code beyond msfa, Grids, TB-3PO, 8W8/9W9 and
  CrispyZebra (all GPL).
- **No ROM or factory patch data, ever.** That covers DX7 ROM cartridges and
  any other factory patch bytes. Every SysEx test fixture is built in test
  code from original, hand-chosen parameter values. The example ships an
  original hand-written patch list.
- **The six new voices are original Rust** written from the papers and
  physics in [References](#references). No source code of any kalimba,
  organ, hurdy-gurdy, VOSIM, GENDYN or scanned-synthesis implementation is
  consulted.
- **Trademarks are not identifiers.** "DX7" and "Hammond" appear only in
  prose and notices. Identifiers are generic: `fm6-*`, `tonewheel-*`,
  `drawbar*`.
- The existing statements that "DX7 patch data is excluded" (in
  `six_op_original.rs`, `design-mutable-audio.md`, the impl-plans and
  `THIRD_PARTY_NOTICES.md`) remain true and are not edited. The new notice
  section adds that `fm6-core` loads user-supplied SysEx at run time and
  bundles none.

## Part 1: six-operator FM engine

### Architecture decision: one engine core, two entry points

The engine lives in the existing FM module. `src/dsp/ugen/fm.rs` keeps
`op`, `modulate` and `phase_distortion` byte-for-byte and gains `pub mod`
declarations for its new children:

| File | Responsibility |
|------|----------------|
| `src/dsp/ugen/fm/algorithms.rs` | `ALGORITHMS: [Algorithm; 32]`. Each `Algorithm` holds a per-operator modulator bitmask, a carrier bitmask and one feedback edge `(source op, destination op)`. Also the fixed render order. |
| `src/dsp/ugen/fm/envelope.rs` | The 4-rate/4-level EG in msfa's log-domain formulation, sample-rate scaled |
| `src/dsp/ugen/fm/scaling.rs` | Keyboard level scaling (four curves), keyboard rate scaling, velocity sensitivity, output-level-to-amplitude, operator frequency (ratio/fixed, coarse/fine, detune) |
| `src/dsp/ugen/fm/patch.rs` | `Fm6Patch`, a POD struct of the 155 unpacked voice parameters. It provides `from_flat(&[f32]) -> Result<Fm6Patch, String>` (strict ranges), the parameter range table, and `macro_patch(algorithm, ratio, index, feedback)` |
| `src/dsp/ugen/fm/engine.rs` | `STATE_FLOATS`, voice start (patch to per-operator coefficients), and the per-sample render of six operators with feedback, used by both entry points |
| `src/dsp/ugen/fm/sysex.rs` | A pure parser, `parse(&[u8]) -> Result<Vec<Fm6Patch>, SysexError>`. It does no I/O and is never called on the audio thread. |

Every file stays under 400 lines. The two entry points share
`engine.rs`:

1. **`fm-mod` algorithm mode.** This entry point makes the `fm` template's
   `algorithm` work. See the next section.
2. **`fm6-core`.** A new UGen that exposes the whole engine, including an
   optional `patch:` payload loaded from SysEx.

Why not a new standalone template for the `fm` behaviour: the request is to
extend the existing FM synth and make the existing `algorithm` parameter
functional. A sibling template would leave `fm`'s `algorithm` dead.

### `fm` template backward compatibility (chosen mechanism)

The golden graph digest hashes `format!("{:?}", def.nodes)` plus every edge's
`from,to,port` (`src/host/tests/e2e/templates/golden.rs:66-78`). It does not
hash `InstDef.params` (header defaults). As a result:

- Passing `algorithm:` in the `fm` body would add a `Param(CtlId(21))` node and
  an edge, which changes `graph fm baseline`. This is **forbidden**.
- Adding a non-row header name to `fm` would shift the custom ids of every
  later template, changing their digests. This is **forbidden**.
- Ports appended at the end of a catalog port list leave existing edge port
  indices unchanged. A port whose name is an `InstParam` control row is bound
  implicitly to the voice's live control by `Template::compile_node`
  (`src/dsp/ugen/template.rs:460-469`, `catalog::implicit_ctl` at
  `catalog.rs:336-341`). This adds no nodes and no edges.

Decision:

1. **Append implicit ports to `FM_MOD`.** The list becomes
   `[in, mod, index, algorithm, freq, ratio, velocity]`. The three existing
   indices are unchanged. All four new names are existing `InstParam` rows
   (21, 0, 19, 6). The lowering table `src/dsp/build/names/table.rs` gains
   the same four names in the same order, so they can also be passed
   explicitly in user instruments.
2. **Make algorithm 0 the legacy value.**
   - Control row 21 changes from `default 1.0, range (1, 32)` to
     `default 0.0, range (0, 32)`.
   - The `fm` header changes from `algorithm: int = 5` to
     `algorithm: int = 0`.
   - Header defaults live in `InstDef.params`, which the graph digest does
     not hash. No other template or effect reads row 21: `dual-mod` has its
     own `ParamDef` (`src/dsp/effects/cross_mod.rs:40`).
3. **`fm-mod` behaviour per block.** The block reads
   `a = ins[3].first()`:
   - `a` non-finite, or `round(a) == 0`: the existing `modulate` body runs
     unchanged (the legacy path). The new ports and the state memory are
     not touched.
   - `round(a)` in `1..=32` (values above 32 clamp to 32): the input ports
     `in` and `mod` are ignored. The node renders the engine with
     `macro_patch(round(a), ratio, index, MACRO_FEEDBACK)` at `freq` and
     `velocity`. The template's `env-adsr` and `amp` still shape the
     result.
4. **Memory.** `mem_need(FmMod)` changes from `(0, 0)` to
   `(engine::STATE_FLOATS, 0)`. Legacy blocks never read or write it. The
   fm template's memory layout changes, but render output does not; the
   golden render lines prove this.

Consequences, all covered by tests:

- `s :fm > note [:a3] > once` takes the legacy path, so
  `render fm center/pan02` and `graph fm baseline` stay byte-identical.
- `s :fm > algorithm 7 > note ...` renders algorithm 7. The control reaches
  the voice through commit's row-control push (`src/sched/commit.rs:546-575`).
- A user instrument that uses `fm-mod` without declaring `algorithm` gets
  the row default 0, so it stays legacy.
- A user instrument that explicitly declares a non-zero `algorithm` default
  now selects that topology. This is the only behaviour change, and it only
  happens when `algorithm` is explicitly used. No shipped example does
  this. `design-music.md`'s illustrative `epiano` sketch is updated to
  `algorithm: int = 0`. See user question FV2.

### Macro patch (used when no SysEx patch is supplied)

`macro_patch(algorithm, ratio, index, feedback)` gives `fm-mod` algorithm
mode and patch-less `fm6-core` an operator set driven only by template
controls:

- **Carriers** (operators in the algorithm's carrier mask):
  - ratio mode, frequency ratio 1;
  - output amplitude 1;
  - EG held at full level. In `fm-mod` the template's `env-adsr` shapes the
    note. In `fm6-core` a carrier EG with about 10 ms attack and about 0.3 s
    release is used.
- **Modulators:**
  - ratio mode, frequency ratio `ratio` (continuous, clamped to
    `[0.0625, 32]`);
  - peak phase deviation `index` radians, scaled by `velocity`;
  - index envelope: a 10 ms linear attack, then exponential decay with a
    0.8 s time constant. This matches the legacy `env-perc 0.01 0.8` index
    shape.
- **Feedback:** applied on the algorithm's feedback edge at level `feedback`
  (0..7, msfa scale). In `fm-mod` mode it is the constant
  `MACRO_FEEDBACK = 4`, so all 32 algorithms are audibly distinct (for
  example, 1 and 2 differ only in their feedback operator). In `fm6-core` it
  is the `fm6-feedback` port, default 4.

### Algorithm topologies

Operators are numbered 1..6 as on the original instrument's algorithm chart.
`m -> c` means operator `m` modulates operator `c`. "fb" is the feedback edge
`source -> destination`; a self-loop is `n -> n`.

Production `ALGORITHMS` is authored from the published algorithm chart. A
test compares it with the msfa table. If this informative table and msfa's
table disagree, **msfa is authoritative**: the production table and this
section are corrected in the same change.

| Alg | Modulation edges | Carriers | fb |
|----:|------------------|----------|----|
| 1 | 2->1, 6->5, 5->4, 4->3 | 1, 3 | 6->6 |
| 2 | 2->1, 6->5, 5->4, 4->3 | 1, 3 | 2->2 |
| 3 | 3->2, 2->1, 6->5, 5->4 | 1, 4 | 6->6 |
| 4 | 3->2, 2->1, 6->5, 5->4 | 1, 4 | 4->6 (loop) |
| 5 | 2->1, 4->3, 6->5 | 1, 3, 5 | 6->6 |
| 6 | 2->1, 4->3, 6->5 | 1, 3, 5 | 5->6 (loop) |
| 7 | 2->1, 4->3, 5->3, 6->5 | 1, 3 | 6->6 |
| 8 | 2->1, 4->3, 5->3, 6->5 | 1, 3 | 4->4 |
| 9 | 2->1, 4->3, 5->3, 6->5 | 1, 3 | 2->2 |
| 10 | 3->2, 2->1, 5->4, 6->4 | 1, 4 | 3->3 |
| 11 | 3->2, 2->1, 5->4, 6->4 | 1, 4 | 6->6 |
| 12 | 2->1, 4->3, 5->3, 6->3 | 1, 3 | 2->2 |
| 13 | 2->1, 4->3, 5->3, 6->3 | 1, 3 | 6->6 |
| 14 | 2->1, 4->3, 5->4, 6->4 | 1, 3 | 6->6 |
| 15 | 2->1, 4->3, 5->4, 6->4 | 1, 3 | 2->2 |
| 16 | 2->1, 3->1, 5->1, 4->3, 6->5 | 1 | 6->6 |
| 17 | 2->1, 3->1, 5->1, 4->3, 6->5 | 1 | 2->2 |
| 18 | 2->1, 3->1, 4->1, 5->4, 6->5 | 1 | 3->3 |
| 19 | 3->2, 2->1, 6->4, 6->5 | 1, 4, 5 | 6->6 |
| 20 | 3->1, 3->2, 5->4, 6->4 | 1, 2, 4 | 3->3 |
| 21 | 3->1, 3->2, 6->4, 6->5 | 1, 2, 4, 5 | 3->3 |
| 22 | 2->1, 6->3, 6->4, 6->5 | 1, 3, 4, 5 | 6->6 |
| 23 | 3->2, 6->4, 6->5 | 1, 2, 4, 5 | 6->6 |
| 24 | 6->3, 6->4, 6->5 | 1, 2, 3, 4, 5 | 6->6 |
| 25 | 6->4, 6->5 | 1, 2, 3, 4, 5 | 6->6 |
| 26 | 3->2, 5->4, 6->4 | 1, 2, 4 | 6->6 |
| 27 | 3->2, 5->4, 6->4 | 1, 2, 4 | 3->3 |
| 28 | 2->1, 5->4, 4->3 | 1, 3, 6 | 5->5 |
| 29 | 4->3, 6->5 | 1, 2, 3, 5 | 6->6 |
| 30 | 5->4, 4->3 | 1, 2, 3, 6 | 5->5 |
| 31 | 6->5 | 1, 2, 3, 4, 5 | 6->6 |
| 32 | none | 1, 2, 3, 4, 5, 6 | 6->6 |

Render order is operator 6 down to 1, as in msfa, so every modulator is
computed before the operators it modulates. A feedback edge reads the
source operator's average of its two most recent outputs from earlier
samples. A feedback loop never reads a value from the current sample.

**Algorithms 4 and 6 (decided, session 352).** Their feedback edges cross
operators (`4->6` and `5->6`). msfa encodes them as `FB_OUT` on the source
and `FB_IN` on the destination, but its renderer applies feedback only to an
operator that carries both flags. msfa therefore renders no feedback for
these two algorithms. vactr keeps the chart's edge instead:

- The engine applies the edge to operator 6 from the source operator's
  delayed two-sample history, with the same scaling as a self-loop.
- The topology cross-check is unchanged: the edge is still derived from the
  `FB_OUT` to `FB_IN` flags, so it matches msfa's table.
- This is a rendering divergence from msfa, listed in "Intentional
  simplifications". Every engine path is new, so no existing golden digest
  is affected. The legacy `fm` path never runs the engine.
- Test: for algorithms 4 and 6, feedback 0 and feedback 7 render different
  output, so the edge is live.

**msfa cross-check.** The cross-check lives in the test file
`src/dsp/tests/dsp/fm_algorithms.rs` and works as follows:

- The test embeds msfa's 32 x 6 bus-flag bytes with an Apache-2.0
  attribution comment. These flags are: output bus (0 = output, 1, 2),
  add-to-bus, input bus, FB_IN and FB_OUT.
- It derives a modulator graph by simulating msfa's bus semantics in msfa's
  processing order:
  - an overwrite resets a bus's contributor set;
  - an add appends to it;
  - an operator's modulators are the current contributors of its input bus;
  - carriers are the operators with output bus 0;
  - the feedback edge goes from the FB_OUT operator to the FB_IN operator.
- For all 32 algorithms it asserts that the derived graph equals production
  `ALGORITHMS`: the modulator bitmask of each operator, the carrier mask, and
  the feedback edge.

### Operator, EG and scaling behaviour

The semantics follow msfa (Apache-2.0, adapted). Parameter numbers are the
155-entry unpacked single-voice order. That order stores **operator 6 first**:
parameters 0..20 are operator 6, and 105..125 are operator 1.

| Feature | Parameters (per operator unless global) | Behaviour |
|---------|-----------------------------------------|-----------|
| Frequency | osc mode (0 ratio, 1 fixed), coarse 0..31, fine 0..99, detune 0..14 (7 = centre) | Ratio mode: coarse 0 means 0.5, otherwise the coarse value, times `1 + fine/100`. Fixed mode: `10^(coarse mod 4)` Hz times the fine exponent, as msfa computes it. Detune is a small fixed log-frequency offset, as in msfa. The note frequency is the voice's `freq`, so the sibling run's tuning applies unchanged. |
| Output level | 0..99 | Converted to a log-domain level through msfa's output-level curve, then combined with the EG, scaling and velocity before conversion to linear amplitude |
| EG | R1..R4, L1..L4 (0..99) | Key-on runs L4 -> L1 (R1), -> L2 (R2), -> L3 (R3), then holds L3. Key-off (`kx.gate` closes) runs -> L4 (R4). Attack segments rise along msfa's exponential curve and the other segments move linearly in the log domain. Rates are scaled for the sample rate as msfa does. |
| Keyboard level scaling | break point 0..99, left/right depth 0..99, left/right curve 0..3 (-LIN, -EXP, +EXP, +LIN) | msfa's level-scaling curves. The key number is `69 + 12*log2(freq/440)` rounded, so microtonal pitches use the nearest key. |
| Keyboard rate scaling | 0..7 | msfa's rate scaling by key number |
| Velocity sensitivity | 0..7 | msfa's velocity-to-level offset; `velocity` 0..1 maps to MIDI 0..127 |
| Feedback | global 0..7 | msfa's shift-based scaling of the two-sample average |
| Algorithm | global 0..31 (shown as 1..32) | Selects the `ALGORITHMS` entry |
| Transpose | global 0..48 (24 = none) | Multiplies `freq` by `2^((t - 24)/12)` |
| Ignored | pitch EG (126..133), osc key sync (136), LFO (137..142), pitch-mod sensitivity (143), amp-mod sensitivity, name (145..154) | Validated for range but not rendered |

Numeric rules:

- **Block-size independence.** The EG advances once per 64 frames on a
  frame counter that is kept across blocks. Within each 64-frame chunk the
  operator amplitude is interpolated linearly. Output is therefore identical
  for 64-, 128- and 256-frame host blocks.
- **Operator sine.** The operator is the analytic `sin` of an f32 phase, as
  in `fm-op`; no sine table is used. The phase increment is clamped to
  `[-0.49, 0.49]` cycles per sample. Modulation input is scaled so that a
  full-level modulator produces the same peak phase deviation as msfa's Q24
  convention.
- **Output.** `y = (sum of carrier outputs) / n_carriers`, so `|y| <= 1` for
  every algorithm. Non-finite values reset the voice state to zero and the
  output to 0.
- **Lifetime.** `fm6-core` is self-enveloped. It calls `finish()` once every
  carrier EG has reached the minimum level after key-off. In `fm-mod` mode,
  lifetime stays with the template's `env-adsr`.

### `fm6-core` UGen

Ports:

| Port | Kind | Range | Default | Notes |
|------|------|-------|---------|-------|
| `freq` | row 0 | Hz | 440 | Note frequency |
| `velocity` | row 6 | 0..1 | 1 | |
| `algorithm` | row 21 | 0..32 | 0 | Macro mode only. 0 or less means 1; values are rounded and clamped. |
| `ratio` | row 19 | 0..32 | 1 | Macro mode only |
| `index` | row 20 | 0..32 | 1 | Macro mode only |
| `fm6-feedback` | custom | 0..7 | 4 | Macro mode only |
| `patch:` | constant list payload | 155 numbers | none | Not a port. With a patch present, `algorithm`, `ratio`, `index` and `fm6-feedback` are ignored. |

Patch payload, following the `frame-keyframe-core frames:` precedent
(`src/dsp/build.rs:765-770, 846-853`):

- Shape:
  - `UGenSpec::Fm6Core { patch: Option<Box<Fm6Patch>> }`.
  - Lowering requires `patch:` to be a constant numeric list.
- Validation:
  - The list goes through `Fm6Patch::from_flat` at realization time, on the
    evaluator side.
  - `from_flat` rejects any length other than 155, and any value that is
    non-integer or outside its parameter range.
  - The error names the parameter index and the allowed range.
- Template build:
  - `Template` copies the patch into a fixed POD slot array,
    `fm6_payloads: [Fm6Patch; MAX_FM6_PAYLOADS]`, with
    `MAX_FM6_PAYLOADS = 4`. This mirrors `frame_payloads` and
    `stage_payloads` in `src/dsp/ugen/template.rs:23-26`.
  - A fifth patched `fm6-core` in one instrument is a build error.
- Audio thread: it only reads the slot. Per-operator coefficients are
  computed into voice memory on the first block, with no allocation.
- Codec: tag 105 carries the optional 155 parameter bytes, so the browser
  engine decodes the same patch. A native-versus-codec parity test covers
  this.

### SysEx import

**Formats accepted.** These are the documented six-operator voice formats.
`n` (0..15) is the device number and is ignored.

| Format | Bytes | Layout |
|--------|------:|--------|
| Single voice | 163 | `F0 43 0n 00 01 1B`, 155 data bytes, checksum, `F7` |
| 32-voice bulk | 4104 | `F0 43 0n 09 20 00`, 4096 data bytes (32 packed 128-byte voices), checksum, `F7` |

**Validation.** The parser checks these in order and stops at the first
failure. Each failure is a `SysexError` variant with a message:

1. `Empty`: the input is empty.
2. `NotSysex`: the first byte is not `F0`.
3. `Manufacturer`: byte 1 is not `43`.
4. `SubStatus`: the high nibble of byte 2 is not 0.
5. `Format`: the format byte is neither `00` nor `09`.
6. `ByteCount`: the declared byte count does not match the format
   (`01 1B` = 155, `20 00` = 4096).
7. `Length { expected, actual }`: the total length is wrong. This also
   covers trailing bytes; exactly one message is accepted.
8. `DataByte { offset, value }`: a data byte is above `7F`.
9. `Checksum { expected, actual }`: the checksum fails. The expected value is
   `(128 - (sum of data bytes mod 128)) mod 128`.
10. `MissingEnd`: the last byte is not `F7`.

**Unpacking.** The bulk 128-byte packed layout is unpacked to the 155-entry
order using the documented bit packing (msfa layout, adapted):

- Bit fields are masked, so most values stay in range.
- Any field above its documented maximum (for example an output level of
  100..127) is clamped to the maximum. Clamping is documented, not an error,
  because the checksum already guards against transport corruption.
- Single-voice data is clamped the same way.

**Native.** `fm6-sysex path` is declared in `src/types/natives.rs` next to
`load` (an effect, `fn path -> [[int]]`):

- It reads bytes through a new `SourceLoader::read_bytes` method.
  - The default implementation fails with `host-unavailable`, so the noop,
    song, test and browser loaders need no change.
  - The native loader (`src/host/native/loader.rs`) uses the same path
    resolution and containment as `read`, with a 64 KiB cap.
  - The session loader (`src/session/session.rs`) delegates to it.
- It parses the bytes on the evaluator thread.
- It returns a list of voices. Each voice is a list of 155 ints, and a
  single-voice file returns a one-element list.
- Errors become `Failure` values whose message starts `fm6-sysex:` and
  includes the `SysexError` text and the path.

Usage. A list is callable with an index and returns that element, or nil
when the index is out of range. Vactr has no `( )` grouping: a nested call
is grouped with `{}` or written as a `>` pipe. `let` takes no `=`, and a
path is a path literal, not a string.

```
let bank fm6-sysex ./my-patches.syx
inst my-epiano: fm6-core freq velocity: velocity patch: {bank 3} > * amp
s :my-epiano > note [:c4 :e4 :g4] > d1
```

Test fixtures and docs use the same forms. For example, the bank length is
`len {fm6-sysex ./bank.syx}`, and the length of the first voice is
`fm6-sysex ./bank.syx > first > len`. The FM1V-21 fixture at
`src/ns/tests/fm6_sysex.rs:92-93` used `( )` grouping, which fails with
`error[paren-form]`. It is rewritten in these forms.

Lowering requires `patch:` to be a constant list. Plan FM1V-40 confirms that
an instrument body can take a list computed by a top-level `let` as that
constant. If it cannot, the documented form is a literal list in the body,
and the example and `lang-reference.md` entry use that form.

### Relation to six-op-original

`six-op-original` stays independent and unchanged:

- Its three banks are original arithmetic configurations with their own
  routings, and their digests are pinned.
- Sharing `engine.rs` with it would risk those digests for no requested
  benefit.

The design-mutable-audio statement that its upstream patch banks are excluded
still holds.

## Part 2: new instrument voices

Common rules for all six voices:

- Each voice has one kernel UGen. All state lives in voice memory
  (`mem_need` fixed floats). The kernel does no allocation, takes no locks,
  and contains only fixed-bound loops.
- Noise and stochastic processes use `prim::Rng` (xorshift32), seeded from
  `kx.seed`. The seed is stored in `st.u[0]`, following the
  `modal_pair.rs:120-125` pattern. Renders are deterministic.
- Output is mono, finite and clamped to `[-1, 1]`. Non-finite state resets
  the affected component to zero.
- Custom control names use a per-voice prefix, as `metal-*`, `choir-*` and
  `chain*` do.
  - Plan FM1V-04 checks every name against `controls.rs` `ROWS`,
    `EXTRA_PARAMS`, all `templates.vact` headers and the natives.
  - The one deliberate exception is `gate-length`, reused from the bass
    templates. It has the same meaning: kernel-owned note-off in
    sixteenth-steps from the hidden `cps`. Custom ids are global by name, so
    reusing it allocates no new id.
- Sustained voices (organ, hurdy-gurdy) own their note-off through
  `gate-length`, as `bass-core` does. The reason is that `legato` never
  reaches audio voices (`design-bass-voices.md`, "Note length and slide").
  These kernels are added to `wants_tempo_anchor`.

Kernel modules: each file stays under 400 lines.

| UGen | Files |
|------|-------|
| `kalimba-core` | `src/dsp/ugen/kalimba.rs` |
| `tonewheel-core` | `src/dsp/ugen/tonewheel.rs` |
| `hurdy-gurdy-core` | `src/dsp/ugen/hurdy_gurdy.rs`, `src/dsp/ugen/hurdy_gurdy/bowed.rs` (waveguide string and friction), `src/dsp/ugen/hurdy_gurdy/buzz.rs` (trompette bridge and wheel strokes) |
| `vosim-core` | `src/dsp/ugen/vosim.rs` |
| `gendyn-core` | `src/dsp/ugen/gendyn.rs` |
| `scanned-core` | `src/dsp/ugen/scanned.rs` |

### Kalimba (`kalimba-core`, template `kalimba`)

Model: a lamellophone tine is a clamped-free (cantilever) beam. Its modal
frequencies follow `(beta_n L)^2` with `beta_n L = 1.8751, 4.6941, 7.8548,
10.9955`. That gives the ratios **1, 6.2669, 17.5475, 34.3861** (Euler-Bernoulli
beam theory; Fletcher and Rossing). Signal path:

- **Modes.** Four tine modes plus a beating twin of the fundamental at
  `f0 + kalimba-beat` Hz, with relative amplitude 0.6. The twin models two
  nearly degenerate fundamentals, from tine asymmetry and the bridge.
  - Each mode is a complex-rotation resonator: radius
    `exp(-3 ln10 / (sr * T60))` with a cos/sin rotation. This is the same
    recurrence family as `effects/resonator_extra.rs:73-99`, written anew
    in the kernel.
  - Modes at or above `0.45 * sr` are skipped.
- **Pluck.** A half-sine force pulse whose width falls from 6 ms
  (`kalimba-hardness` 0) to 0.5 ms (1), on a log scale. A shorter pulse
  excites higher modes more, so brightness comes from contact time.
- **Damping.**
  - Fundamental T60 is `kalimba-decay` seconds.
  - Mode `n` has `T60_n = kalimba-decay / ratio_n^p`, where
    `p = 0.5 + 1.5 * kalimba-damping`.
- **Body.** Two fixed two-pole resonances, at about 220 Hz (cavity) and
  about 650 Hz (plate). They are driven by the tine sum and mixed by
  `kalimba-body`. These are tuned constants.
- **Buzz** (mbira bottle-cap rattle):
  - When `kalimba-buzz > 0`, seeded noise is band-passed around 4 kHz.
  - It is gated by `max(0, |x0| - threshold)` on the fundamental's
    displacement, so it rattles on the large early swings and dies with the
    note.
  - `kalimba-buzz = 0` is an exact bypass (bit-identical to the
    no-buzz render).
- **Lifetime.** The kernel ignores `kx.gate` and rings out. It calls
  `finish()` when the summed mode energy falls below -80 dB. It is listed as
  self-enveloped.

| Control | Range | Default |
|---------|-------|--------:|
| `kalimba-beat` | 0..8 Hz | 1.5 |
| `kalimba-hardness` | 0..1 | 0.5 |
| `kalimba-decay` | 0.1..10 s | 2.5 |
| `kalimba-damping` | 0..1 | 0.5 |
| `kalimba-body` | 0..1 | 0.3 |
| `kalimba-buzz` | 0..1 | 0 |

### Drawbar tonewheel organ (`tonewheel-core`, template `tonewheel-organ`)

Model: additive tonewheel partials with this registration layout:

| Drawbar | Footage | Harmonic | Tempered ratio used |
|---------|---------|---------:|--------------------:|
| `drawbar1` | 16' | 0.5 | 0.5 |
| `drawbar2` | 5 1/3' | 1.5 | 2^(7/12) = 1.4983 |
| `drawbar3` | 8' | 1 | 1 |
| `drawbar4` | 4' | 2 | 2 |
| `drawbar5` | 2 2/3' | 3 | 2^(19/12) = 2.9966 |
| `drawbar6` | 2' | 4 | 4 |
| `drawbar7` | 1 3/5' | 5 | 2^(28/12) = 5.0397 |
| `drawbar8` | 1 1/3' | 6 | 2^(31/12) = 5.9932 |
| `drawbar9` | 1' | 8 | 8 |

Rules:

- **Tempered ratios.** Tonewheel generators share equal-tempered wheels, so
  each non-octave partial uses the nearest equal-tempered ratio
  (public-domain music theory).
- **Foldback.**
  - A partial above `FOLD_TOP = 5_900` Hz is halved until it is at or below
    `FOLD_TOP`. This models the top of the tonewheel range.
  - A partial below `FOLD_BOTTOM = 32.70` Hz (C1) is doubled until it is at
    or above that frequency.
  - Partials are near-sines: analytic sines with independent phases.
- **Levels.** A drawbar at `d` (0..8, rounded) has gain 0 when `d = 0`, and
  otherwise `10^(-3 * (8 - d) / 20)`, which is 3 dB per step. The sum is
  scaled by `1/9`.
- **Key click.** At onset, a seeded noise burst with a 4 ms exponential
  decay, band-passed around 3 kHz and scaled by `organ-click`. `0` is an
  exact bypass.
- **Percussion:**
  - `organ-perc` selects the voice: 0 off, 1 second harmonic (ratio 2),
    2 third harmonic (ratio 2.9966).
  - It is a decaying sine with T60 of 0.25 s (fast) or 1.0 s when
    `organ-perc-slow = 1`.
  - Level is normal, or 10 dB lower when `organ-perc-soft = 1`.
  - While percussion is on, `drawbar9` is muted, and at normal volume the
    drawbar sum is lowered by 3 dB. This is the documented tonewheel-organ
    behaviour.
  - **Single trigger.** A voice cannot see other held notes. Single-trigger
    behaviour is therefore the per-event control `organ-perc-trigger`
    (default 1). A pattern sets it to 0 on legato notes. See user question
    FV5.
- **Vibrato/chorus scanner:**
  - `organ-vibrato` 0..6 selects off, V1, V2, V3, C1, C2 or C3.
  - The scanner is a modulated fractional delay at a fixed 6.9 Hz, with
    peak delay modulation of 0.25, 0.5 and 1.0 ms for depths 1 to 3.
  - Chorus settings mix the dry and scanned signals 1:1.
  - The delay memory is about 2 ms plus a guard.
- **Note length.** The kernel owns note-off through `gate-length`, followed
  by a 5 ms release, and then calls `finish()`.
- **Rotary.** The rotary sound is not part of the kernel. Examples route the
  organ to a bus with the existing `rotary` effect:
  `bus :leslie:` / `rotary speed: 6 depth: 0.7 mix: 1`.

| Control | Range | Default |
|---------|-------|--------:|
| `drawbar1`..`drawbar9` | 0..8 | 8 8 8 0 0 0 0 0 0 |
| `organ-click` | 0..1 | 0.3 |
| `organ-perc` | 0..2 | 0 |
| `organ-perc-slow`, `organ-perc-soft` | 0..1 | 0, 0 |
| `organ-perc-trigger` | 0..1 | 1 |
| `organ-vibrato` | 0..6 | 0 |
| `gate-length` | 0.05..64 sixteenth-steps | 4 |

### Hurdy-gurdy (`hurdy-gurdy-core`, template `hurdy-gurdy`)

Bow-model decision: **a new bowed digital waveguide.** The existing
Elements-style "bow" is a free-running `tanh` sine with no string coupling,
so it cannot produce Helmholtz motion (see Overview).

- **Strings.** Each string is a single-loop delay line of length `sr / f`,
  with fractional linear interpolation and a one-pole loss filter for
  damping and brightness.
- **Friction.**
  - The bow (the rosined wheel) acts at a fixed relative position of 0.12
    from the bridge.
  - The velocity difference `dv = v_bow - v_string` passes through a
    hyperbolic friction characteristic scaled by bow force. This is the
    McIntyre-Schumacher-Woodhouse model in the form given by Smith (PASP,
    bowed strings). The equations are implemented directly; no STK or other
    code is used.
  - `v_bow` is the wheel speed (`gurdy-wheel`) and the force is
    `gurdy-pressure`.
- **Four strings per voice:**
  - the chanterelle (melody) at `freq`;
  - the bourdon drone at the key `gurdy-drone-key` (MIDI note number);
  - the fifth drone (mouche) at key + 7;
  - the trompette at key + 12.

  Each has its own level control.
- **Trompette buzz bridge (chien):**
  - The effective wheel speed is
    `w = gurdy-wheel * (1 + gurdy-stroke-depth * stroke(t))`.
  - `stroke(t)` is a train of raised-cosine pulses, `gurdy-strokes` per
    pattern cycle (tempo-synced through `cps` and `onset-time`; 0 = steady).
    This is the coup de poignet.
  - The bridge buzzes when `w` exceeds `gurdy-buzz-threshold`, which models
    the tirant setting.
  - Buzz model: the trompette string's bridge force drives a one-sided
    impact (rattle) nonlinearity. The bridge contacts only when the force
    exceeds a gap that shrinks with `w - threshold`. The resulting impact
    impulses are mixed by `gurdy-buzz`.
  - With `gurdy-strokes > 0`, buzz bursts recur at the stroke rate.
- **Memory.** Each string line holds `sr/20 + guard` floats, the same sizing
  as `rings_part::line_len`, with frequencies clamped to at least 20 Hz.
  Four lines need about 9.6k floats at 48 kHz, which fits the 24_000-float
  test budget.
- **Drones in a voice-per-note engine.** Every voice carries all four
  strings, so drones restart with each note. The documented recipe is two
  patterns:
  - a drone pattern: `gurdy-melody 0`, long `gate-length`;
  - a melody pattern: `gurdy-bourdon 0 gurdy-fifth 0 gurdy-trompette 0`,
    `cut 1`.

  The default template plays all strings so a single `note` sounds complete.
  See user question FV4.
- **Note length and lifetime.** Note-off comes from `gate-length` (the
  wheel stops and the strings decay). The kernel calls `finish()` at
  -80 dB. It is in `wants_tempo_anchor`.

| Control | Range | Default |
|---------|-------|--------:|
| `gurdy-wheel` | 0..1 | 0.5 |
| `gurdy-pressure` | 0..1 | 0.5 |
| `gurdy-melody`, `gurdy-bourdon`, `gurdy-fifth`, `gurdy-trompette` | 0..1 | 1, 0.6, 0.4, 0.5 |
| `gurdy-drone-key` | 24..72 (MIDI note) | 43 (G2) |
| `gurdy-buzz` | 0..1 | 0.6 |
| `gurdy-buzz-threshold` | 0..1 | 0.5 |
| `gurdy-strokes` | 0..16 per cycle | 0 |
| `gurdy-stroke-depth` | 0..1 | 0.5 |
| `gate-length` | 0.05..64 sixteenth-steps | 8 |

### VOSIM (`vosim-core`, template `vosim`)

Model from Kaegi and Tempelaars (1978):

- Each fundamental period `1/freq` holds `N = vosim-pulses` (1..8)
  sin-squared pulses of width `T = 1/vosim-formant`. Pulse `k` has amplitude
  `vosim-decay^k`. Silence `M` fills the rest of the period.
- If `N*T` exceeds the period, `T` shrinks to `period/N` and `M = 0`.
- `vosim-formant` is read per sample, so formant sweeps are smooth. `N` is
  read once per period.
- The output has its DC removed by a one-pole high-pass at 10 Hz.
- It is not self-enveloped. The template body is
  `vosim-core ... > * {env-adsr attack decay sustain release} > * amp`, like
  `fm`.

| Control | Range | Default |
|---------|-------|--------:|
| `vosim-formant` | 100..8000 Hz | 900 |
| `vosim-pulses` | 1..8 | 3 |
| `vosim-decay` | 0..1 | 0.7 |

### GENDYN (`gendyn-core`, template `gendyn`)

Model: Xenakis's dynamic stochastic synthesis (Formalized Music, GENDY3 as
analysed by Serra 1993 and Hoffmann).

- **Waveform.** One period is a polygon of `P = gendyn-points` breakpoints
  (3..32; `MAX_POINTS = 32`) with linear interpolation.
- **Second-order random walks**, for each breakpoint every period:
  - amplitude: a step velocity `va_i` takes a random step of size
    `gendyn-amp-step`, bounded to `+-gendyn-amp-step`; then
    `a_i += va_i`, mirrored at `+-1`;
  - duration: the same scheme with `gendyn-dur-step`. Durations are mirrored
    inside `[(1 - s), (1 + s)] * (1 / (freq * P))`, where
    `s = gendyn-spread`.
- **Pitch locking.** At `s = 0` every period is exactly `1/freq`, so the
  pitch is locked to the note. A larger `s` lets the pitch wander, as in
  Xenakis's original.
- **Distributions.** `gendyn-dist` selects 0 uniform, 1 Cauchy, 2 logistic or
  3 hyperbolic cosine, each by inverse-CDF from `Rng::unit()`. Cauchy and
  logistic samples are clamped to `+-8` scale units, so a sample can never be
  infinite.
- **State.** `4 * MAX_POINTS` floats plus the phase and the current segment,
  all in voice memory.
- **Output.** DC-blocked at 10 Hz. The template uses `env-adsr` as `vosim`
  does.

| Control | Range | Default |
|---------|-------|--------:|
| `gendyn-points` | 3..32 | 12 |
| `gendyn-amp-step` | 0..1 | 0.2 |
| `gendyn-dur-step` | 0..1 | 0.1 |
| `gendyn-spread` | 0..1 | 0.1 |
| `gendyn-dist` | 0..3 | 1 |

### Scanned synthesis (`scanned-core`, template `scanned`)

Model from Verplank, Mathews and Shaw (2000):

- **String.** A closed ring of `N = 64` masses. Each mass has springs to its
  neighbours (`scan-stiffness`), a centring spring to rest (`scan-centering`)
  and damping (`scan-damping`).
- **Haptic-rate dynamics.**
  - The ring is updated at `scan-update` Hz (50..2000), which is far below
    audio rate.
  - Integration is semi-implicit Euler. The stiffness is clamped so that
    `k * dt^2 < 1` keeps the integration stable.
- **Excitation.** At onset, a raised-cosine displacement is applied, centred
  at `scan-position` with width `scan-hammer`.
- **Scanning.**
  - The ring shape is read around the ring once per period at `freq`, with
    cubic interpolation between masses.
  - Between dynamic updates the scanner crossfades from the previous shape
    to the current one, so there is no zipper noise.
  - The timbre evolves as the shape relaxes.
- **State.** Positions, velocities and previous positions (`3 * 64` floats)
  plus the scan phase and update counter, all in voice memory.
- **Output.** DC-blocked and normalized by the onset peak displacement. The
  template uses `env-adsr`.

| Control | Range | Default |
|---------|-------|--------:|
| `scan-stiffness` | 0..1 | 0.5 |
| `scan-damping` | 0..1 | 0.3 |
| `scan-centering` | 0..1 | 0.1 |
| `scan-hammer` | 0.02..1 | 0.3 |
| `scan-position` | 0..1 | 0.5 |
| `scan-update` | 50..2000 Hz | 400 |

### Templates

The new `inst` blocks are appended at the end of `src/prelude/templates.vact`,
in this order. Each body passes its header controls by name and ends in
`> * amp`.

| Template | Body |
|----------|------|
| `kalimba` | `kalimba-core freq ... > * amp` |
| `tonewheel-organ` | `tonewheel-core freq ... > * amp` |
| `hurdy-gurdy` | `hurdy-gurdy-core freq ... > * amp` |
| `vosim` | `vosim-core freq ... > * {env-adsr attack decay sustain release} > * amp` |
| `gendyn` | `gendyn-core freq ... > * {env-adsr attack decay sustain release} > * amp` |
| `scanned` | `scanned-core freq ... > * {env-adsr attack decay sustain release} > * amp` |

`fm6-core` gets no prelude template, because a patch payload has to be
supplied per instrument. The patch-less engine is already reachable through
`s :fm > algorithm N`. See user question FV7.

## Registration and digest stability

The current values below were read at `9ac8d1f`. The bass design's numbers
are stale.

| Item | Current | After |
|------|---------|-------|
| Highest codec tag | 104 (`BassCore`) | `fm6-core` 105, `kalimba-core` 106, `tonewheel-core` 107, `hurdy-gurdy-core` 108, `vosim-core` 109, `gendyn-core` 110, `scanned-core` 111. Existing tags are never renumbered. |
| `UGEN_NAMES` (`catalog.rs`) | 98 | 105, appended at the end |
| `TEMPLATE_NAMES` (`catalog.rs` and `src/ns/insts.rs`, `[&str; 73]`) | 73 | 79, appended after `reese-bass` |
| `template_slots` (`src/dsp/ring.rs:810`) | 102 | Raised from 96 to preserve the previous local-song headroom after six FM1 voices were added to the prelude. 79 prelude + 2 live-input + 4 quad-stem = 85, or 87 with the stage-linked example. |
| Control rows | last id 161 | No new rows. Row 21's default and range change (see Part 1). |
| Custom header ids | first-use order from 192 | New names are first used only by the appended templates, so no existing id shifts |

Files touched by registration. One serialized plan owns each group, and
registry plans run with `--max-concurrency 1`.

- **Graph and node:**
  - `src/dsp/graph.rs`: seven `UGenSpec` variants, `Fm6Core { patch }`
    among them.
  - `src/dsp/ugen/mod.rs`: `pub mod` lines, the `Node` variants, and `is_env`
    for fm6, kalimba, tonewheel and hurdy-gurdy.
  - `src/dsp/ugen/template.rs`: the self-enveloped list (same four), the
    `fm6_payloads` slot array, and the `fm6` patch copy.
  - `src/dsp/ugen/build_helpers.rs`: the `mem_need` arms, including
    `FmMod => (engine::STATE_FLOATS, 0)`.
  - `src/dsp/ugen/mixer.rs`: the render arms.
- **Catalog and names:**
  - `src/dsp/ugen/catalog.rs`: the four appended `FM_MOD` ports, `ports`,
    `ugen_name`, `UGEN_NAMES`, `TEMPLATE_NAMES` and `node_of`.
  - `src/dsp/ugen/catalog/voice_ports.rs`: seven port lists.
  - `src/dsp/ugen/catalog/codec.rs`: tags 105..111, including the fm6 patch
    bytes.
  - `src/dsp/build/names.rs` and `src/dsp/build/names/table.rs`: the names,
    port orders, and the four appended `fm-mod` names.
  - `src/dsp/build.rs`: the `patch:` payload lowering beside `frames:`.
  - `src/dsp/controls.rs`: row 21's default and range.
- **Natives:**
  - `src/types/natives_domain.rs`: seven `dsp("...")` entries appended at
    the end of the UGen block, after `dsp("granular")` and before the
    effects block. The sibling wf/fm1-tuning run appends pattern functions
    elsewhere, so the two runs touch different regions. When merging, keep
    both runs' appended entries.
  - `src/types/natives.rs`: `fm6-sysex`, placed next to `load`.
  - The native implementation goes in a new `src/ns/fm6_sysex.rs`.
  - `SourceLoader::read_bytes` goes in `src/ns/load.rs`, with
    implementations in `src/host/native/loader.rs` and
    `src/session/session.rs`.
- **Metadata and templates:**
  - `src/dsp/meta.rs`: the `ugen_node` arms, the template-to-node arms, and
    port ranges.
  - A new `src/dsp/meta/templates/voices.rs` holds the six param lists and
    `DEFAULT_OVERRIDES`. It is wired into both default lookups
    (`template_default_override`, meta.rs:185-214, and the port-default
    lookup near meta.rs:304) and into the exclusion list near
    meta.rs:309-328.
  - `src/dsp/meta/templates.rs`: `mod voices;` and six `TEMPLATE_PARAMS`
    entries.
  - `src/ns/insts.rs`: `TEMPLATE_NAMES` length 79.
  - `src/prelude/templates.vact`.
- **Runtime:** `src/sched/commit.rs`: `wants_tempo_anchor` gains
  `TonewheelCore | HurdyGurdyCore`.
- **Tests:**
  - The module lines in `src/dsp/tests/dsp.rs` and
    `src/host/tests/e2e/templates.rs`.
  - `src/host/tests/e2e/templates/bass.rs`: the "bass templates are last"
    assertion becomes "the bass block is immediately followed by the six
    fm1 voices, which are last". The guarantee stays the same strength,
    moved to the new tail.
  - `src/host/tests/e2e/templates/golden_digests.txt`: new lines only.

Editor completion and metadata need no frontend file:

- Template keywords come from `catalog::TEMPLATE_NAMES` and parameter keys
  from `meta::decl_for` (`src/complete/sources.rs:83-93, 147, 175`).
- Editor panels receive `meta::all()` over the session protocol
  (`src/session/editors.rs:128`).
- No `editor/` test enumerates templates.
- A Rust completion test asserts that the six templates, `fm6-core` and
  `fm6-sysex` are offered.

**Digest guard:**

- Each new template adds one `graph`, one `render ... center` and one
  `render ... pan02` line (all kernels are mono), so 18 lines in total.
- `golden_digests.txt` is sorted, so the new lines interleave with the
  existing ones.
- After blessing with `VACTR_BLESS_GOLDEN=1` (additions only), these must
  both hold:
  - `git diff -U0 -- src/host/tests/e2e/templates/golden_digests.txt | grep -c '^-[^-]'`
    is `0`;
  - the added-line count is exactly 18.
- `graph fm baseline 29a87235be1d7b30 0`, `render fm center
  f84c6215540e7cb1 24064` and `render fm pan02 013b4c5a6d6773d5 24064` must
  be present and unchanged. The same holds for every `six-bank-*` line.

**Line budgets.** Every touched Rust file stays under 1000 lines:

- `src/ns/insts.rs` grows from 964 to about 970.
- `names/table.rs` grows from 780 to about 900. If it would reach 1000, the
  new port tables move to a `names/table/voices.rs` submodule.
- Any other file that would reach 1000 is split as the rust-coding standard
  requires.

## Presets and examples

| File | Content |
|------|---------|
| `examples/fm6-algorithms.vact` | `s :fm` stepping `algorithm` through several topologies, and the legacy `algorithm 0`. A user `inst` over `fm6-core` with an **original hand-written 155-value patch list**, so no file is needed. A commented `fm6-sysex` usage line. |
| `examples/kalimba.vact` | A kalimba melody with beat and buzz variations |
| `examples/tonewheel-organ.vact` | Registrations (888000000, 838000000, 886000000 written as drawbar controls), percussion, scanner, and a `bus :leslie:` with `rotary` |
| `examples/hurdy-gurdy.vact` | The two-pattern drone-plus-melody recipe with `gurdy-strokes` |
| `examples/experimental-oscillators.vact` | VOSIM formant sweeps, GENDYN spread, scanned stiffness |

Example rules:

- Every example is a formatter fixed point and is checked by
  `src/fmt/tests/corpus.rs` and `src/lsp/tests/analysis.rs`.
- Levels keep the mix peak at or below 1.0.
- An e2e test renders each file for at least 2 cycles and asserts that the
  output is finite, `rms > 1e-3` and `peak <= 1.0`.

Documentation:

- `design-music.md`: a new section 4.4, the template list, and the sound
  vocabulary row.
- `lang-reference.md` section 5: a domain-vocabulary row naming the six
  templates, `fm6-core` and `fm6-sysex`. This file is `include_str!`'d by
  the reader and no-abort tests, which must stay green.
- `README.md` `## Status`: one bullet group.

## Verification

Test names contain one of `fm_algo`, `fm6`, `fm_mod`, `kalimba`, `tonewheel`,
`gurdy`, `vosim`, `gendyn` or `scanned`. The focused filter is
`cargo nextest run -E 'test(/fm_algo|fm6|fm_mod|kalimba|tonewheel|gurdy|vosim|gendyn|scanned/)'`,
and its count must be positive.

**FM engine** (`src/dsp/tests/dsp/fm_algorithms.rs`, `fm6_engine.rs`,
`fm6_sysex.rs`):

- Topology: for all 32 algorithms, the msfa-derived graph equals
  `ALGORITHMS` (modulators per operator, carrier mask, feedback edge).
- Routing is observable:
  - For every algorithm, silencing one operator changes the output exactly
    when that operator reaches a carrier.
  - Setting the feedback level from 0 to 7 changes the output only through
    the feedback source operator.
- EG: with `kx.gate` held, the level settles to L3. After key-off it reaches
  L4. A higher rate gives a shorter segment (monotonic). Timing matches the
  msfa formulas within a stated tolerance for rates {0, 25, 50, 75, 99}.
- Scaling:
  - Each level-scaling curve has the correct sign and monotonicity either
    side of the break point.
  - Rate scaling shortens segments for higher keys.
  - Velocity sensitivity 0 is velocity-independent.
- Block-size independence: renders with 64, 128 and 256-frame blocks are
  bit-identical. Determinism: two renders are bit-identical.
- Stability: the grid covers all algorithms, feedback 7, every operator at
  output level 99, and `freq` 20 and 8000. Every sample is finite and
  `|y| <= 1`.
- SysEx:
  - The synthetic single-voice and 32-voice fixtures are built in test code
    from original values with a computed checksum. They parse, and they round
    trip to the expected 155-value lists.
  - Each `SysexError` variant is produced by a minimal malformed input
    (bad checksum, wrong length, trailing byte, high data byte, wrong
    manufacturer, wrong format, wrong byte count, missing `F7`, empty) and
    has a readable message.
  - Out-of-range packed fields are clamped.
  - `Fm6Patch::from_flat` rejects a wrong length and an out-of-range value
    and names the index.

**`fm` template** (`src/host/tests/e2e/templates/fm_mod_algorithm.rs`):

- `algorithm` 0, and an unset `algorithm`, are bit-identical to the
  pre-change render (the golden lines).
- `> algorithm N` for N in 1..32 gives 32 pairwise-distinct renders. Each
  is bit-identical to a direct engine render with `macro_patch(N, 14, 2, 4)`.
- A user `inst` using `fm-mod` with no `algorithm` header is bit-identical to
  the legacy path.

**Native** (`src/host/tests/e2e/fm6_sysex.rs`, using a test loader that
implements `read_bytes`):

- `fm6-sysex` returns 32 lists of 155 ints for the bulk fixture and 1 for
  the single-voice fixture.
- A bad-checksum file fails with a `fm6-sysex:` message that names the
  checksum.
- The default loader fails with `host-unavailable`.
- A patched `fm6-core` renders, and the native render equals the codec
  (browser) render.

**Voice kernels** (`src/dsp/tests/dsp/{kalimba,tonewheel,hurdy_gurdy,vosim,gendyn,scanned}.rs`):

- **Kalimba:**
  - Spectral peaks are within 1% of `f0 * {1, 6.2669, 17.5475}` (34.3861
    when below Nyquist).
  - The amplitude-envelope beat period is `1/kalimba-beat` within 5%.
  - Higher `kalimba-damping` shortens the decay of the upper modes.
  - `kalimba-buzz 0` is an exact bypass, and buzz raises 3-6 kHz energy.
  - The voice finishes.
- **Tonewheel:**
  - Registration 888000000 gives peaks at `0.5, 1.4983, 1 * f0`.
  - At C7, every partial is at or below 5900 Hz (foldback).
  - Percussion adds energy at 2 or 2.9966 times `f0` that decays at the
    selected T60, and it mutes `drawbar9`.
  - `organ-perc-trigger 0` adds no percussion.
  - The scanner shows 6.9 Hz frequency modulation.
  - `organ-click 0` is an exact bypass.
  - Note-off happens at `gate-length / (16 * cps)` within one sample.
- **Hurdy-gurdy:**
  - With `gurdy-wheel > 0`, the chanterelle pitch from autocorrelation is
    within 1% of `freq`.
  - With the wheel at 0 there is no sustained output.
  - The drones are at the key and key+7.
  - 3-8 kHz buzz energy rises when the wheel is above the threshold.
  - With `gurdy-strokes 4` at a known `cps`, buzz bursts recur at
    `4 * cps` Hz within 2%.
- **VOSIM:**
  - The fundamental equals `freq` within 1%.
  - The spectral peak lies near `vosim-formant`.
  - `vosim-pulses 1` with `vosim-decay` has no effect.
- **GENDYN:**
  - It is deterministic per seed, and the output is bounded.
  - At `gendyn-spread 0` each period is exactly `sr/freq` samples within
    one sample.
  - A larger spread increases the variance of the period.
  - Every distribution is finite.
- **Scanned:**
  - The pitch equals `freq` within 1%.
  - The spectral centroid changes between 0-100 ms and 400-500 ms.
  - Higher `scan-stiffness` changes the timbre faster.
  - The stability grid is finite.
- **All kernels:**
  - The stability grid over control extremes is finite with `|y| <= 1`.
  - `mem_total` equals the declared state, and `MemExceeded` occurs at one
    float less.
  - Native versus browser codec parity holds at 44.1, 48 and 96 kHz with
    64- and 256-frame blocks, following `braids_struck.rs:95-130`.
  - `NativeRig` and `E2e` assert zero allocations per rendered block.

**E2e** (`src/host/tests/e2e/templates/fm1_voices.rs`):

- All six templates are realized and audible at `note [:c4]`.
- The commit stage delivers `cps` to the tonewheel and hurdy-gurdy kernels.
- Golden lines exist for every template.
- A seed-order case shows that seed ordinals equal node positions.

**Gates.** Each plan runs the focused filter and rustfmt on its own files.
Integration and acceptance run the following serially. Full nextest and
every browser run hold the measurement lock at
`/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`
(`editor/test/e2e/README.md:22-33`).

- `CARGO_TERM_QUIET=true cargo build`
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check`
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`
- `mise run build-wasm-release`
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
- in `editor/`: `npm run check`, `npm run test`,
  `VACTR_REQUIRE_SESSION_ABI=1 npm run build`, `npm run test:style`
- `mise run fmt-vact-check`

Tests are silent: nothing plays audio. The macOS volume is never changed.

## Implementation partition

These were recommendations for the plan step. The accepted plans
(`impl-plans/active/fm1-voices-dispatch.json`) split them further. The
table below uses the recommendation names; the actual plans are:

| Recommendation | Actual plans |
|----------------|--------------|
| FM1V-01 engine core | FM1V-00 scaffold, FM1V-10 algorithms, FM1V-11 EG and scaling, FM1V-20 engine |
| FM1V-02 SysEx parser | FM1V-12 |
| FM1V-03 fm registry | FM1V-21 (`fm6-sysex` native and `read_bytes`), FM1V-40 (`fm-mod` ports, row 21, `fm6-core`, tag 105, payload) |
| FM1V-04 voice kernels | FM1V-13 to FM1V-18 |
| FM1V-05 voice registry | FM1V-30 |
| FM1V-06 examples and docs | FM1V-50 examples, FM1V-51 docs and notices |

Run order for the remaining plans is serial: FM1V-21, FM1V-20, FM1V-30,
FM1V-40, FM1V-50, FM1V-51. FM1V-30 runs before FM1V-40, so it takes codec
tags 106..111 and leaves tag 105 unused until FM1V-40 assigns it to
`fm6-core`. No tag is renumbered.

| Plan | Owns | Depends on | Parallel |
|------|------|------------|----------|
| FM1V-01 engine core | `src/dsp/ugen/fm.rs` (only its `pub mod` lines), `src/dsp/ugen/fm/{algorithms,envelope,scaling,patch,engine}.rs`, `src/dsp/tests/dsp/fm_algorithms.rs`, `src/dsp/tests/dsp/fm6_engine.rs`, plus their module lines in `src/dsp/tests/dsp.rs` | none | No registry work. Runs first. |
| FM1V-02 SysEx parser | `src/dsp/ugen/fm/sysex.rs`, `src/dsp/tests/dsp/fm6_sysex.rs` | FM1V-01 (`patch.rs`) | Yes, parallel with FM1V-04. Its `pub mod` and test-module lines are reserved by FM1V-01. |
| FM1V-03 fm registry | `FM_MOD` ports, row 21, the `fm` header default, `fm6-core` registration (tag 105), payload lowering and slots, `fm6-sysex` native and `read_bytes`, `fm_mod_algorithm.rs`, `fm6_sysex.rs` e2e, `design-music.md` epiano line | FM1V-01, FM1V-02 | No (registry, serial) |
| FM1V-04 voice kernels | the six kernel files and their unit tests. A scaffold step first adds the six `pub mod` lines and test-module lines, so kernels can be implemented in parallel without touching shared files | FM1V-01 (shares `tests/dsp.rs`) | Kernels in parallel after the scaffold |
| FM1V-05 voice registry | all six voices' registration files, templates, `commit.rs`, `bass.rs` ordering test, golden blessing, `fm1_voices.rs` | FM1V-03, FM1V-04 | No (registry, serial) |
| FM1V-06 examples and docs | the five examples, the example render test (`src/host/tests/e2e/templates/fm1_examples.rs` and its module line), `README.md`, the template list and sound-vocabulary row in `design-music.md` (section 4.4 itself already exists from the design step), `lang-reference.md`, the `THIRD_PARTY_NOTICES.md` msfa section, `design-docs/references/README.md` | FM1V-05 | No |

The FM1V-03 e2e file `src/host/tests/e2e/fm6_sysex.rs` also needs its
module line in the e2e test root. FM1V-03 owns that line.

Plans list only concrete tracked files in `writePaths`/`sharedPaths`. They
declare `target/`, `tmp/<evidence-root>`, the tree-sitter grammar wasm and
Playwright outputs as `artifactRoots`, never as write paths.

## Intentional simplifications

- **FM engine:**
  - No LFO, pitch EG, AMS/PMS or oscillator key sync. Every voice starts
    with all operator phases at 0, the equivalent of key sync on.
  - Patch names are ignored.
  - The output is normalized by the carrier count, unlike the original
    instrument's unnormalized sum. This bounds the output for every
    algorithm.
  - The analytic sine replaces msfa's table, and f32 math replaces Q24
    integers. Behaviour matches within the stated tolerances, not bit for
    bit.
  - Algorithms 4 and 6 render their cross-operator feedback edge from
    delayed history, where msfa renders none (see "Algorithm topologies").
- **`fm` algorithm mode** uses a macro operator set, not a patch. Patches
  are available through `fm6-core`.
- **Organ:**
  - Single-trigger percussion is an explicit per-event control.
  - No tonewheel leakage, crosstalk or wheel-shape harmonics.
  - The rotary sound is the existing effect on a bus.
- **Hurdy-gurdy:**
  - Drones are per voice.
  - The buzz bridge is a one-sided impact model, not a full contact
    simulation.
  - Friction uses a static hyperbolic curve with no thermal hysteresis.
- **GENDYN** locks pitch to the note at `gendyn-spread 0`, a playable
  variant of Xenakis's free-pitch process.
- **Scanned** uses a fixed 64-mass closed ring with a linear scan path.

## Risks

- **Digest drift in `fm`.** This is mitigated by the implicit-port
  mechanism, the bit-exact legacy branch, and the golden plus removed-line
  checks in FM1V-40 and FM1V-30 acceptance.
- **msfa access.** Implementers need the msfa source at a pinned SHA to
  write the cross-check fixture and formulas. If the source cannot be
  reached, the plan stops at FM1V-00 and reports it. It does not substitute
  another source. (FM1V-00 and FM1V-10 are accepted, so this is resolved.)
- **Row 21 default change.** Only the `fm` template declares `algorithm`,
  and only `fm-mod` reads row 21. The full suite and the editor metadata
  tests confirm this.
- **Payload constant-ness of `patch: (bank 3)`.** This is checked early in
  FM1V-40, with a literal-list fallback (see "SysEx import").
- **Sound quality is only partly provable by automated checks.** The
  objective checks above bound behaviour; the example renders support a
  manual listening pass.
- **Merge overlap with wf/fm1-tuning.** The overlap is limited to the
  natives-file append regions described in "Registration".

## References

See also
[`../references/README.md`](../references/README.md#six-operator-fm-and-new-voices).

- google/music-synthesizer-for-android (msfa), Apache-2.0: the
  algorithm bus table, EG, scaling, frequency and feedback formulas, and the
  bulk-dump layout. The commit is pinned in `THIRD_PARTY_NOTICES.md`.
- J. M. Chowning, "The synthesis of complex audio spectra by means of
  frequency modulation", JAES 21(7), 1973.
- Yamaha DX7 Owner's Manual and published MIDI data format, used only for the
  user-level algorithm chart, parameter ranges and SysEx framing. No patch
  data is used.
- N. H. Fletcher and T. D. Rossing, *The Physics of Musical Instruments*,
  2nd ed., Springer, 1998: clamped-free bar modes and lamellophones.
- D. Chapman, "The tones of the kalimba (African thumb piano)", JASA 131(4),
  2012: measured tine partials, beating and buzzers. The plan verifies the
  citation before adding it to the references index.
- J. O. Smith III, *Physical Audio Signal Processing*, W3K, 2010: digital
  waveguides and the bowed-string friction model.
- M. E. McIntyre, R. T. Schumacher and J. Woodhouse, "On the oscillations of
  musical instruments", JASA 74(5), 1983.
- W. Kaegi and S. Tempelaars, "VOSIM: a new sound synthesis system", JAES
  26(6), 1978.
- I. Xenakis, *Formalized Music*, rev. ed., Pendragon, 1992 (dynamic
  stochastic synthesis).
- M.-H. Serra, "Stochastic composition and stochastic timbre: GENDY3 by
  Iannis Xenakis", Perspectives of New Music 31(1), 1993.
- P. Hoffmann, "The New GENDYN Program", Computer Music Journal 24(2), 2000.
- B. Verplank, M. Mathews and R. Shaw, "Scanned synthesis", Proc. ICMC 2000.
- Tonewheel organ service documentation, used for user-level facts only:
  drawbar footages, foldback, percussion and the scanner vibrato.

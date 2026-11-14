# Stereo Texture Effect Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-29

## Design Document Reference

This splits FX-001 in `modular-audio-effects.md` into bounded modes from
the pinned MIT Clouds `GranularProcessor`: granular, stretch, looping delay,
and spectral. The stereo processor requires install-time audio storage and
all common parameters. Generated filters or crossfade tables must be
reviewed separately and may be replaced analytically; no recorded or
unaudited audio asset is imported. An approximation must not be labeled a
complete source port.

## Related Plans

- **Previous**: `impl-plans/active/modular-audio-effects.md`
- **Depends On**: `modular-audio-foundation.md`

## Modules

### `src/dsp/effects/texture.rs`

```rust
pub fn params(kind: EffectKind) -> &'static [ParamDef];
pub fn mem_len(kind: EffectKind, sample_rate: f32, caps: &CapabilitySet) -> usize;
pub fn init(kind: EffectKind, state: &mut FxState, mem: &mut [f32], sample_rate: f32);
pub fn process(kind: EffectKind, state: &mut FxState, mem: &mut [f32],
               left: &mut [f32], right: &mut [f32], values: &[f32], ctx: &mut FxCtx<'_>);
```

The bus effect owns stereo capture and fixed grain/FFT state in its
preallocated region. Preserve the common source controls: position, size,
pitch, density, texture, dry/wet, stereo spread, feedback, reverb, freeze,
trigger and gate. A mode selector may be an extra control. Each mode must
have an explicit signal and memory contract at 44.1, 48 and 96 kHz.
The current four effects are distinct mode selections. Source `quality`
encodes stereo/mono and high/low fidelity as four settings; all four
adaptations now expose that selector as a thirteenth control. FX-001F
still requires source comparison before complete parity can be claimed.
The pinned source maps quality bits through `set_quality`: bit zero selects
mono, bit one selects low fidelity. Its low-fidelity path converts 32 kHz
processing to 16 kHz and uses 8-bit capture with a generated SRC filter.
Any first Vactr selector can be a procedural adaptation; source resampling
and filter parity remain separate verification work.

### `src/dsp/effects/texture/{granular,stretch,loop,spectral}.rs`

```rust
pub fn process(state: &mut FxState, mem: &mut [f32],
               left: &mut [f32], right: &mut [f32], values: &[f32], ctx: &mut FxCtx<'_>);
```

Audit the pinned `clouds/dsp` dependencies for each mode separately. The
spectral path requires an FFT size, window and phase-state policy that fits
native/browser caps. No effect may allocate or fetch audio inside callback.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| FX-001A | Stereo capture, bus controls, freeze, trigger/gate and bounded memory | MOD-004–005 | Eight-grain adaptation implemented; source comparison pending |
| FX-001B | Granular playback mode, density/window/position/size/pitch | FX-001A | Eight-grain adaptation implemented; source comparison pending |
| FX-001C | Stretch/WSOLA playback and texture filter | FX-001A | Two-window bounded alignment adaptation implemented; source comparison pending |
| FX-001D | Looping delay with pitch and feedback | FX-001A | Bounded stereo live/frozen-loop adaptation implemented; source comparison pending |
| FX-001E | Spectral phase-vocoder mode with native/browser FFT caps | FX-001A | Bounded 512-point STFT adaptation implemented; source comparison pending |
| FX-001F | Source comparison, quality selector, all controls and four-mode provenance/fidelity review | FX-001B–E, MOD-006 | Completed: four modes measured and classified; source parity remains open |
| FX-001G | Translate loop transport, delay glide, cubic reads and freeze wrap from pinned player | FX-001D | Completed and independently verified; full Clouds processor parity remains separate |

### FX-001G looping playback stages

**Design Reference**: `design-docs/specs/design-mutable-audio.md`,
`texture-loop` paragraph and pinned MIT `clouds/dsp/looping_sample_player.h`
and `audio_buffer.h`.

**Deliverables**: Update `src/dsp/effects/texture_loop.rs` with a host-rate
scaled delay glide, tap synchronization and frozen loop point/duration
mapping, a pitch-aware wrap crossfade, and an independently implemented
four-point cubic read from preallocated stereo capture. Preserve the thirteen
`.vact` controls, source-independent live pitch and texture extensions,
quality mode, bus/editor/wire compatibility and bounded install memory.

**Completion Criteria**:
- [x] Delay glide, trigger/tap and frozen loop transitions are bounded, deterministic and partition-invariant; cubic reads preserve constant and linear signals and wrap safely.
- [x] Every declared loop control remains codeable, including live/frozen pitch and separate stereo outputs; native/browser 44.1/48/96 kHz × 64/256 tests, freeze and memory preflight pass with zero callback allocation.
- [x] Notice identifies the translated MIT player roles and continuing source deviations without calling the mode a full port or importing generated tables/audio assets.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Stereo capture and common controls | `src/dsp/effects/texture.rs` | Granular adaptation | Native/browser/rate/control tests |
| Stretch kernel | `src/dsp/effects/texture_stretch.rs` | Two-window adaptation | Native/browser/rate/control tests |
| Loop kernel | `src/dsp/effects/texture_loop.rs` | Adaptation with translated playback stages | Cubic/glide/loop, partition, native/browser/rate/control tests |
| Spectral kernel | `src/dsp/effects/texture_spectral.rs` | Four-frame stereo STFT adaptation | Native/browser/rate/control tests |
| Effect metadata | `src/dsp/effects/mod.rs`, `src/dsp/meta.rs` | Four runnable modes and shared quality selector available | Editor and `.vact` tests |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Bus stereo and fixed memory | `modular-audio-foundation.md` | Bounded bus path available |
| Generated-table provenance | `MOD-001`, `THIRD_PARTY_NOTICES.md` | Per-mode audit required |
| Reference audio comparison | `MOD-006` | Pending |

## Completion Criteria

- [ ] Four playback modes render stereo audio with bounded install memory.
- [ ] Every common control and mode-specific role is addressable from `.vact` and editor.
- [ ] Freeze, trigger, gate, dry/wet, feedback and stereo spread are tested.
- [ ] Native/browser rates/blocks, capacity rejects and allocation-free callbacks pass.
- [x] Source/resource notices and fidelity comparisons are recorded.
- [ ] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass.

## Progress Log

### Session: 2026-09-28 — FX-001G looping playback stages

`texture-loop` now has a host-rate-scaled source-style delay glide, tap
synchronization, frozen loop point and cubic size mapping, and a
pitch-dependent wrap fade based on 64 source frames at 32 kHz. Its stereo
capture reads use an independently derived four-point Lagrange polynomial;
no source Hermite implementation, generated table or sound asset was
imported. All thirteen `.vact` controls and the existing live pitch,
diffusion, filter, reverb, gate and quality extensions remain. Independent
review passed cubic constant/linear/wrap, live→tap→freeze→pitch partition,
native/browser 44.1/48/96 kHz × 64/256, codec/editor, memory-preflight
and zero-allocation checks. Quiet native/no-default/wasm builds, strict
Clippy, rustfmt, Taplo, 19 mise validations and full Cargo tests pass
(1,444 passed, one ignored). Source Hermite, processor scheduling, SRC,
pitch shifter and numerical parity remain open; the mode stays an
`Adaptation`.

### Session: 2026-09-28 — FX-001F quality-control adaptation

All four texture modes expose `quality: 0..3` as the thirteenth effect
parameter and a stepped editor field. Values retain source bit roles:
0 stereo/high, 1 mono/high, 2 stereo/low, 3 mono/low. Mono averages the
inputs and emits dual-mono wet/dry; low fidelity uses original host-rate
two-sample hold and signed 8-bit quantization on input and output, with
phase and held values persistent across block boundaries in fixed scalar
state. Quality 0 bypasses the conversion exactly. Source 32/16 kHz
resampling, generated filter and numerical parity remain open; FX-001F is
not complete. Tests cover `.vact`/editor controls, mode outputs,
native/browser rates and blocks, freeze/trigger compatibility, memory
preflight and allocation-free callbacks.

### Session: 2026-09-28

The pinned `clouds/dsp/parameters.h` and `granular_processor.cc` show four
playback modes and twelve common parameter/flag roles. They depend on a
stereo capture buffer and distinct granular, WSOLA, looper and phase-vocoder
players. The Vactr bus effect slot has preallocated stereo memory and an
effect-local parameter limit of sixteen, so the control envelope is feasible;
exact memory and FFT sizing remain to be specified by FX-001A.

### Session: 2026-09-28 — granular adaptation

`texture-grain` now has independent preallocated stereo capture, eight
bounded grains and all twelve common controls on a bus or master chain.
Installation rejects a region smaller than 0.30 seconds per channel before
retiring any live chain. Native and browser share the appended effect wire
ordinal. Generated lookup tables are replaced by analytic pitch/window
mappings. This remains an adaptation: source 32 kHz conversion, 32/40 or
64-grain scheduling, diffuser/reverb and exact density mapping differ.
Stretch, looping delay and spectral remain pending, as does source-audio
comparison. No complete Clouds port is claimed.

### Session: 2026-09-28 — stretch adaptation

`texture-stretch` adds independent stereo capture, two triangular
overlap-add windows and a bounded nine-candidate alignment search.
`density` controls the search width/diffusion; `texture` controls a
post-window low-pass filter. Both are audible and tested. It shares the
twelve common `.vact` controls, 0.30-second-per-channel budget and
pre-retirement install rejection with `texture-grain`. Native/browser
rate-block tests and zero-allocation probes pass. The source uses its
bit-sign correlator, exact window scheduling and 32 kHz conversion; this
kernel does not. Looping delay, spectral processing and source-audio
comparison remain open.

### Session: 2026-09-28 — alignment review correction

An independent review found that the float alignment reference advanced
the active window twice: `head` was already moved per sample, while the
correlator added elapsed `age` again. The reference now begins at the
current playback head, and a nonperiodic transient regression test
distinguishes the correct splice from the double-advanced candidate.
Source bit-sign correlation and exact window scheduling remain fidelity
work for FX-001F.

### Session: 2026-09-28 — looping-delay adaptation

`texture-loop` adds a live stereo variable delay, independent capture,
analytic dual-tap pitch resampling, frozen loop with a bounded wrap
crossfade, and trigger tap synchronization. All twelve common controls are
codeable; `density` becomes smooth read-position diffusion, `texture` a
low-pass coefficient, `stereo-spread` a width control, and `size` also
sets the live delay span. These intentional mapping differences have
response tests. It shares the 0.30-second-per-channel install budget and
pre-retirement rejection. Native/browser rate-block and callback-allocation
tests pass. Source Hermite reads, exact pitch-shifter/filter/diffuser and
32 kHz numerical behavior are not reproduced; spectral mode and source
comparison remain pending.

### Session: 2026-09-28 — spectral adaptation

`texture-spectral` adds a host-rate stereo 512-point STFT with analytic
windows, four fixed spectral history frames per channel, bin warp/shift,
quantization, phase decorrelation and gate glitch. The existing FFT gains
an allocation-free inverse. All twelve controls remain codeable and
response-tested, with gate default 1 as the normal path. The bus requires
full preallocated capture, overlap-add and history memory before install.
Native/browser rate-block tests and callback-allocation probes pass.
Source fixed-point phase/history, generated window, glitch algorithms,
source quality SRC and exact 32 kHz behavior remain for FX-001F comparison;
four runnable adaptations do not establish source-port parity.

### Session: 2026-09-28 — inverse FFT numerical review

Focused complex forward/inverse tests now cover sizes 2, 8, 64 and 512,
complex impulses, mixed real/imaginary signals, normalization and
undersized-buffer no-op behavior. This pins down the allocation-free
inverse independently of spectral-effect audio tests.

### Session: 2026-09-29 — FX-001F pinned Clouds source comparison

Added `verification/clouds_texture_reference.cc`,
`verification/compare_clouds_texture.py` and
`examples/clouds_texture_reference.rs`, plus the `compare-clouds-texture`
mise task. The probe validates Eurorack `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`
and stmlib `e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`, compiles the upstream
processor and `clouds/resources.cc` into a temporary directory, and feeds both
implementations the same Python-generated three-second stereo input (two
distinct tone mixtures plus a click train for two seconds, followed by one
second of silence). `clouds/clouds.cc` calls `codec.Init(..., 32000)` and
`codec.Start(32, ...)`, and `clouds/dsp/frame.h` sets `kMaxBlockSize` to 32.
The upstream buffers match firmware declarations `block_mem[118784]` and
`block_ccm[65536 - 128]`. Upstream and Vactr run at 32 kHz in 32-frame blocks.
The source converts normalized input to signed `ShortFrame`; quality 2/3 activates
its 32-to-16 kHz SRC. Vactr runs its effect kernels directly at 32 kHz and
retains its host-rate two-sample hold/quantization adaptation for low quality.
The source uses an equal-power dry/wet lookup; the raw Vactr kernels are
wrapped in the host `dry + (wet - dry) * mix` bus convention. Trigger pulses
occur every 1024 frames, gate stays open, and freeze begins at frame 32,000
when enabled.

Every mode ran three documented control settings, freeze off/on, and all four
quality settings (24 cases per mode, 96 total). Metrics below are medians over
the 24 cases. In the L/R RMS column, each pair is Clouds/Vactr. Correlation
uses a regular 16:1 decimation for long signals; RMS, stereo width and tail
windows use full-rate samples. Spectral bands use 8:1 decimation to a 4 kHz
analysis rate. The JSON report contains per-channel values and all six
octave-band ratios for every case.

| Mode | Classification | Median correlation / normalized RMS error | L/R RMS Clouds → Vactr | Centroid Clouds → Vactr | Stereo correlation Clouds → Vactr | Width RMS Clouds → Vactr | Dry RMS ratio Clouds → Vactr | Tail to −40 dB Clouds → Vactr | Lag Clouds → Vactr |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Granular | Measured gap | 0.228 / 2.990 | 0.030/0.070; 0.027/0.071 | 431 / 499 Hz | 0.104 / 0.678 | 0.030 / 0.041 | 0.159 / 0.391 | 0.20 / 0.68 s | 0 / −10.25 ms |
| Stretch | Measured gap | 0.100 / 5.778 | 0.020/0.120; 0.017/0.098 | 434 / 639 Hz | 0.104 / 0.701 | 0.020 / 0.056 | 0.103 / 0.623 | not reached / 0.45 s | 0 / −7.75 ms |
| Looping delay | Measured gap | 0.081 / 3.470 | 0.040/0.141; 0.039/0.132 | 460 / 333 Hz | 0.094 / 0.985 | 0.035 / 0.057 | 0.217 / 0.777 | 0.56 / 0.15 s | 0 / 18.0 ms |
| Spectral | Measured gap | 0.046 / 2.056 | 0.072/0.165; 0.067/0.157 | 748 / 1071 Hz | 0.111 / 0.576 | 0.083 / 0.042 | 0.391 / 0.914 | 0.33 / 0.78 s | 0 / −29.75 ms |

Median octave-band energy ratios in order 31.25–62.5 / 62.5–125 / 125–250 /
250–500 / 500–1000 / 1000–2000 Hz were:

- Granular: Clouds 0 / .001 / .504 / .173 / .216 / .009; Vactr 0 / .001 / .294 / .295 / .181 / .006.
- Stretch: Clouds 0 / .001 / .395 / .387 / .209 / .008; Vactr 0 / .001 / .292 / .231 / .193 / .069.
- Looping delay: Clouds 0 / .001 / .265 / .149 / .155 / .073; Vactr 0 / 0 / .224 / .211 / .128 / .002.
- Spectral: Clouds .001 / .002 / .043 / .136 / .169 / .422; Vactr .018 / .027 / .030 / .074 / .156 / .539.

Thresholds classify a mode as close only at median correlation
≥0.95 and normalized RMS error ≤0.20; all four are measurable gaps. These
results show response differences under this input and mapping; random grain
scheduling means correlation alone is not a parity test. No texture kernel fix
was justified by a clear translation error, and no coverage label changed.
The quality/resource notices describe the local-only build and non-import.

Independent review (2026-09-29) found that Vactr's 2–5× higher median
output level is not a probe scaling artifact. The per-scenario ratios range
from about 0.8× to 6×, and they follow documented DSP differences: host-rate
quality reduction instead of source 32→16 kHz conversion, linear instead of
equal-power dry/wet, stochastic grain scheduling, and analytic replacements
for generated windows. The probe's int16 input (×32767) and output (÷32768)
conversions differ by only 0.003%. Matching level structure is a fidelity
task, not a probe fix.


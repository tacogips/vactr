# Warps Vocoder Fidelity Plan

**Status**: Completed
**Design Reference**: `design-docs/specs/design-mutable-audio.md`, vocoder
fidelity paragraph
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Scope

Advance the `dual-mod` vocoder from its original authored 20-band
approximation toward the pinned MIT Warps signal path. Keep each translated
stage's provenance explicit, preserve Vactrol's `.vact` controls and
native/browser stereo bus contract, and avoid importing oscillator wave
tables or restricted drum assets. Source operating rate is 96 kHz with a
60-frame codec block; host-rate behavior is a separately verified Vactrol
adaptation.

## Subtasks

### WV-001: Output limiter

**Status**: Completed
**Parallelizable**: No
**Design Reference**: `design-docs/specs/design-mutable-audio.md`, vocoder
fidelity paragraph; pinned MIT `warps/dsp/limiter.h` and
`stmlib/dsp/dsp.h`.
**Deliverables**: A fixed-state limiter in a separate Rust source module or
small helper, integrated after the vocoder kernel and before the bridge.
Persist its peak in unused `FxState` scalar storage with source initial
value 0.5. Preserve seven public `dual-mod` parameters; no new control is
needed for a fixed source stage. Update source notice and module claims.

**Completion Criteria**:
- [x] Scalar tests cover source pre/post gain, attack/release, overload and bounded finite samples.
- [x] Vocoder block-partition continuity, `.vact` controls, main/aux routing, native/browser matrix and armed allocation checks pass; XMOD output stays unchanged.
- [x] Notice states exactly what was translated and what remains authored.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### WV-002: Source 20-band filter and follower design

**Status**: Completed
**Parallelizable**: No (depends on WV-001)
**Deliverables**: Audit `filter_bank.{h,cc}`, `vocoder.{h,cc}`,
`sample_rate_converter.h`, `sample_rate_conversion_filters.h`,
`resources/filter_bank.py`, `resources.cc` and `stmlib` SVF equations.
Specify preallocated state and installation capacity, 12×/3× decimation,
band gains/delays, 60-frame source cadence, follower peak and formant
interpolation, and behavior at 44.1/48/96 kHz. No Rust code in this task.

**Completion Criteria**:
- [x] Every candidate filter/SRC coefficient has a verified, individually permissive notice; oscillator and audio wave data remain excluded.
- [x] Memory and latency estimates fit host limits or produce install-time diagnostics without replacing a live bus.
- [x] Numerical and perceptual verification cases distinguish source 96-kHz parity from host-rate adaptation.

### WV-003: Streaming filter-bank and follower implementation

**Status**: Completed
**Parallelizable**: No (depends on WV-002)
**Deliverables**: Implement the audited 20-band analysis/synthesis,
multirate conversion, follower, formant gain and compensated delay path
at the source's 96 kHz operating rate. Buffer in source-sized 60-frame
groups independently of host callback partition; account for that latency.
Keep the present authored bank as an explicitly labeled non-96-kHz bridge
until WV-004. Retain fixed callback cost and `.vact`/browser/native routing.
Split touched Rust files that would reach 1,000 lines.

**Completion Criteria**:
- [x] Impulse, frequency, formant, release/freeze, band-delay and block-partition tests pass.
- [x] Strict install capacity rejects one-float-short memory before live-bus retirement; exact-fit renders audio.
- [x] Source-rate 96-kHz numerical tests and native/browser 44.1/48/96 kHz × 64/256 functional and armed zero-allocation tests pass; non-96-kHz fallback remains labeled.
- [x] Provenance, quiet Cargo, strict Clippy, wasm, full tests and independent review pass.

### WV-004: Host-rate conversion

**Status**: Completed
**Parallelizable**: No (depends on WV-003)
**Deliverables**: Add bounded, install-time generated, phase-interpolated
windowed-sinc FIR conversion around the source-rate vocoder for the full
8–192-kHz engine rate range. Use separate carrier/modulator histories,
a shared 96-kHz-to-host output history, relative phase accumulators that
stay bounded over long sessions, and rate-dependent anti-alias cutoff/
tap count. Keep 96 kHz as an exact converter bypass. Reserve coefficient
tables and rings in effect memory, update the exact `CrossMod` preflight
for any rate, preserve the fixed 60-frame internal cadence, channel roles,
live parameter changes and no-allocation callback. Remove the authored
fallback only after rate/latency behavior is measured and native/browser
tests pass. Retire its 100-float memory region when the converter succeeds.

**Completion Criteria**:
- [x] 8/44.1/48/96/192-kHz unity, impulse, pitch, alias, formant, freeze and latency tests establish the conversion boundary, including long-session phase stability.
- [x] Native/browser 44.1/48/96 kHz × 64/256-frame routing, exact/one-short install preflight and armed zero-allocation checks pass; other valid host rates stay finite and functional.
- [x] Notices and fidelity claims distinguish translated source stages from host-rate adaptation; independent review and full checks pass.

## Progress Log

### Session: 2026-09-28, WV-004 host-rate boundary

The previous 100-float authored non-96-kHz bank has been retired. The same
60-frame 96-kHz vocoder now runs behind Vactrol-generated input/output
Blackman-windowed sinc filters at other rates, with 33 coefficient phases,
48–576 taps, bounded high/low relative phase words and separate
carrier/modulator/output rings. At exactly 96 kHz, conversion is bypassed.
The source-rate limiter runs before output conversion. Required `dual-mod`
memory is 23,833/7,853/7,513/2,466/7,561 floats at
8/44.1/48/96/192 kHz respectively, with exact install preflight.

Focused unity, pitch, impulse onset, input/output alias, formant, freeze,
two-million-frame phase, stereo routing, native/browser host matrix and
armed zero-allocation tests pass. Independent check-and-test review found
no specific bug. Quiet fmt/native/no-default/wasm checks, strict
all-target Clippy, diff check and full tests pass: 1,483 passed, one
ignored. All touched Rust files remain under 1,000 lines.
Host-rate FIR phase response and carrier/amplifier differences still
prevent a full firmware parity claim.

### Session: 2026-09-28, WV-003 source-rate vocoder

Translated the MIT 20-band Warps bank and follower at 96 kHz with the
source 60-frame cadence. The 3×/36 and 4×/48 FIRs, two `CrossoverSvf`
passes per band, source post-gains and capped delay compensation,
previous-peak formant targets, envelope follower, 60-frame gain ramps
and synthesis order use fixed effect memory. The compact layout is 1,071
floats per bank, 2,402 for the source-rate vocoder, and 2,566 total for
`dual-mod` including XMOD and the non-96-kHz authored fallback. Strict
bus preflight rejects one-float-short installs before retiring live audio.
The output pipeline adds exactly 60 host frames of staging latency at
96 kHz. Seven `.vact` controls, stereo main/aux and the lower-rate
fallback remain functional.

The separate pinned C++ probe and Rust raw-kernel tests agree on six
96-kHz RMS/tail cases within 5×10⁻⁶ absolute and seven sample positions
within 10⁻⁵ after latency alignment. Independent review confirmed all
20 source filter rows and the 3×/4× FIR halves are f32-identical and
found no blocker. Quiet native/no-default/wasm checks, strict Clippy,
rustfmt and full tests passed: 1,475 passed, one ignored. The source
carrier/amplifier and non-96-kHz conversion remain distinct work.

### Session: 2026-09-28, WV-002 filter-bank design

Completed the pinned-source audit and 60-frame layout derivation above.
The source bank uses two cascaded `CrossoverSvf` passes per band, with
13 low-pass/interior bands after 12× conversion, six after 3× conversion,
and one full-rate high-pass band. Analysis uses two banks; synthesis sums
delay-compensated carrier bands and returns through 4× and 3× FIRs.
Follower rates derive from release and each band's sample rate; 60-frame
peak smoothing and formant interpolation drive block-ramped gains.
The 2,806-float core estimate is below default bus memory at supported
rates, with exact installation preflight required in WV-003. The source
caps compensation at 256 samples; 60-frame input staging adds at least
one block to the Vactrol port's latency and must be measured.

`mise run probe-warps-vocoder` compiled only the pinned MIT filter
coefficients into a temporary source reference and measured 6,000
finite 96-kHz samples per case for release/formant settings (0.1,0.25),
(0.5,0.5) and (0.9,0.75), plus gate-off tails at fast, slow and frozen
release. WV-003 should compare sample sequence,
steady-state RMS, tail after gate-off, impulse onset, and spectral
energy near 87, 1,397, 5,588 and 7,040 Hz against that separate
source, then listen to vowel sweeps and transient consonant material
at matched level. WV-004 must separately test pitch, formant movement,
aliasing and latency at 44.1 and 48 kHz. No waveform asset or output
recording enters the repository.

### Session: 2026-09-28, WV-001 output limiter

Translated the pinned MIT limiter stage after Vactrol's vocoder synthesis
and before the XMOD/vocoder bridge. Fixed scalar state starts its peak at
0.5; the source's 1.4 pre-gain, attack/release, reciprocal gain reduction,
0.8 post-gain and soft-limit equation are preserved, with coefficients
time-scaled from 96 kHz for the host rate. Source-shaped scalar, overload,
block partition, unaffected XMOD/aux, `.vact`, native/browser and armed
allocation checks pass. Independent review found no blocker; quiet
native/no-default/wasm checks, strict Clippy, rustfmt and full tests passed:
1,471 passed, one ignored. The 20-band filter and follower remain authored.

### Session: 2026-09-28

Audited pinned MIT limiter, vocoder and filter-bank structure. Source
vocoder uses 20 bands grouped at 12×, 3× and native rate, with a limiter
after synthesis; Vactrol currently uses an authored host-rate bank. WV-001
isolates the source output stage before the larger multirate replacement.
The pinned `filter_bank.py`, `resources.cc`, `filter_bank.{h,cc}`,
`sample_rate_conversion_filters.h`, `vocoder.{h,cc}`, `limiter.h` and
`stmlib/dsp/filter.h` each carry MIT notices. The generated filter-bank
entries are filter coefficients and per-band delay/gain data; the same
aggregate resource file also contains oscillator wave resources, which
remain excluded. The source runs at 96 kHz in 60-frame codec blocks,
with 13 bands at 8 kHz, six at 32 kHz and one at 96 kHz. Host-rate and
arbitrary-block equivalence require explicit design and tests in WV-002.
The source reserves a 6,144-float delay pool and 960-float band-sample
pool per bank for its 96-frame maximum. At its actual 60-frame codec
size, the 13/6/1 band grouping consumes 245 sample floats. Applying
`FilterBank::Init`'s 256-sample capped delay and converter compensation
to all 20 pinned table entries yields 456 delay floats per bank, including
one cursor slot per band. Its four SRC histories use 192 floats per bank
(72+12+96+12), and the two scratch arrays need 120. A compact Rust bank
can therefore budget 1,013 floats for these arrays before 240 SVF state
floats and 20 delay cursors: 1,273 floats per bank, or 2,546 for two
banks. Followers, previous gains, 60-frame input/output staging and
alignment add roughly 260 floats. WV-003 must finalize exact layout and
strictly preflight that size before retiring a live bus. The source's
reserved 6,144/960 figures remain useful as a conservative comparison;
they are not the active 60-frame requirement.

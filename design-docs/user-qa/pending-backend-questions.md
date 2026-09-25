# Pending: back-end questions from TASK-007..008 (2026-09-25, issue #3)

These questions came up while designing the scheduler, the DSP graph
and the native and wasm hosts (design section 12.8 in
`design-docs/specs/design-implementation.md`). Each one has a
recommendation, and the implementation follows it until the question
is answered. None of them blocks issue #3. B1 matters at review time,
because it decides whether one kind of evidence is accepted.

Already decided and not asked here: the scope chain, SOUND FIRST, d1..d9
as the only sinks, the named-argument rule, `inst` header parameters as
control names (M3), Q3/Q4/M1, host tiers, VM-only execution, no
allocation or locking on the audio callback, ratio time in the
scheduler, and `use-clock :link` as a diagnostic.

## B1. Evidence for the real-worklet criteria when headless Chrome cannot run

TASK-008 has two criteria that must run in a real AudioWorklet: cell
transport with `VoiceRelease`, and the 16.1 lifecycle checks. The plan
says a headless `process()` test alone is not accepted as proof. The
primary path is automated: `node editor/dev-harness/run-headless.mjs`
starts headless Chrome and writes a JSON report (design 12.8.11). The
workflow sandbox may not be able to start Chrome or open an audio
context.

- Recommendation: if the automated run exits 2 (blocked), the operator
  runs the same page headed with `run-headless.mjs --headed`. The
  report it writes is accepted as evidence, and the review lists it as
  operator-run. A blocked run never counts as a pass.
- Alternative: keep both criteria open until the automated run passes.
- Why ask: the issue exempts only the audible `examples/beep.rs` check.

## B2. Names inside `inst` and `bus` bodies

1. Implicit control names. The design-music examples use controls in
   `inst` bodies without declaring them in the header, for example
   `attack`, `release`, `amp`, `note` and `n` in `sampler` and
   `analog`.
   - Recommendation: inside an `inst` or `bus` body, a free name that
     is a row of the control table (design 12.8.7) and is not bound in
     scope is a control parameter. Everywhere else it stays
     `undefined-name`.
   - Alternative: every control must be declared in the header, and
     the spec examples get `undefined-name`.
2. DSP names that match pattern controls or signals. This is the
   TASK-008 follow-up that M1 announced: `saw`, `tri`, `lpf`, `hpf`,
   `bpf`, `delay`, `comb`, `gain`, `pan`, `room`, `bus`, `range`, and
   every effect name that is also a control.
   - Recommendation: add them to the fixed subject-overload group of
     M1. A pattern subject selects the pattern control or signal. A
     ugen subject, a numeric argument inside an `inst` body, or no
     subject inside a `bus` body selects the ugen or effect. `saw` and
     `tri` with no arguments stay the signals.
   - Alternative: rename one side, as Hydra `repeat` became `tile`.

## B3. `inst drum: sampler bank: :bd-haus ...:` (design-music section 4)

The sampler example reads as `inst drum:` with the body
`sampler bank: :bd-haus begin: 0 end: 1 loop: false:` followed by an
indented block. It could mean "define `drum` from the `sampler`
template with these defaults", or it could be the definition of the
`sampler` template itself.

- Recommendation: treat it as a spec erratum for the template
  definition, `inst sampler bank: :bd-haus begin: 0 end: 1 loop: false:`.
  Deriving an instrument from a template can be added later.
- Until answered: the prelude templates use the plain header form
  (design 12.8.6). The spec fixture for that block pins whatever
  diagnostics the current reader and checker produce, with a note that
  points here.

## B4. Sound quality of complex effects in v1

The effect catalog (design-music section 5) includes kinds whose
reference-quality algorithms need large state or lookahead:
`linear-phase-eq`, `group-delay-eq`, `pitch-shift-hq`, `denoise`,
`codec`, `spatial-map`, `crosstalk-cancel` and `fir-crossover`.

- Recommendation: v1 ships each of them as a documented approximation
  with fixed, preallocated state. The acceptance bar for every kind is
  finite, bounded output, bit-exact bypass, and no callback allocation.
  Sound quality is not part of acceptance in v1 (design 12.8.8).
- Alternative: implement reference-quality algorithms now. That
  enlarges TASK-008 a lot.

## B5. Moving the session socket from TASK-008 to TASK-009

The core plan lists a tungstenite session socket under
`NativeAudioHost`. The socket needs the `ClientMsg`/`ServerMsg`
protocol and the token rule of design section 17, and both belong to
TASK-009. No TASK-008 criterion exercises the socket.

- Recommendation: move it to TASK-009. The FINAL wave of issue #3
  amends the TASK-008 deliverable text (design 12.8.1).
- Related decision, recorded here for confirmation: `host-wasm` uses a
  raw `extern "C"` export ABI instead of wasm-bindgen. wasm-bindgen
  would need its CLI to generate JS glue, which is a build step beyond
  the wasm module (design 12.8.10, section 4).

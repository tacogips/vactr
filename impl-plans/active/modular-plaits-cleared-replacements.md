# Plaits ROM-Backed Engine Replacements

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

This plan began with four unavailable positions from
`modular-plaits-engines.md`. Plaits positions 2–4 register the MIT
`SixOpEngine` with three DX7-derived factory patch banks. Position 15
registers `SpeechEngine`, whose naive/SAM/LPC architecture includes
source word data described as extracted from TI speech ROMs. Neither
bundled bank nor extracted word sequence is cleared for inclusion in
Vactrol. A runnable replacement may use original operator configurations
and original phoneme/word definitions, or a user-provided data contract;
its coverage must remain `Adaptation`/`Replacement` until source fidelity
and resource status are independently established.

## Modules

### `src/dsp/ugen/six_op_original.rs`

```rust
pub fn render(model: u8, state: &mut VoiceState, controls: &SixOpControls,
              main: &mut [f32], aux: &mut [f32], sample_rate: f32);
```

Use six bounded operators, codeable ratios/levels/feedback/algorithm or
explicit original patch choices, and all source engine macro controls.
The three public positions must have distinct, stable selectable roles.
No DX7 bytes, encoded patches, imported ROM presets or table-derived
audio may be included.

### `src/dsp/ugen/speech_original.rs`

```rust
pub fn render(state: &mut VoiceState, controls: &SpeechControls,
              main: &mut [f32], aux: &mut [f32], sample_rate: f32);
```

Provide naive/SAM-like oscillator/formant roles and independently authored
LPC-like or articulatory data where feasible. Preserve group, timbre,
morph, accent, trigger and main/aux controls. An unavailable word-bank
range must produce a diagnostic rather than silently alias another range.
Review `SpeechEngine` subdependencies individually; do not import the TI
word arrays or aggregate `resources.cc`.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| ROM-001 | Six-op engine and speech dependency/control/resource audit | MOD-006 | Render contracts audited; transitive model assets and candidate data still need individual clearance |
| ROM-002 | Original six-op bank A/B/C replacements and user-data policy | ROM-001 | Three original 32-configuration banks runnable; user data loading unsupported |
| ROM-003 | Original speech/formant/LPC-like replacement with three model roles | ROM-001 | Three modes and four original upper-range tokens runnable; source comparisons open |
| ROM-004 | Main/aux, envelope, trigger, accent and all `.vact`/editor controls | ROM-002..003 | Positions 2–4 and 15 routed/tested; exact source envelope parity open |
| ROM-005 | Manifest/provenance, deterministic source-role and native/browser comparison | ROM-002..004 | Positions 2–4 and 15 covered; source numeric comparison open |

## Completion Criteria

- [x] Positions 2–4 and 15 have working, explicitly original replacement paths.
- [x] Every source macro control and both outputs are codeable and tested through the replacement interface.
- [x] No DX7-derived patch, TI ROM word, generated aggregate resource or LXR asset is imported.
- [x] Native/browser rates, capacity and zero callback allocations pass.
- [x] Fidelity status and third-party notices distinguish replacement from source port.
- [x] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass.

Source-numeric comparisons, user-data loading, and exact published-word
fidelity remain open outside these original replacement criteria.

## Progress Log

### Session: 2026-09-28 — original speech replacement at position 15

`speech-voice` covers naive pulse/formant, SAM-like reset formants, and an
LPC-like reflection filter with procedural excitation. Its upper selector
range contains four original Vactrol syllable tokens (`ava`, `omi`, `era`,
`unu`) generated from authored analytic vowel trajectories; it never aliases
or imports the TI word banks. Pitch, `speech-harmonics` model/token selection,
`timbre`, `morph`, `velocity`, and `speech-sustain` are codeable and
editor-visible. Event onset resets state; source-style main formant and
auxiliary excitation outputs remain distinct. Two fixed 20-float states
are allocated before callbacks. The public manifest marks position 15
Adaptation/Replacement and retains its TI flag only as upstream provenance.
No source phoneme array, LPC frame/coefficient/energy lookup, excitation
pulse table, TI word bytes, aggregate resource, or LXR data is imported.
Exact phonemes, prosody, source quantizer boundaries, filters, envelope
curves, word audio, and numerical comparisons remain open. `catalog.rs`
was split into a main catalog and `catalog/voice_ports.rs` before adding
this voice, keeping both Rust files below 1000 lines.


### Session: 2026-09-28 — original six-operator FM banks 2–4

`six-bank-a-voice`, `six-bank-b-voice`, and `six-bank-c-voice` expose 32
independently generated configurations per bank. Their six analytic sine
operators use three distinct routing topologies rather than published DX7
factory patches. `six-patch`, `timbre`, `morph`, `velocity`, `six-sustain`,
and note/frequency are codeable and editor-visible. Event onset resets the
operator phases and envelopes. Main and auxiliary duplicate the same signal,
matching the source output relationship. Two 19-float states are preallocated
per template, one for each output, and low memory budgets fail at install.
Upstream DX7 flags remain provenance facts while these rows are marked
Adaptation/Replacement. Source LFO scrub, patch envelope/ratio/level data,
staggered multi-voice scheduling, exact numerics, and user-data loading are
not represented. No SYX, factory bytes, generated resource or LXR asset is
imported. At that checkpoint, position 15 remained Unavailable pending its
separate replacement.


### Session: 2026-09-28 — source boundary

The pinned `plaits/dsp/engine2/six_op_engine.h` exposes bank loading,
user-data loading and main/aux rendering. The pinned
`plaits/dsp/engine/speech_engine.cc` interpolates naive, SAM and LPC
models, and selects LPC word banks in its upper group range. The bundled
factory patches and TI-derived word data remain excluded. No replacement
engine had landed at the initial audit; all four positions were then
`Unavailable` in the public manifest rather than being misreported as
playable source ports.

The `SixOpEngine::Render` audit at the pinned revision confirms that
`harmonics` quantizes one of 32 patches per bank, `timbre` controls
brightness, `morph` controls envelope/LFO scrub, `accent` supplies
velocity, and note/trigger select pitch and gate behavior. Its source
currently writes the same soft-clipped mix to main and aux; replacement
tests should retain this documented output relationship unless an
intentional Vactrol extension is declared. The source uses multiple
staggered FM voices and a proprietary-format factory-patch payload;
neither patch bytes nor patch-derived ratios/levels will be copied.

`SpeechEngine::Render` maps `harmonics * 6` over naive, SAM and LPC
models for the lower range, then selects LPC phonemes and word banks in
the upper range. `morph` and `timbre` are passed to all three model
renderers, while `accent` is passed to LPC word replay. A replacement
can offer original phoneme definitions and a clearly separate Vactrol
word set, but must not silently stand in for the excluded TI-derived
word banks. Before implementation, audit the naive/SAM/LPC transitive
data files individually and define the exact available `harmonics`
range and any diagnostic for the unsupported source word-bank range.
The transitive audit also found embedded phoneme/formant arrays in
`naive_speech_synth.cc` and `sam_speech_synth.cc`, a formant amplitude
table in the latter, and an LPC excitation-pulse table referenced via
`plaits/resources.h`. These are separate resource decisions: the
replacement should author its own phoneme definitions and synthesize
excitation procedurally, rather than importing the aggregate resource
or assuming every transitive numeric table has verified provenance.

# Elements Exciter, Resonator and Stereo Effect Coverage

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-29

## Design Document Reference

Split SYN-005 and FX-004. The pinned Elements `dsp/patch.h` has twenty
continuous fields; `dsp/part.h` adds gate, note, modulation and strength,
two audio excitation inputs, main/aux outputs, bypass and an alternate
voice. Its `dsp/voice.h` selects modal, string or strings resonators.
The Rust host's 30-port node ceiling carries the 30-port internal instrument
(twenty Patch fields, four performance fields, model, main/aux selection,
alternate selection and frequency). The 29-port effect uses one audio port
plus 28 parameters;
`exciter-bank` maps its stereo L/R bus
inputs to blow/strike excitation and L/R outputs to main/aux. The instrument
graph now has named `ex-external-blow` and `ex-external-strike` ports,
fed by opt-in `host-in-l/r` source UGens. Both paths are adaptations of the
source `Part` boundary.

## Resource Boundary

`elements/resources/samples.py` explicitly declares GPL-3.0-or-later and
packages `hit_01.wav` through `hit_09.wav` and `noise.wav` into generated
sample data. Those recordings have no individually verified redistribution
grant in the inspected tree. Do not import that generator, the recordings,
or the sample arrays embedded in aggregate `resources.cc` into Vactr's
MIT core. Inspect source tables individually; use original procedural
exciters or a user-provided sample contract with clear diagnostics.
The inspected DSP source headers carry Emilie Gillet's MIT terms, which
does not settle rights in those distinct resource files.

## Modules

### `src/dsp/ported/elements.rs`

```rust
pub struct ElementsSpec {
    pub mode: u8,
    pub source_name: &'static str,
    pub vactr_template: Option<&'static str>,
    pub status: Fidelity,
    pub resources: ResourceState,
}
pub fn resonator_modes() -> &'static [ElementsSpec; 3];
```

Keep the twenty patch roles in a stable typed schema. Allocate delay,
modal and reverb state at install time; the callback performs no I/O,
allocation or resource parsing. Mark any original exciter/reverb
replacement `Adaptation` until source comparison.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| ELM-001 | Full Patch, performance, alternate voice and resource inventory | SYN-005 | Twenty-field patch, three models and sample boundary enumerated; deeper dependency audit pending |
| ELM-002 | Typed transport for twenty patch controls and two excitation inputs | ELM-001 | Instrument fields, separate bus L/R blow/strike and opt-in instrument-graph inputs runnable |
| ELM-003 | Bow, blow, strike and envelope source roles with original cleared excitation | ELM-002 | Procedural internal exciters runnable; source sample and exact exciter parity pending |
| ELM-004 | Modal/string/strings resonators and main/aux paths | ELM-002 | Three original roles and separate outputs runnable; five-string source parity pending |
| ELM-005 | Diffuser/reverb, modulation and alternate voice/effect placement | ELM-003..004 | Procedural alternate FM/spatial voice runnable in instrument and bus; source FX/oversampling parity pending |
| ELM-006 | Source comparisons, notices, native/browser and allocation verification | ELM-003..005 | Completed local Part comparison for all three resonator models; 9 bow-only measured gaps and 6 sample-dependent replacements; no parity claimed |

## Completion Criteria

- [x] Twenty patch fields and all internal performance/mode controls reach `.vact` and editor metadata.
- [x] Bow, blow and strike roles, gate and main/aux have defined internal/bus routing; named instrument external ports are opt-in and tested.
- [x] Three resonator modes and alternate voice/effects have truthful adaptation coverage and focused tests.
- [x] GPL sample generator and uncleared recordings are excluded from the internal adaptation.
- [x] Native/browser rates, capacity and zero callback allocation pass for the internal adaptation.
- [x] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass for the internal adaptation.
- [x] Source comparison covers MODAL, STRING and STRINGS with three bow-only and two sample-dependent scenarios each; metrics are recorded without claiming source parity.

## Progress Log

### Session: 2026-09-28 — exact space-freeze boundary

Regression tests now assert that the normal bus echo and alternate FM echo
stop writing at exactly `ex-space=1.75` and resume writing at `1.749`.
The DSP threshold and control mapping were not changed.

### Session: 2026-09-28 — alternate voice adaptation

Added `ex-alternate` to `exciter-voice` and `exciter-bank`, preserving
existing port and effect parameter indices. The fourth manifest row is now
`Adaptation/Replacement`. An authored two-pair FM oscillator with spatial
motion and fourfold analytic oversampling uses 320 already preallocated
floats per path; it imports no source FIR, generated oscillator lookup,
sample array or numeric table. All twenty Patch fields and gate/note/
modulation/strength influence the alternate kernel; the normal resonator
model selector is intentionally inapplicable while alternate is selected.
The bus effect retains independent left blow/right strike excitation and
main/aux outputs. The `ex-space` range is 0..2, with bounded echo-write
freeze from 1.75. The source's exact 8×/101-tap antialiasing, multimode
filter, spatial diffuser and raw-gain regions are not reproduced. Named
instrument-graph external audio ports and source numerical comparisons
remain open.

### Session: 2026-09-28 — independent blow/strike bus effect

Added `exciter-bank`, a stereo bus/master effect. Left input excites a
procedural blow path; right input excites a distinct strike path. The effect
returns main and auxiliary on left/right and exposes all twenty Patch fields,
four performance controls, three model choices, external/internal blend and
dry/wet mix. The fixed effect array grew from 17 to 27 parameters and node
ports from 27 to 28 for voice-effect compatibility; the effect kind was
appended to preserve earlier wire ordinals. Four strings remain rate-sized to
20 Hz at 96 kHz. Installation checks the exact state budget before retiring
a live bus and does not require capture allowance. Separate source
sensitivity, silence, all-control response, codec/editor, native/browser
rate/block and callback-allocation tests cover this adaptation.

The source `Part` takes two mono excitation inputs and uses a different
exciter/filter/reverb topology. The Vactr stereo bus mapping is explicit;
the source numerical topology remains Pending. Opt-in named instrument-graph
external ports are runnable. No GPL generator, bundled WAV or aggregate
resource enters the DSP.

### Session: 2026-09-28 — opt-in instrument graph excitation

`exciter-external-voice` wires validated host L/R to new blow/strike ports
28/29 of the existing internal core. Prior port indices and internal-template
defaults remain unchanged; `MAX_PORTS=30` permits the last port without
truncation. Both main and aux react independently in native/browser tests,
including stereo and quad hosts. This remains an event-local original
adaptation, not the source `Part` exciter topology.

### Session: 2026-09-28 — procedural internal voice slice

Added `exciter-voice` with twenty Patch controls, gate, note, modulation,
strength and three resonator selections in `.vact`/editor. Two fixed-state
nodes render main and aux independently. A 27-port contract and appended
graph codec ordinal 76 carry the last output selector without truncation;
`MAX_PARAMS=48` and event `MAX_CTLS=32` remain sufficient. The renderer uses
original analytic bow/blow/strike excitation, eight modal resonances,
single-string and four-string roles, host-rate-sized lines to 20 Hz, and a
short feedback space. The default half-second per-voice memory budget holds
both output nodes: 18,936/20,496/39,696 floats at 44.1/48/96 kHz.

The source Strings role uses five strings and a shared `Part` with separate
blow/strike audio inputs; neither external input nor the alternate voice is
claimed by this internal template. Source sample recordings, GPL generator,
LUTs and aggregate arrays are excluded. Control-response, editor, codec,
capacity and native/browser rate/block tests cover the runnable adaptation.

### Session: 2026-09-28 — source/resource inventory

At revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`, the DSP
`Patch` lists twenty floats, and the `Part` interface has separate blow and
strike inputs plus main and aux outputs. `voice.h` registers three
resonator models. The sample generator carries a GPL-3.0-or-later header;
the bundled WAV recordings were not individually cleared. No Elements
source or data has been imported into Vactr. Further implementation
must keep the MIT DSP code and resource rights separate.

### Session: 2026-09-29 — ELM-006 Part source comparison

Added a local `Part::Process` reference driver and an `exciter-core` raw-kernel
probe. The pinned source rate is 32 kHz and `elements/dsp/dsp.h` sets a
16-frame maximum block, also used by the device callback. Each run held gate on
at note 60/C4 with fixed strength and compared the main and auxiliary outputs.
The twenty Patch controls were mapped in the `exciter-core` `ex-*` order.
Each resonator has three sample-independent bow-only patch settings and two
sample-dependent strike/blow settings. `elements/resources.cc` was compiled
only for the temporary local reference executable because source exciter
symbols depend on its aggregate tables; it and the GPL-3.0-or-later sample
generator remain excluded from Vactr.

| Model | Scenario | Classification | Main RMS ref/Vactr | Aux RMS ref/Vactr | Main/aux correlation | Main/aux normalized RMS error |
|---|---|---|---:|---:|---:|---:|
| MODAL | bow-warm | measured gap | 0.086/0.490 | 0.086/0.112 | 0.091/-0.017 | 5.692/1.658 |
| MODAL | bow-bright | measured gap | 0.086/0.655 | 0.082/0.310 | 0.010/-0.046 | 7.627/3.945 |
| MODAL | bow-muted | measured gap | 0.225/0.526 | 0.225/0.444 | -0.147/-0.189 | 2.674/2.375 |
| MODAL | sample-blow | replacement | 0.460/0.371 | 0.421/0.323 | -0.006/0.003 | 1.289/1.258 |
| MODAL | sample-strike | replacement | 0.074/0.190 | 0.063/0.158 | 0.016/0.018 | 2.751/2.660 |
| STRING | bow-warm | measured gap | 0.039/0.104 | 0.039/0.077 | -0.036/-0.040 | 2.894/2.272 |
| STRING | bow-bright | measured gap | 0.127/0.760 | 0.127/0.459 | 0.005/0.009 | 6.083/3.742 |
| STRING | bow-muted | measured gap | 0.211/0.092 | 0.211/0.076 | -0.014/-0.022 | 1.096/1.070 |
| STRING | sample-blow | replacement | 0.534/0.057 | 0.534/0.070 | -0.002/-0.005 | 1.006/1.009 |
| STRING | sample-strike | replacement | 0.070/0.146 | 0.070/0.067 | 0.069/0.006 | 2.279/1.390 |
| STRINGS | bow-warm | measured gap | 0.035/0.008 | 0.037/0.031 | 0.002/0.012 | 1.023/1.294 |
| STRINGS | bow-bright | measured gap | 0.092/0.051 | 0.103/0.162 | 0.017/0.002 | 1.133/1.858 |
| STRINGS | bow-muted | measured gap | 0.070/0.010 | 0.071/0.041 | -0.004/0.007 | 1.010/1.152 |
| STRINGS | sample-blow | replacement | 0.317/0.014 | 0.358/0.061 | -0.002/0.002 | 1.001/1.014 |
| STRINGS | sample-strike | replacement | 0.035/0.095 | 0.036/0.022 | 0.031/0.072 | 2.888/1.141 |

The comparison script additionally reports per-channel spectral centroid,
normalized octave-band power ratios, -20/-40 dB decay times, and a
fixed-note-range autocorrelation fundamental estimate. The nine bow-only
cases all classify as measured gaps; their aligned correlations range from
-0.189 to 0.091 and normalized errors from 1.010 to 7.627. The six
sample-dependent cases are classified `replacement - not expected to match`.
These results show substantial numerical and spectral differences at the
raw kernel boundary and do not establish full graph, effect, resource, or
source parity. No kernel fix was justified as a clear translation error;
Elements coverage labels remain `Adaptation`/`Replacement`, with no
`SourcePort` label.

Verification for this session passed rustfmt, native Cargo check, strict
Clippy, wasm32 library check, `mise tasks validate`, the comparison task, and
the upstream audit (240 inventoried files, no warnings or errors). Nextest
reported 1,000 passed, 2 failed, 1 skipped, and 530 not run: the two failures
were loopback HTTP fixture tests whose `127.0.0.1` bind returned
`PermissionDenied` in this sandbox.

# Elements Exciter, Resonator and Stereo Effect Coverage

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Split SYN-005 and FX-004. The pinned Elements `dsp/patch.h` has twenty
continuous fields; `dsp/part.h` adds gate, note, modulation and strength,
two audio excitation inputs, main/aux outputs, bypass and an alternate
voice. Its `dsp/voice.h` selects modal, string or strings resonators.
The Rust host's 30-port node ceiling carries the 30-port internal instrument
(twenty Patch fields, four performance fields, model, main/aux selection,
alternate selection and frequency). The 29-port effect uses one audio port
plus 28 parameters;
`elements-bank` maps its stereo L/R bus
inputs to blow/strike excitation and L/R outputs to main/aux. The instrument
graph now has named `el-external-blow` and `el-external-strike` ports,
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
| ELM-006 | Source comparisons, notices, native/browser and allocation verification | ELM-003..005 | Internal adaptation notices and native/browser/allocation checks passed; source comparisons and pending paths remain |

## Completion Criteria

- [x] Twenty patch fields and all internal performance/mode controls reach `.vact` and editor metadata.
- [x] Bow, blow and strike roles, gate and main/aux have defined internal/bus routing; named instrument external ports are opt-in and tested.
- [x] Three resonator modes and alternate voice/effects have truthful adaptation coverage and focused tests.
- [x] GPL sample generator and uncleared recordings are excluded from the internal adaptation.
- [x] Native/browser rates, capacity and zero callback allocation pass for the internal adaptation.
- [x] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass for the internal adaptation.

## Progress Log

### Session: 2026-09-28 — exact space-freeze boundary

Regression tests now assert that the normal bus echo and alternate FM echo
stop writing at exactly `el-space=1.75` and resume writing at `1.749`.
The DSP threshold and control mapping were not changed.

### Session: 2026-09-28 — alternate voice adaptation

Added `el-alternate` to `elements-voice` and `elements-bank`, preserving
existing port and effect parameter indices. The fourth manifest row is now
`Adaptation/Replacement`. An authored two-pair FM oscillator with spatial
motion and fourfold analytic oversampling uses 320 already preallocated
floats per path; it imports no source FIR, generated oscillator lookup,
sample array or numeric table. All twenty Patch fields and gate/note/
modulation/strength influence the alternate kernel; the normal resonator
model selector is intentionally inapplicable while alternate is selected.
The bus effect retains independent left blow/right strike excitation and
main/aux outputs. The `el-space` range is 0..2, with bounded echo-write
freeze from 1.75. The source's exact 8×/101-tap antialiasing, multimode
filter, spatial diffuser and raw-gain regions are not reproduced. Named
instrument-graph external audio ports and source numerical comparisons
remain open.

### Session: 2026-09-28 — independent blow/strike bus effect

Added `elements-bank`, a stereo bus/master effect. Left input excites a
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

`elements-external-voice` wires validated host L/R to new blow/strike ports
28/29 of the existing internal core. Prior port indices and internal-template
defaults remain unchanged; `MAX_PORTS=30` permits the last port without
truncation. Both main and aux react independently in native/browser tests,
including stereo and quad hosts. This remains an event-local original
adaptation, not the source `Part` exciter topology.

### Session: 2026-09-28 — procedural internal voice slice

Added `elements-voice` with twenty Patch controls, gate, note, modulation,
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

# Implementation Plans

This directory contains implementation plans that translate design documents into actionable implementation specifications.

The [GPU canvas editor cutover](active/canvas-cutover-dispatch.json) is Completed (2026-10-07; design 15.3.8): all plans accepted; canonical silent release-wasm `run-001` passes in Chromium and WebKit; WebKit input p95 is 16 ms and sync p95 is 12.7 ms. The historical canvas-editor-224 chain is superseded.

The [editor UI style redesign](completed/ui-style-dispatch.json) is Completed (design-ui-style.md, 2026-10-05): [shell](completed/ui-style-shell.md), [bind and params](completed/ui-style-bind-params.md), [code](completed/ui-style-code.md), [panels](completed/ui-style-panels.md) and [verify](completed/ui-style-verify.md). Follow-up F1 (canvas palette mapping) remains open.

[Query-issued song authority](active/song-mode-issued-query-authority.md) is In Progress. Its reviewed eight-path scope is released for the live query/replay provenance bridge after accepted retained geometry.

[Immutable route authority](completed/song-mode-immutable-route-authority.md) is Completed (session 261 final receipt).

[Frozen issued events](active/song-mode-frozen-issued-events.md) is released for its five-path implementation. [Issued playback](completed/song-mode-issued-playback.md) is Completed (session 261 final receipt).

[Retained execution membership](active/song-mode-retained-execution-membership.md) has its two-path source accepted after membership0003 verification; actual immutable-route consumer adoption remains open. It authenticates genuine retained executions without exporting raw records; varying seeds and permitted first executions still require admission work.

[Authenticated issued route resolution](completed/song-mode-issued-route-resolution.md) is Completed (session 261 final receipt).

[Structural clock hooks](completed/song-mode-structural-clock-hooks.md) is Completed (session 261 final receipt).

[Route preparation metering](active/song-mode-route-preparation-meter.md) has held0003 source accepted after eight focused tests, 425 broader Rust tests, and fresh frontend/build verification. Actual route-builder adoption is now released in the following phase. It preserves original nested depth and certification failure work before the immutable routing bridge.

## DOM renderer comparison (2026-10-08)

The [DOM renderer comparison](../design-docs/specs/design-dom-renderer.md) plans are Completed: [core](completed/dom-renderer-core.md), [mount](completed/dom-renderer-mount.md), [harness](completed/dom-renderer-harness.md), and [evidence](completed/dom-renderer-evidence.md); the [dispatch manifest](completed/dom-renderer-s296-dispatch.json) is archived. The measured report and raw data remain at [design-renderer-comparison.md](../design-docs/specs/design-renderer-comparison.md) and `design-docs/specs/evidence/renderer-comparison/rc-001/`. The DOM backend was removed afterwards because the comparison measured no performance benefit over canvas.

## Purpose

Implementation plans bridge design documents (what to build) and actual code (how to build). They provide:
- Clear deliverables without code
- Trait and function specifications
- Dependency mapping for concurrent execution
- Progress tracking across sessions

## Directory Structure

```
impl-plans/
├── README.md              # This file
├── active/                # Currently active implementation plans
│   └── <feature>.md       # One file per feature being implemented
├── completed/             # Completed implementation plans (archive)
│   └── <feature>.md       # Completed plans for reference
└── templates/             # Plan templates
    └── plan-template.md   # Standard plan template
```

## File Size Limits

**IMPORTANT**: Implementation plan files must stay under 400 lines to prevent OOM errors.

| Metric | Limit |
|--------|-------|
| Line count | MAX 1000 lines |
| Modules per plan | MAX 8 modules |
| Tasks per plan | MAX 10 tasks |

Large features are split into multiple related plans with cross-references.

## Active Plans


| Plan | Status | Design Reference | Last Updated |
|------|--------|------------------|--------------|
| [song-mode-live-isolation.md](active/song-mode-live-isolation.md) | Ready; reject live clock and reverse legacy mixing during finite ownership | design-song-mode.md timing and playback | 2026-10-03 |
| [song-mode-controller-ack-pressure.md](active/song-mode-controller-ack-pressure.md) | Ready; actual controller critical acknowledgment saturation | design-song-mode.md live Apply review | 2026-10-03 |
| [song-mode-sample-forwarding.md](active/song-mode-sample-forwarding.md) | Ready; checked sample forwarding in genuine provider fixtures | design-song-mode.md sample admission reassessment | 2026-10-03 |
| [song-mode-atomic-apply-pressure.md](active/song-mode-atomic-apply-pressure.md) | In Progress; bounded receipts, reservations and lifecycle proof | design-song-mode.md implementation review | 2026-10-03 |
| [song-mode-atomic-apply-controller.md](active/song-mode-atomic-apply-controller.md) | In Progress; exact live replacement and semantic mute continuity | design-song-mode.md implementation review | 2026-10-03 |
| [song-mode-handoff-recovery.md](active/song-mode-handoff-recovery.md) | In Progress; authenticated cancellation restores original Apply authority | design-song-mode.md implementation review | 2026-10-03 |
| [song-mode-index-occupancy.md](active/song-mode-index-occupancy.md) | In Progress; authenticated Slice support and bounds consumer | design-song-mode.md routing, identity and bounds | 2026-10-02 |
| [song-mode-index-static-joint.md](active/song-mode-index-static-joint.md) | Planning; shared static compiler implemented and focused tests pass; full geometry remains open | design-song-mode.md routing and bounds | 2026-10-03 |
| [song-mode-canonical-index-occupancy.md](active/song-mode-canonical-index-occupancy.md) | In Progress; collector source under review; immutable consumers and varying-seed bounds pending | design-song-mode.md remaining occupancy implementation direction | 2026-10-03 |
| [song-mode-canonical-query-replay.md](completed/song-mode-canonical-query-replay.md) | Completed; genuine replay and348-test checkpoint verified | design-song-mode.md canonical collector review | 2026-10-03 |
| [song-mode-canonical-clock-frames.md](completed/song-mode-canonical-clock-frames.md) | Completed; genuine runtime clocks,355 Rust tests and fresh590 editor tests verified | design-song-mode.md immutable consumers and authentic clock capture | 2026-10-03 |
| [song-mode-clock-transaction-fixtures.md](completed/song-mode-clock-transaction-fixtures.md) | Completed; actual snapshot publication and normal-stack regressions verified | design-song-mode.md current completion path | 2026-10-03 |
| [song-mode-canonical-sampling-relations.md](completed/song-mode-canonical-sampling-relations.md) | Completed; nine genuine sampling tests and joined gates accepted | design-song-mode.md immutable consumers and authentic clock capture | 2026-10-03 |
| [song-mode-source-query-extraction.md](completed/song-mode-source-query-extraction.md) | Completed; extraction-only equivalence and joined gates accepted | design-song-mode.md immutable consumers and authentic clock capture | 2026-10-03 |
| [song-mode-sampled-replay-binding.md](completed/song-mode-sampled-replay-binding.md) | Completed; genuine independent-owner to sampled-source rebinding accepted | design-song-mode.md immutable consumers and authentic clock capture | 2026-10-03 |
| [song-mode-sampling-fixture-inventory.md](completed/song-mode-sampling-fixture-inventory.md) | Completed; same-original inventory and dependency preparation accepted | design-song-mode.md current completion path | 2026-10-03 |
| [song-mode-owner-invocation-retention.md](active/song-mode-owner-invocation-retention.md) | In Progress; actual invocation records implemented, alias repair undergoing independent final gates | design-song-mode.md current completion path | 2026-10-03 |
| [song-mode-retained-invocation-lookup.md](active/song-mode-retained-invocation-lookup.md) | In Progress; execution gates accepted380Rust/590editor; geometry companion pending | design-song-mode.md immutable consumers | 2026-10-03 |
| [song-mode-retained-index-geometry.md](completed/song-mode-retained-index-geometry.md) | Completed;397 Rust and590 frontend tests plus scoped gates accepted; production integration remains | design-song-mode.md current completion path | 2026-10-03 |
| [song-mode-canonical-native-meter.md](completed/song-mode-canonical-native-meter.md) | Completed; transitive meter verified in joined307-test checkpoint | design-song-mode.md canonical collector review | 2026-10-03 |
| [song-mode-routing-consumer-splits.md](completed/song-mode-routing-consumer-splits.md) | Completed; original13 fixtures and joined gates pass | design-song-mode.md canonical collector review | 2026-10-03 |
| [song-mode-index-runtime-fixtures.md](active/song-mode-index-runtime-fixtures.md) | In Progress; genuine Index runtime witnesses and public Subject regression | design-song-mode.md routing and identity | 2026-10-02 |
| [song-mode-index-families.md](active/song-mode-index-families.md) | In Progress; connected multiple leaves/copies and positive affine issuer clocks | design-song-mode.md routing, identity and bounds | 2026-10-02 |
| [song-mode-fixed-callable-sources.md](active/song-mode-fixed-callable-sources.md) | In Progress; genuine fixture repairs under independent verification | design-song-mode.md identity, editing and bounds | 2026-10-02 |
| [song-mode-bounded-reservations.md](active/song-mode-bounded-reservations.md) | In Progress; explicit complete-lease resource/work bounds awaiting joined verification | design-song-mode.md bounds and routing | 2026-10-02 |
| [song-mode-checked-graph-admission.md](active/song-mode-checked-graph-admission.md) | In Progress; typed browser graph admission verified in 21 scoped tests; wider integration pending | design-song-mode.md playback and bounds | 2026-10-02 |
| [song-mode-host-preparation.md](active/song-mode-host-preparation.md) | In Progress; consuming owner implementation underway; joined verification pending | design-song-mode.md playback, application and bounds | 2026-10-02 |
| [song-mode-time-source-inference.md](active/song-mode-time-source-inference.md) | In Progress; coherent time/result pattern inference implemented, public verification pending | design-song-mode.md editing and bounds | 2026-10-02 |
| [song-mode-time-source-effects.md](active/song-mode-time-source-effects.md) | In Progress; local query-effect guard held; joined verification pending | design-song-mode.md editing and bounds | 2026-10-02 |
| [song-mode-placement-overlap.md](active/song-mode-placement-overlap.md) | In Progress; checked half-open ordinary overlap held; joined verification pending | design-song-mode.md routing and bounds | 2026-10-02 |
| [song-mode-native-configured-preparation.md](active/song-mode-native-configured-preparation.md) | In Progress; genuine configured Native capacity and full owner fixtures held | design-song-mode.md playback and application | 2026-10-02 |
| [cmp-closeout-dispatch.json](active/cmp-closeout-dispatch.json) | Dispatch manifest for the session-234 closeout (plans [CMP-40]; 11 accepted dependencies; all 12 plans completed and archived session 234) | design-completion.md 8-9; design-formatter-and-syntax.md 8 | 2026-09-30 |
| [fst-dispatch.json](active/fst-dispatch.json) | Dispatch manifest for FST-10..40 (waves [10,11] [20,21,22,23] [30] [40]; all eight plans completed and archived session 222) | - | 2026-09-30 |
| [modular-audio-handoff.md](active/modular-audio-handoff.md) | Ready; PLV-001 voice layer measured SourceStage and wired into 24 templates; MOD-004 stereo/multi-output edges complete; remaining audio TODOs prioritized | design-mutable-audio.md; design-music.md 4.1 | 2026-09-30 |
| [modular-audio-foundation.md](active/modular-audio-foundation.md) | In progress; MOD-001 inventory/audit, MOD-002 neutral names and MOD-004 stereo and multi-output edges complete; MOD-005/006 pending | design-mutable-audio.md | 2026-09-30 |
| [mod004-00-baseline.md](active/mod004-00-baseline.md) | Completed (85a300a); wave 0 golden digests and test scaffolding | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-10-shape-contract.md](active/mod004-10-shape-contract.md) | Completed (85a300a); wave 1 shape contract and edge output index | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-11-engine-split.md](active/mod004-11-engine-split.md) | Completed (85a300a); wave 1 behavior-neutral engine render split | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-12-kernel-pairs.md](active/mod004-12-kernel-pairs.md) | Completed (85a300a); wave 1 main/aux kernel pairs and stereo sample playback | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-20-select-lowering.md](active/mod004-20-select-lowering.md) | Completed (b6fa077); wave 2 `.vact` selection, checker and lowering | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-21-codec.md](active/mod004-21-codec.md) | Completed (b6fa077); wave 2 graph codec shapes and output indexes | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-22-voice-runtime.md](active/mod004-22-voice-runtime.md) | Completed (b6fa077); wave 2 dense buffers, stereo voices, balance pan | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-30-template-migration.md](active/mod004-30-template-migration.md) | Completed (0a15742); wave 3 nine-template migration | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [mod004-40-regression-closeout.md](active/mod004-40-regression-closeout.md) | Completed; wave 4 cross-path regression and MOD-004A..F closeout | design-mutable-audio.md MOD-004 | 2026-09-30 |
| [modular-fm-drums.md](active/modular-fm-drums.md) | In progress; three percussion voices, Peaks FM source stages verified, numerical parity and broader EFM family pending | design-mutable-audio.md | 2026-09-28 |
| [modular-synth-engines.md](active/modular-synth-engines.md) | Planning; all eligible voice and oscillator engines | design-mutable-audio.md | 2026-09-27 |
| [modular-plaits-engines.md](active/modular-plaits-engines.md) | In progress; 24-position inventory and PLV-001 voice layer measured SourceStage; engine/source/resource parity remains | design-mutable-audio.md | 2026-09-30 |
| [modular-live-input.md](active/modular-live-input.md) | In progress; host stereo input for bus effects, device and instrument ports pending | design-mutable-audio.md | 2026-09-28 |
| [modular-plaits-resonant-noise.md](active/modular-plaits-resonant-noise.md) | In progress; positions 18–20 engines; PLV-001 voice-layer SourceStage recorded | design-mutable-audio.md | 2026-09-30 |
| [modular-plaits-oscillators.md](active/modular-plaits-oscillators.md) | In progress; positions 7–9 and 11; PLV-001 voice-layer SourceStage recorded | design-mutable-audio.md | 2026-09-30 |
| [modular-plaits-wave-replacements.md](active/modular-plaits-wave-replacements.md) | In progress; positions 5, 6, 13 and 14; PLV-001 voice-layer SourceStage recorded | design-mutable-audio.md | 2026-09-30 |
| [modular-plaits-cleared-replacements.md](active/modular-plaits-cleared-replacements.md) | Ready; original FM banks and speech data for positions 2–4 and 15; PLV-001 voice-layer SourceStage recorded | design-mutable-audio.md | 2026-09-30 |
| [modular-braids-shapes.md](active/modular-braids-shapes.md) | Ready; 47 accessible macro-oscillator shapes and resource audit | design-mutable-audio.md | 2026-09-28 |
| [modular-rings-resonator.md](active/modular-rings-resonator.md) | Ready; six resonator models, string synth and external excitation | design-mutable-audio.md | 2026-09-28 |
| [modular-elements-model.md](active/modular-elements-model.md) | Ready; twenty patch controls, three resonators and sample-rights boundary | design-mutable-audio.md | 2026-09-28 |
| [modular-tides-functions.md](active/modular-tides-functions.md) | Ready; two generations, 24 Tides2 combinations and Tides1 wave audit | design-mutable-audio.md | 2026-09-28 |
| [modular-segments-keyframes.md](active/modular-segments-keyframes.md) | Ready; Stages audio segments and Frames analog boundary | design-mutable-audio.md | 2026-09-28 |
| [modular-peaks-functions.md](active/modular-peaks-functions.md) | Ready; twelve published functions, four drums and digits asset boundary | design-mutable-audio.md | 2026-09-28 |
| [modular-clouds-texture.md](active/modular-clouds-texture.md) | In progress; four stereo texture adaptations, source parity pending | design-mutable-audio.md | 2026-09-28 |
| [modular-streams-controls.md](active/modular-streams-controls.md) | Ready; six control functions and explicit analog audio boundary | design-mutable-audio.md | 2026-09-28 |
| [modular-audio-effects.md](active/modular-audio-effects.md) | In progress; audio effects and XMOD SRC | design-mutable-audio.md | 2026-09-28 |
| [bass-00-scaffold.md](completed/bass-00-scaffold.md) | Completed (session 232; integration review comm-003043); bass kernel module tree and 32-port contract | design-bass-voices.md | 2026-09-30 |
| [bass-10-filters.md](completed/bass-10-filters.md) | Completed (session 232; integration review comm-003043); ZDF transistor and diode ladders | design-bass-voices.md | 2026-09-30 |
| [bass-11-sources.md](completed/bass-11-sources.md) | Completed (session 232; integration review comm-003043); polyBLEP/BLAMP oscillators, feedback FM, ADAA folder, bit depth | design-bass-voices.md | 2026-09-30 |
| [bass-12-mods.md](completed/bass-12-mods.md) | Completed (session 232; integration review comm-003043); envelopes, gate length, accent, glide, drift-free tempo-synced LFO | design-bass-voices.md | 2026-09-30 |
| [bass-20-kernel.md](completed/bass-20-kernel.md) | Completed (session 232; integration review comm-003043); `bass-core` render and six models | design-bass-voices.md | 2026-09-30 |
| [bass-30-registry.md](completed/bass-30-registry.md) | Completed (session 232; integration review comm-003043); registry, six templates, metadata, 18 golden digest lines | design-bass-voices.md | 2026-09-30 |
| [bass-40-presets.md](completed/bass-40-presets.md) | Completed (session 232; integration review comm-003043); 19-patch preset library and preset renders; manual listening pending | design-bass-voices.md | 2026-09-30 |
| [bass-41-examples.md](completed/bass-41-examples.md) | Completed (session 232; integration review comm-003043); four techno examples, example renders, README; manual listening pending | design-bass-voices.md | 2026-09-30 |
| [bass-voices-232-dispatch.json](completed/bass-voices-232-dispatch.json) | Dispatch manifest for BASS-00..41 (session 232, checkpoint f986f19; all plans accepted) | - | 2026-09-30 |
| [bass-voices-229-dispatch.json](completed/bass-voices-229-dispatch.json) | Superseded session 229 dispatch manifest (checkpoint 843bd8a) | - | 2026-09-30 |
| [digital-drums.md](active/digital-drums.md) | Completed 2026-09-29; four digital drum families, kit and editor metadata (audible review pending) | design-music.md 4.1 | 2026-09-29 |
| [vactr-core.md](active/vactr-core.md) | Completed (implementation, TASK-001..010, 2026-09-26; manual audible/browser/Tauri confirmations pending user sign-off) | design-docs/specs/design-implementation.md | 2026-09-26 |
| [vactr-editor-scaffold.md](completed/vactr-editor-scaffold.md) | Completed (ED-SCAFFOLD, issue #5 TASK-010, wave 1; npm project, protocol client, store, transports, host.js options; holds the common ED contract; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.3, 15.1.4, 15.1.6 | 2026-09-26 |
| [vactr-editor-wire.md](completed/vactr-editor-wire.md) | Completed (ED-WIRE, wave 2; Rust G2-G6; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.2 | 2026-09-26 |
| [vactr-editor-code.md](completed/vactr-editor-code.md) | Completed (ED-CODE, wave 2; CodeMirror surface, highlighting, transport, samples; `highlight.ts` beats fix by the operator before session 188; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.4, 15.1.5 | 2026-09-26 |
| [vactr-editor-midi.md](completed/vactr-editor-midi.md) | Completed (ED-MIDI, wave 2; WebMIDI access, picker, learn, forwarding; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.9 | 2026-09-26 |
| [vactr-editor-wasm.md](completed/vactr-editor-wasm.md) | Completed (ED-WASM, wave 3; Rust G1 browser Session over the raw ABI; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.2 G1; command.md "Browser transport" | 2026-09-26 |
| [vactr-editor-bind.md](completed/vactr-editor-bind.md) | Completed (ED-BIND, wave 3; slider panel, write-back, directives, persistence; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.6, 13, 13.5 | 2026-09-26 |
| [vactr-editor-visual.md](completed/vactr-editor-visual.md) | Completed (ED-VISUAL, wave 3; WebGL2 RenderHost panes, analyzer displays; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.8, 9 | 2026-09-26 |
| [vactr-editor-params.md](completed/vactr-editor-params.md) | Completed (ED-PARAMS, wave 4; parameter editors, sampler waveform, display-only grid/roll; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.7, 13.5 | 2026-09-26 |
| [vactr-editor-pkg.md](completed/vactr-editor-pkg.md) | Completed (ED-PKG, wave 4; browser package import, fetch driver, OPFS; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.10 | 2026-09-26 |
| [vactr-editor-tauri.md](completed/vactr-editor-tauri.md) | Completed (ED-TAURI, wave 4; standalone Tauri shell crate; `cargo check` exit=0 in session 188, `cargo tauri build` pending user confirmation; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.11 | 2026-09-26 |
| [vactr-editor-finalize.md](completed/vactr-editor-finalize.md) | Completed (ED-FINAL, serial reconciliation, wave 5; real-wasm criteria/packages tests, full-tree verification and TASK-010 bookkeeping in session 188; test-integrity, adversarial and integration review accepted; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; archived 2026-09-26) | design-implementation.md 15.1.12, 6.5.7 | 2026-09-26 |
| [ed-editor-20260926-s186-dispatch.json](completed/ed-editor-20260926-s186-dispatch.json) | Dispatch manifest for the eleven ED plans (issue #5, TASK-010; amended by checkpoints 1c02480 and ea95f1b; all plans implemented, ED-FINAL session 188; manifest unchanged by ED-FINAL) | - | 2026-09-26 |
| [vactr-backend-contracts.md](completed/vactr-backend-contracts.md) | Completed (BE-CONTRACTS, issue #3, wave 1; archived 2026-09-25) | design-implementation.md 12.8.2, 12.8.3, 12.8.5, 12.8.10, 12.8.12 | 2026-09-25 |
| [vactr-backend-sched.md](completed/vactr-backend-sched.md) | Completed (BE-SCHED, TASK-007, wave 2; archived 2026-09-25) | design-implementation.md 11.2-11.6, 12.8.3 | 2026-09-25 |
| [vactr-backend-dsp.md](completed/vactr-backend-dsp.md) | Completed (BE-DSP, TASK-008, wave 2; archived 2026-09-25) | design-implementation.md 12, 16.1, 12.8.8, 12.8.9 | 2026-09-25 |
| [vactr-backend-inst.md](completed/vactr-backend-inst.md) | Completed (BE-INST, TASK-008, wave 2; archived 2026-09-25) | design-implementation.md 12.1, 12.8.6, 12.8.7 | 2026-09-25 |
| [vactr-backend-midi.md](completed/vactr-backend-midi.md) | Completed (BE-MIDI, TASK-007, wave 3; archived 2026-09-25) | design-implementation.md 11.7, 12.8.12 | 2026-09-25 |
| [vactr-backend-native.md](completed/vactr-backend-native.md) | Completed (BE-NATIVE, TASK-008, wave 3; archived 2026-09-25) | design-implementation.md 12.8.10 | 2026-09-25 |
| [vactr-backend-wasm.md](completed/vactr-backend-wasm.md) | Completed (BE-WASM, TASK-008, wave 3; archived 2026-09-25) | design-implementation.md 16, 16.1, 12.8.10, 12.8.11 | 2026-09-25 |
| [vactr-backend-finalize.md](completed/vactr-backend-finalize.md) | Completed (BE-FINAL, serial reconciliation, wave 4; archived 2026-09-25) | design-implementation.md 6.5.7, 12.8.12 | 2026-09-25 |
| [be-backend-20260925-s181-dispatch.json](completed/be-backend-20260925-s181-dispatch.json) | Dispatch manifest for the eight BE plans (issue #3; BE-CONTRACTS..BE-WASM accepted, BE-FINAL session 186) | - | 2026-09-25 |
| [vactr-middle-masks.md](completed/vactr-middle-masks.md) | Completed (ME-MASKS, issue #2, wave 1; archived 2026-09-25) | design-implementation.md 5.5, 7, 7.1.3, 7.1.6 | 2026-09-25 |
| [vactr-middle-frontend.md](completed/vactr-middle-frontend.md) | Completed (ME-FRONTEND, wave 2; archived 2026-09-25) | design-implementation.md 6.5.8 | 2026-09-25 |
| [vactr-middle-check.md](completed/vactr-middle-check.md) | Completed (ME-CHECK, TASK-004, wave 3; archived 2026-09-25) | design-implementation.md 5.6, 7, 7.1.4 | 2026-09-25 |
| [vactr-middle-vm.md](completed/vactr-middle-vm.md) | Completed (ME-VM, TASK-005, wave 3; archived 2026-09-25) | design-implementation.md 5.5-5.7, 8, 13 | 2026-09-25 |
| [vactr-middle-pattern.md](completed/vactr-middle-pattern.md) | Completed (ME-PATTERN, TASK-006, wave 3; archived 2026-09-25) | design-implementation.md 9, 10, 11.1, 11.7 | 2026-09-25 |
| [vactr-middle-reactive.md](completed/vactr-middle-reactive.md) | Completed (ME-REACTIVE, TASK-005, wave 4; archived 2026-09-25) | design-implementation.md 5.6, 7.1.3 | 2026-09-25 |
| [vactr-middle-integrate.md](completed/vactr-middle-integrate.md) | Completed (ME-INTEGRATE, TASK-004..006, wave 5; archived 2026-09-25) | design-implementation.md 7.1.1, 7.1.3, 7.1.7 | 2026-09-25 |
| [vactr-middle-finalize.md](completed/vactr-middle-finalize.md) | Completed (ME-FINAL, serial reconciliation, wave 6; archived 2026-09-25) | design-implementation.md 6.5.7, 7.1.7 | 2026-09-25 |
| [me-middle-20260925-s175-dispatch.json](completed/me-middle-20260925-s175-dispatch.json) | Dispatch manifest for the eight ME plans (issue #2; all plans completed, session 183) | - | 2026-09-25 |

## Completed Plans

| Plan | Completed | Design Reference |
|------|-----------|------------------|
| [canvas-cutover-clock.md](completed/canvas-cutover-clock.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-diag-offpath.md](completed/canvas-cutover-diag-offpath.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence-editcost.md](completed/canvas-cutover-evidence-editcost.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence-framecost.md](completed/canvas-cutover-evidence-framecost.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence-runstart.md](completed/canvas-cutover-evidence-runstart.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence-sched.md](completed/canvas-cutover-evidence-sched.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence-scope.md](completed/canvas-cutover-evidence-scope.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence-silent.md](completed/canvas-cutover-evidence-silent.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence-viewport.md](completed/canvas-cutover-evidence-viewport.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-evidence.md](completed/canvas-cutover-evidence.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-mount.md](completed/canvas-cutover-mount.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-native.md](completed/canvas-cutover-native.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-backdrop.md](completed/canvas-cutover-opt-backdrop.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-dom.md](completed/canvas-cutover-opt-dom.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-harness.md](completed/canvas-cutover-opt-harness.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-history.md](completed/canvas-cutover-opt-history.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-render-a.md](completed/canvas-cutover-opt-render-a.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-render-b.md](completed/canvas-cutover-opt-render-b.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-render-c.md](completed/canvas-cutover-opt-render-c.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-render.md](completed/canvas-cutover-opt-render.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-opt-text.md](completed/canvas-cutover-opt-text.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-render.md](completed/canvas-cutover-render.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-shell.md](completed/canvas-cutover-shell.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-syntax-worker.md](completed/canvas-cutover-syntax-worker.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-telemetry-lead.md](completed/canvas-cutover-telemetry-lead.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-cutover-visual.md](completed/canvas-cutover-visual.md) | Completed (2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-dispatch.json](completed/canvas-editor-224-dispatch.json) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-clock.md](completed/canvas-editor-224-clock.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-consumers.md](completed/canvas-editor-224-consumers.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-contracts.md](completed/canvas-editor-224-contracts.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-dependency-evidence.md](completed/canvas-editor-224-dependency-evidence.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-editor-join.md](completed/canvas-editor-224-editor-join.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-execution.md](completed/canvas-editor-224-execution.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-gpu.md](completed/canvas-editor-224-gpu.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-input.md](completed/canvas-editor-224-input.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-native-clock.md](completed/canvas-editor-224-native-clock.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-native-shell.md](completed/canvas-editor-224-native-shell.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-package-preparation.md](completed/canvas-editor-224-package-preparation.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-state.md](completed/canvas-editor-224-state.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-telemetry.md](completed/canvas-editor-224-telemetry.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-verification.md](completed/canvas-editor-224-verification.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [canvas-editor-224-visual.md](completed/canvas-editor-224-visual.md) | Superseded by the canvas-cutover plans (completed 2026-10-07) | design-implementation.md 15.3.8 |
| [song-mode-slice-matcher-budget.md](completed/song-mode-slice-matcher-budget.md) | 2026-10-02; focused accounting acceptance ROOT0494, actual20 tests | design-song-mode.md bounds and identity |
| [sample-timestamps.md](completed/sample-timestamps.md) | 2026-09-30 | design-sample-timestamps.md |
| [sampled-breakcore.md](completed/sampled-breakcore.md) | 2026-09-30 | design-genre-tracks.md sampled breakcore |
| [shimmer-ambient-tracks.md](completed/shimmer-ambient-tracks.md) | 2026-09-30 | design-genre-tracks.md shimmer ambient |
| [genre-tracks.md](completed/genre-tracks.md) | 2026-09-30 | design-genre-tracks.md |
| [lofi.md](completed/lofi.md) | 2026-09-30 | design-lofi.md |
| [rumble-kick.md](completed/rumble-kick.md) | 2026-09-30 | design-music.md RUM-001 |
| [musicdsp-effects.md](completed/musicdsp-effects.md) | 2026-09-30 | design-musicdsp-effects.md |
| [musicdsp-synths.md](completed/musicdsp-synths.md) | 2026-09-30 | design-musicdsp-synths.md |
| [cmp-10-complete-core.md](completed/cmp-10-complete-core.md) | 2026-09-30 (CMP-10, context-aware completion engine core; CMP-40 combined gate passed; archived session 234) | design-completion.md 3 |
| [cmp-15-completion-types.md](completed/cmp-15-completion-types.md) | 2026-09-30 (CMP-15, editor completion contract types; CMP-40 combined gate passed; archived session 234) | design-completion.md 6.2 |
| [fst-50-fmt-space-repair.md](completed/fst-50-fmt-space-repair.md) | 2026-09-30 (FST-50, conservative formatter space-indent repair; CMP-40 combined gate passed; archived session 234) | design-formatter-and-syntax.md 3.9 |
| [eds-10-syntax-span-core.md](completed/eds-10-syntax-span-core.md) | 2026-09-30 (EDS-10, CodeMirror-free tree-sitter span core; CMP-40 combined gate passed; archived session 234) | design-formatter-and-syntax.md 5.5 |
| [eds-11-wasm-format-loader.md](completed/eds-11-wasm-format-loader.md) | 2026-09-30 (EDS-11, `VACTR_WASM`-aware format test loader; CMP-40 combined gate passed; archived session 234) | design-formatter-and-syntax.md 5.4 |
| [eds-12-format-core-tool-wasm.md](completed/eds-12-format-core-tool-wasm.md) | 2026-09-30 (EDS-12, CodeMirror-free format core and shared `ToolWasm`; CMP-40 combined gate passed; archived session 234) | design-formatter-and-syntax.md 5.5; design-completion.md 6.4 |
| [cmp-20-complete-lsp.md](completed/cmp-20-complete-lsp.md) | 2026-09-30 (CMP-20, LSP completion on shared engine; CMP-40 combined gate passed; archived session 234) | design-completion.md 4 |
| [cmp-21-complete-wasm.md](completed/cmp-21-complete-wasm.md) | 2026-09-30 (CMP-21, wasm completion exports; CMP-40 combined gate passed; archived session 234) | design-completion.md 5 |
| [cmp-30-completion-service.md](completed/cmp-30-completion-service.md) | 2026-09-30 (CMP-30, UI-agnostic completion service; CMP-40 combined gate passed; archived session 234) | design-completion.md 6.1 |
| [cmp-31-completion-popup.md](completed/cmp-31-completion-popup.md) | 2026-09-30 (CMP-31, DOM completion popup; CMP-40 combined gate passed; archived session 234) | design-completion.md 6.3 |
| [cmp-32-completion-view-wiring.md](completed/cmp-32-completion-view-wiring.md) | 2026-09-30 (CMP-32, EditorView adapter and minimal wiring; merge note: expect conflicts with the main canvas-editor plans J1 (`code/mount.ts`) and C1 (`app/deps.ts`), and in `app/main.ts`; CMP-40 combined gate passed; archived session 234) | design-completion.md 6.4, 6.5 |
| [cmp-40-closeout.md](completed/cmp-40-closeout.md) | 2026-09-30 (CMP-40, combined integration review, full gate, docs and archive; gate passed; archived session 234) | design-completion.md 8, 9 |
| [cmp-dispatch.json](completed/cmp-dispatch.json) | 2026-09-30 (session-226 dispatch manifest, superseded for CMP-40 by `cmp-closeout-dispatch.json`; archived unchanged session 234) | design-completion.md; design-formatter-and-syntax.md 3.9, 5.4, 5.5 |
| [fst-10-fmt-core.md](completed/fst-10-fmt-core.md) | 2026-09-30 (FST-10, formatter core; FST-40 combined gate passed; archived session 222) | design-formatter-and-syntax.md 3.1-3.6, 3.8 |
| [fst-11-ts-grammar.md](completed/fst-11-ts-grammar.md) | 2026-09-30 (FST-11, C scanner, queries and WASM grammar; FST-40 combined gate passed; archived session 222) | design-formatter-and-syntax.md 2, 4, 6 |
| [fst-20-fmt-cli.md](completed/fst-20-fmt-cli.md) | 2026-09-30 (FST-20, `vactr fmt`; FST-40 combined gate passed; archived session 222) | design-formatter-and-syntax.md 3.7.1; command.md |
| [fst-21-fmt-lsp.md](completed/fst-21-fmt-lsp.md) | 2026-09-30 (FST-21, LSP formatting; FST-40 combined gate passed; archived session 222) | design-formatter-and-syntax.md 3.7.2 |
| [fst-22-fmt-wasm.md](completed/fst-22-fmt-wasm.md) | 2026-09-30 (FST-22, raw WASM formatter ABI; combined export and build gate passed; archived session 222) | design-formatter-and-syntax.md 3.7.3 |
| [fst-23-editor-syntax.md](completed/fst-23-editor-syntax.md) | 2026-09-30 (FST-23, web-tree-sitter highlighting and StreamLanguage fallback; FST-40 editor gate passed; archived session 222) | design-formatter-and-syntax.md 5 |
| [fst-30-editor-format.md](completed/fst-30-editor-format.md) | 2026-09-30 (FST-30, Shift-Alt-f editor format command; FST-40 editor gate passed; archived session 222) | design-formatter-and-syntax.md 3.7.4, 5.4 |
| [fst-40-closeout.md](completed/fst-40-closeout.md) | 2026-09-30 (FST-40, serial reconciliation and full gate; archived session 222) | design-formatter-and-syntax.md 7-8 |
| [plv-10-voice-layer-dsp.md](completed/plv-10-voice-layer-dsp.md) | 2026-09-30 (PLV-10, wave 1 pure shared voice-layer DSP; f5e623b; archived session 209) | design-mutable-audio.md PLV-001 |
| [plv-12-manifest-registration.md](completed/plv-12-manifest-registration.md) | 2026-09-30 (PLV-12, voice registration; measured SourceStage label recorded by PLV-40; archived session 209) | design-mutable-audio.md PLV-001 |
| [plv-20-gate-nodes.md](completed/plv-20-gate-nodes.md) | 2026-09-30 (PLV-20, wave 2 vactrol-gate/decay-mod kinds, controls and registry; f5e623b; archived session 209) | design-mutable-audio.md PLV-001 |
| [plv-21-voice-probe.md](completed/plv-21-voice-probe.md) | 2026-09-30 (PLV-21, voice probe; final rerun eligible for SourceStage, `tmp/plv/s209/PLV-40/7-compare.json`; archived session 209) | design-mutable-audio.md PLV-001 |
| [plv-30-voice-lifetime.md](completed/plv-30-voice-lifetime.md) | 2026-09-30 (PLV-30, session 209 serial wave 1: gate-elided seed order and layer-shaped voice lifetime; archived session 209) | design-mutable-audio.md PLV-001 |
| [plv-31-template-wiring.md](completed/plv-31-template-wiring.md) | 2026-09-30 (PLV-31, session 209 serial wave 2: transparent default-off wiring of 24 templates, editor lists, 48 golden graph lines, 0 render lines, mem expectations; archived session 209) | design-mutable-audio.md PLV-001 |
| [plv-40-evidence-closeout.md](completed/plv-40-evidence-closeout.md) | 2026-09-30 (PLV-40, session 209 serial wave 3: probe evidence, SourceStage label, provenance and plan closeout; adversarial review accepted; archived session 209) | design-mutable-audio.md PLV-001 |
| [plv-dispatch.json](completed/plv-dispatch.json) | 2026-09-30 (dispatch manifest for PLV-30/31/40, checkpoint 870b12c; accepted dependencies PLV-10/12/20/21; archived session 209) | design-mutable-audio.md PLV-001 |
| [product-rename.md](completed/product-rename.md) | 2026-09-28 (Vactr source, docs, editor, remote and local checkout) | architecture.md product identifier contract |
| [modular-warps-vocoder.md](completed/modular-warps-vocoder.md) | 2026-09-28 (WV-001..004, 96-kHz source-rate stages and generated 8–192-kHz host boundary; full firmware parity remains outside this plan) | design-mutable-audio.md vocoder fidelity |
| [vactr-session-contracts.md](completed/vactr-session-contracts.md) | 2026-09-26 (SS-CONTRACTS, issue #4 TASK-009, wave 1; archived in f345e62) | design-implementation.md 14.5.2, 14.5.3, 14.5.6, 14.5.9, 14.5.12 |
| [vactr-session-pkg.md](completed/vactr-session-pkg.md) | 2026-09-26 (SS-PKG, wave 2; archived in f345e62) | design-implementation.md 5.7, 14.5.7 |
| [vactr-session-directives.md](completed/vactr-session-directives.md) | 2026-09-26 (SS-DIRECTIVES, wave 2; archived in f345e62) | design-implementation.md 13.5, 14.5.8 |
| [vactr-session-analysis.md](completed/vactr-session-analysis.md) | 2026-09-26 (SS-ANALYSIS, wave 2; archived in f345e62) | design-implementation.md 12.3, 14.5.9 |
| [vactr-session-core.md](completed/vactr-session-core.md) | 2026-09-26 (SS-SESSION, wave 3; archived in f345e62) | design-implementation.md 14.1-14.4, 14.5.4-14.5.6 |
| [vactr-session-cli.md](completed/vactr-session-cli.md) | 2026-09-26 (SS-CLI, wave 4; archived in f345e62) | command.md; design-implementation.md 14.5.10 |
| [vactr-session-lsp.md](completed/vactr-session-lsp.md) | 2026-09-26 (SS-LSP, wave 4; archived in f345e62) | design-implementation.md 14.3, 14.5.11 |
| [vactr-session-finalize.md](completed/vactr-session-finalize.md) | 2026-09-26 (SS-FINAL, serial reconciliation, wave 5; archived in f345e62) | design-implementation.md 14.5.12, 6.5.7 |
| [ss-session-20260925-s183-dispatch.json](completed/ss-session-20260925-s183-dispatch.json) | 2026-09-26 (SS dispatch manifest, issue #4; unedited since checkpoint 9d6db6e; archived in f345e62) | - |
| [vactr-frontend-value.md](completed/vactr-frontend-value.md) | 2026-09-25 (FE-VALUE, TASK-001, wave 1; archived in 850c606) | design-docs/specs/design-implementation.md 5.1-5.4, 6.5.1-6.5.3 |
| [vactr-frontend-reader.md](completed/vactr-frontend-reader.md) | 2026-09-25 (FE-READER, TASK-002, wave 2; archived in 850c606) | design-docs/specs/design-implementation.md 5.7, 6.1-6.3, 6.5.4, 6.5.6 |
| [vactr-frontend-expander.md](completed/vactr-frontend-expander.md) | 2026-09-25 (FE-EXPAND, TASK-003, wave 3; archived in 850c606) | design-docs/specs/design-implementation.md 6.4, 6.5.5, 6.5.6 |
| [vactr-frontend-finalize.md](completed/vactr-frontend-finalize.md) | 2026-09-25 (FE-FINAL, serial reconciliation, wave 4; archived in 850c606) | design-docs/specs/design-implementation.md 6.5.7 |
| [fe-frontend-20260925-s165-dispatch.json](completed/fe-frontend-20260925-s165-dispatch.json) | 2026-09-25 (FE dispatch manifest; closure note added in issue #2) | - |

## Phase Dependencies (for impl-exec-auto)

**IMPORTANT**: This section is used by impl-exec-auto to determine which plans to load.
Only plans from eligible phases should be read to minimize context loading.

### Phase Status

| Phase | Status | Depends On |
|-------|--------|------------|
| 1 | NOT_STARTED | - |
| 2 | BLOCKED | Phase 1 |
| 3 | BLOCKED | Phase 2 |

### Phase to Plans Mapping

```
PHASE_TO_PLANS = {
  1: [
    # Add Phase 1 plan files here
  ],
  2: [
    # Add Phase 2 plan files here
  ],
  3: [
    # Add Phase 3 plan files here
  ]
}
```

## Workflow

### Creating a New Plan

1. Use the `/impl-plan` command with a design document reference
2. Or manually create a plan using `templates/plan-template.md`
3. Save to `active/<feature-name>.md`
4. Update this README with the new plan entry
5. **IMPORTANT**: If plan exceeds 400 lines, split into multiple files

### Working on a Plan

1. Read the active plan
2. Select a subtask to work on (consider parallelization)
3. Implement following the deliverable specifications
4. Update task status and progress log
5. Mark completion criteria as done

### Completing a Plan

1. Verify all completion criteria are met
2. Update status to "Completed"
3. Move file from `active/` to `completed/`
4. Update this README

## Guidelines

- Plans contain NO implementation code
- Plans specify traits, functions, and file structures
- Subtasks should be as independent as possible for parallel execution
- Always update progress log after each session
- **Keep each plan file under 400 lines** - split if necessary

## Song-mode plans (2026-09-30)

Accepted design: [Finite multi-track song mode](../design-docs/specs/design-song-mode.md).
Riela accepted the design and all 16 plans with no material findings on 2026-09-30.
Implementation is in progress; statuses and evidence are recorded per plan. Shared files follow serial dependencies; indexes and archiving belong to SONG-16.

| Plan ID | Plan | Depends On |
|---|---|---|
| SONG-01 | [Finite symbolic values](active/song-mode-values.md) | Accepted design |
| SONG-02 | [Value and VM compatibility](active/song-mode-value-integration.md) | SONG-01 |
| SONG-03 | [Checker types and native signatures](active/song-mode-checker.md) | SONG-02 |
| SONG-04A | [Producer trace core](active/song-mode-producer-trace.md) | SONG-01, SONG-02 |
| SONG-04B | [Combinator trace propagation](active/song-mode-trace-combinators.md) | SONG-04A |
| SONG-04P | [Selected source and route contracts](active/song-mode-source-contracts.md) | SONG-03, SONG-04B |
| SONG-04Q | [Source provenance and canonical context](active/song-mode-source-provenance.md) | SONG-04P |
| SONG-04R | [Sample classification](active/song-mode-sample-classification.md) | SONG-04Q |
| SONG-04 | [Canonical realization and pure edits](active/song-mode-query-edits.md) | SONG-01, SONG-04B, SONG-04P, SONG-04Q, SONG-04R |
| SONG-05 | [Song native execution](active/song-mode-natives.md) | SONG-03, SONG-04 |
| SONG-06A | [Isolated song assets](active/song-mode-assets.md) | SONG-05 |
| SONG-06 | [Snapshot and session request contracts](active/song-mode-snapshot-contracts.md) | SONG-05, SONG-06A |
| SONG-07 | [Isolated whole-code candidate evaluation](active/song-mode-candidate-evaluation.md) | SONG-06 |
| SONG-08 | [Audio commands and acknowledgments](active/song-mode-audio-contracts.md) | SONG-06 |
| SONG-09 | [Private branches, tails and audio gates](active/song-mode-dsp-routing.md) | SONG-08 |
| SONG-10 | [Native and worklet song adapters](active/song-mode-host-adapters.md) | SONG-09 |
| SONG-11 | [Finite song transport and atomic activation](active/song-mode-transport.md) | SONG-07, SONG-10 |
| SONG-ACTIVATION-CORRELATION | [Correlated activation and exact initial onset](active/song-mode-activation-correlation.md) | SONG-HOST-PREPARATION |
| SONG-ACTIVATION-WIRE | [Activation DTO and wire](active/song-mode-activation-wire.md) | verified host cohort |
| SONG-SAMPLE-ADMISSION | [Checked sample sender admission](active/song-mode-sample-admission.md) | verified host/wire cohort |
| SONG-ACTIVATION-PIPELINE | [Activation pipeline and exact empty-song lifecycle](active/song-mode-activation-pipeline.md) | SONG-ACTIVATION-WIRE, SONG-HOST-PREPARATION |
| SONG-11A | [Owned finite transport core](active/song-mode-transport-core.md) | SONG-07, SONG-HOST-PREPARATION, SONG-ACTIVATION-CORRELATION, verified routes |
| SONG-11B | [Runtime/session transport integration](active/song-mode-transport-integration.md) | SONG-11A, SONG-HOST-PREPARATION |
| SONG-12 | [Streaming complete-song WAV export](active/song-mode-export-core.md) | SONG-11 |
| SONG-13 | [Automatic run completion and render command](active/song-mode-cli.md) | SONG-12 |
| SONG-14 | [Browser song protocol integration](active/song-mode-browser-session.md) | SONG-11 |
| SONG-15 | [Whole-code Apply and instrument mute UI](active/song-mode-editor-controls.md) | SONG-14 |
| SONG-16 | [Serial integration evidence and documentation — Completed (session 261 final receipt)](completed/song-mode-reconciliation.md) | SONG-13, SONG-15 |

## Completed song runtime checkpoints

| Plan | Verified scope |
|---|---|
| [Browser runtime evidence](completed/song-mode-browser-runtime-evidence.md) | Four actual WASM ABI fixtures: catalog refusal, frozen PCM, finite playback and acknowledged mute/unmute without replay. Atomic replacement and browser UI/device acceptance remain separate. |

- [Issued invocation compatibility witnesses](completed/song-mode-issued-invocation-compatibility.md): Completed; original full invocation witness preserved with fresh issued member proofs; joined405 Rust and590 frontend plus build gates accepted.

[Shared issued query work](completed/song-mode-shared-issued-query-work.md) is Completed (session 261 final receipt).

FM1 microtonal tuning and chord performance ([design-tuning-and-strum.md](../design-docs/specs/design-tuning-and-strum.md)) is Completed (session 354, 2026-10-11; integration review comm-006021 accepted all 7 plans; full nextest 2888/2888, vitest 841/841, test:style 4/4): [01 tuning model](completed/fm1-tuning-01-model.md), [02 pattern graph](completed/fm1-tuning-02-pattern-graph.md), [03 frequency resolution](completed/fm1-tuning-03-resolution.md), [04 tuning-aware music](completed/fm1-tuning-04-music.md), [05 natives](completed/fm1-tuning-05-natives.md), [06 docs and examples](completed/fm1-tuning-06-docs-examples.md) and [07 closeout](completed/fm1-tuning-07-closeout.md). The dispatch manifest [fm1-tuning-dispatch.json](active/fm1-tuning-dispatch.json) stays in `active/` because the running workflow references that path; archive it after the branch commit.

# Digital Drums and Mutable Audio Handoff

**Status**: Ready (work paused after WV-004 review)
**Design Reference**: `design-docs/specs/design-mutable-audio.md` and
`design-docs/specs/design-music.md#41-programmable-digital-drums-author-2026-09-27`
**Created**: 2026-09-28
**Last Updated**: 2026-09-29

## Current boundary

Vactr has runnable versions of all 24 Plaits positions and all 47
accessible Braids shapes. This is breadth of independent adaptations and
source-stage translations, not complete source-port fidelity: Plaits has
15 adaptations, nine source-stage translations and zero verified full
ports; Braids has 47 adaptations/replacements and no completed per-shape
source comparison. The other published audio families have narrower
adaptations and active plans. `phase-drum`, `fusion-drum` and
`feedback-metal-drum` are runnable FM percussion voices; a complete
commercially usable digital kit and an Elektron hardware emulation are
not claimed. No LXR/Machinedrum code, waveform asset or product identity
is included.

The immediate stopping point is WV-004 in
`impl-plans/completed/modular-warps-vocoder.md`: the 20-band 96-kHz
vocoder has a generated host-rate FIR boundary and the old
non-96-kHz fallback is retired. Independent check-and-test review passed
with 1,483 tests passing and one ignored; full source firmware parity is
still not claimed. The working tree contains uncommitted prior and
current goal changes; preserve it when resuming.

## Future TODOs, in dependency order

| Priority | TODO | Plan and completion evidence |
|---|---|---|
| 1 | Source/resource/license inventory is complete and checked (MOD-001, 2026-09-29: 217 files, `mise run audit-upstream`). MOD-002 is complete (2026-09-29): neutral names and Clouds/Warps inventories, with naming tests. The wave, map and digit binaries stay excluded until their origin is established. | `modular-audio-foundation.md` MOD-001/MOD-002; `verification/upstream_inventory.toml`; `THIRD_PARTY_NOTICES.md` |
| 2 | Complete original digital kit mapping across tonal, snare, metallic and hat voices, including every declared `.vact` and live-cell parameter, kit/editor API, diagnostics, and native/browser end-to-end tests. | `digital-drums.md` DDRUM-001, 003–006; four family control/response and allocation checks |
| 3 | Finish FM drum source comparisons and broader original EFM-inspired family, without restricted firmware, patch banks or unlicensed Faust dependencies. | `modular-fm-drums.md` FMD-006/007; pinned raw-kernel metrics, stereo/control/rate tests, truthful fidelity status |
| 4 | Complete Plaits voice-level trigger/LPG, engine-specific parity and resource-safe replacements; compare all 24 positions independently. | `modular-plaits-engines.md` and its engine subplans; dual-output, parameter, source-metric and native/browser matrix |
| 5 | Complete Braids pitch/timbre/strike/sync and per-shape source comparisons; audit or replace every generated resource. | `modular-braids-shapes.md` BRA-007/008; 47-position coverage report and resource evidence |
| 6 | Finish Elements, Rings, Clouds, Warps, Tides, Peaks, Streams, Stages and Frames audio paths, including remaining source-stage fidelity and module-specific controls. | Their `impl-plans/active/modular-*.md` plans; native/browser output, bounded state and source-comparison evidence per mode |
| 7 | Verify arbitrary stereo graph edges, live external-input/device routes, rate/block and real-time bounds, and final coverage/notice claims across all families. | `modular-audio-foundation.md`, `modular-live-input.md`, `modular-audio-effects.md`; full quiet Cargo/wasm/Clippy/test, browser and audible review |

## Resume rule

Select a subtask from the linked active plan, update its progress log and
fidelity/resource status, use the repository's specialized Rust coding
agent for Rust edits, and invoke the independent check-and-test agent
after every Rust modification. Do not call an adaptation a complete port
without individual source and output evidence. Use
`CARGO_TERM_QUIET=true` for Cargo commands.

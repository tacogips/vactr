# Editor background comments implementation plan

**Status**: Completed
**Design Reference**: [Editor background comments](../../design-docs/specs/design-visual.md#editor-background-comments-2026-10-18)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Scope

Implement comment-driven image/video/shader backgrounds and real master
oscilloscope, spectrum, rolling spectrogram and level overlays. Preserve the
current header changes and all user work. No music evaluation or source rewrite
is permitted when changing editor presentation.

## Modules and contracts

| Module | Deliverables | Status |
|---|---|---|
| Host analysis | Fixed 256-sample chronological final-output waveform in HostSigs/native shared signals/browser host signal records | Completed |
| Session/reader boundary | Optional master waveform telemetry; reserve `#@ editor` without parameter-binding warnings | Completed |
| Comment settings | `editor/src/visual/settings.ts`: typed normalized config, validation and change-aware source observer | Completed |
| Background media | `editor/src/visual/background.ts`: DOM media lifecycle, errors, opacity/fit/blur, shader source selection | Completed |
| Analyzer overlays | `editor/src/visual/overlays.ts`: waveform, bands, bounded history and level overlays | Completed |
| Integration | Visual mount/CSS, code API/renderer scrim, TS wire validation and native CSP | Completed |
| Verification/documentation | Focused Rust/TS tests, real browser media/overlay checks, examples and user documentation | Completed |

Public contracts (signatures only):
- Rust host signals: `waveform: Option<[f32; 256]>`; master wire level optional waveform vector.
- Frontend wire level: `waveform?: number[]`.
- `parseEditorSettings(source: string): ParsedEditorSettings`.
- `EditorSettingsSource(surface: CodeSurface, onChange: (settings: ParsedEditorSettings) => void)`.
- `CodeApi.setBackgroundOpacity?(opacity: number): void`.

## Tasks and dependencies

### TASK-001: Real master waveform
**Status**: Completed
**Parallelizable**: Yes, Rust files only via rust-coding agent.
Publish a bounded waveform from the existing final-output ring through both
host paths. Reserve the editor comment namespace. Preserve existing analysis,
MIDI/control directives, sound, bus graphs, and finite-song lifecycle.

### TASK-002: Comment parser and media
**Status**: Completed
**Parallelizable**: Yes, frontend only.
Implement validated own-line JSON comments, replacement/removal/undo semantics,
image/video lifecycle, shader selection, scrim and visible failures.

### TASK-003: Overlays and integration
**Status**: Completed
**Depends On**: TASK-001 and TASK-002
**Parallelizable**: No.
Draw real scope/bands/level/rolling spectrogram, bound resources and histories,
connect native/browser telemetry and media settings, and preserve editor input.

### TASK-004: Verify and finish
**Status**: Completed
**Depends On**: TASK-003
**Parallelizable**: No.
Run focused tests and independent Rust verification, build fresh wasm/editor,
inspect actual image/video/oscilloscope rendering and document syntax.

## Completion criteria

- [x] Comments alone configure image/video backgrounds and the four overlays.
- [x] Own-line comments are distinguished from strings/ordinary comments; unknown or invalid values are visible errors.
- [x] Loading/removing/replacing/undoing comments updates the presentation without eval or code writes.
- [x] Scope uses actual final audio samples for browser/native and finite-song audio, with backward-compatible telemetry.
- [x] Video is muted and visibility-aware; failures, stale loads and disposal release resources.
- [x] Background media is beneath editable/readable code and never copied into the text GPU pipeline.
- [x] Relevant tests, checks and actual browser evidence pass; examples and usage documented.

## Progress log

### 2026-10-18
Confirmed that current manual video/shader panes and analyzer displays have no
comment-config integration. Current editor namespace would be treated as a
parameter directive. Found existing final-output ring suitable for scope data;
using it avoids altering effects graphs or adding analyzer IDs.

### Implementation and verification: 2026-10-18
- Rust: optional fixed 256-sample final-output waveform is published through native coherent atomics and the backwards-compatible browser signal codec. Exact editor comment namespace is reserved through reader trivia, without changing user audio graphs or directive bindings.
- Coding-agent checks: formatting, strict all-target clippy, native and wasm compilation, and 89 focused tests passed. Independent verification: 67 focused tests plus native/wasm compilation and formatting passed. Actual finite-song output and allocation-free audio rendering are covered.
- Frontend: validated JSON comments with change-aware scans; image/video/shader selection; opacity, fit, blur and readable scrim; real scope, bands, rolling spectrogram and level overlays; visible errors, stale-data handling, undo/removal and cleanup. Code/media/overlay allocations share the existing ResourceLedger in normal editor instances.
- Native CSP permits HTTPS images/video and bundled/data/blob media. Native notice now refers specifically to unavailable shader outputs, not all visuals.
- Added bundled SVG, runnable `examples/editor-background.vact`, Documents registry entry, root README usage, and `npm run e2e:background`.
- `npm run check`: passed. Eight related frontend suites: 63/63 passed. Actual Chromium and WebKit checks passed image compositing (1120 changed sampled pixels each), non-flat real waveform pixels, all four overlays, muted/looping/inline playing video, undo, removal, failures/unsafe schemes, shader output o1 and the bundled example.
- The test server now serves media MIME types and byte ranges; WebKit video decoding passed after this fixture-server correction. No playback workaround was added to production code.
- Fresh waveform-enabled release wasm and editor build succeeded. Browser evidence: `tmp/editor-background/results.json` and PNGs. Test/build instructions are in `editor/test/e2e/README.md`.

### Completion audit: 2026-10-18
- Comment-only configuration is proven by parser/lifecycle tests and browser checks that presentation changes leave evaluation idle until Play.
- Image visibility is proven by compositor screenshots and 1120 changed sampled pixels in each engine; video advances with muted/loop/inline settings in each engine. Shader output o1 and the registered example also pass.
- Real oscilloscope pixels and all four overlays pass in both engines. Native/finite-signal correctness, chronology, silence, optional legacy telemetry, and zero audio allocations are covered by focused Rust tests and independent review.
- Validation, unsafe schemes, failed media, stale source completion/data, visibility handling, undo/removal and disposal are covered by the related frontend suites and browser checks.
- Final resource-sharing checks passed: 32/32 tests across canvas mount, media lifecycle and overlays. The broader related run passed 63/63. TypeScript check, fresh release wasm/editor build, final Chromium/WebKit run and diff whitespace check passed.
- Final evidence: `tmp/editor-background/results.json`, `build.log`, and per-engine image/video/scope/shader/example PNGs. No required work remains.

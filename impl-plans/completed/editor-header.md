# Editor header implementation plan

**Status**: Completed
**Design Reference**: [Editor header](../../design-docs/specs/design-editor-header.md#behavior-and-hierarchy)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Scope and modules

Implement the requested header/menu redesign and truthful playback counter. Preserve existing editing, document persistence, evaluation shortcuts, and effect-tail stop semantics.

| Module | Files | Status |
|---|---|---|
| Scheduler/session pause | `src/sched/midi_clock.rs`, `src/session/protocol.rs`, `src/session/session.rs`, `src/session/publish.rs`, `src/sched/{midi_in.rs,runtime/live.rs,runtime/song.rs}`, focused Rust tests | Completed |
| Playback model and protocol client | `editor/src/code/transport.ts`, `editor/src/code/mount.ts`, `editor/src/protocol/{types,envelope,client}.ts` | Completed |
| Header and document menu | `editor/src/ui/{transport-view,icons,picker-view}.tsx`, `editor/src/documents/{mount,types}.ts`, `editor/src/app/{app.css,song.ts}`, `editor/src/ui/picker.css`, `editor/package.json` | Completed |
| Focused behavior and visual checks | `editor/test/code/transport.test.ts`, `editor/test/e2e/header.mjs`, `editor/test/e2e/README.md`, existing document/UI suites, browser inspection | Completed |

## Public contracts

- Session: `ClientMsg::TransportPause(Empty)` and `ClientMsg::TransportContinue(Empty)`.
- Client: `pauseTransport(): void`, `continueTransport(): void`.
- TransportBar: primary Play/Stop action, separate pause/resume action, existing `stopAll`, `hush`, `evalStarted`, `tick` entry points retained.
- Picker view: reactive document title and unsaved state from DocumentSession.

## Tasks and dependencies

### TASK-001: Runtime pause
**Status**: Completed
**Parallelizable**: Yes; owns Rust only.
**Deliverables**: Add protocol commands and real internal-clock freeze/resume, preserve MIDI behavior, reject finite-song pause explicitly. Focused regression tests.

### TASK-002: Header and menus
**Status**: Completed
**Parallelizable**: Yes; owns frontend only.
**Deliverables**: Group transport, position, output and editor settings; truthful playback state; document title/dirty state; subordinate song row; responsive layout and accessible disclosures.

### TASK-003: Verify and finish
**Status**: Completed
**Parallelizable**: No; depends on TASK-001 and TASK-002.
**Deliverables**: Focused tests, TypeScript check/build, Rust verification agent, browser layout/interaction inspection, completed progress record.

## Completion criteria

- [x] No counter movement before playback or after Stop/Cut.
- [x] Primary Play/Stop toggle and adjacent Pause/Resume work with actual engine state.
- [x] Update, document actions, settings, output and song controls have clear grouping.
- [x] Title/unsaved indicator and menu keyboard dismissal work.
- [x] Relevant checks pass and browser layout is inspected at desktop and narrow widths.

## Progress log

### 2026-10-18
Read existing transport, document picker, song controls, scheduler and reference applications. Initial clock progression and independent run/stop controls explain the reported confusion. Runtime pause currently exists only for MIDI; extend it for live internal-clock playback.

### Implementation and checks: 2026-10-18
- Added real internal-clock pause/resume and authoritative scheduled-music activity telemetry. Finite-song pause is explicitly unsupported; the frontend disables it with a reason.
- Grouped playback, tempo/position, update and audio; moved Vim/help and slots into disclosures. Added document title/unsaved state and menu dismissal. Retained song application on a subordinate row.
- `CARGO_TERM_QUIET=true mise run build-wasm-release`: exit 0; fresh wasm used for browser checks.
- Rust coding agent: check, strict all-target clippy, format and 77 focused tests passed. Independent verification agent: mise fmt-check/check and 25 focused tests passed.
- `npm run check`: exit 0. Eight focused frontend suites: 81/81 passed.
- `npm run test:style`: exit 0; 16 engine/viewport combinations passed; log `tmp/editor-header/style.log`.
- Added `npm run e2e:header` for actual wasm playback, empty-session/constant counters, pause/resume, stop, disclosures and widths 1440/768/390 in Chromium and WebKit. Evidence and screenshots: `tmp/editor-header/`.
- Browser inspection revealed an isolated Editor menu row at tablet width; moved that menu beside document navigation. Stop may correctly display Effects tail while its counter is cleared.

### Completion: 2026-10-18
- Final Chromium and WebKit header runs passed playback, counter, menu dismissal, bounds and occlusion checks at 1440/768/390. `tmp/editor-header/results.json` records success.
- Fixed narrow-screen menu stacking so both the document row and closed slot disclosure stay below an open menu. Inspected desktop, tablet and phone screenshots.
- Final transport regression run: 16/16 passed, including Play recovery after the last paused slot stops. TypeScript check passed.
- Read-only Rust follow-up confirmed textures are visual shaders, not audio sources; Pause tooltip explains that visual animation continues. Full finite-song pause remains outside this implementation, with the control disabled and its reason exposed.
- Plan complete. Existing broader song-mode plans were not changed.

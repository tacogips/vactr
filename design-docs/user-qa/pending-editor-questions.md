# Pending: editor questions from TASK-010 (2026-09-26, issue #5)

These questions came up while designing the editor: the `editor/` app,
the browser delivery and the Tauri shell (design section 15.1 in
`design-docs/specs/design-implementation.md`; wire additions in
`design-docs/specs/command.md`). Each one has a recommendation, and the
implementation follows it until the question is answered. Only E1 can
block an acceptance check. E1 decides whether the Tauri `cargo check`
can run at all.

The following are already decided and are not asked here:
- the Editor Requirements (slider panel with source-edit and overlay
  modes, DAW-style editors, display-only grid and roll, `#@` directives
  as the default persistence);
- the visual model (WebGL2 RenderHost, ping-pong feedback);
- the self-analysis model (12.3);
- the host tiers (browser first, then a Tauri shell over the same
  frontend, no Swift).

## E1. crates.io access for the Tauri shell crate

Issue #5 requires `cargo check` of the Tauri shell crate to pass, and
it also says "Node dependencies are installed from the public registry
... no other network access". The shell crate needs `tauri` 2,
`tauri-build`, `tauri-plugin-dialog` and `tauri-plugin-fs` from
crates.io. The two requirements conflict unless those crates are
already in the local cargo cache.

- Recommendation: allow a one-time `cargo fetch` of the shell crate's
  locked tree from crates.io. crates.io is a public read-only registry,
  the same class as npm. The crate's `Cargo.lock` is committed, and the
  versions follow the 12.8.10 policy on Rust 1.83. If access is denied,
  or the tree cannot build on 1.83, the gate is recorded as BLOCKED
  (a verification gap, not a pass) and reported as a dependency
  blocker for the operator.
- Alternative: vendor the crates (`cargo vendor`) in a separate,
  operator-run step, then check offline.

## E2. Browser capability gaps left as "not available on this host"

TASK-009 moved "browser self-analysis taps" to TASK-010. No TASK-010
completion criterion needs them. They would add record traffic on the
audio rendering thread, which the 16.1 bounded-work rule governs. The
same holds for browser MIDI output (the `midi n` sink and
`midi-clock-out`) and for the editor's oscilloscope of raw frames.

- Recommendation:
  - Keep `scope`, `spectrum` and `capture` on live sources, and MIDI
    out, as "not available on this host" in the browser for v1.
  - The editor's meters and scopes (level, spectrum, spectrogram,
    oscilloscope, pitch and stereo meters) render from the analyzer
    cells that already reach the runtime on both tiers. They are
    published in `levels.analyzers` (design 15.1.2 G4), so an
    oscilloscope needs an `oscilloscope` analyzer unit on a bus.
  - A follow-up issue adds browser taps and MIDI out.
- Alternative: implement a `:master` browser tap now: the worklet posts
  master frames per quantum, and the main half keeps a ring.

## E3. Per-slot level meters

The TASK-010 deliverable lists "per-slot mute/level meters". The engine
does not mix slots separately (S2: voices go to orbit buses), so no
per-slot level exists. The protocol has no mute message either.

- Recommendation: the transport bar shows the master meter from
  `levels`, and a per-slot ACTIVITY light driven by `playing`
  telemetry. Per-slot mute is `stop` (the only per-slot message). Real
  per-slot levels arrive with a per-slot mix in a later issue.
- Alternative: add per-slot level cells to the engine now. That is a
  DSP change outside the editor's scope.

## E4. ExternalFile persistence handled by the editor

The session's ExternalFile mode keeps its binding set in memory, and
the protocol has no message to switch modes, load a set or save one.
The criteria need the editor to switch to ExternalFile, save
`<doc>.bindings.json`, and read it back.

- Recommendation:
  - The session stays in Directive mode on both tiers.
  - In ExternalFile mode the editor keeps its own binding set and
    writes `<doc>.bindings.json` in exactly the 14.5.8 format
    (`{"v": 1, "bindings": [...]}`, keys `label.site.n.param`, overlays
    kept).
  - It never sends `learn` in that mode and never inserts or edits
    `#@` text.
  - User-written `#@` lines are not stripped, so "source untouched"
    holds.
  - This needs no protocol change.
- Alternative: add `set-persistence` (C->S, with an optional loaded
  set) and `bindings-file` (S->C) messages, so that the session's own
  ExternalFile implementation is used.

## E5. Node version

mise does not pin node, and the shim reports v26.9.0. Pinning node in
`mise.toml` would make `mise install` download a node build. That is
network access outside the npm registry.

- Recommendation: declare `engines.node >= 20` in
  `editor/package.json`, use the environment's node, and do not change
  `mise.toml`.
- Alternative: pin `node = "22"` in `mise.toml`. This is an
  operator-run install.

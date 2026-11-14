# Canvas Cutover: Tauri Desktop IPC and iOS Shell Implementation Plan

**Status**: Completed (2026-10-07)
**Plan ID**: CANVAS-SHELL (wave 2; depends on CANVAS-NATIVE and CANVAS-CLOCK)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.5 (desktop Tauri, iOS), 15.3.8.4 (native probe correlation), 15.3.8.8 (simulator profile S)
**Manifest**: impl-plans/completed/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

The Tauri app must run the Session Protocol over IPC against the native engine (CPAL audio with
real latency provenance), and must build and launch on an iPad simulator. The iOS build covers
audio-session activation, interruption and route-change handling, background audio, and
safe-area layout.

**Decomposition note.** Design 15.3.8.9 placed the desktop Tauri files and
`protocol/tauri.ts` in NATIVE. They live here so that NATIVE stays a pure `vactr`-crate plan,
verifiable by nextest alone. The architecture is unchanged.

Facts at c9e5a05 and after wave 1:

- `editor/src-tauri/src/main.rs` (16 lines) only registers the dialog and fs plugins. There is
  no `lib.rs`. `build.rs` is `tauri_build::build()`. The package name is `vactr-editor`, and
  there is no `[lib]` section.
- CANVAS-NATIVE provides `vactr::cli::owner::{SessionOwner, AudioSessionEvent}`, and
  `vactr::cli::args::HostChoice`.
- CANVAS-CLOCK provides `AudibleClock` and `ProbeCorrelation` (`app/clock.ts`), and `boot()`
  already creates `deps.audible` per tier.
- `app/main.ts:isTauri(win)` exists. Today the Tauri app runs the browser Wasm tier with
  `TauriFiles`.
- `@tauri-apps/api` 2.11.1 is already pinned. Use `invoke` and `Channel` from
  `@tauri-apps/api/core`. No new dependency.
- iOS template paths were confirmed from the pinned
  `tmp/canvas/tools/cargo-tauri` (2.11.5) embedded templates:
  - `Sources/{{app.name}}/main.mm` and `Sources/{{app.name}}/bindings/bindings.h`;
  - `{{app.name}}_iOS/Info.plist` and `{{app.name}}_iOS/{{app.name}}_iOS.entitlements`;
  - `project.yml`, `Podfile`, `ExportOptions.plist`, `LaunchScreen.storyboard`;
  - `Assets.xcassets` with 18 AppIcon PNGs;
  - `{{app.name}}.xcodeproj/...`.

  Tauri derives `{{app.name}}` from the Cargo package name, so it is `vactr-editor`. The
  manifest lists the resulting concrete files.

## Non-goals

- No Apple provisioning, signing-identity change, device build or upload. No Swift UI.
- No Hydra on the native tier. The visual pane keeps `NATIVE_NOTICE`, which is a recorded
  limitation.
- No change to `code/*`, `visual/*` or Rust crate sources outside `editor/src-tauri`.

## Ownership

writePaths: see the manifest entry `CANVAS-SHELL`. It lists every file, including the 33
`editor/src-tauri/gen/apple/...` files and `editor/.gitignore`.

**`editor/.gitignore`.** Line 3 is currently `src-tauri/gen/`, which ignores the whole
generated tree, including `gen/apple`. Replace that single line with `src-tauri/gen/schemas/`,
because Tauri build-generated capability schemas must stay ignored. Keep `dist/` and
`src-tauri/target/` unchanged. Make this edit first, before `ios init`.

sharedPaths (conditional):

- `editor/src-tauri/Cargo.lock`: expected unchanged. If `cargo check` changes it, record the
  diff and verify that no new crate appears.
- `editor/src-tauri/capabilities/default.json`: edit only if Tauri rejects the app commands
  without permission entries.
- `editor/src-tauri/Info.ios.plist` (new): see TASK-004.

**Generated-file rule.** If `ios init` emits a path that is not in the manifest:

- record the exact path list in the progress log;
- if there are at most 5 such paths and they are small text or PNG files, add them as recorded
  sharedPaths additions;
- build outputs (`gen/apple/build/`, `gen/apple/Externals/`) must be ignored by the generated
  `gen/apple/.gitignore` and are never committed;
- if the app name is not `vactr-editor`, stop and report. That is a manifest-drift blocker.

## Contracts (pinned)

Tauri commands (`editor/src-tauri/src/session.rs`; registered in `lib.rs`):

- `session_connect(channel: tauri::ipc::Channel<String>) -> Result<u32, String>`: lazily
  `SessionOwner::spawn(HostChoice::Native, app_data_dir)`, creating the directory if needed;
  then `owner.connect(tx)`, where a forwarding thread sends each `String` from `rx` to `channel`.
- `session_send(id: u32, text: String)` and `session_close(id: u32)`.
- `self_check_enabled() -> bool`, which is `std::env::var("VACTR_SELF_CHECK") == Ok("1")`.
- `self_check(report: String)`, which prints one line to stderr: `VACTR_SELF_CHECK <report>`.
  The frontend passes `JSON.stringify(...)`, so no direct `serde_json` dependency is added.

C ABI, iOS only, in `lib.rs`:

- `#[no_mangle] pub extern "C" fn vactr_audio_session_event(kind: u32)`, where 1 is
  `Interrupted`, 2 is `Resumed` and 3 is `RouteChanged`. It posts to the global owner if one
  exists, using a `OnceLock<Mutex<Option<..>>>` handle, and otherwise ignores the event.

`editor/src/protocol/tauri.ts`:

```ts
export class TauriTransport implements Transport {
  static connect(api?: { invoke: typeof invoke; Channel: typeof Channel }): Promise<TauriTransport>;
  send(text: string): void; onText(cb: (text: string) => void): void; close(): void;
}
```

Imitate `protocol/wasm.ts:WasmTransport` (the `closed` flag and `offs` list).

## Tasks

### TASK-001: Shared bootstrap and desktop IPC (lib.rs, main.rs, session.rs, Cargo.toml)

- Add `[lib] name = "vactr_editor_lib"` and `crate-type = ["staticlib", "cdylib", "rlib"]` to
  `Cargo.toml`.
- `main.rs` keeps the `windows_subsystem` attribute and calls `vactr_editor_lib::run()`.
- `lib.rs`:
  - `pub fn run()` keeps both plugins, manages `SessionState`, registers the commands, and calls
    `owner.shutdown()` on `RunEvent::Exit`;
  - add `#[cfg_attr(mobile, tauri::mobile_entry_point)]`.
- `session.rs` keeps the owner logic testable without a Tauri runtime:
  `SessionBridge { owner: Mutex<Option<SessionOwner>>, threads }`, with `connect`, `send` and
  `close` methods that take a plain `Sender<String>`.

### TASK-002: Frontend IPC tier (protocol/tauri.ts, app/main.ts)

- `boot()`: when `isTauri(win)` and there is no `?session=`, try `TauriTransport.connect()`. On
  success use `tier: 'native'`, the native `Client`, and
  `audibleFor('native', { client })` from `app/clock.ts`, with start and dispose exactly as
  CANVAS-CLOCK wired the `?session=` tier.
- On rejection, show the error through the existing boot status path and continue with the
  browser Wasm tier.
- After `createEditor`, if `invoke('self_check_enabled')` is true, invoke `self_check` once with
  `report: JSON.stringify({ tier, webgl2, renderer: deps.code ? 'mounted' : 'absent', gpuStatus, effectiveDpr, latencyKind })`.
  `latencyKind` comes from `store.transportSample?.latency_kind`, read after the first tempo
  telemetry or after 3 s, whichever comes first.
  Read `gpuStatus` and `effectiveDpr` from the code pane `role=status` text and
  `window.devicePixelRatio`.

### TASK-003: iOS project (gen/apple/*, tauri.conf.json, index.html)

- Run init once from `editor/src-tauri` with `../../tmp/canvas/tools/cargo-tauri ios init --ci`.
  Use `RUSTUP_TOOLCHAIN=1.83.0`; if init fails only because of the toolchain, use 1.98.1 for
  this command and record that.
- Precondition: the `editor/.gitignore` edit (see Ownership) is already applied.
- After init, make sure `editor/src-tauri/gen/apple/.gitignore` ignores `build/`,
  `Externals/`, `xcuserdata/` and `Pods/`. Append any of those that are missing; this file is
  owned by this plan.
- Leave the generated files listed in the manifest as normal, non-ignored files for the
  workflow commit. Never commit `build/`, `Externals/`, `xcuserdata/` or `Pods/`.
- `tauri.conf.json`: add `bundle.iOS.minimumSystemVersion` ("16.0"). Leave other keys
  untouched.
- `editor/index.html`: add `viewport-fit=cover` to the viewport meta, and an inline style giving
  `#vactr-app` padding from `env(safe-area-inset-*)` (top, right, bottom, left).

### TASK-004: Audio session glue (main.mm, project.yml, Info.ios.plist)

- Edit `gen/apple/Sources/vactr-editor/main.mm`. Before `ffi::start_app()`:
  - import `AVFoundation`;
  - declare `extern "C" void vactr_audio_session_event(uint32_t);`;
  - set the `AVAudioSession` category to `AVAudioSessionCategoryPlayback` and
    `setActive:YES`, logging any `NSError` to stderr;
  - observe `AVAudioSessionInterruptionNotification`: on began send 1; on ended, if
    `AVAudioSessionInterruptionOptionShouldResume` is set, `setActive:YES` and send 2;
  - observe `AVAudioSessionRouteChangeNotification`: send 3.
- Add `AVFoundation.framework` as an SDK dependency in `project.yml`, then regenerate the
  project with `xcodegen generate --spec project.yml` inside `gen/apple`. Commit the regenerated
  `project.pbxproj`.
- Background audio: create `editor/src-tauri/Info.ios.plist` with `UIBackgroundModes = [audio]`
  (Tauri merges it at build). After the simulator build, verify with
  `plutil -p <app>/Info.plist | grep -A2 UIBackgroundModes`. If it was not merged, add the key to
  `gen/apple/vactr-editor_iOS/Info.plist` instead and record which path was used.

### TASK-005: Tests

| File | Situation | Expected outcome |
|------|-----------|------------------|
| editor/test/protocol/tauri.test.ts | Fake `invoke` and `Channel` | `connect` invokes `session_connect` with a Channel |
| same | Channel message | `onText` callback receives it |
| same | `send(text)` | `session_send` called with `{ id, text }` |
| same | `close()` | `session_close` called; later channel messages are ignored |
| same | Rejected connect | Promise rejects with the message |
| editor/test/app/main.test.ts | All CANVAS-CLOCK boot and tier-wiring rows | Still pass, unmodified |
| editor/test/app/main.test.ts | `isTauri` window with a fake API that connects | Tier `native`, `deps.audible` uses `ProbeCorrelation`, `client.clockProbe` called |
| same | Connect rejects | Browser path used (Wasm core faked as in the existing tests) and the error is shown |
| `editor/src-tauri/src/session.rs` `#[cfg(test)]` | `SessionBridge` with Noop host | A `clock-probe` text sent after connect yields a `clock-probe` reply on the receiver |
| same | `close` | Further sends are ignored |

## Pitfalls

- Never use `git add -f` (or any force-add) for ignored files. Fix the ignore rules instead.
- Keep CANVAS-CLOCK's `editor/test/app/main.test.ts` rows unchanged: `audibleFor` browser
  provenance, native probe start, dispose stopping the probe, and boot `?session=` wiring. Add
  the Tauri rows next to them. Boot must keep using `audibleFor` for every tier.
- Doing session work on the Tauri main thread. The `Session` lives only on the owner thread.
- Sending `correlation: null` or renaming probe fields. The wire shape comes from NATIVE and
  CLOCK.
- Committing `gen/apple/build`, Xcode user data (`xcuserdata`) or signing settings. Set no
  `DEVELOPMENT_TEAM`.
- Adding crates (for example objc2) for audio-session glue. The glue is Objective-C in `main.mm`
  only.
- Forgetting that the iOS frontend needs `dist`. Build it first:
  `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`.

## Session 266 Amendment (operator decisions A and D)

- **Artifact roots** (manifest `CANVAS-SHELL.artifactRoots`, each also a writePath; all
  gitignored and never committed):
  - `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `tmp/canvas-cutover/shell`,
    `tmp/canvas/ios-scratch`, `editor/node_modules/.vite`;
  - `editor/dist` (from `npm run build`);
  - `editor/src-tauri/target` and `editor/src-tauri/gen/schemas` (from the separate Tauri
    workspace and tauri-build);
  - `editor/src-tauri/gen/apple/build`, `editor/src-tauri/gen/apple/Externals` and
    `editor/src-tauri/gen/apple/Pods`;
  - `editor/src-tauri/gen/apple/vactr-editor.xcodeproj/xcuserdata` and
    `editor/src-tauri/gen/apple/vactr-editor.xcodeproj/project.xcworkspace/xcuserdata`.
- **Tracked generated files stay writePaths.** The enumerated `editor/src-tauri/gen/apple/...`
  project files are committed. The `editor/.gitignore` change (`src-tauri/gen/` becomes
  `src-tauri/gen/schemas/`) and the generated `gen/apple/.gitignore` (`build/`, `Externals/`,
  `xcuserdata/`, `Pods/`) keep every artifact root ignored. After `ios init`, check:
  `git check-ignore -q` exits 0 for a probe path inside each artifact root and exits 1 for each
  committed gen/apple path.
- **DerivedData.** Record the DerivedData and `.app` locations that the cargo-tauri build
  actually used in `tmp/canvas-cutover/shell/ios-build.log`. The default
  `~/Library/Developer/Xcode/DerivedData` is outside the repository and is never committed. If
  the xcodebuild fallback is used, run it from `editor/src-tauri/gen/apple` with
  `-derivedDataPath build/DerivedData`, so its outputs land inside the `gen/apple/build` artifact
  root. Any other gitignored in-repo output means: stop, record it, and amend the manifest
  serially before acceptance.
- **Setup (not gating)**: `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`,
  then the host-wasm library build.
- **Rule D**: the gating list contains only final-source commands that are expected to pass.
  Failed-then-fixed runs go into history notes only, and negative controls go under
  `mutationEvidence`. A simulator blocker is recorded as a blocker with its exact command, exit
  and log path. It is never listed as a pass.

## Verification (record in tmp/canvas-cutover/shell/checks.log; simulator steps run outside the sandbox)

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |
| `git check-ignore -q <path>` for each of the 33 manifest `editor/src-tauri/gen/apple/...` paths | exit 1 for every path (none ignored); a loop records each exit code in checks.log |
| `git check-ignore -q editor/src-tauri/gen/apple/build/x`, `git check-ignore -q editor/src-tauri/gen/schemas/x`, `git check-ignore -q editor/src-tauri/gen/apple/Externals/x` | exit 0 for each (still ignored) |
| `git status --porcelain --untracked-files=all editor/src-tauri/gen` | Lists only manifest paths, plus at most 5 recorded additions |
| `git diff editor/.gitignore` | Exactly one line changed: `src-tauri/gen/` becomes `src-tauri/gen/schemas/` |
| `CARGO_TERM_QUIET=true cargo clippy --manifest-path editor/src-tauri/Cargo.toml --all-targets -- -D warnings` | exit 0 |
| `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml` | all pass |
| `rustfmt --edition 2021 --check editor/src-tauri/src/lib.rs editor/src-tauri/src/main.rs editor/src-tauri/src/session.rs` | exit 0 |
| `cd editor && npm run check && ./node_modules/.bin/vitest run test/protocol/tauri.test.ts test/app/main.test.ts` | exit 0, all pass |
| Full `cd editor && ./node_modules/.bin/vitest run` (after the Wasm lib build) | all pass |
| `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 |
| `cd editor/src-tauri && ../../tmp/canvas/tools/cargo-tauri ios build --target aarch64-sim --debug` (fallback, run from `editor/src-tauri/gen/apple`: `xcodebuild -project vactr-editor.xcodeproj -scheme vactr-editor_iOS -sdk iphonesimulator -configuration Debug -derivedDataPath build/DerivedData CODE_SIGNING_ALLOWED=NO build`) | exit 0; log `tmp/canvas-cutover/shell/ios-build.log`; exact command recorded |
| `xcrun simctl boot "<iPad simulator name from xcrun simctl list devices available>"`, then `xcrun simctl install booted <.app path>` | exit 0; `simctl-install.log` |
| `SIMCTL_CHILD_VACTR_SELF_CHECK=1 xcrun simctl launch --console-pty booted me.tacogips.vactr`, with a 60 s capture | A `VACTR_SELF_CHECK {...}` line saved to `self-check.json` |
| `xcrun simctl io booted screenshot tmp/canvas-cutover/shell/ipad-sim.png` | File exists |

## Overwrite and Drift Protocol

Record fresh-read sha256 values before each edit (`tmp/canvas-cutover/shell/intent.json`) and
after it (`receipt.json`). On drift, stop editing that file and repair serially after the join.
Edit only this plan's progress log.

## Completion Criteria

- [x] Desktop: commands work in the `SessionBridge` tests, `cargo check`/`clippy`/`test` pass,
      and `TauriTransport` tests pass
- [x] `app/main.ts` selects the IPC native tier with `ProbeCorrelation`, and the fallback works
- [x] iOS project has exactly the manifest-listed generated files and is ready for downstream
      commit; `main.mm` audio-session glue is present
- [x] Simulator build, install, launch and self-check evidence captured, or an exact blocker
      recorded
- [x] No signing or provisioning changes

## Progress Log

### Session: 2026-10-05
**Tasks Completed**: Plan authored
**Notes**: A plan-time scratch `ios init` under `tmp/canvas/ios-scratch` needed approval that
was not granted in the plan node. The template paths were extracted from the pinned tauri-cli
binary instead.

### Session: 2026-10-05 (session 266 plan amendment)
**Tasks Completed**: Plan amended per operator decisions A and D. Artifact roots declared, including the Xcode and Tauri outputs; the DerivedData rule and the fallback `-derivedDataPath` added; setup separated from gating. Tasks and contracts are unchanged.

### Session: 2026-10-05 (CANVAS-SHELL implementation)
**Tasks Completed**: TASK-001 through TASK-005 implemented and locally verified. Desktop IPC commands and
`TauriTransport` pass Rust and frontend checks; `app/main.ts` selects native IPC with audible-clock
probe lifecycle and falls back to Wasm with a visible connection error. Tauri iOS project files match
the 33 manifest entries; AVAudioSession interruption/route glue and background audio are present.
The iOS simulator app built, installed and launched on iPad Pro 11-inch (M5); the captured
`VACTR_SELF_CHECK` reports `tier=native`, `webgl2=true`, `renderer=mounted`, `effectiveDpr=2`, and
`latencyKind=measured`. No signing identity or provisioning setting was changed. Implementation gates
and full logs are recorded under `tmp/canvas-cutover/shell/`; the first frontend check and two
simulator build attempts were superseded by successful final-source reruns documented there.
**Downstream**: independent test-integrity, adversarial and combined-tree reviews, generated-file
staging, commit and push remain owned by later workflow steps.

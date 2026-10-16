# EX-30: Tauri Default-Template Commands and TauriTemplateStore

**Status**: Completed
**Plan ID**: EX-30 (wave 1, parallel)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 6, 6.1, 6.2
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

## Intent and Context

In the Tauri app the user chooses which example or document opens by
default, and can reset it. The choice must persist across app restarts.
The accepted design stores it through three narrow Rust commands that
read/write ONE fixed file, `<app_config_dir>/default-template.json`, and a
TypeScript `TauriTemplateStore` that calls them.

Context:
- `editor/src-tauri/src/lib.rs` (115 lines) registers `session_connect`,
  `session_send`, `session_close`, `self_check_enabled`, `self_check` in
  `tauri::generate_handler!`.
- `editor/src-tauri/build.rs` is plain `tauri_build::build()`: app commands
  are not ACL-gated, so `capabilities/default.json` does NOT change.
- `tauri.conf.json` `frontendDist` is `../dist`; `generate_context!` needs
  `editor/dist` to exist, so `cargo check/test` here require a built dist.
- The crate pins exact versions (`tauri = "=2.11.6"`).

## Non-goals

- Do NOT edit `editor/src-tauri/capabilities/default.json`, `build.rs`,
  `tauri.conf.json`, or the fs plugin scope.
- No command takes a path, file name or directory from the caller.
- No webview `localStorage` use for the app template.
- No startup or picker wiring (EX-60).

## Dependencies

- **dependsOn**: EX-00 (`TemplateStore`, `DefaultTemplate`,
  `StorageResult`, `isDefaultTemplate`, `DOC_MAX_CHARS`)
- **Blocks**: EX-60

## writePaths

- `editor/src-tauri/src/template.rs`
- `editor/src-tauri/src/lib.rs`
- `editor/src-tauri/Cargo.toml`
- `editor/src-tauri/Cargo.lock`
- `editor/src/documents/tauri-templates.ts`
- `editor/test/documents/tauri-templates.test.ts`
- `impl-plans/active/examples-30-tauri-template.md`
- `target` (artifact root)
- `editor/src-tauri/target` (artifact root)
- `editor/src-tauri/gen/schemas` (artifact root)
- `editor/dist` (artifact root; built only to satisfy `generate_context!`)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-30` (artifact root; evidence logs)

## sharedPaths

- `editor/src-tauri/src/lib.rs` (add `mod template;` and three handler
  names only)
- `editor/src-tauri/Cargo.lock` (dependency list of the root package only)

## Patterns to Imitate

- Command style and `Result<_, String>` errors: `editor/src-tauri/src/lib.rs:session_send`,
  `self_check`.
- App path access: `lib.rs:run` uses `app.path().app_data_dir()`; use
  `app.path().app_config_dir()` from a `tauri::AppHandle` argument.
- TS injected-dependency store: `editor/src/platform/files.ts:TauriFiles`
  (constructor takes a loader so tests inject fakes).

## File-level Changes

### `editor/src-tauri/Cargo.toml` / `Cargo.lock`

Add `serde = { version = "=<locked>", features = ["derive"] }` and
`serde_json = "=<locked>"`, where `<locked>` is the version already in
`editor/src-tauri/Cargo.lock` (`name = "serde"` / `name = "serde_json"`
blocks). `cargo check` then adds only these two names to the
`vactr-editor` package's `dependencies` list in the lock; no version
changes. If resolution needs the network and fails, record a blocker.

### `editor/src-tauri/src/template.rs`

- `#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)] #[serde(tag = "kind", rename_all = "lowercase")] pub enum DefaultTemplate { Example { id: String }, Snapshot { name: String, text: String } }`
- File shape `{"version": 1, "template": {...}}` (private struct).
- `pub const FILE_NAME: &str = "default-template.json";`
  `pub const MAX_TEXT_BYTES: usize = 1_048_576;`
- `pub fn validate(t: &DefaultTemplate) -> Result<(), String>`: id 1-96
  chars, first char `[a-z0-9]`, rest `[a-z0-9/-]` (char checks, no regex
  crate); name 1-120 chars without control characters; text at most
  `MAX_TEXT_BYTES` bytes.
- `pub fn read_template(dir: &Path) -> Option<DefaultTemplate>`: missing,
  unreadable, unparsable, `version != 1` or invalid -> `None`; never
  deletes the file.
- `pub fn write_template(dir: &Path, t: &DefaultTemplate) -> Result<(), String>`:
  validate first (invalid -> `Err`, file untouched); `create_dir_all`;
  write `default-template.json.tmp` in `dir`; `rename` over the target;
  remove the tmp file on failure.
- `pub fn remove_template(dir: &Path) -> Result<(), String>`: missing file
  -> `Ok(())`.
- Commands (each `#[tauri::command]`, taking `app: tauri::AppHandle`):
  `default_template_get() -> Result<Option<DefaultTemplate>, String>`,
  `default_template_set(template: DefaultTemplate) -> Result<(), String>`,
  `default_template_reset() -> Result<(), String>`. Each resolves the dir
  with `app.path().app_config_dir()` (error -> `Err(String)`).

### `editor/src-tauri/src/lib.rs`

Add `mod template;` and `template::default_template_get`,
`template::default_template_set`, `template::default_template_reset` to
`generate_handler!`. Nothing else changes.

### `editor/src/documents/tauri-templates.ts`

- `export type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;`
- `export class TauriTemplateStore implements TemplateStore` with
  `constructor(invoke: InvokeFn)`.
- `get()`: `invoke('default_template_get')`; result passing
  `isDefaultTemplate` -> it; anything else (including `undefined` from a
  test mock) or a rejection -> `null`.
- `set(t)`: invalid per `isDefaultTemplate` -> `{ok:false, reason:'too-large'}`
  when the snapshot text is oversize, else `invalid-name`, without
  invoking; otherwise `invoke('default_template_set', { template: t })`;
  rejection -> `{ok:false, reason:'error', message: String(e)}`.
- `reset()`: `invoke('default_template_reset')`; rejection -> `error`.
- Never rejects.

## Invariants

- The template file path is fixed in Rust; the frontend never sends a path.
- `capabilities/default.json` byte-identical (`git diff --exit-code editor/src-tauri/capabilities/default.json`).
- Existing commands unchanged.

## Pitfalls

- serde tag must be `kind` with lowercase variants to match the TS union
  `{kind:'example', id}` / `{kind:'snapshot', name, text}`.
- Tauri maps camelCase JS argument keys to snake_case Rust parameters;
  the single parameter name `template` needs no mapping.
- Tests must not use `AppHandle`; test the `dir`-taking functions with a
  unique dir under `std::env::temp_dir()` (process id plus a counter) and
  remove it at the end. No new dev-dependencies.
- `generate_context!` fails if `editor/dist` is missing: build it first
  (Verification 1). If that build fails only because another wave-1 plan's
  files are mid-edit, wait and retry; record it.
- Do not log template text.

## Tests (input -> expected)

Rust (`#[cfg(test)] mod tests` in `template.rs`):
- write example `{id:"acid-techno"}` then read from the same dir -> equal
  (persistence across a fresh read = across restarts).
- write snapshot then read -> equal; the `.tmp` file does not remain.
- read from an empty dir -> `None`.
- file with `not json` -> `None`, file still exists.
- file with `{"version":2,...}` -> `None`.
- id `"../x"`, `"A"`, a 97-char id -> `write_template` `Err`, no file
  written.
- name empty, 121 chars, or containing `\n` -> `Err`.
- text of `MAX_TEXT_BYTES + 1` bytes -> `Err`.
- remove existing -> file gone; remove missing -> `Ok`.
- serde: `{"kind":"snapshot","name":"n","text":"t"}` deserializes to
  `Snapshot`.

TS (`editor/test/documents/tauri-templates.test.ts`):
- invoke resolves `{kind:'example', id:'showcase'}` -> `get()` returns it,
  invoked with `'default_template_get'`.
- invoke resolves `undefined` or `{kind:'bad'}` -> `null`; invoke rejects -> `null`.
- `set(example)` -> invoke called with `('default_template_set', {template})`, `{ok:true}`.
- `set(snapshot oversize)` -> `too-large`, invoke not called.
- invoke rejects on set/reset -> `{ok:false, reason:'error'}` with message.

## Verification

Evidence in `tmp/examples/EX-30/`, each log ending with its exit status.

1. `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build > ../tmp/examples/EX-30/build.log 2>&1` -> exit 0 (prerequisite for `generate_context!`).
2. `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-30/cargo-check.log 2>&1` -> exit 0.
3. `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-30/cargo-test.log 2>&1` -> exit 0, template tests listed as passed.
4. `cd editor && npx vitest run test/documents/tauri-templates.test.ts > ../tmp/examples/EX-30/vitest.log 2>&1` -> all pass.
5. `cd editor && npm run check > ../tmp/examples/EX-30/check.log 2>&1` -> exit 0 for this plan's files.
6. `git diff --exit-code editor/src-tauri/capabilities/default.json` -> exit 0; `git diff editor/src-tauri/Cargo.lock` shows only the two added dependency names (save to `tmp/examples/EX-30/lock-diff.log`).

Records use `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`.

## Completion Criteria

- [x] Three commands registered; `template.rs` functions and tests exist.
- [x] `TauriTemplateStore` and its tests exist and pass.
- [x] cargo check and cargo test pass; capabilities unchanged; lock diff
      limited to the two names.
- [x] Progress Log with records and pre/post sha256.

## Edit Protocol

Fresh read before each edit; sha256 before/after; writePaths only; stop and
record on drift; no git index or branch operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored.
**Blockers**: None.

### Session: 2026-10-17
**Tasks Completed**: Implemented versioned fixed-path Rust template persistence, registered the three Tauri commands, added the injected TypeScript `TauriTemplateStore`, and added Rust and Vitest coverage. Updated direct serde dependencies to versions already present in the lockfile. No blockers remain for EX-30 implementation.

**Verification**:

```json
{"command":"cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build > ../tmp/examples/EX-30/build-retry.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/build-retry.log","notes":"Build completed; required by tauri::generate_context!."}
{"command":"CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-30/cargo-check-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/cargo-check-final.log","notes":"Tauri crate compiled against the final source."}
{"command":"CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-30/cargo-test-final.log 2>&1","exitStatus":0,"testsRun":9,"testsPassed":9,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/cargo-test-final.log","notes":"All 9 crate tests passed, including template persistence, validation, removal and serde tests."}
{"command":"cd editor && npx vitest run test/documents/tauri-templates.test.ts > ../tmp/examples/EX-30/vitest-final.log 2>&1","exitStatus":0,"testsRun":8,"testsPassed":8,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/vitest-final.log","notes":"All 8 TauriTemplateStore tests passed."}
{"command":"cd editor && npm run check > ../tmp/examples/EX-30/check-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/check-final.log","notes":"TypeScript check passed on the final shared tree."}
{"command":"git diff --exit-code editor/src-tauri/capabilities/default.json > tmp/examples/EX-30/capabilities.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/capabilities.log","notes":"Capability file is byte-identical."}
{"command":"git diff editor/src-tauri/Cargo.lock > tmp/examples/EX-30/lock-diff.log 2>&1; removed_count=$(git diff editor/src-tauri/Cargo.lock | awk '/^-[^-]/{count += 1} END {print count + 0}'); added_count=$(git diff editor/src-tauri/Cargo.lock | awk '/^\\+[^+]/{count += 1} END {print count + 0}'); test \"$removed_count\" = 0 && test \"$added_count\" = 2","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/lock-validation.log","notes":"No lock lines removed; exactly the serde and serde_json names were added to the vactr-editor package dependency list."}
{"command":"rustfmt --edition 2021 --check editor/src-tauri/src/template.rs editor/src-tauri/src/lib.rs > tmp/examples/EX-30/rustfmt.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-30/rustfmt.log","notes":"Both changed Rust files are formatted."}
```

**Earlier attempts**: The first build wrapper used zsh's read-only `status` variable and exited 1 after the build output completed; the corrected `build-retry.log` run above exited 0. The first TypeScript check found mock typing errors in this plan and transient type errors in the concurrent EX-50 `picker-view.tsx`; the mock was corrected and the final shared-tree check passed. Initial rustfmt check found formatting differences in `template.rs`; the file was formatted and the final selected-file check passed. Earlier logs remain in `build.log`, `check.log`, and `rustfmt` command output from the session; none are current unresolved failures.

**Source SHA-256 (before -> after)**:

| File | Before | After |
|---|---|---|
| `editor/src-tauri/src/template.rs` | absent | `028b211054f3e29c6f81a868886e8ab85c17f5cac42bbceb8aa954b14c0be557` |
| `editor/src-tauri/src/lib.rs` | `ce78d8a2fa708018defccc3b784bc2549b4730464edaf6f4e460403d9feea35a` | `4e8456de93e6e65f5e5ebec031d9300de06ecb8ab330a1446f05f81681ee6001` |
| `editor/src-tauri/Cargo.toml` | `4d046ee9d8fbc3504188f9e6965b1ac8908129901665a9eea94dc70249032b1b` | `2b3b80f9f4332925ce6e5bdeb5daaf8823950c6d8f632020d7d1c0ed6125ae35` |
| `editor/src-tauri/Cargo.lock` | `c9e5194b2e908d0d283dad56ee52932f4b2d22691d194dc7661a48459fc79950` | `3c009a97f43c3d25afab9ed98b059ae5f04d6afbb37d9243951cc461c88ba174` |
| `editor/src/documents/tauri-templates.ts` | absent | `22828bd86de63e1506b619c645b502a7fb0190d34e460f43c84bd8a3f0c67ca1` |
| `editor/test/documents/tauri-templates.test.ts` | absent | `4c97a5afdaba902f837f7d49056fa4794dc5d0d738dd3f58fbb738c42f2a97fb` |
| `impl-plans/active/examples-30-tauri-template.md` | `03bcdbf188fd8b92ca4b9d892a652d34ae6a67e31da16e5156c60a7f06a66144` | recorded after this update in `tmp/examples/EX-30/plan-sha256.txt` |

Formal review, integration review, shared documentation/index changes, staging, commit and push remain owned by later workflow steps.

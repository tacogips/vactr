# Vim Keymap: Tauri App-Config Store Implementation Plan

**Status**: Completed
**Plan ID**: VK-02 (wave 0)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md section 8.4
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Parallel (wave 0)**: VK-00, VK-01, VK-03, VK-04
- **Consumer**: VK-03 `TauriKeymapStore` invokes these commands by name; VK-30 wires the store
- Common Execution Rules: `impl-plans/completed/vim-keymap-00-contract.md` (apply in full; this plan runs cargo, not npm)

## Intent and Context

The Tauri app persists the keymap JSON text in the app config directory
through three narrow commands, following
`editor/src-tauri/src/template.rs` (`read_template`, `write_template`,
`remove_template`, `default_template_get/set/reset`, the `TestDir`
test helper). Deep schema validation stays in TypeScript; Rust checks
only size, JSON-object shape and `version == 1`.

## Artifact Roots

`target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`,
`tmp/vim-keymap/VK-02`, plus the two Tauri build outputs:

- `editor/src-tauri/target`: the cargo output directory of the standalone
  Tauri crate (`editor/src-tauri/Cargo.toml` declares its own
  `[workspace]`, so its build does not go to the root `target`).
- `editor/src-tauri/gen/schemas`: the gitignored schemas that
  `tauri-build` generates during cargo builds.

`editor/src-tauri/gen/apple` is tracked source (the iOS project); it is
not an artifact root and must not be modified.

## Non-Goals

- No capability change (`editor/src-tauri/capabilities/default.json`
  stays as is). No new crate dependencies (`serde_json` is already used).
- No change to `template.rs` or `session.rs`.

## File-Level Changes

### editor/src-tauri/src/vim_keymap.rs (new, budget 220 lines)

```rust
pub const FILE_NAME: &str = "vim-keymap.json";
pub const MAX_BYTES: usize = 262_144;
pub fn validate(text: &str) -> Result<(), String>;
pub fn read_keymap(dir: &Path) -> Option<String>;
pub fn write_keymap(dir: &Path, text: &str) -> Result<(), String>;
pub fn remove_keymap(dir: &Path) -> Result<(), String>;
#[tauri::command] pub fn vim_keymap_get(app: tauri::AppHandle) -> Result<Option<String>, String>;
#[tauri::command] pub fn vim_keymap_set(app: tauri::AppHandle, text: String) -> Result<(), String>;
#[tauri::command] pub fn vim_keymap_reset(app: tauri::AppHandle) -> Result<(), String>;
```

- `validate`: `text.len() <= MAX_BYTES`, `serde_json::from_str::<serde_json::Value>`
  is an object whose `"version"` is the integer 1 (`as_u64() == Some(1)`;
  `1.0` and `"1"` are rejected). Error strings: `keymap is too large`,
  `keymap is not a JSON object`, `keymap version must be 1`.
- The file stores the text exactly as given (no re-serialization).
- `write_keymap` validates first, creates the dir, writes
  `vim-keymap.json.tmp`, renames over the target, removes the temp file
  on failure (same as `template.rs:write_template`).
- `read_keymap` returns `None` for a missing, unreadable or invalid file
  and never deletes it.
- `remove_keymap` treats `NotFound` as success.
- Commands resolve `app.path().app_config_dir()` exactly like
  `template.rs`. There is no path parameter anywhere.

### editor/src-tauri/src/lib.rs (shared)

Add `mod vim_keymap;` next to `mod template;` and register
`vim_keymap::vim_keymap_get, vim_keymap::vim_keymap_set, vim_keymap::vim_keymap_reset`
after the template commands in `generate_handler!`. No other edits.

## Invariants

- Only the fixed file name inside `app_config_dir()` is ever written.
- Existing Tauri tests (template, session) still pass.

## Test Cases (`#[cfg(test)] mod tests` in vim_keymap.rs, reuse the TestDir pattern)

- valid `{"version":1,"maps":{}}` -> write then read returns the identical string; temp file absent
- 262,145-byte text -> `write_keymap` errs, no file created
- `[]`, `"x"`, `{"version":2}`, `{"version":"1"}`, `{"version":1.5}` -> errors, nothing written
- invalid file content on disk -> `read_keymap` is `None` and the file still exists
- `remove_keymap` on a missing file -> Ok; on an existing file -> file gone

## Verification (from the worktree root)

1. `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` -> exit 0
2. `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml` -> exit 0, failureCount 0, testsRun includes the new vim_keymap tests (at least 7) plus the existing ones

## Completion Criteria

- [x] vim_keymap.rs exports the pinned items; lib.rs registers the three commands
- [x] Eight new Rust tests pass; cargo check and test exit 0
- [x] capabilities/default.json unchanged
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17
**Tasks Completed**: VK-02 implementation and assigned verification
**Notes**: Added `editor/src-tauri/src/vim_keymap.rs` with size/object/version-1 validation, exact-text reads/writes, atomic temp-file replacement, reset behavior, three app-config commands, and eight unit tests. Registered the commands in `editor/src-tauri/src/lib.rs`. `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` exited 0 (`tmp/vim-keymap/VK-02/logs/cargo-check.log`). `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml` exited 0 with 17 passed and 0 failed (`tmp/vim-keymap/VK-02/logs/cargo-test.log`). `CARGO_TERM_QUIET=true cargo fmt --manifest-path editor/src-tauri/Cargo.toml -- --check` and `git diff --check` exited 0. `editor/src-tauri/capabilities/default.json` has no changes. Independent review and serial finalization remain downstream workflow steps.

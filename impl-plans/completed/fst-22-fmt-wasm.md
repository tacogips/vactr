# FST-22: Raw wasm Export of the Formatter

**Status**: Completed
**Plan ID**: FST-22 (wave 2; parallel with FST-20, FST-21, FST-23)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 3.7.3
**Dispatch**: `impl-plans/active/fst-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The web editor already loads `vactr.wasm`. It is built by
`cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
(`README.md:174`). This plan adds three raw `extern "C"` exports so that
the editor (FST-30) can format documents in the browser and in Tauri.
The exports are pure. They do not use the session outbox and need no
`*_init`, so any instance can call them.

## Non-goals

- No change to `abi.rs`, the outbox, or the session, main or worklet
  halves.
- No wasm-bindgen and no new crate.
- No editor code (that is FST-30).

## Dependencies

- **dependsOn**: FST-10 (`crate::fmt::format_bytes`)
- **Blocks**: FST-30

## writePaths

- `src/host/wasm/mod.rs`: add `pub mod fmt_abi;` in alphabetical order,
  and one line in the module doc listing the export.
- `src/host/wasm/fmt_abi.rs` (new).
- This plan's Progress Log.

## sharedPaths (read-only)

- `src/host/wasm/abi.rs`, the pattern to imitate: `alloc`/`free` at
  lines 106-124, `input` at 94, and the `thread_local!` `RefCell`
  `OUTBOX` with its `outbox_ptr`/`outbox_len` at 36-40 and 128-138.
- `src/fmt/mod.rs` (`format_bytes`, `STATUS_*`).
- `src/host/mod.rs:20`: the gate is `cfg(all(target_arch = "wasm32", feature = "host-wasm"))`, and it already covers everything under `host/wasm`.

## Contract (pinned; FST-30 calls these)

```rust
#[no_mangle] pub unsafe extern "C" fn fmt_source(ptr: *const u8, len: u32) -> u32; // returns STATUS_*
#[no_mangle] pub extern "C" fn fmt_out_ptr() -> *const u8;
#[no_mangle] pub extern "C" fn fmt_out_len() -> u32;
```

- JS writes the UTF-8 source into memory it got from `alloc(len)`, calls
  `fmt_source(ptr, len)`, then reads `fmt_out_len()` bytes starting at
  `fmt_out_ptr()`. After that, JS calls `free(ptr, len)`.
- The output buffer lives in a `thread_local!`
  `RefCell<Vec<u8>>` named `FMT_OUT`. It stays valid until the next call
  to `fmt_source`.
- Status 1 (refused) and status 2 (not UTF-8) both leave the input bytes
  in the output buffer.

## Implementation Key Points

1. In `fmt_source`, use `abi::input(ptr, len)` to borrow the bytes.
   Call `crate::fmt::format_bytes`, store the resulting bytes in
   `FMT_OUT`, and return the status.
2. Every `unsafe` block needs a `// SAFETY:` comment, and every
   `unsafe fn` needs a `# Safety` doc section, following the style of
   `abi.rs`.
3. Never touch `OUTBOX`. Session records must not interleave with
   formatter output.
4. Do not panic. `format_bytes` never panics, and the export code must
   not add `unwrap`.
5. Pitfall: the pointer returned by `fmt_out_ptr` becomes invalid once
   `FMT_OUT` reallocates on a later call. Document in the doc comment
   that JS must read the output before calling `fmt_source` again, and
   must re-read `memory.buffer` after every call because memory may have
   grown.
6. This module does not compile on native targets, so it cannot have a
   native unit test. `format_bytes` is tested natively in FST-10, and the
   real-wasm behavior is tested in FST-30's node test. This plan proves
   the exports with the node commands below.

## Execution Protocol

- Before editing `src/host/wasm/mod.rs`, re-read it and record its
  sha256 and your intent. Record the post-edit sha256 afterwards.
- Make no git state changes.
- Write logs to `tmp/fst/FST-22/<n>-<name>.log`, each ending with an
  `exit=<status>` line.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
2. `CARGO_TERM_QUIET=true cargo clippy --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm -- -D warnings`
3. Export listing. Run:
   `node --input-type=module -e "import fs from 'node:fs'; const m = await WebAssembly.compile(fs.readFileSync('target/wasm32-unknown-unknown/debug/vactr.wasm')); console.log(WebAssembly.Module.exports(m).map(e => e.name).filter(n => n.startsWith('fmt_') || n === 'session_init').sort().join(' '))"`
   It must print `fmt_out_len fmt_out_ptr fmt_source session_init`.
4. Smoke call. Run a node one-liner (it may be longer than one line)
   that does the following:
   - instantiate with `{}`;
   - `alloc` the bytes of `s :bd\n\t\t\t> d1\n` and copy them in;
   - call `fmt_source`;
   - read the output and print the status and the output text.
   It must print status `0` and `s :bd\n\t> d1\n`. If that sample is not
   reader-clean, use FST-10's `continuation.in` instead.
   Run the same one-liner on `s "abc\n`: it must print status `1` and
   the input unchanged.
5. `CARGO_TERM_QUIET=true cargo build` (native) still succeeds, and
   `cargo fmt --check` passes.

## Completion Criteria

- [x] The three exports exist with the pinned signatures and are gated only through `host/wasm`.
- [x] Verification steps 1-5 pass, with the logs showing the printed export names and the smoke-call output.
- [x] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 222 step 4).
**Blockers**: Waits for FST-10.

### Session: 2026-09-30 (wasm formatter exports)
**Tasks Completed**: Added `src/host/wasm/fmt_abi.rs` with `fmt_source`, `fmt_out_ptr`, and `fmt_out_len`; registered the module and documented its exports in `src/host/wasm/mod.rs`. Confirmed the existing `src/host/mod.rs` target/feature gate is the only gate. Updated this plan with implementation and verification evidence.
**Verification**:
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit 0 (`tmp/fst/FST-22/11-wasm-build.log`).
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo clippy --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm -- -D warnings` — exit 0 (`tmp/fst/FST-22/12-wasm-clippy.log`).
- Node export listing — exit 0; printed `fmt_out_len fmt_out_ptr fmt_source session_init` (`tmp/fst/FST-22/13-exports.log`).
- Node wasm smoke call — exit 0; continuation input formatted with status 0 to `s :bd\n\t> d1\n`, and unterminated string returned status 1 unchanged (`tmp/fst/FST-22/14-node-smoke.log`).
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo build` — exit 0 (`tmp/fst/FST-22/15-native-build.log`).
- `rustfmt --check --edition 2021 src/host/wasm/mod.rs src/host/wasm/fmt_abi.rs` — exit 0 (`tmp/fst/FST-22/17-rustfmt-changed-files.log`).
- Earlier `cargo fmt --check` attempt — exit 1 because the concurrent FST-20 `src/cli/mod.rs` declared `src/cli/fmt.rs` before that file appeared (`tmp/fst/FST-22/10-fmt-check.log`). After the file appeared, the final rerun progressed and reported only the remaining `src/cli/tests/fmt.rs` formatting differences.
- Historical attempt: `cargo fmt --check` — exit 1 because rustfmt reported formatting differences only in concurrent FST-20 file `src/cli/tests/fmt.rs` (`tmp/fst/FST-22/16-fmt-check-final.log`). Superseded by the successful final-source rerun below after FST-20 formatted its file.
**Blockers**: None. FST-20 formatted its owned file; all required gates passed in the final-source rerun below.

### Session: 2026-09-30 (verification rerun and completion)
**Tasks Completed**: Re-ran FST-22 verification after the FST-20 formatting dependency was resolved. Marked the plan complete.
**Verification**:
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit 0 (`tmp/fst/FST-22/19-wasm-build-rerun.log`).
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo clippy --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm -- -D warnings` — exit 0 (`tmp/fst/FST-22/20-wasm-clippy-rerun.log`).
- Node export listing — exit 0; printed `fmt_out_len fmt_out_ptr fmt_source session_init` (`tmp/fst/FST-22/21-exports-rerun.log`).
- Node wasm smoke call — exit 0; continuation formatted with status 0 to `s :bd\n\t> d1\n`, and unterminated string returned status 1 unchanged (`tmp/fst/FST-22/24-node-smoke-rerun.log`).
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo build` — exit 0 (`tmp/fst/FST-22/25-native-build-rerun.log`).
- `cargo fmt --check` — exit 0 (`tmp/fst/FST-22/18-fmt-check-rerun.log`).
**Blockers**: None.

### Session: 2026-09-30 (foreground step6 verification)
**Tasks Completed**: Re-ran the assigned FST-22 verification in the foreground against the current worktree. The node smoke command asserted both required behaviors and reported 2/2 passing cases.
**Verification**:
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit 0 (`tmp/fst/FST-22/step6-rerun/01b-wasm-build.log`).
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo clippy --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm -- -D warnings` — exit 0 (`tmp/fst/FST-22/step6-rerun/02-wasm-clippy.log`).
- Node export assertion — exit 0; printed `fmt_out_len fmt_out_ptr fmt_source session_init` (`tmp/fst/FST-22/step6-rerun/03-exports.log`).
- `node tmp/fst/FST-22/wasm-smoke-rerun.mjs` — exit 0; continuation formatted with status 0, unterminated string returned status 1 unchanged; 2/2 assertions passed (`tmp/fst/FST-22/step6-rerun/04-node-smoke.log`).
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/fst/FST-22/target cargo build` — exit 0 (`tmp/fst/FST-22/step6-rerun/05-native-build.log`).
- `CARGO_TERM_QUIET=true cargo fmt --check` — exit 0 (`tmp/fst/FST-22/step6-rerun/06-fmt-check.log`).
**Blockers**: None. Formal integration review and FST-30 editor wiring remain downstream workflow work.

### Session: 2026-09-30 (review-feedback behavioral evidence)
**Tasks Completed**: Copied the review-requested `node:test` ABI harness unchanged into the FST-22 evidence directory and ran it in the foreground against the current isolated WASM build. All six export, continuation, refusal, byte-preservation, idempotence, and empty-input checks passed.
**Verification**:
- `VACTR_WASM=tmp/fst/FST-22/target/wasm32-unknown-unknown/debug/vactr.wasm node --test tmp/fst/FST-22/fmt-abi.test.mjs` — exit 0; 6 tests passed, 0 failed (`tmp/fst/FST-22/step6-feedback-rerun/03-node-test.log`).
**Blockers**: None for FST-22. FST-30 editor wiring and formal test-integrity/adversarial review remain downstream workflow work.

## Related Plans

- **Depends On**: FST-10
- **Next**: FST-30 (editor format command)

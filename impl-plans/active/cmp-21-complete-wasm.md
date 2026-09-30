# CMP-21: Wasm Completion Export

**Status**: Ready
**Plan ID**: CMP-21 (wave 2; parallel with CMP-20, CMP-30, CMP-31)
**Design Reference**: `design-docs/specs/design-completion.md` 5.1, 5.2, 7.3
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The web editor calls the engine through raw, pinned wasm exports, in the
same style as `src/host/wasm/fmt_abi.rs`. The export is pure: it does
not touch the outbox, needs no `*_init`, and runs on the editor's
dedicated tool instance.

## Non-goals

- No change to `src/complete/**`, `fmt_abi.rs`, `abi.rs` or any editor
  source file.
- No session-held snapshot (user-QA C3).

## Dependencies

- **dependsOn**: CMP-10 (`complete::complete_bytes`)
- **Blocks**: CMP-32, CMP-40

## writePaths

- `src/host/wasm/complete_abi.rs` (new)
- `src/host/wasm/mod.rs`: add `pub mod complete_abi;` in alphabetical
  order (before `pub mod fmt_abi;`), plus one doc line after the
  `fmt_abi` doc sentence (`mod.rs:15-16`).
- `editor/test/wasm/complete.test.ts` (new)
- `impl-plans/active/cmp-21-complete-wasm.md` (Progress Log only)

## sharedPaths (read only)

`src/host/wasm/fmt_abi.rs` (the pattern to copy), `src/host/wasm/abi.rs`
(`input`), `src/complete/mod.rs`, `editor/test/support/wasm.ts`,
`editor/test/wasm/format.test.ts` (the test pattern).

## Pinned Exports

```rust
#[no_mangle] pub unsafe extern "C" fn complete_source(ptr: *const u8, len: u32, cursor: u32, limit: u32) -> u32;
#[no_mangle] pub extern "C" fn complete_out_ptr() -> *const u8;
#[no_mangle] pub extern "C" fn complete_out_len() -> u32;
```

## Key Points

- Mirror `fmt_abi.rs` line for line:
  - a `thread_local!` `COMPLETE_OUT: RefCell<Vec<u8>>`;
  - `unsafe { super::abi::input(ptr, len) }` with a `// SAFETY:` comment;
  - store the bytes from `crate::complete::complete_bytes(input, cursor, limit)`;
  - return the status;
  - a `# Safety` doc section.
- Doc comments must say that the output stays valid until the next
  `complete_source`, and that the JS side must refresh `memory.buffer`
  after each call.
- Do not name anything `init`, and do not write to the outbox.

## Tests (`editor/test/wasm/complete.test.ts`, `// @vitest-environment node`)

Use `loadVactrWasm()` and `wasm.withBytes`, then call
`complete_source(ptr, len, cursor, 0)`. Read the output through
`complete_out_ptr` and `complete_out_len` over a FRESH
`new Uint8Array(memory.buffer)`, and `JSON.parse` it.

- `fn f alpha:\n\tal` with the cursor at the byte length: status 0,
  `v === 1`, `context === 'head'`, `items[0].label === 'alpha'`,
  `items[0].kind === 'local'`, and `from`/`to` equal the byte offsets of
  `al` and the end.
- A Japanese comment line before `si` (bytes are not UTF-16 units): `from`
  equals the UTF-8 byte offset of `s`.
- `s :an` at the end: some item has label `:analog`, and `context` is
  `keyword`.
- `[0xff]` with cursor 0 -> status 2. The text `abc` with cursor 99 ->
  status 3.
- After these calls, `wasm.drainRecords()` equals `[]`.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol` and its `retryPolicy`. Write
logs to `tmp/cmp/CMP-21/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
2. `CARGO_TERM_QUIET=true cargo clippy --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm -- -D warnings`
3. `node --input-type=module -e "import fs from 'node:fs'; const m = await WebAssembly.compile(fs.readFileSync('target/wasm32-unknown-unknown/debug/vactr.wasm')); console.log(WebAssembly.Module.exports(m).map(e => e.name).filter(n => n.startsWith('complete_') || n.startsWith('fmt_') || n === 'session_init').sort().join(' '))"`
   prints exactly
   `complete_out_len complete_out_ptr complete_source fmt_out_len fmt_out_ptr fmt_source session_init`.
4. `cd editor && npx vitest run test/wasm/complete.test.ts` (a positive
   count, all passing)
5. `CARGO_TERM_QUIET=true cargo build` (the native build is unaffected)
6. `rustfmt --edition 2021 --check src/host/wasm/complete_abi.rs src/host/wasm/mod.rs`

## Completion Criteria

- [ ] The three exports exist with the pinned signatures and appear in the
      export listing.
- [ ] The node test passes against the real artifact, the outbox stays
      empty, and statuses 2 and 3 are covered.
- [ ] Steps 1-6 are logged, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: Waits for CMP-10.

## Related Plans

- **Depends On**: CMP-10
- **Next**: CMP-32, CMP-40

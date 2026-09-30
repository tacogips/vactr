# EDS-11: Wasm Format Test Honors `VACTR_WASM`

**Status**: In Progress
**Plan ID**: EDS-11 (wave 1; parallel with CMP-10, CMP-15, FST-50, EDS-10, EDS-12)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 5.4 (session-226 amendment D)
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

`editor/test/wasm/format.test.ts:19-24` `readWasm()` hardcodes
`${root}/../target/wasm32-unknown-unknown/debug/vactr.wasm`. So the
`WasmFormatter` test ignores `$VACTR_WASM`, while the rest of the file
uses `loadVactrWasm()` from `editor/test/support/wasm.ts`, which honors
it. A run with `VACTR_WASM` pointing at another artifact therefore tests
two different binaries.

## Non-goals

- No change to `editor/test/support/wasm.ts`, to `format.ts` or
  `format-core.ts`, or to any other test.
- The `WasmFormatter('test://vactr.wasm', fetchFn)` call keeps the
  two-argument URL form, which EDS-12 keeps compatible.

## Dependencies

- **dependsOn**: none. EDS-12 keeps `WasmFormatter(url, fetchFn)` and
  keeps re-exporting it from `format.ts`, so this file's import stays
  valid.
- **Blocks**: CMP-40

## writePaths

- `editor/test/wasm/format.test.ts`
- `impl-plans/active/eds-11-wasm-format-loader.md` (Progress Log only)

## sharedPaths (read only)

`editor/test/support/wasm.ts` (`loadVactrWasm`, `VactrWasm.path`).

## File-level Change

- Replace the body of `readWasm()` so it reads the bytes from the path of
  `(await loadVactrWasm()).path`, keeping the existing `node:fs` dynamic
  import idiom.
- Alternatively, remove `readWasm()` and read the path inline in the
  third test.
- After the change, the literal `target/wasm32-unknown-unknown` must not
  appear anywhere in the file.

## Key Points

- `loadVactrWasm()` resolves `VACTR_WASM` relative to `process.cwd()`
  and throws when the file is missing. Reuse its `path`, and do not
  re-implement path joining.
- Keep all three existing `it` cases and their assertions unchanged.

## Tests

The three existing cases are the behavioral evidence. They must still
pass with the default artifact, and also with an explicit `VACTR_WASM`.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`. Write logs to
`tmp/cmp/EDS-11/attempt-<n>/`. The wasm artifact may need
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
first. Run it from the repository root. A compile error located in
another plan's in-progress writePaths is transient: follow the
`retryPolicy`.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
2. `cd editor && npx vitest run test/wasm/format.test.ts`: 3 tests pass.
3. `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npx vitest run test/wasm/format.test.ts`:
   3 tests pass.
4. `grep -c 'target/wasm32-unknown-unknown' editor/test/wasm/format.test.ts`
   prints `0`. This grep exits 1 when there are zero matches; record that
   as the expected pass.
5. `cd editor && npx tsc --noEmit`

## Completion Criteria

- [x] No hardcoded artifact path remains in the file, and every artifact
      read goes through `loadVactrWasm()`.
- [x] Steps 1-5 are logged, and all three tests pass in both runs.
- [x] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

### Session: 2026-09-30 (session 226, step 6 implementation)
**Tasks Completed**: `readWasm()` now calls `loadVactrWasm()` and reads the
resolved `wasm.path`; the three existing test cases and assertions are
unchanged. The file no longer contains the hardcoded target path.
**Pre-edit snapshot**: `editor/test/wasm/format.test.ts`
`4665970704b65fb11a4945fcbe9a7266b5117a717e99232fa33f97bd3d3a4374`;
intent in `tmp/cmp/EDS-11/attempt-1/edit-intent-01.json`.
Post-edit SHA-256:
`9b7f3b289852437d99120233665f464f5cc8044591bdd5c91dc6b9ea02d746f5`.
**Verification** (complete logs under `tmp/cmp/EDS-11/attempt-1/`):
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit 0 (`01-wasm-build.log`).
- `cd editor && npx vitest run test/wasm/format.test.ts` — exit 0, 3 passed (`02-vitest-default.log`).
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npx vitest run test/wasm/format.test.ts` — exit 0, 3 passed (`03-vitest-vactr-wasm.log`).
- `grep -c 'target/wasm32-unknown-unknown' editor/test/wasm/format.test.ts` — output `0`, expected grep exit 1 (`04-hardcoded-path-check.log`).
- `cd editor && npx tsc --noEmit` — exit 0 (`05-tsc.log`).
**Edit protocol**: The first build wrapper exited 1 after cargo completed
because zsh reserves `status`; the build was rerun with a non-reserved
variable and exited 0. Plan edit intent and pre-edit hash are in
`tmp/cmp/EDS-11/attempt-1/edit-intent-02.json`.
**Blockers**: None.

## Related Plans

- **Next**: CMP-40

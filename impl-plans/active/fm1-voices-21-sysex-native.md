# FM1V-21: `fm6-sysex` Native and `SourceLoader::read_bytes`

**Status**: Ready
**Plan ID**: FM1V-21 (wave 2)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("SysEx import" -> "Native", "Usage"; user question FV6)
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

Users load their own six-operator SysEx files from `.vact` code:

```
let bank = fm6-sysex "./my-patches.syx"
```

`bank` is a list of voices, and each voice is a list of 155 ints. The bytes
are read on the evaluator thread through a new `SourceLoader::read_bytes`
method:

- only the native loader implements it, with the same path resolution and
  containment as `read` and a 64 KiB cap;
- the session loader delegates to its inner loader;
- every other loader, including the browser, keeps the default, which fails
  with `host-unavailable` (FV6).

The bytes are parsed by FM1V-12's `fm::sysex::parse`.

## Non-goals

- No UGen or registry change. Rendering a patch is FM1V-40.
- No browser file loading.
- No change to `load` behaviour.
- No song-mode support: song loaders keep the default `host-unavailable`.
- Do not edit `src/types/natives_domain.rs`, which belongs to the registry
  plans.

## Dependencies

- **dependsOn**: FM1V-12 (`parse`, `SysexError` and its `Display`, the
  test-only `encode_single`/`encode_bulk`)
- **Blocks**: FM1V-40

## writePaths

- `src/ns/fm6_sysex.rs` (new)
- `src/ns/load.rs`
- `src/ns/mod.rs`
- `src/ns/tests/mod.rs`
- `src/ns/tests/fm6_sysex.rs` (new)
- `src/types/natives.rs`
- `src/host/native/loader.rs`
- `src/session/session.rs`
- `src/vm/tests/native_table.rs`
- `impl-plans/active/fm1-voices-21-sysex-native.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-21` (artifact root)

## sharedPaths

None. The sibling run wf/fm1-tuning also appends to `src/types/natives.rs`.
Put this entry directly after the `load` entry, not at the end of the
table, so the merge regions differ.

## Read-only References

- `src/ns/load.rs:76-140`: `register_load`, `load` (effect-mode check,
  `LoaderHost` downcast, failure codes).
- `src/host/native/loader.rs:165-265`: `contained`, `read_limited`,
  `NativeSampleLoader::read`/`resolve`.
- `src/session/session.rs:105-113`: `SessionLoader` delegation.
- `src/ns/tests/load.rs` and `src/ns/tests/reactive_basic.rs::MapLoader`:
  the test loader and evaluator harness.
- A list-returning native, for building `Value::List`: find one with
  `grep -rn '"enumerate"' src/vm`.

## File-level Changes

**`src/ns/load.rs`**

- Add a trait method to `SourceLoader`. Its doc comment states that the
  default fails with `host-unavailable`.

  ```rust
  fn read_bytes(&mut self, path: &PathVal, limit: u64) -> Result<Vec<u8>, Failure> { /* default: Err(HostUnavailable) */ }
  ```

- `register_load` also registers `fm6-sysex` (`crate::ns::fm6_sysex::fm6_sysex`),
  with the same `slot(..).is_none()` guard.

**`src/ns/fm6_sysex.rs`**

- `pub(crate) const MAX_SYSEX_BYTES: u64 = 64 * 1024;`
- `pub(crate) fn fm6_sysex(cx: &mut NativeCx<'_>, args: &[Value], kw: &[(KwId, Value)]) -> Result<Value, Failure>`.
  It mirrors `load`:
  - query mode gives `EffectInQuery` with the message
    "`fm6-sysex` is not allowed inside a query";
  - a non-path argument gives `FailCode::Type`;
  - with no `LoaderHost`, it gives `HostUnavailable`.
  - Otherwise it calls `loader.read_bytes(&path, MAX_SYSEX_BYTES)` and then
    `parse`.
  - A parse error gives
    `Failure::new(FailCode::LoadFailed, format!("fm6-sysex `{}`: {err}", path.text))`.
  - On success it returns a list of lists of `Value::Int`, one inner list
    per voice in file order, each holding 155 values in `patch.rs` order.

**`src/ns/mod.rs`**: add `pub mod fm6_sysex;`.

**`src/ns/tests/mod.rs`**: add `mod fm6_sysex;`.

**`src/types/natives.rs`**: right after the `load` entry, add
`f("fm6-sysex", 1, 1, &["fn path -> [[int]]"], &[V]).effect(),`.

**`src/host/native/loader.rs`**: implement `read_bytes` for
`NativeSampleLoader`:

- use `self.resolve(path)?` and then `read_limited(&file, limit, &path.text)`;
- do not register a `FileId`, because bytes are not source text.

**`src/session/session.rs`**: in `SessionLoader`,
`fn read_bytes(..) { self.inner.read_bytes(path, limit) }`.

**`src/vm/tests/native_table.rs`**: in
`the_full_prelude_is_the_table_minus_load`:

- the domain prelude now lacks both `load` and `fm6-sysex`;
- assert `table.len() - 2`;
- assert `p.slot(intern_sym("fm6-sysex")).is_none()`;
- rename the test to `the_full_prelude_is_the_table_minus_loader_natives`.

This is an exact count, not a loosened one.

## Pitfalls

- **Respect the file limit.** Reading over the limit must fail through
  `read_limited`'s existing error, not by truncating.
- **Keep the existing `read` path unchanged.** Only add.
- **Parse on the evaluator thread.** Never touch audio-thread code.
- **Check `Value::Int`.** Use the integer constructor that natives such as
  `len` use. If `int` in the type signature requires a specific variant,
  match it, so that `fn path -> [[int]]` type-checks.
- **Expect related test failures.** `infer_call.rs`/types tests may
  enumerate effectful natives. Run the focused filters below and fix only
  real failures inside writePaths. Do not weaken assertions.

## Tests (`src/ns/tests/fm6_sysex.rs`; names contain `fm6_sysex`)

The test loader is a `MapLoader`-like struct that implements both `read`
and `read_bytes`, serving bytes from FM1V-12's `encode_single`/`encode_bulk`.

- `fm6_sysex_native_returns_bulk_voices`: evaluating
  `len (fm6-sysex "./bank.syx")` gives 32, and
  `len ((fm6-sysex "./bank.syx") 0)` gives 155. The first voice's values
  equal the fixture's `params`.
- `fm6_sysex_native_returns_single_voice`: the length is 1.
- `fm6_sysex_native_reports_checksum_error`: corrupted bytes give a failure
  whose message contains `fm6-sysex` and `checksum`.
- `fm6_sysex_native_without_bytes_support_is_host_unavailable`: a loader
  with only `read` gives `FailCode::HostUnavailable`.
- `fm6_sysex_native_rejects_query_mode`, if the harness can express a query.
  Otherwise test it the way `load`'s query test does in
  `src/ns/tests/load.rs`.
- `fm6_sysex_native_type_signature`: the native table entry is effectful
  and its type prints as `fn path -> [[int]]`.
- `fm6_sysex_native_loader_enforces_limit`. Use `NativeSampleLoader` over a
  unique directory under `std::env::temp_dir()`, removed at the end of the
  test. It holds a 70 KiB file and a 4104-byte file. Reading the 70 KiB file
  fails, and the 4104-byte file reads back exactly. Do not write test files
  inside the repository.

## Verification (logs under `tmp/fm1-voices/FM1V-21/`)

1. `rustfmt --edition 2021 --check` on every `.rs` in writePaths must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6_sysex|native_table|natives|load|infer|no_abort|session/)' > tmp/fm1-voices/FM1V-21/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 7 tests from this plan and 0 failed.
3. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/fm1-voices/FM1V-21/wasm.log 2>&1; echo "exit=$?"`
   must print `exit=0`. This proves the default trait method compiles for
   the browser.
4. `git diff --stat` shows only writePaths, and each file stays under 1000
   lines.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

- Fresh-read each file before editing it.
- Record the `shasum -a 256` before and after for `natives.rs`, `load.rs`,
  `loader.rs` and `session.rs`.
- FM1V-20 and FM1V-30 run in the same wave on other files. If a build fails
  outside your writePaths, wait and re-run.

## Done Criteria

- [ ] `read_bytes`, the native and its registration are implemented.
- [ ] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

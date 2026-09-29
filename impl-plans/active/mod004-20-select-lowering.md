# MOD004-20: `.vact` Output Selection, Checker Typing and Lowering

**Status**: Ready
**Plan ID**: MOD004-20 (wave 2; parallel with MOD004-21 and MOD004-22)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (`.vact` selection and template migration: Notation, selection data flow, Selected root, Bus bodies; Capacity)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004A lowering part, MOD-004E selector part)
**Baseline**: `85a300a`. MOD004-00/10/11/12 are committed and accepted.
**Created**: 2026-09-29
**Last Updated**: 2026-09-29 (refined for session 192: surface syntax, current line references)

## Intent and Context

Owner decision (2026-09-29): select a multi-output UGen's output by
calling the node value with a keyword or with an index. The design writes
this as `(p :aux)` / `(p 1)` in s-expression notation. **In `.vact`
source, `( )` is not syntax** (`src/reader/lexer.rs` reports
`paren-form`). The source form is `p :aux`, or `{p :aux}` where grouping
is needed, exactly like the dict call `{d :gain}`
(`src/types/tests/check_basic.rs:69`). Every `.vact` string in this plan
uses the source form. There is no grammar change. A bad selection fails
when the `inst` is defined, and an unselected use means output 0. This
plan implements the VM call, the static-checker typing, and the lowering
of the evaluator-only `UGenKind::Output(k)` wrapper to an edge with
`output = k`. It also enforces the shape, voice-layout and slice-capacity
rules at definition time, using the MOD004-10 functions.

Already in place at `85a300a` (do not re-create):
- `src/dsp/graph/shape.rs` (re-exported by `src/dsp/graph.rs` through
  `pub use shape::*`): `select_output`, `OutputSelector`, `SelectError`,
  `derive_shapes`, `voice_layout`, `assign_slices`, `decl_for_spec`,
  `ShapeError`, `MAX_AUDIO_BUFFERS`;
- `UGenKind::Output(u8)` in `src/dsp/graph.rs`, with its doc comment;
- `Edge.output`;
- two placeholder arms in `src/dsp/build.rs` (lines 182-183 and 423-424)
  that return `LowerError::ty("output selection is not supported yet")`.
  This plan replaces both.

`inst` bodies compile as function bodies
(`src/compile/compiler.rs:281`, `def_fn(n, true, FnDef::Inst)`), so
`let` works inside them, as in `fn` bodies
(`src/types/tests/scope.rs:55`).

## Non-goals

- No template edits (MOD004-30), codec (MOD004-21) or runtime (MOD004-22)
  work.
- No `validate_ugen_output` in `src/ns/insts.rs`. The VM already validates
  selection, so `insts.rs` is untouched.
- No new `FailCode` and no grammar or reader change.

## Dependencies

- **dependsOn**: MOD004-10
- **Blocks**: MOD004-30

## writePaths

- `src/vm/call.rs`: a `Value::UGen` arm in `call_collection`
- `src/types/infer_call.rs`: a `Ty::UGen` callee arm
- `src/dsp/build.rs`: `Src::Node` gains the output index; wrapper
  lowering; the post-lowering shape checks. Replace the MOD004-10
  placeholder arms.
- `src/dsp/build/select.rs` (new, optional): helpers if `build.rs` would
  pass about 900 lines
- `src/types/tests/inst/ugens.rs`: append checker tests
- `src/host/tests/e2e/templates/select_output.rs` (stub from MOD004-00;
  fill it)
- this plan's Progress Log

## sharedPaths (read-only)

`src/dsp/graph/shape.rs` (`select_output`, `derive_shapes`,
`voice_layout`, `assign_slices`, `OutputSelector`, `SelectError`,
`ShapeError`), `src/vm/fail.rs`, `src/value/access.rs` (the `index`/`get`
pattern), `src/ns/insts.rs`, `src/types/tests/inst/overloads.rs`.

`src/types/tests/inst/overloads.rs:47` asserts that `saw 440` outside a
body is `not-callable`. `saw` is a `Value::Signal`, not a `Value::UGen`,
so this plan does not affect it. Do not edit it.

## File-level Changes

### `src/vm/call.rs:call_collection`

- Add `Value::UGen(_)` to the arity branch: zero or more than one argument
  -> `FailCode::Arity`, with the message "a unit generator takes one
  output name or index".
- New arm `Value::UGen(n)`. The argument decides the selector:
  - `Value::Keyword(k)` -> `OutputSelector::Name(k)`;
  - `Value::Int(i)` or `Int64` -> `Index`;
  - anything else -> `FailCode::Type`, with "select an output with a
    keyword or an int".
- Call `crate::dsp::graph::select_output(&n.kind, sel)`:
  - `Ok(k)` -> return `Value::UGen(Rc::new(UGenNode { kind: UGenKind::Output(k), args: [(None, UGenInput::Node(Rc::clone(n)))] }))`;
  - `SingleOutput` -> `not_callable` (the existing helper and message);
  - `Unknown` -> `FailCode::UnknownField`, with a message that names the
    selector and lists the node's declared names.
- Return the wrapper even for index 0, so that selecting from a selection
  fails.

### `src/types/infer_call.rs` (`value_check`, the `match self.u.shallow(callee)` at about line 569; imitate the `Ty::Dict(_, v)` arm at about line 626)

- Add `Ty::UGen =>`:
  - exactly one positional argument whose type unifies with keyword or int
    -> `Ty::UGen`;
  - otherwise emit `DiagCode::TypeMismatch` ("a ugen is called with one
    output name or index") and return `Ty::UGen`.
  - Do not validate names; the checker cannot know the node kind.

### `src/dsp/build.rs`

- Local `enum Src { Node(u16), List(..) }` becomes `Node(u16, u8)`, where
  the `u8` is the source output index. `input()` maps
  `UGenInput::Node(n)` as follows:
  - if `n.kind` is `UGenKind::Output(k)`, lower `args[0]`'s inner node
    through `self.node(inner, depth + 1)` (memoized by pointer) and return
    `Src::Node(idx, k)`;
  - otherwise return `Src::Node(idx, 0)`.
  Edges get `output` from `Src`.
- **Root handling in `lower_inst`**: if the root value is a wrapper with
  `k != 0`, lower the inner node, then push a `UGenSpec::Add` node fed by
  `(inner, output k)` on port 0, leaving port 1 unconnected. That Add is
  the sink. If `k == 0`, lower the inner node as the root, as today. The
  wrapper never becomes a node.
- **Bus bodies** (`lower_bus`): a wrapper anywhere -> `LowerError::ty("output selection is only valid in an inst body")`.
- **After all nodes and edges are lowered in `lower_inst`**:
  - `derive_shapes(n, |i| decl_for_spec(&nodes[i]), &edges, &mut shapes)`;
  - `voice_layout(...)`;
  - `assign_slices(&identity_order, ...)`, where lowered order is already
    topological because sources are pushed before users.
  Map errors to `LowerError` messages:
  - `Mismatch` -> "stereo output connected to mono-only input `<port>` of `<node kind>`";
  - `StereoAuxOut` -> "a stereo voice cannot also use aux-out";
  - `BadOutput` -> "output index out of range";
  - `TooManyBuffers` -> a `LowerError` with `FailCode::InstFailed` and
    `DiagCode::GraphTooLarge` (the same code and fault as `too_large`), but
    with a message that names the 512 channel-buffer cap (for example "the
    instrument needs more than 512 audio buffers"). Build it with a small
    sibling helper next to `too_large` (`build.rs:232`). Do not reuse
    `too_large`, because it formats "has more than {NODE_CAP} nodes", which
    would be a false message for a 205-node instrument.
- **Lowering invariant (MUST hold)**: for graphs with no wrapper, the node
  list, node order, edge list, edge order and ports are exactly as before,
  with `output = 0`. Do not reorder pushes, and do not add dedup or any
  other normalization.

## Pitfalls

- `insts.rs` passes the root through `Value::UGen(n)` (`src/ns/insts.rs:716`).
  A wrapper root arrives there as a `UGenNode` of kind `Output`. Handle it
  in `lower_inst`, not in `insts.rs`.
- `Rc::as_ptr` memoization is what makes `{p :main}` and `{p :aux}` one
  node. Memoize the inner node, never the wrapper.
- No existing test is expected to break. If a test outside writePaths
  fails because it asserted that calling a ugen *value* with one keyword
  or int argument is `not-callable`, do not edit it. Record the test name
  and failure in the Progress Log as a blocker for serial repair by the
  orchestrator.
- If `let` bindings are not accepted inside an `inst` body, do not change
  the grammar. Record it as a blocker and stop, because the design's
  migration form depends on it.
- Never write `(p :aux)` in a `.vact` string. The lexer rejects `( )`. Use
  `p :aux` or `{p :aux}`.
- The VM path is `src/vm/call.rs` `call_collection` (line 262), which
  `call` reaches through the `other =>` arm at about line 205. That arm
  already rejects keyword arguments (`p.kw` non-empty) with
  `not_callable`, so `{p main: 1}` stays `not-callable`. Do not change
  that arm.
- `build.rs` is 591 lines. Keep it under 1000, and move helpers to
  `build/select.rs` if needed. If you create `build/select.rs`, declare it
  with `mod select;` inside `build.rs`. `src/dsp/build/names.rs` shows the
  existing submodule pattern.

## Test Cases

All `.vact` strings below are in source form (`{p :aux}`, never
`(p :aux)`). This plan's first test pins the exact source form that
MOD004-30 will copy, so it must pass before anything else. If
`let p {...}` or `{p :main} > ...` does not parse, do not change the
reader. Record the exact diagnostic in the Progress Log as a blocker and
stop.

In `src/host/tests/e2e/templates/select_output.rs` (imitate
`voice_engines.rs` for `E2e::new()`, `eval`, and registry `entries()`
lookup):
- `inst sel-a:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p :main} > * amp > + {{p :aux} > * amp > aux-out}`
  -> defines with no fault. `entry.def.nodes` contains exactly one
  `VaFilter` and one `VaSource`. Edges from the `VaFilter` have `output`
  values {0, 1}.
- The same with `{p 0}`/`{p 1}` -> the same `InstDef` as `sel-a`.
- An unselected `p > * amp` -> every edge has `output == 0`.
- `{p :left}` -> the definition fails with `unknown-field`. `{p 2}` ->
  `unknown-field`. `{p 1.5}` -> `type`. `{p :main :aux}` -> `arity`.
- A single-output ugen value called with `:main` (for example
  `let o {sin-osc freq}` or any other single-output ugen, then
  `{o :main}`) -> `not-callable`. `{{p :aux} :main}` -> `not-callable`.
  Do not use `saw`: without arguments it is a `Signal`, not a ugen.
- `let s {sample-play bank}` then `{s :stereo} > lpf 800` -> the
  definition fails with the stereo-to-mono message.
  `{s :stereo} > + {x > aux-out}` -> the definition fails with the
  stereo aux-out message.
- A root `{p :aux}` -> the definition succeeds. The last node is `Add`,
  fed by an edge with `output == 1`. Port 1 of that `Add` has no edge.
- The `{s :stereo}` root through a voice effect (for example `> gain 0`)
  -> the definition succeeds.
- A selection inside a `bus` body (for example a bus whose body uses a
  bound multi-output node, if a multi-output ugen is available there) ->
  a lowering type error with "output selection is only valid in an inst
  body". Skip this case with a Progress Log note if no multi-output ugen
  can be constructed in a bus body.
- The golden graph digests are unchanged for every prelude template, which
  covers the lowering invariant.
- Buffer-cap diagnostic from source: build a `.vact` program in the test
  with `format!`/a loop that binds 103 `sample-play` nodes (`let s0
  {sample-play bank}` ... `let s102 {sample-play bank}`) and sums their
  `:stereo` outputs with `+` (`{s0 :stereo} > + {s1 :stereo} > + ...`). That
  is 103*3 + 102*2 = 513 channel slices with about 205 nodes (under
  NODE_CAP 256), so the `inst` definition fails with
  `DiagCode::GraphTooLarge` and a message that names the 512-buffer cap, not
  a node count. The same program with 102 `sample-play` nodes (508 slices)
  defines successfully. Imitate the assertion pattern at
  `src/dsp/tests/dsp/caps.rs:73`
  (`assert_eq!(err.code, DiagCode::GraphTooLarge)`). Pitfall: if the E2e
  harness cannot expose the diagnostic code, assert the `graph-too-large`
  fault/diagnostic through whatever the existing `caps.rs`/`instdef.rs`
  tests use.

Use the real `.vact` names of the node kinds (`sample-play`, `sin-osc`,
`va-source`, `va-filter`, and so on) from
`src/dsp/ugen/catalog.rs` / `src/dsp/build/names/table.rs`. Where the
names above differ from the table, use the table's names and record the
substitution.

In `src/types/tests/inst/ugens.rs`:
- a valid selection program (the `sel-a` text) -> no diagnostics;
- `{p "x"}` -> a `type-mismatch` diagnostic;
- two arguments -> a `type-mismatch` diagnostic.

## Verification Commands (logs in `tmp/mod004/MOD004-20/`)

1. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
2. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
3. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --features lsp` -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run select_output ugens golden` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0.
   E2e renders of `{s :stereo}` programs may need MOD004-22's runtime.
   Test only the definition and lowering here, and leave rendering to
   MOD004-30/40.
7. `wc -l src/vm/call.rs src/types/infer_call.rs src/dsp/build.rs` -> each below 1000
8. `rustfmt --edition 2021 --check <changed .rs files>` -> exit 0

## Completion Criteria

- [x] `{p :aux}` and `{p 1}` (source form) lower to one node with an `output = 1` edge, and unselected uses mean output 0. The sel-a source form is proven, and it is recorded in the Progress Log for MOD004-30.
- [x] Selector failures occur during `inst` definition with the design-specified `not-callable`, `unknown-field`, `type`, and `arity` failure codes.
- [x] The checker types `(ugen kw|int)` as `ugen` and emits `type-mismatch` for invalid selectors.
- [x] Shape, voice-layout and capacity errors are definition-time lowering errors.
- [x] Golden graph and render digests are unchanged in the focused golden tests.
- [x] Local checks 1-5 and 7-8 pass, including focused source-form tests and formatting.
- [ ] Full nextest check 6 passes; the coordinator runs it outside the sandbox.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff` and `git status` are allowed.
2. Record pre-edit hashes and an intent snapshot in the Progress Log.
3. Re-read each file just before editing it; re-apply only your own change
   if it drifted.
4. Never edit outside writePaths. MOD004-21 and MOD004-22 run at the same
   time. If cargo fails in their files, wait about 60 s and retry, up to
   10 times, then record a blocker.
5. Format only your own files.
6. Record post-hashes, exit statuses and log paths in this Progress Log
   only.

## Progress Log

### Session: 2026-09-29 MOD004-20 implementation

**Status**: In Progress

**Tasks completed**: VM selection calls, checker typing, edge output lowering, selected-root Add sink, bus-body selection rejection, definition-time shape/layout/slice-capacity validation, and focused source-form coverage.

**Behavioral evidence**:
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run select_output ugens golden` first exposed three test-assumption issues; complete log: `tmp/mod004/MOD004-20/nextest-focused.log`, exit 100. Tests were updated to the actual `inst` wrapper semantics and top-level checker behavior.
- The same focused command then passed: 21 tests passed, 1541 skipped, exit 0; complete log: `tmp/mod004/MOD004-20/nextest-focused-rerun.log`.
- The focused run includes the 513-slice `GraphTooLarge` / 512-cap case and the 508-slice definition case, plus unchanged golden checks.
- `rustfmt --edition 2021 --check src/vm/call.rs src/types/infer_call.rs src/dsp/build.rs src/types/tests/inst/ugens.rs src/host/tests/e2e/templates/select_output.rs` passed, exit 0; log: `tmp/mod004/MOD004-20/rustfmt-check.log`.
- The touched Rust files are below 1000 lines; counts and exit 0 are in `tmp/mod004/MOD004-20/line-counts.log`.
- `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` and `CARGO_TERM_QUIET=true cargo check -q --features lsp` passed, exit 0; logs: `check-wasm.log`, `check-lsp.log`.

**Shared-tree aggregate gates pending**:
- `CARGO_TERM_QUIET=true cargo check -q --all-targets` failed with exit 101 in concurrent `src/dsp/tests/dsp/voice_layout.rs` (`(f32, f32)` is not an iterator at `.map`); log: `tmp/mod004/MOD004-20/check-all-targets.log`.
- `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` failed with exit 101 on concurrent `src/dsp/voice.rs` `needless_option_as_deref` findings and one local needless lifetime. The local test-helper lifetime was removed; runtime findings are outside this plan; log: `tmp/mod004/MOD004-20/clippy-all-targets.log`.
- Retry full compile, Clippy, focused tests, and the full nextest suite after concurrent runtime files settle. The bus-body selector test is skipped: bus bodies accept only their fixed effect chain and cannot construct a multi-output UGen value; `lower_bus` directly rejects an Output wrapper with the planned type message.

### Retry results and final handoff: 2026-09-29

- `CARGO_TERM_QUIET=true cargo check -q --all-targets` passed, exit 0; complete log: `tmp/mod004/MOD004-20/check-all-targets-retry1.log`.
- `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` passed, exit 0; complete log: `tmp/mod004/MOD004-20/clippy-all-targets-retry2.log`.
- `CARGO_TERM_QUIET=true cargo fmt --check` passed, exit 0; log: `tmp/mod004/MOD004-20/cargo-fmt-check.log`.
- Final focused command `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run select_output ugens golden` passed: 21 passed, 1549 skipped, exit 0; log: `tmp/mod004/MOD004-20/nextest-focused-final-quiet.log`.
- Final `cargo check` wasm and LSP commands passed, exit 0; logs: `tmp/mod004/MOD004-20/check-wasm-final.log` and `check-lsp-final.log`.
- At this checkpoint the full suite had two `voice_stereo` pan assertions that were subsequently updated under MOD004-22; the coordinator owns the full-suite run after the review fixes.
- Selected bus body coverage remains skipped as allowed by the plan; a bus body cannot construct a multi-output UGen through the current fixed effect-chain grammar. `lower_bus` contains the planned rejection for an Output wrapper.

### Final-source verification after assertion tightening: 2026-09-29

- Final focused command including the exact declared-output-name and all-edge-default assertions passed: 21 passed, 1549 skipped, exit 0; complete log: `tmp/mod004/MOD004-20/nextest-focused-final-assertions.log`.
- Final-source gates passed: `CARGO_TERM_QUIET=true cargo check -q --all-targets` (`check-all-targets-final.log`), `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` (`clippy-all-targets-final.log`), wasm lib check (`check-wasm-final-source.log`), LSP check (`check-lsp-final-source.log`), `CARGO_TERM_QUIET=true cargo fmt --check` (`cargo-fmt-check-final.log`), exact changed-file rustfmt (`rustfmt-check-final.log`), and the under-1000-line gate (`line-counts-final.log`); each exit 0.
- The coordinator runs the full suite outside the sandbox after the review fixes; this plan's final full-suite criterion remains open until that result is recorded.

### Review fixes: 2026-09-29

- Selector definition failures now preserve the design-specified failure codes instead of wrapping selection errors as `inst-failed`; source-form tests assert the codes.
- The checker accepts an unresolved/`Any` selector type, with a regression test.
- Final checks passed: `cargo fmt --check`; `cargo check -q --all-targets`; `cargo clippy -q --all-targets -- -D warnings`; wasm32 lib and LSP feature checks; focused nextest covering `select_output`, `ugens`, and `golden` (42 passed, 1,531 skipped across the combined requested filter); all touched Rust files are below 1,000 lines. Cargo checks used `CARGO_TERM_QUIET=true`.
- The full nextest suite is run by the coordinator outside this sandbox.

### Final review fixes: 2026-09-29

- Replaced message-text classification of output-selection failures with `FailureKind::OutputSelection`, set at the VM's UGen selection failure sites. Instrument lowering passes through not-callable, unknown-field, type and arity codes only when this marker is present; unrelated failures retain the existing wrapping behavior.
- Existing source-form tests continue asserting the public failure codes. The focused `select_output` / `voice_layout` nextest subset passed 15/15 after the change.
- Final gates after these changes all passed with `CARGO_TERM_QUIET=true`: `cargo fmt --check`, `cargo check -q --all-targets`, `cargo clippy -q --all-targets -- -D warnings`, `cargo check -q --target wasm32-unknown-unknown --lib`, and `cargo check -q --features lsp`.
- Expanded focused nextest passed: 42 passed, 1,531 skipped across `voice_layout`, `voice_stereo`, `select_output`, `ugens`, `codec_shapes`, `golden`, and `templates::vact_instrument_routes`. The full nextest suite remains coordinator-owned.

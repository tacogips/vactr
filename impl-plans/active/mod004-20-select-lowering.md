# MOD004-20: `.vact` Output Selection, Checker Typing and Lowering

**Status**: Ready
**Plan ID**: MOD004-20 (wave 2; parallel with MOD004-21 and MOD004-22)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (`.vact` selection and template migration: selection data flow; Capacity)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004A lowering part, MOD-004E selector part)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29

## Intent and Context

Owner decision (2026-09-29): select a multi-output UGen's output by
calling the node value with a keyword, `(p :aux)`, or with an index,
`(p 1)`. There is no grammar change. A bad selection fails when the
`inst` is defined, and an unselected use means output 0. This plan
implements the VM call, the static-checker typing, and the lowering of
the evaluator-only `UGenKind::Output(k)` wrapper to an edge with
`output = k`. It also enforces the shape, voice-layout and slice-capacity
rules at definition time, using the MOD004-10 functions.

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
pattern).

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

### `src/types/infer_call.rs` (callee-type match, around lines 615-634)

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
  - `TooManyBuffers` -> `too_large(span, "the instrument")`, i.e. the
    existing graph-too-large path.
- **Lowering invariant (MUST hold)**: for graphs with no wrapper, the node
  list, node order, edge list, edge order and ports are exactly as before,
  with `output = 0`. Do not reorder pushes, and do not add dedup or any
  other normalization.

## Pitfalls

- `insts.rs` passes the root through `Value::UGen(n)` (`src/ns/insts.rs:716`).
  A wrapper root arrives there as a `UGenNode` of kind `Output`. Handle it
  in `lower_inst`, not in `insts.rs`.
- `Rc::as_ptr` memoization is what makes `(p :main)` and `(p :aux)` one
  node. Memoize the inner node, never the wrapper.
- Existing tests may assert that calling a ugen is `not-callable`
  (for example `src/types/tests/inst/overloads.rs:47`). Keep any test that
  calls a native function correct. Change an assertion only if it really
  concerned calling a ugen *value* with one keyword or int argument, and
  record every changed assertion with a reason in the Progress Log.
- If `let` bindings are not accepted inside an `inst` body, do not change
  the grammar. Record it as a blocker and stop, because the design's
  migration form depends on it.
- `build.rs` is 580 lines. Keep it under 1000; move helpers to
  `build/select.rs` if needed.

## Test Cases

In `src/host/tests/e2e/templates/select_output.rs` (imitate
`voice_engines.rs` for `E2e::new()`, `eval`, and registry `entries()`
lookup):
- `inst sel-a:\n\tlet p (va-source freq > va-filter freq: freq)\n\t(p :main) > * amp > + {(p :aux) > * amp > aux-out}`
  -> defines with no fault. `entry.def.nodes` contains exactly one
  `VaFilter` and one `VaSource`. Edges from the `VaFilter` have `output`
  values {0, 1}.
- The same with `(p 0)`/`(p 1)` -> the same `InstDef` as `sel-a`.
- An unselected `p > * amp` -> every edge has `output == 0`.
- `(p :left)` -> the definition fails with `unknown-field`. `(p 2)` ->
  `unknown-field`. `(p 1.5)` -> `type`. `(p :main :aux)` -> `arity`.
- A single-output ugen value called with `:main` (for example
  `let o (saw freq)` then `(o :main)`) -> `not-callable`.
  `((p :aux) :main)` -> `not-callable`.
- `let s (sample-play bank)` then `(s :stereo) > lpf 800` -> the
  definition fails with the stereo-to-mono message.
  `(s :stereo) + {x > aux-out}` -> the definition fails with the
  stereo aux-out message.
- A root `(p :aux)` -> the definition succeeds. The last node is `Add`,
  fed by an edge with `output == 1`.
- The `(s :stereo)` root through a voice effect (for example `> gain 0`)
  -> the definition succeeds.
- The golden graph digests are unchanged for every prelude template, which
  covers the lowering invariant.

In `src/types/tests/inst/ugens.rs`:
- a valid selection program -> no diagnostics;
- `(p "x")` -> a `type-mismatch` diagnostic;
- two arguments -> a `type-mismatch` diagnostic.

## Verification Commands (logs in `tmp/mod004/MOD004-20/`)

1. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
2. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
3. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --features lsp` -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run select_output ugens golden` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0.
   E2e renders of `(s :stereo)` programs may need MOD004-22's runtime.
   Test only the definition and lowering here, and leave rendering to
   MOD004-30/40.
7. `wc -l src/vm/call.rs src/types/infer_call.rs src/dsp/build.rs` -> each below 1000
8. `rustfmt --edition 2021 --check <changed .rs files>` -> exit 0

## Completion Criteria

- [ ] `(p :aux)` and `(p 1)` lower to one node with an `output = 1` edge, and unselected uses mean output 0.
- [ ] Every failure code in the design occurs at `inst` definition.
- [ ] The checker types `(ugen kw|int)` as `ugen`.
- [ ] Shape, voice-layout and capacity errors are definition-time lowering errors.
- [ ] Golden graph and render digests are unchanged. Checks 1-8 pass, with logs recorded.

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

(empty)

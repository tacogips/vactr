# music-things extraction: common execution protocol

**Applies to**: every MTX plan in
[`music-things-dispatch.json`](music-things-dispatch.json).
**Design**: [`design-music-things-extraction.md`](../../design-docs/specs/design-music-things-extraction.md),
the single source of truth.
**Created**: 2026-10-31. This file is read-only for workers. Each worker
edits only its own plan's Progress Log.

Read this file and design sections 3-6 before starting any MTX plan.

## 1. User intent (all plans)

The user asked on 2026-10-31 for vactr's synth and effect DSP to become
reusable outside vactr. This run moves the pure kernels into three
dependency-free crates under `music-things/crates/`. vactr depends on them
by path and must sound **bit-identical**. The operator later copies
`music-things/crates/*` unchanged into https://github.com/tacogips/music-things.

## 2. Moving a file

Workers cannot write `.git`. Never run `git mv`, `git add`, `git stash`,
`git checkout`, `git reset`, `git commit`, branch commands or any other
command that writes git state. Reading git (`git diff`, `git show`,
`git status`) is allowed.

To move `SRC` to `DST`:

1. Read `SRC` fresh, and record `sha256sum SRC` in the Progress Log.
2. Create `DST` with the identical content.
3. Apply only the allowed edits (design section 5, repeated in 3 below).
4. Delete `SRC`, unless the plan says `SRC` stays as a vactr shim or
   adapter. In that case, rewrite `SRC` to the shim content.
5. Record `sha256sum DST`, and the shim's sha256 if any.

Git's rename detection keeps history when the orchestrator commits.

### 2.1 Compile-preserving module order (mandatory for every move plan)

Move **one module at a time**, together with its own submodule and
`#[path]` files. Examples: `fm.rs` with `fm/*.rs`; `cross_mod.rs` with its
nine `cross_mod_*.rs` files; `creative_reverb.rs` with
`creative_reverb/*.rs`.

For each module, do steps (a) to (f) in this order:

- (a) Copy the source file or files to the destination.
- (b) Apply every allowed edit in the destination (section 3): imports,
  context prologue, visibility, docs, comment scrub and renames. The copy
  must already pass V-grep for its own files before step (c).
- (c) Add `pub mod <m>;` to your own crate's `lib.rs`.
- (d) In vactr, switch `pub mod <m>;` to `pub use music_things_<crate>::<m>;`,
  or write the shim or adapter the plan specifies.
- (e) Delete the vactr source files the plan says to delete.
- (f) Run `cargo check -p <own crate> --all-targets`, then
  `cargo check -p vactr --all-targets`. Log both as
  `tmp/music-things/<ID>/mod-<m>-check-{crate,vactr}.log`.

Do not start the next module until step (f) passes. The one exception is a
failure whose every error location lies in the concurrent sibling plan's
writePaths or sharedPaths (section 5). In that case, record
`BLOCKED-BY-SIBLING` and continue.

Never leave a copied, unrewritten file under `music-things/` across module
steps. At any moment, at most one of your modules is mid-move. This keeps
the sibling plan's broken-build window to a single module.

After the last module's step (f), append
`EDITS-COMPLETE <ISO-8601 timestamp>` to your own Progress Log. Only then
start the gate phase (section 5.1).

## 3. Bit-identity edit allow-list (design section 5)

These are the only edits allowed in moved code:

1. **`use` paths and module declarations.**
   - Inside a crate, use `super::` and `super::super::`.
   - Reach sibling crates by name (`music_things_dsp::prim::Rng`).
   - Never write `crate::` and never write `$crate`.
2. **Context prologue.**
   - A `pub` kernel fn that took `kx: &Kx<'_>` (or `&mut Kx<'_>`)
     becomes `pub fn name<C: KernelContext + ?Sized>(…, kx: &C)`.
   - Its first statement is `let kx = &kx.kernel_ctx();`.
   - Private helpers take `kx: &KernelCtx`.
   - For fx, a `pub` fn that took `ctx: &mut FxCtx<'_>` becomes
     `<C: FxContext + ?Sized>(…, ctx: &mut C)`, with first statement
     `let ctx = &mut ctx.fx_ctx();`.
   - A `pub` fn that took `kind: EffectKind` takes
     `kind: impl Into<EffectKind>`, with first statement
     `let kind: EffectKind = kind.into();`.
   - If a body matches on `kind` before anything else, the prologue
     still comes first.
3. **Visibility.** Widen `pub(super)` or `pub(crate)` to `pub` only for
   items that vactr code, tests or examples reach. Each widened item
   needs a `///` doc comment.
4. **Doc comments.** Add or reword doc comments. Every public item needs
   one, because `#![warn(missing_docs)]` turns into a clippy error under
   `-D warnings`. A kernel entry's doc lists its input port indices and
   meanings. For classic ugens without a `PORTS` table, copy them from
   `src/dsp/ugen/catalog/voice_ports.rs` and the `src/dsp/ugen/catalog.rs`
   port rows.
5. **Renames (design 4.4).**
   - `vactrol_gate` -> `lpg`
   - `VactrolEnvelope` -> `OptoEnvelope`
   - identifiers containing `VACTROL` -> `OPTO`
   - `dynamic_control::vactr` -> `opto`
   - `EffectKind::StreamVactr` -> `StreamOpto` (crate enum only)
   - Comments that say "Vactr"/"vactr" are reworded to "this
     implementation" or "original", keeping the same provenance claim.
6. **FxCtx field changes.** `ctx.store.get(id)` -> `ctx.samples.sample(id)`
   (only at `reverb.rs` convolution). `caps` -> `limits` (only in
   `granular`).
7. **Resource-resolution split.** Only for `sample`, `wavetable` and the
   granular ugen.
8. **`lpg` registry argument.**
9. **In moved tests.** Change only imports and context construction:
   - `Kx {..}` -> `KernelCtx { sr, gate, seed }`
   - `FxCtx {..}` -> crate `FxCtx` with `samples: &NoSamples`,
     `limits: GrainLimits {..}` and crate `FxStats`.

Forbidden:

- reordering arithmetic, operands or loops;
- changing literal suffixes or `f32`/`f64` promotion;
- `mul_add`;
- swapping helpers;
- changing defaults, ranges, port order or names, `*_STATE_FLOATS`,
  `PORTS`, `mem_len` or param tables;
- editing any assertion, expected value, tolerance, golden file or
  fixture;
- adding `#[inline]` or removing existing attributes;
- "cleaning up" code you move.

## 4. Fn-pointer shims (design 4.1)

A vactr shim is a vactr file that stays at its old path with content of
this shape:

- a module doc line;
- `pub use music_things_synth::<m>::*;`
- one forwarding fn per listed entry point, with the **exact original
  signature** `kx: &Kx<'_>` (import `Kx` from `super`) and a one-call body.

The local fn shadows the glob re-export. The shim holds no DSP loop and no
arithmetic. Shims exist only where design 4.1 and 4.3 list them.

## 5. Edits and drift

Before each edit, re-read the file and record its sha256 and a one-line
intent in the Progress Log. If the file changed since your last read and
the change is not yours, stop editing it and record `DRIFT <path>`. Never
edit outside your plan's `writePaths` and `sharedPaths`.

### 5.1 Edit phase, gate phase and sibling waits

In waves W1-W3, one synth plan and one fx plan run at the same time on the
same tree. Their paths are disjoint, but their **builds are coupled**:

- `music-things-fx` depends on `music-things-synth`;
- `vactr` depends on both crates;
- all vactr tests share one test crate.

So while the synth plan is mid-move, every fx and every vactr cargo command
fails. While the fx plan is mid-move, every vactr command fails.

Each plan therefore runs in two phases:

1. **Edit phase.** Do all module moves in the section 2.1 order. When done,
   append the line `EDITS-COMPLETE <ISO-8601 timestamp>` to your own
   Progress Log. Do not append it earlier.
2. **Gate phase.** Run the plan's Verification commands.

Handling a sibling-located failure, in either phase:

- **When it applies.** A command (cargo check, clippy, nextest, V-names,
  V-wasm, V-crate-wasm, V-examples, V-vitest or V-grep) fails, and every
  error location or grep match lies in the concurrent sibling's writePaths
  or sharedPaths. The sibling is listed in the dispatch `waves`.
- **What to do.** Do not touch that file. Record
  `BLOCKED-BY-SIBLING <file:line> <command>` and retry the same command
  about every 60 s.
- **Keep waiting** while either:
  - (a) the sibling's Progress Log has no `EDITS-COMPLETE` line, or
  - (b) any sibling-owned file's sha256 changed within the last 15
    minutes.

  Compare `sha256sum` of the sibling's writePaths between retries; reading
  the files is allowed.
- **Record a blocker** only when either:
  - (1) the sibling shows `EDITS-COMPLETE`, its files have not changed for
    at least 15 minutes, and the error persists, or
  - (2) the overall wait for this plan reaches **120 minutes** in total.

  The cap is deliberately below the node timeout.
- **Records.** A sibling-located failure is never recorded as a pass.
  Final verification records still require exitStatus 0 on the combined
  tree, so rerun each blocked command after the wait clears.

Other cases:

- A failure with any error location in your own paths is yours to fix. It
  is not BLOCKED-BY-SIBLING.
- If `cargo` reports a lock wait, wait and retry; the same 120-minute cap
  applies.

## 6. Evidence

Each plan writes complete logs to `tmp/music-things/<PLAN-ID>/`.
This directory is gitignored (`**/tmp`) and declared as an artifactRoot.
Use one file per command, named `NN-<name>.log`, written with
`… > LOG 2>&1; echo "exit=$?" >> LOG`. A missing log, a truncated log or one
without the `exit=` line is not a pass.

The Progress Log records each verification as
`{command, exitStatus, testsRun, testsPassed, failureCount, outcome, log, notes}`.
A passing record has `exitStatus: 0`, `failureCount: 0` and
`outcome: "passed"`. Intentional negative controls go only under a
`mutationEvidence` heading, never in the verification records.

The standard environment for cargo commands is:
`CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1`.

## 7. Standard verification commands

Each plan's Verification section lists which of these it runs, with its own
file lists filled in.

| Id | Command | Must show |
|----|---------|-----------|
| V-check | `cargo check --workspace --all-targets` | exit 0 |
| V-fmt | `rustfmt --edition 2021 --check <every .rs file this plan created or modified>` | exit 0. Format only your own files; never run `cargo fmt` on the workspace |
| V-clippy | `cargo clippy -p <crate> --all-targets -- -D warnings` for each crate the plan touched, and `cargo clippy -p vactr --all-targets -- -D warnings` | exit 0 |
| V-nextest | `cargo nextest run --workspace` | exit 0, failureCount 0 |
| V-names | `cargo nextest list --workspace --message-format oneline > tmp/music-things/<ID>/tests-after.txt`, then `python3 -I tmp/music-things/MTX-00/check_test_names.py tmp/music-things/MTX-00/baseline-tests.txt tmp/music-things/<ID>/tests-after.txt` | exit 0, prints `unmatched=0` and a total of at least the baseline count plus 2 (fx_kind_map) plus the new crate tests added so far. A failure caused only by a sibling compile error is BLOCKED-BY-SIBLING (section 5.1) |
| V-wasm | `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| V-crate-wasm | `cargo build --manifest-path music-things/crates/<crate>/Cargo.toml --target wasm32-unknown-unknown` and `cargo build -p <crate> --target wasm32-unknown-unknown` for each touched crate | exit 0 |
| V-examples | `cargo build --examples` | exit 0. All 15 examples compile unedited |
| V-grep | `! grep -rniE 'crate::|vactr|CtlId|InstDef|wire::Ctl' music-things/` | exit 0 (no match). Matches located only in the concurrent sibling's paths are BLOCKED-BY-SIBLING (section 5.1), not a fail. The final record needs exit 0 |
| V-protected | `git diff --exit-code a6e5bea -- src/dsp/tests tests examples editor src/dsp/ported/tests.rs ':(exclude)src/dsp/tests/dsp.rs' ':(exclude)src/dsp/tests/dsp/fx_kind_map.rs' ':(exclude)src/dsp/tests/dsp/quad_mixer_alloc.rs'`, then `git diff a6e5bea -- src/dsp/tests/dsp.rs` | First: exit 0. Second: the only changed lines are `+mod fx_kind_map;` and (from MTX-F3 on) `+mod quad_mixer_alloc;`. `git status --porcelain -- src/dsp/tests` lists no other new file |
| V-gone | `for f in <moved sources>; do test ! -e "$f" || echo "LEFT $f"; done` | no `LEFT` lines (shim and adapter paths excluded) |
| V-lines | `wc -l <every .rs file this plan created or modified>` | every count below 1000 |
| V-vitest | `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm && (cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run --testTimeout=60000 --exclude test/e2e/large-eval.test.ts -t '^(?!.*does no whole-document work on an edit frame or active animation-only frames).*$')` | exit 0, at least 2384 passed. May run outside the Codex sandbox; record who ran it and the log path |

V-nextest and V-vitest are the behavioral bit-identity proof. They must pass
on the combined tree before the plan is accepted.

Operator amendment (2026-10-31, after session 581 wave 1):
- `editor/test/canvas/mount.test.ts` > "does no whole-document work on an edit
  frame or active animation-only frames" fails identically on the unmodified
  checkpoint sources (baseline failure on origin/main a6e5bea, unrelated to
  DSP; no plan touches editor/). V-vitest skips exactly that test with the
  `-t` negative lookahead above; every other mount test still runs.
- `editor/test/e2e/large-eval.test.ts` is a load-sensitive language-eval
  scaling test with a hard-coded 120 s timeout; on this shared host (load
  average 40-50) it times out regardless of the change. It is excluded from
  V-vitest. MTX-91 runs it once as informational evidence in `notes` (not in
  `verification`).
- `--testTimeout=60000` replaces vitest's 5 s default, which load-induced
  timeouts (for example the canvas gpu atlas test) exceeded.
- V-nextest takes 25-40 minutes on this host; the slowest test is
  `complete::tests::robust::every_prefix_and_mutant_is_panic_free`. Never
  interrupt it: run the full suite in the background (for example
  `nohup ... > log 2>&1 &` then poll until the process exits) and record its
  terminal exit status. An interrupted run is not evidence. Run them in the gate
phase. If they fail only because of a sibling compile error, apply section
5.1.

## 8. Progress Log format

Append, per session:

```
### Session YYYY-MM-DD HH:MM (<PLAN-ID>)
- Edits: <path> pre=<sha256> post=<sha256> intent=<one line>
- Verification: {command, exitStatus, testsRun, testsPassed, failureCount, outcome, log, notes}
- Blockers / DRIFT / BLOCKED-BY-SIBLING: ...
- mutationEvidence: (only for intentional negative controls)
```

Tick a plan's completion checkboxes only when their evidence is logged.

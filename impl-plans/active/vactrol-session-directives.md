# Vactrol Session Layer: Directives (SS-DIRECTIVES) Implementation Plan

**planId**: SS-DIRECTIVES (issue #4, wave 2; `#@` parsing, attachment, labels, selector resolution, `BindingKey`, both `BindingPersistence` impls, learned-CC write-back, vocabulary fixtures)
**Status**: Completed (accepted by the session-185 integration review; SS-FINAL re-verified the joined tree in session 186)
**Design Reference**: design-docs/specs/design-implementation.md 13.5 (Decided shape, PROPOSED vocabulary, ADDRESSED SELECTOR RESOLUTION, `BindingKey`, labels, write-back), 13 (persistence mode scope, overlays never in source), 14.5.8 (all rules), 14.5.6 (`ChangeSet`), 14.5.12; architecture.md Editor Requirements; design-docs/user-qa/pending-session-questions.md S4
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: SS-CONTRACTS
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

The `#@` directive machinery is a pure, wasm-safe library over reader output. SESSION calls it on every eval, and the
LSP calls it for lint. What already exists:
- The reader keeps trivia: `ReadResult.trivia: Trivia { items: Vec<TriviaItem { span, kind: Comment | Directive }> }`.
  The lexer marks `#@` comments as `TriviaKind::Directive`.
- `HostManifest` exposes `editor_decl`/`editor_decls` (declared parameter order of builtins) and `template_params`.
- SS-CONTRACTS supplies `session::changes::ChangeSet` and the directive `DiagCode`s (all warnings).

The language never reads a directive (13.5): nothing here may change evaluation.

## Non-Goals

- No protocol messages, no `edit_epoch` checks (SESSION's authority layer), no file IO (the caller reads and writes
  `<doc>.bindings.json`).
- No change to the reader, checker or evaluator.
- No new vocabulary beyond the 13.5 PROPOSED grammar. `range:` is reserved and rejected.
- The spec manifest `tests/fixtures/spec/manifest.toml` is NOT touched (ANALYSIS writes it in this wave).

## writePaths (exclusive in wave 2)

- `src/directives/mod.rs` (fills the CONTRACTS stub), `src/directives/parse.rs`, `src/directives/attach.rs`,
  `src/directives/labels.rs`, `src/directives/resolve.rs`, `src/directives/key.rs`, `src/directives/persist.rs`,
  `src/directives/writeback.rs`
- `src/directives/tests/mod.rs`, `src/directives/tests/parse.rs`, `src/directives/tests/attach.rs`,
  `src/directives/tests/labels.rs`, `src/directives/tests/resolve.rs`, `src/directives/tests/key.rs`,
  `src/directives/tests/persist.rs`, `src/directives/tests/writeback.rs`
- `tests/directive_fixtures.rs`, `tests/fixtures/directives/vocabulary.toml`
- `impl-plans/active/vactrol-session-directives.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **`mod.rs`.**
   - Entry point: `build_table(src: &str, file: FileId, nodes: &[Node], trivia: &Trivia, manifest: &HostManifest) ->
     (DirectiveTable, Vec<Diagnostic>)`.
   - `DirectiveTable { file_level: FileDefaults { midi_ch: Option<u8> }, entries: Vec<Directive>, labels:
     LabelRegistry, resolved: Vec<ResolvedBinding> }`.
   - `ResolvedBinding { key: Option<BindingKey>, target_span: Span /*call site or definition header*/, param:
     KwId, param_index: u16, cc: Option<u8>, ch: Option<u8>, directive_span: Span }`.
   - A table summary serializable for the protocol (`serde::Serialize`).
2. **`parse.rs` (13.5 PROPOSED grammar).**
   - `Directive { span, kind: Positional | Addressed, body }`.
   - `body` is one of:
     - `FileDefault { pairs }`;
     - `LabelDef { name, rest: Option<positional> }`, for `#@ name X` and the leading `ident:` short form;
     - `Positional { sites: Vec<SiteSel { name, ordinal: Option<u16> }>, pairs }`;
     - `Addressed { label, sel, ordinal, pairs }`.
   - `Pair::Cc(Vec<Option<u8>>)` (`_` is `None`) and `Pair::Ch(u8)`.
   - Classification by first token: `midi` and `label.sel[.n]` are Addressed; everything else is Positional.
   - `range:` (and any unknown key) gives `reserved-key`. A CC outside 0..127 or a channel outside 1..16 gives
     `cc-out-of-range`.
   - One directive per line.
3. **`attach.rs` (the Decided attachment rule).**
   - A trailing `#@` on a code line attaches to that line's statement.
   - Consecutive own-line `#@` lines form one block. The block attaches to the nearest PRECEDING statement, block or
     definition whose indentation (column of its first token) is no deeper than the block's. At column 0 after an
     `inst` body, that is the whole `inst`; inside a body, it is the previous body line.
   - A positional block with no preceding target gives `unknown-directive-site`.
   - Addressed directives are position-free.
   - A second `#@ midi ch:` replaces the first and gives `duplicate-key`.
4. **`labels.rs`.**
   - `LabelRegistry`: explicit labels (trailing `label:` short form, `#@ name X`, block-following for bodies) plus
     implicit labels (every top-level `let`/`fn`/`inst`/`bus`/`look` name and every named slot) in ONE per-document
     namespace, with exact match.
   - Any collision (explicit vs explicit, explicit vs implicit) gives `duplicate-label`, and the name is marked
     ambiguous.
   - `resolve(name) -> LabelLookup { Target(LabelTarget { span, kind: Line | Definition{params} }), Ambiguous, Unknown }`.
5. **`resolve.rs` (ADDRESSED SELECTOR RESOLUTION, exactly 13.5).**
   - Resolution order:
     1. a definition target with declared parameters: `sel` matching a PARAMETER keyword selects that parameter;
     2. otherwise, `sel` matching a CALL-SITE name inside the labeled line or block selects that call site, and `cc:`
        maps positionally over ITS declared parameter order (builtin order from `HostManifest`; `inst`/`fn`/`bus` order
        from the header node);
     3. a `sel` matching BOTH a parameter and a call-site name gives `ambiguous-selector`;
     4. a repeated same-named call site used bare gives `ambiguous-selector`; the ordinal `.n` selects the n-th in
        source order.
   - Positional first tokens resolve against the attach target the same way (`#@ lpf.2`).
   - `cc:` numbers map in declared parameter order, `_` skips, and fewer numbers leave the rest panel-only. `ch:`
     overrides the file default.
   - Diagnostics: `unknown-directive-site`, `unknown-parameter`, `unknown-label`, `ambiguous-selector`. A reference to
     an ambiguous label gives `duplicate-label` and falls back to positional provenance (no `BindingKey`).
6. **`key.rs`.**
   - `BindingKey { label: Rc<str>, site: Option<(Rc<str>, u16)>, param: Rc<str> }`, with `Display` as
     `label.site.n.param` or `label.param`, plus `FromStr`.
   - `KeyTable` maps each key to its last resolved span and revision.
   - `migrate(&mut self, changes: &ChangeSet, new_table: &DirectiveTable) -> Vec<(BindingKey, KeyState { Live, Stale })>`:
     - each span is mapped through the change set and the same-named call sites of the new revision are counted again;
     - a clean mapping onto a same-named site MIGRATES the key, rewriting the ordinal;
     - a `Touched` mapping, or one that does not land on a same-named site, marks the key `Stale` and keeps its data.
     It never guesses.
7. **`persist.rs`.**
   - `BindingEntry { key: BindingIdent { Key(BindingKey), Positional { span, param } }, panel: bool, midi: Option<{cc,
     ch}>, overlay: Option<f64> }` and `BindingSet`.
   - `trait BindingPersistence { fn load(&self, doc: &DocInput) -> BindingSet; fn save(&mut self, doc: &DocInput, set:
     &BindingSet) -> Persisted }`, where `Persisted { Edits(Vec<TextEdit { span, expected, text }>), File(String) }`.
   - `DirectivePersistence`: `load` reads the table; `save` renders directive text edits for panel membership and
     mappings and NEVER writes overlay values (13, S4).
   - `ExternalFilePersistence`: JSON `{"v":1,"bindings":[...]}` through serde_json, keys spelled with `Display`; it
     keeps overlays.
8. **`writeback.rs`.**
   - `learn_edit(doc_text, table, target: &BindingIdent, cc: u8, ch: Option<u8>) -> Result<TextEdit, LearnError {
     Stale, NotDirectiveBacked, Unknown }>`.
   - It makes the minimal edit that inserts or replaces the `cc:` number at the binding's parameter position inside
     its directive, or appends a new directive block after the target statement if none exists.
   - `expected` holds the current directive text, so the editor can verify it before applying.

## Required Tests (`src/directives/tests/*.rs`, `tests/directive_fixtures.rs`)

- **attach.rs.** The spec's own examples parse with correct attachment:
  - same-line trailing;
  - a consecutive-line block to the nearest preceding statement or block at no deeper indentation;
  - a whole-`inst` binding at column 0;
  - Addressed `hats.hpf` and a file-level `#@ midi ch: 1`.

  The examples are architecture.md Editor Requirements and design 13.5; copy their text.
- **labels.rs.** Explicit and implicit labels resolve. `duplicate-label` fires for explicit vs explicit AND explicit vs
  implicit, its addressed references are rejected, and the sites fall back to positional provenance.
- **resolve.rs.**
  - `analog.cutoff` selects the inst PARAMETER.
  - `hats.hpf cc: 30` selects the hpf CALL SITE on the hats line, with 30 on its FIRST declared parameter (asserted
    exactly).
  - A parameter/call-site clash is `ambiguous-selector`.
  - A repeated `lpf` bare is `ambiguous-selector`, while `hats.lpf.2` and positional `#@ lpf.2` select the second.
  - `cc: 74 _ 30` skips the second declared parameter.
  - Fewer numbers leave the rest panel-only.
  - A directive `ch:` overrides the file `#@ midi ch:`.
  - `range:` gives `reserved-key`.
  - Unknown site, param and label give their codes.
  - Directives never change evaluation: an `Evaluator` result for a program with and without its directives is
    identical.
- **key.rs.**
  - Independent keys on `hats.lpf` and `hats.hpf` (same `cutoff` keyword) are distinct, and so are `hats.lpf.1` and
    `hats.lpf.2`.
  - Moving the labeled line keeps every key.
  - Reordering the two `lpf` sites MIGRATES each binding with its site.
  - An edit that breaks the mapping marks it `Stale`.
- **persist.rs.**
  - The same `BindingSet` (the lpf/hpf and lpf.1/lpf.2 keys, panel and MIDI) round-trips through BOTH impls without
    cross-talk.
  - Overlays round-trip in ExternalFile mode and are absent from Directive-mode edits (S4).
- **writeback.rs.**
  - A MIDI-learn writes the CC into the directive text at the right position.
  - A missing directive gets a new block.
  - A `Stale` binding is refused.
- **`tests/directive_fixtures.rs` + `tests/fixtures/directives/vocabulary.toml`.** One `[[case]]` per vocabulary
  construct (file default, `#@ name`, trailing short form, positional `cc:` with `_`, fewer numbers, `ch:` override,
  addressed param, addressed call site, ordinal, reserved key, duplicate label). Each case has `id`, `source`,
  expected diagnostic codes and expected resolved bindings, plus `class = "authority-question"` and a `question`. The
  runner uses `tests/support`'s TOML subset parser through `mod support;` WITHOUT editing `tests/support/`, and
  asserts that every case carries the channel.

## Invariants

- Wasm-safe, with no IO.
- Directive processing never changes reader, checker or evaluator output.
- Every diagnostic here is warning severity.
- No `.rs` file reaches 800 lines.

## Edit Protocol

The common protocol in `vactrol-session-contracts.md`. Evidence goes under
`tmp/ss-session-20260925-s183/SS-DIRECTIVES/attempt-<n>/`. Sibling-caused crate-wide failures are recorded, never
fixed.

## Verification (`<wave>` = `directives`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| D1 | LOG(`ss-directives-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/directives::tests/) \| binary(directive_fixtures)'` | `exit=0`, run > 0 |

## Completion Criteria

- [x] Items 1-8 implemented
- [x] Every required test passes, including the authority-question channel on every vocabulary case
- [x] V1-V9 and D1 pass with logs cited; `final-hashes.txt` written. In session 184, V2, V2l, V3 and V3t failed only in
  sibling SS-ANALYSIS files. In session 185 every row passes on unchanged directives source (see the session-185
  entry; logs `target/fe-logs/ss-directives-<check>-s185-3.log`, `attempt-2/final-hashes.txt`).

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-DIRECTIVES implementer)` entry. Edit only this log.)

### Session: 2026-09-25 (session 184, SS-DIRECTIVES implementer)

**Tasks completed**: items 1-8 and every Required Test. Evidence is in
`tmp/ss-session-20260925-s183/SS-DIRECTIVES/attempt-1/`: `intent.md`, `notes.md`, `pre-edit-hashes.txt`,
`post-edit-hashes.txt` and `final-hashes.txt`.

**Files** (all new except `mod.rs`, which fills the CONTRACTS stub):
- `src/directives/{mod,parse,attach,labels,resolve,key,persist,writeback}.rs`
- `src/directives/tests/{mod,parse,attach,labels,resolve,key,persist,writeback}.rs`
- `tests/directive_fixtures.rs`
- `tests/fixtures/directives/vocabulary.toml`

The largest file, `persist.rs`, has 587 lines, so no file reaches 800.

**Deliverables and plan-level refinements**:
- `build_table` returns `DirectiveTable { file_level, entries: Vec<Placed>, labels, resolved, hits, doc }`.
  - `ResolvedBinding` carries the planned fields plus `ident`, `param_name`, `directive` and `cc_slot`. Write-back
    and persistence need these extra fields.
  - `DirectiveTable::summary()` is `serde::Serialize`.
- Attachment. A trailing `#@` binds to the innermost statement covering its line. Consecutive own-line `#@` lines form
  one block only at the same indentation, so a line at another indentation starts a new block. This is how an
  indented `#@` under an `inst` body line and a following column-0 `#@` bind to the body line and to the whole
  `inst` respectively. Each block binds no deeper than its own comment's indentation.
- Keys. A binding under a labeled statement is keyed by that statement's first unambiguous label, with explicit
  labels ahead of implicit ones. A body statement with no label of its own is keyed by its top-level form's label,
  and the site ordinal is counted within that label's target.
- The call-site identity span is the head symbol (`lpf`), so that editing the literal arguments does not stale the
  key.
- `KeyTable::migrate` keeps the planned signature. `migrate_moves` returns `from -> to` detail. A Live migration
  takes precedence over a stale key with the same spelling; the stale entry is kept in `superseded()` and never
  dropped.
- `learn_edit(doc_text, table, keys: &KeyTable, target, cc, ch)`. The added `keys` argument is how a `Stale` key is
  detected, which the planned signature cannot express. The edit spans the governing directive, and `expected` is
  that directive's current text.
- If a directive's channel is shared and a learn brings a different channel, the edit adds an override line. Later
  mappings win in the merge.
- `DocInput { text, table }`. `ExternalFilePersistence { file: Option<String> }` holds the caller-read session file,
  and the module does no IO.
- `DirectivePersistence::save` writes addressed overrides at the end of the file for keyed groups. For positional
  groups it inserts after the statement's directive block. A directive that covers only the changed group is
  rewritten in place (pairs only, head kept), and it is deleted when the group leaves the panel. A label definition
  keeps its label.
- `apply_edits` verifies `expected` before applying.

**Verification** (logs `target/fe-logs/ss-directives-<check>-s184-1.log`; all run on the shared tree while SS-PKG
and SS-ANALYSIS were mid-edit):

| Row | Check | Result |
|-----|-------|--------|
| V1 | `build` | exit=0 |
| V1l | `build-lsp` | exit=0 |
| V2 | `clippy` | exit=101, sibling-caused (below) |
| V2l | `clippy-lsp` | exit=101, sibling-caused (below) |
| V3 | `nextest` | exit=100, sibling-caused (below) |
| V3t | `cargotest` | exit=101, sibling-caused (below) |
| V3f | `fixtures` | exit=0; 10 run, 10 passed, 1 skipped (pre-existing on-demand report) |
| D1 | `own` | exit=0; 39 run, 39 passed |
| V6a | `wasm32` | exit=0 |
| V6b | `wasm32-hostwasm` | exit=0 |
| V7 | `fmt` | exit=0 |
| V4 | `linecount` | largest file 799 (`src/dsp/build.rs`, not this plan) |
| V5 | `io-grep` | prints `none` |
| V9 | `tree-wasm32`, `tree-wasm32-hostwasm` | 0 matches for the gated crates |

The sibling-caused failures:
- **V2/V2l**: the only error is `clippy::match_like_matches_macro` at `src/types/infer_call.rs:358`, an SS-ANALYSIS
  write path in flight. Diagnostic runs with that one lint allowed (`clippy-own`, `clippy-lsp-own`) exit 0, so the
  directives code is clippy-clean.
- **V3/V3t**: the full failure set, from `nextest-nff` with `--no-fail-fast`, is 901 run, 899 passed, 2 failed.
  `cargotest` reports lib 887 passed and 2 failed.
  - `dsp::tests::contracts::native_accepts_its_advertised_limits`: native `OfflineRender` is now accepted.
  - `types::tests::natives::only_scale_and_shape_are_overloaded`: the new `scope`/`spectrum`/`render` overloads.

  Both are self-analysis surfaces of SS-ANALYSIS (14.5.9), outside this plan, so they are recorded here and not
  fixed. The SESSION join re-verifies V2, V2l, V3 and V3t on the combined tree.

**Notes**:
- A crate-wide format pass at 23:45:14, not run by this plan, reformatted the new files. It was formatting only
  (see `notes.md`). This plan ran `rustfmt --edition 2021` only on its own files.
- The Rust was written by the single Step-6 owner. The Riela contract makes one owner responsible for edits, so this
  deviates from the rust-coding-agent convention; the deviation is recorded in `notes.md`.
- Residual risks:
  - The vocabulary is PROPOSED. Every fixture case carries `class = "authority-question"` and a question.
  - With several sites and a `cc:` list, the numbers map over the concatenated declared parameters in written
    order.
  - Removing one site from a shared multi-site directive in Directive mode leaves that directive in place: only
    exclusive directives are rewritten or deleted.
  - An entry with no channel under a file default reloads with the file default's channel.

### Session: 2026-09-26 (session 185, SS-DIRECTIVES implementer)

**Tasks completed**: re-verification after the operator updated the two pre-amendment tests
(`src/types/tests/natives.rs`, `src/dsp/tests/contracts.rs`) to the 12.3 amendment. Evidence is in
`tmp/ss-session-20260925-s183/SS-DIRECTIVES/attempt-2/`: `intent.md`, `pre-edit-hashes.txt`,
`post-verify-hashes.txt` and `final-hashes.txt`.

**Source**: no directives source or test file changed. Every hash in `attempt-2/pre-edit-hashes.txt` equals
`attempt-1/final-hashes.txt`, and the hashes did not change during the run. The only edit in this session is to this
plan (Status, the verification criterion and this entry).

**Verification** (logs `target/fe-logs/ss-directives-<check>-s185-3.log`, except V1, V1l, V2, V2l and V3t, which
are in `-s185-2.log`; all run on the shared tree):

| Row | Check | Result |
|-----|-------|--------|
| V1 | `build` | exit=0 |
| V1l | `build-lsp` | exit=0 |
| V2 | `clippy` | exit=0 |
| V2l | `clippy-lsp` | exit=0 |
| V3 | `nextest` | exit=0; 901 run, 901 passed, 1 skipped |
| V3t | `cargotest` | exit=0; lib 889 passed, `directive_fixtures` 2 passed, `spec_fixtures` 10 passed and 1 ignored, 0 failed |
| V3f | `fixtures` | exit=0; 10 run, 10 passed, 1 skipped |
| D1 | `own` | exit=0; 39 run, 39 passed |
| V6a | `wasm32` | exit=0 |
| V6b | `wasm32-hostwasm` | exit=0 |
| V7 | `fmt` | exit=0 |
| V9 | `tree-wasm32`, `tree-wasm32-hostwasm` | exit=0; 0 matches for tungstenite, getrandom, tower-lsp and tokio |
| V4 | `linecount` | largest file 799 (`src/dsp/build.rs`, not this plan) |
| V5 | `io-grep` | prints `none` |

**Runner note**: the `nextest`, `fixtures` and `own` logs with the `-s185-2` suffix exit 2. My log runner did not
word-split the nextest environment variables under zsh, so nextest rejected the `--status-level` value before any
test ran. These are not test failures, and the `-s185-3` reruns supersede them.

**Status**: every completion criterion is checked. Formal review, the SESSION join and the commit belong to later
workflow steps.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-contracts.md. **Parallel**: vactrol-session-pkg.md, vactrol-session-analysis.md
- **Next**: vactrol-session-core.md, vactrol-session-lsp.md


### STEP6 OUTPUT NOTE (operator, 2026-09-25, after the SS-ANALYSIS attempt-1 failure)

- The step6-implement output contract requires `changedFiles` to be an ARRAY of
  path strings (SS-ANALYSIS attempt 1 failed with "$.changedFiles must be of type
  array"). Carry `planId`; leave `verificationGaps` empty when every automated
  command passed (manual checks go under `residualRisks`). Crate-wide test
  failures caused only by a sibling branch's in-progress files or by a
  pre-existing test outside every plan's ownership are reported in the
  progress log as a dependency blocker for the operator, never fixed by
  editing unowned files.

### INTEGRATION REVIEW OUTPUT NOTE (operator, 2026-09-26, after two adapter rejections in session 185)

- The integration-review step output MUST be an ENVELOPE with two top-level
  keys: `"when"` (the routing flags `needs_revision`, `redispatch_required`,
  `repair_in_place`, `plans_remaining`) and `"payload"` (an OBJECT holding the
  review itself: `needs_revision`, `loopGate`, `acceptedPlanIds`, `findings`,
  `recoveryDiagnostic`, summaries, evidence paths). Two attempts were rejected
  with "payload must be an object when when is provided" because the review
  fields were emitted at the top level next to `when` instead of inside
  `payload`. `acceptedPlanIds` lists only plans present in the manifest's
  `plans[]`.

### CLOSING NOTE (SS-FINAL, session 186, 2026-09-26)

- Status set to Completed by SS-FINAL. Join integrity: tmp/ss-session-20260925-s183/SS-FINAL/attempt-1/join-integrity.txt.
- Final-tree evidence: target/fe-logs/ss-final-<check>-s186-1.log (build, build-lsp, clippy, clippy-lsp, fmt, nextest 984/984, cargotest 984, fixtures 10/10, lsp-smoke 1/1, cli 9/9, session 132/132, example, wasm32, wasm32-hostwasm; all exit=0).

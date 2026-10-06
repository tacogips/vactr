# Canvas Cutover OPT-HISTORY: Delta-Charged History, Pinned Revisions and Line-Table Wire Mapping Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-OPT-HISTORY (session 286, wave 9; runs alone after CANVAS-OPT-HARNESS is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 2 (history, pinned revisions and wire mapping) and section 6 (budgets); 15.3.5 (history ceilings, as amended); 15.3.8.10 (bounded edit path, line-based sync)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-HISTORY`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

The user wants playing highlights to stay synchronized while editing. Today they stop after
about 9 edits (`tmp/canvas-cutover/diag-shape/REPORT.md` section 2):

- `RevisionHistory.retainedBytes` (`editor/src/code/history.ts:98-100`) charges every kept
  revision as a full `Text` copy: `128 + length*2 + lines*64`, about 3.3 MB at 1 MiB and
  20,000 lines.
- `CodeSurface.boundHistory` (`editor/src/code/surface.ts:76-87`) then trims the history to
  fit 32 MiB, and the evaluated revision falls out.
- After that, `mapWireSpan(span, evalRev)` returns `null`. Playing highlights die, and about
  7,400 bind sites flip to `stale`. That flip is what triggers the WebKit 8 fps paint
  problem.

Related costs:

- `RevisionHistory.index` builds a whole-text `Utf8Index` (6 MiB for 1 MiB) per revision.
- `bind/mount.ts:296-302` and `bind/write.ts:336` build `new Utf8Index(doc.toString())`, the
  second one per slider write.
- `pointer.ts:84,95,111` call `doc.toString()` on long-press and double-click, and on every
  drag `pointermove`.
- `keyboard.ts` `wordMoveAt` (lines 32-52) scans lines without bound.

## Non-goals

- No change to `HISTORY_UNDO_BYTES` (32 MiB), `HISTORY_LIMIT` (256), `INDEX_BYTES` (8 MiB), the
  undo semantics or the `@codemirror/commands` history adapter.
- No Session Protocol change. `doc-changed` bytes, dirty spans, revisions and epochs stay
  byte-identical (15.3.8.10).
- No `CodeSurface` or `DocumentSync` signature change except the additive `pin`/`unpin`.
- `CodeApi.mapWireSpan` and `currentRevision` keep their signatures. `CodeApi` gains only the
  additive `toWireSpan`.
- No rendering, layout, syntax, bind-panel or completion work. Those belong to later OPT
  plans.
- No Rust edit and no dependency.

## Ownership

writePaths:

- `editor/src/code/line-bytes.ts` (new)
- `editor/src/code/history.ts`
- `editor/src/code/sync.ts`
- `editor/src/code/surface.ts`
- `editor/src/code/highlight.ts`
- `editor/src/code/diagnostics.ts`
- `editor/src/code/eval.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/pointer.ts`
- `editor/src/code/keyboard.ts`
- `editor/src/app/apis.ts`
- `editor/src/protocol/utf8.ts`
- `editor/src/bind/mount.ts`
- `editor/src/bind/write.ts`
- tests:
  - `editor/test/code/history.test.ts`
  - `editor/test/code/sync.test.ts`
  - `editor/test/code/highlight.test.ts`
  - `editor/test/code/diagnostics.test.ts`
  - `editor/test/canvas/state.test.ts`
  - `editor/test/canvas/edit-cost.test.ts`
  - `editor/test/canvas/input.test.ts`
  - `editor/test/canvas/mount.test.ts`
  - `editor/test/canvas/contracts.test.ts`
  - `editor/test/protocol/utf8.test.ts`
  - `editor/test/bind/write.test.ts`
  - `editor/test/bind/fixtures.ts`
  - `editor/test/app/song.test.ts`
  - `editor/test/params/sampler.test.ts`
- `impl-plans/active/canvas-cutover-opt-history.md` (checkboxes and progress log only)
- `tmp/canvas-cutover/opt-history` (artifact root)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`

sharedPaths:

- `editor/src/params/mount.ts`: no edit expected. Its `evalRev` mapping is covered by the
  `eval` pin. Edit it only if a type error from the `CodeApi` change requires it.
- `editor/src/params/roll.ts`: same rule as `params/mount.ts`.

The test fakes `fixtures.ts`, `song.test.ts`, `contracts.test.ts` and `sampler.test.ts` build
`CodeApi` objects. Adding the required `toWireSpan` member needs a one-line fake in each
(for example `toWireSpan: () => ({ start: 0, end: 0 })`, or the real `sync.toWireSpan`). No
assertion changes.

## Contracts and Key Points

### 1. `editor/src/code/line-bytes.ts` (new)

- Move `utf8LengthRange` and `class LineBytes` from `sync.ts:20-98` unchanged, and export
  both. `sync.ts` imports them, so its behavior is unchanged.
- Add `LineBytes.starts(): Uint32Array`. It returns a copy of the prefix: one entry per line
  for the byte start of that line, plus a last entry equal to the total byte length. This is
  numeric copying only, with no text read.
- Add `export class LineTable`:
  - `constructor(text: Text, starts: Uint32Array)`;
  - `static build(text: Text): LineTable` walks the lines once with `text.iterLines()` and
    sums `utf8Length(lineString) + 1` per line. That is linear: do not call `text.line(n)`
    per line, which is O(n log n) and too slow for the 1,048,576-line test fixture. It never
    calls `toString` and increments `LineTable.builds`.
  - `byteLength: number`;
  - `toByte(pos: number): number`;
  - `toUtf16(byte: number): number`;
  - `spanToUtf16(span: Span): Range16`;
  - `spanToBytes(from: number, to: number): Span`;
  - `bytes: number`, which is `128 + starts.byteLength`;
  - `static builds: number`, a test counter.

  Semantics must be byte-identical to `Utf8Index` on `text.toString()` (`protocol/utf8.ts`
  `toByte`/`toUtf16`): clamping; a byte inside a multi-byte character maps to that
  character's start; the offset between surrogate halves maps to the pair start; one byte
  per line break (CodeMirror `Text` joins lines with one `\n`). `toUtf16` binary-searches
  `starts` for the line, then scans that line's text in slices of at most 64 KiB.
- `protocol/utf8.ts`: add `static builds = 0` to `Utf8Index`, incremented in the constructor.
  Nothing else changes.

### 2. `RevisionHistory` (`history.ts`): delta charging

- `Entry` gains `bytes: number`.
- `record()` computes the charge against the previous entry's text (the base):
  `512 + sum over changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => 2*inserted.length + 2*baseLinesLength + 64*baseLineCount)`,
  where the base lines are `base.lineAt(fromA).number .. base.lineAt(toA).number`.
  - `baseLinesLength` is `lastLine.to - firstLine.from`, plus 1 when the last line is not
    the final line.
  - Use `Text.lineAt`, which is O(log n); never `toString`.
  - A gap restart resets the total and drops all pins (contract 3).
  - The initial entry has charge 0. When an entry becomes the oldest kept entry (its
    `changes` are set to `null` in `trimToBytes`), its charge is removed from the total,
    because its change set and the base leaves it accounted for are gone.
- A private running `total` is updated on push and shift. The `retainedBytes` getter returns
  `total + pinBytes()` in O(1), where `pinBytes()` is the sum of the cached charges of the
  pins' composed sets.
- `trimToBytes(budget)` keeps its count and byte loop, using the O(1) total. If the chain is
  down to one entry and the total still exceeds the budget, it releases the least recently
  pinned owner (`pinsEvicted += 1`) until the total fits or no pin remains. This safety bound
  keeps pins "under the byte ceilings" (15.3.5 as amended).
- Delete the `retainedBytes(e.text)` term. Keep the exported helper function
  `retainedBytes(value)`; `surface.ts` and the composed-set charge use it.

### 3. `RevisionHistory`: pins

Public API: `pin(owner: string, rev: number): boolean`, `unpin(owner: string): void`,
`pinnedRevisions(): readonly number[]` (for tests), and
`stats = { pinsEvicted, pinsRefused, lineTableBuilds }`.

- *Owners.* At most `MAX_PINS = 32` owners exist, each holding one revision. Several owners
  may share one `Pin` record by refcount.
- *LRU order.* Pinning an existing owner refreshes its position and may move it to a new
  revision. A 33rd owner evicts the least recently pinned owner.
- *Pin record:*
  - `{ rev, text, composed: ChangeSet, composedBytes, table: LineTable, memo: Map<string, Range16> }`.
  - `composed` starts as `ChangeSet.empty(text.length)`. On each `record()`,
    `composed = composed.compose(changes)` and `composedBytes = retainedBytes(composed)`.
    When a pin is created for a revision older than the current one, compose the chain
    entries `rev+1 .. current` once.
- *When `pin()` fails.* It returns `false` (`pinsRefused += 1` only when the cause is
  budget) when the revision is neither kept in the chain nor already pinned, or when the
  line table cannot fit the 8 MiB `indexByteLimit` after on-demand tables are evicted. A pin
  is never evicted to make room for an on-demand table.
- *Line table.*
  - When `rev === current` and the caller passes the live starts (`DocumentSync.pin`
    supplies `LineBytes.starts()`), the table is built from those starts with no text
    read.
  - Otherwise use `LineTable.build(text)` (`lineTableBuilds += 1`).
- *Memo.* The memo holds at most 32,768 entries. When full, delete the oldest entry first
  (Map insertion order). Charge 64 bytes per entry to the 8 MiB budget, with the table
  bytes.
- *Restart.* A history restart (revision gap in `record`) clears all pins and on-demand
  tables.

### 4. `RevisionHistory`: mapping

- `mapWireSpan(span, rev)` resolves in this order:
  1. *Pinned:* validate `span` against `table.byteLength` (same rejections as today:
     non-integer, negative, end before start, end past the length). Take the UTF-16 range
     from the memo, or from `table.spanToUtf16` and then store it. If `rev === current`,
     return it. Otherwise, if `touches(composed, from, to)`, return `null`; else map with
     the same `mapPos` and assoc rules as the chain loop (`history.ts:191-197`).
  2. *Current, not pinned:* use the live table that `DocumentSync` passes through an
     internal setter `setCurrentStarts(starts)` (called after each `LineBytes.update`), or
     convert directly through `LineBytes` in `DocumentSync.mapWireSpan` before calling
     history. Pick one, keep it O(log n + line length), and record the choice.
  3. *In the chain:* an on-demand `LineTable.build` in an LRU of at most 4 (`INDEX_CACHE`),
     within the 8 MiB budget shared with pins (on-demand tables are evicted first), then
     `mapSpan` through the chain as today.
  4. *Otherwise:* `null`.
- Replace `index(rev): Utf8Index | null` with `lineTable(rev): LineTable | null`. It returns
  the pinned table, the cached on-demand table, or builds one. `indexCount` and `indexBytes`
  stay as getters, now over on-demand tables plus pin tables and memos. History never
  constructs a `Utf8Index`.
- `DocumentSync.toWireSpan(from, to)` (`sync.ts:170-173`) converts through the live
  `LineBytes`, with no `Utf8Index`: `this.bytes.toByte(currentDoc, from)` and the same for
  `to`.
- `DocumentSync.pin(owner, rev)` passes `this.bytes.starts()` when `rev === this.revision`.
  `DocumentSync.unpin(owner)` delegates to history.

### 5. Undo accounting (`surface.ts`)

`trimUndoHistory(value, budget, memo?: WeakMap<object, number>)`: when `memo` is given, an
event's charge is read from the memo or computed once with `retainedBytes(event)` and then
stored. `CodeSurface` owns one `WeakMap` and passes it in both `boundHistory` calls. The
eviction order and results stay identical; `state.test.ts` rows 109-200 must pass
unchanged.

### 6. Consumers

- **`apis.ts`.** Add `toWireSpan(from: number, to: number): Span` to `CodeApi`, with a doc
  comment: "current revision; UTF-16 range to UTF-8 byte span; O(changed lines)".
- **`code/mount.ts`.**
  - The `api` object gains `toWireSpan: (f, t) => sync.toWireSpan(f, t)`.
  - In the `client.on('eval-result')` handler, call `sync.pin('eval', env.body.doc_revision)`
    for `DOC_FILE`.
  - Pass `pin`/`unpin` callbacks and `revision: () => sync.revision` to
    `HighlightScheduler`.
- **`code/highlight.ts`.**
  - `HighlightOptions` gains optional `pin?(owner, rev): boolean`, `unpin?(owner): void` and
    `revision?(): number`.
  - On acceptance of an event, call `pin('playing:' + rev, rev)`, which refreshes the LRU.
  - Keep `lastAccepted` per rev, on the scheduler clock (`audible.now()` or `clock.now()`).
    In `tick()`, unpin `playing:<rev>` when no entry has that rev and
    `now - lastAccepted > 2`.
  - `clear()` unpins every `playing:*` owner.
  - `Entry` gains `mapped?: { at: number; range: Range16 | null }`. In `tick()`, when
    `revision()` equals `mapped.at`, reuse `range` and do not call `opts.map`. Without
    `revision`, behave exactly as today.
- **`code/diagnostics.ts`.** It pins `diag:static` (the `eval-result` batch revision) and
  `diag:check` (the `checkBatch` revision). It pins `diag:runtime:<slot>` to the latest
  batch revision per slot and unpins the slot owner when the slot is cleared
  (`env.body.clear`). `dispose()` unpins all three kinds. One owner per slot keeps the owner
  count bounded.
- **`code/eval.ts`.** Uses `sync.toWireSpan`, now line-table based. No other change.
- **`bind/mount.ts` `bytes()`.** Use `this.code.toWireSpan(r.from, r.to)`. Delete
  `indexCache` and the `Utf8Index` import.
- **`bind/write.ts` around line 336.** Replace `new Utf8Index(surface.state.doc.toString())`
  with `host.toWireSpan` (add `toWireSpan` to the writer host options, wired from
  `code.toWireSpan` in `bind/mount.ts`). The whole-text `client.eval` argument at line 361
  stays (allowed transfer).
- **`code/pointer.ts`.**
  - Lines 84 and 95: replace `wordRange(doc.toString(), pos)` with a new
    `keyboard.ts` `wordRangeAt(doc: Text, pos)`. It segments only the containing line,
    within `[max(line.from, pos - 32768), min(line.to, pos + 32768)]`, and returns document
    offsets.
  - Line 111: replace `boundary(doc.toString(), pos)` with the line-local boundary helper.
    Export `keyboard.ts` `lineBoundary`, or add `boundaryAt(doc, pos, bias)` using the same
    slice rule.
- **`code/keyboard.ts` `wordMoveAt`.** Stop scanning after 65,536 UTF-16 units of line text
  have been examined. If no word is found, return the start (backward) or end (forward) of
  the last line examined.

### Patterns to imitate

- `sync.ts` `LineBytes` for line-local reads.
- `history.ts` `mapSpan` loop for the assoc rules.
- `test/canvas/edit-cost.test.ts` `setup()` and its `vi.spyOn(Text.prototype, 'toString')`
  counting for the counter rows.

## Tasks

### TASK-Y1: line-bytes.ts and LineTable, plus `Utf8Index.builds`
### TASK-Y2: Delta charging and the undo memo
### TASK-Y3: Pins, composed sets, memo and mapping order
### TASK-Y4: Consumers (`apis`, `mount`, `highlight`, `diagnostics`, `eval`, `bind`, `pointer`, `keyboard`) and test fakes
### TASK-Y5: Tests, verification and progress log

Each task's completion criterion is its contract above plus its test rows below, passing.

## Test Cases

`editor/test/protocol/utf8.test.ts` or `editor/test/code/sync.test.ts`:

- For documents with ASCII, Japanese, emoji (surrogate pairs), CRLF-input and empty lines:
  for every byte offset `0..byteLength`, `LineTable.build(text).toUtf16(b)` equals
  `new Utf8Index(text.toString()).toUtf16(b)`. For every UTF-16 offset, `toByte` matches.
- `LineBytes.starts()` equals `LineTable.build(text)` starts after 50 random edits.

`editor/test/code/history.test.ts`:

- After `pin('eval', r)` and 300 single-character edits outside the span on a 20,000-line
  `Text`, `mapWireSpan(span, r)` is non-null and equals the sequential reference (a
  `RevisionHistory` with limit 999 and a byte limit of 32 MiB, mapped without pins).
- The same 300 edits without a pin -> `mapWireSpan(span, r)` is `null` once r leaves the
  256-entry chain (control branch).
- A 300-edit run grows `retainedBytes` by at most
  `300 * (512 + 2 * (L + 1) + 64 + 2)`, where L is the edited line length.
- Pinning the current revision through `DocumentSync.pin` -> `LineTable.builds` unchanged
  and `Text.prototype.toString` not called.
- A 33rd owner evicts the least recently pinned owner (`pinsEvicted === 1`). Pinning `eval`,
  then 8 `playing:*` owners and 16 `diag:runtime:*` owners -> `eval` is still pinned.
- A revision gap (`record` with `rev !== current + 1`) -> `pinnedRevisions()` is empty.
- An edit inside the pinned span -> `null`. The cancelling case (insert "x", then delete it,
  inside the span) -> the documented composed result (non-null, the same range), with a
  comment pointing to 15.3.8.14.
**Byte formula for fixtures.** A line table costs `128 + 4 * (lines + 1)` bytes. It scales
with the line count, not the text length. A single-line document's table is 136 bytes. A
`Text.of(Array(1_048_576).fill(''))` document (1,048,576 lines) has a table of
`128 + 4 * 1,048,577 = 4,194,436` bytes, and two such tables (8,388,872) exceed
`INDEX_BYTES` (8,388,608). Each memo entry adds 64 bytes. Derive every fixture below from
this formula.

- **8 MiB pin refusal** (1,048,576-empty-line document, at least two kept revisions A and B,
  for example A = rev 1 and B = rev 2 after one edit):
  - `pin('a', A)` -> `true`;
  - `pin('b', A)` -> `true`, sharing the record, and `indexBytes` is unchanged;
  - `pin('c', B)` -> `false`, with `pinsRefused === 1` and `indexBytes <= INDEX_BYTES`;
  - control: after `unpin('a')` and `unpin('b')`, `pin('c', B)` -> `true`.
- **On-demand tables are evicted before a refusal** (same document): call `lineTable(C)` for
  a third kept revision C, which caches one on-demand table (4,194,436 bytes). Then
  `pin('a', A)` -> `true`. The on-demand table for C was evicted (`indexCount` counts only
  the pin's table) and `pinsRefused === 0`. A pin is never evicted to make room for an
  on-demand table: a later `lineTable(C)` with `a` still pinned returns an uncached table
  and leaves the pin in place.

`editor/test/canvas/state.test.ts` "revision and index ceilings" rows (lines 217-260) are
ported from `index()` to `lineTable()`. Every `expect()` line is kept. Only the method name,
a constructor limit argument or the document fixture changes, as listed:

- **Row :218-231, "enforces 256 revisions and four cached indexes".** Replace
  `h.index(rev)` with `h.lineTable(rev)`. The document is a single line, so each table is
  136 bytes. `indexCount === 4` (LRU of 4), `retainedBytes <= HISTORY_UNDO_BYTES`,
  `indexBytes <= INDEX_BYTES` and the 256-revision assertions are unchanged.
- **Row :233-246, "evicts byte-heavy old revisions and refuses to cache oversized indexes".**
  - Replace `h.index` with `h.lineTable`.
  - Change the `indexByteLimit` constructor argument from `400` to `128`, which is below one
    136-byte single-line table, so no table can be cached. `lineTable()` returns an uncached
    table when it cannot be cached, matching today's `index()` (`history.ts:165`).
  - Keep the 800-byte history limit. Under delta charging each kept revision of this
    single-line, about 84-character document is charged about
    `512 + 2*1 + 2*(80 + k) + 64`, about 740 bytes. A second kept entry (about 1,480) exceeds
    800, so the oldest entries are trimmed, and an entry that becomes oldest drops its
    charge. That keeps `retainedBytes <= 800`, and revision 1 leaves the history, so the
    `mapWireSpan({0, 3}, 1)` null assertion holds.
  - `indexBytes <= limit`, `indexCount === 0`, `text(5)` and
    `lineTable(5)?.toByte(4) === 10` are unchanged. If the computed charges differ, adjust
    only the 800 limit by a recorded port that still forces trimming (any value below two
    entries' charge).
- **Row :248-260, "enforces the real 8MiB index byte cap before the four-entry count cap".**
  - The document `'a'.repeat(1024 * 1024)` becomes `Text.of(Array(1_048_576).fill(''))`,
    because a single line now gives a 136-byte table and could no longer reach the byte cap.
  - Keep the appends of `'x'` at the end.
  - `h.index` becomes `h.lineTable`.
  - Two 4,194,436-byte tables exceed `INDEX_BYTES`, so `indexCount === 1` and
    `indexBytes <= INDEX_BYTES` hold.
  - `mapWireSpan({start: 0, end: 1}, 1)` still equals `{from: 0, to: 1}` (byte 0..1 is the
    first line break, UTF-16 0..1, untouched by appends at the end).

Each changed row records the port (file:line, old and new argument) in the progress log.
No assertion is deleted or loosened.

`editor/test/canvas/edit-cost.test.ts` (20,000-line `setup()`):

- One `replaceSelection('x', 'input.type')` -> `Utf8Index.builds` unchanged,
  `LineTable.builds` unchanged, and no `Text.toString` on a document of 64 KiB or more.

`editor/test/code/highlight.test.ts`:

- With `revision` constant, 10 `tick()` calls on accepted entries -> `map` called once per
  entry in total (the first tick). After the revision changes -> mapped again.
- `clear()` -> `unpin` is called for each `playing:*` owner.
- No event for rev r for more than 2 s -> `unpin('playing:r')`.

`editor/test/code/diagnostics.test.ts`:

- A runtime batch for slot `d1` at rev 5 -> `pin('diag:runtime:d1', 5)`. A clear of `d1`
  -> `unpin`.

`editor/test/bind/write.test.ts`:

- A slider source-edit write on a multi-line document -> `Utf8Index.builds` unchanged, and
  the anchors are identical to the previous expectations.

`editor/test/canvas/input.test.ts` (pointer):

- A double-click word select on line 10,000 of the 20,000-line document ->
  `Text.prototype.toString` not called on the document. A drag `pointermove` -> not called.

`keyboard`:

- Ctrl+ArrowLeft at the start of 70,000 characters of punctuation-only lines -> returns
  within the 64 KiB bound (a line start).
- A normal word move -> unchanged results (the existing rows).

## Pitfalls

- Charge the delta against the base revision's lines (`fromA`/`toA` on the old text), not
  the new text. Using `Text.length` of either revision brings back the full-copy bug.
- `ChangeSet.compose` requires matching lengths: `composed.newLength` must equal
  `changes.length`. Compose in record order, before `trimToBytes`.
- Never construct `Utf8Index` in `history.ts` or `sync.ts`. Tests count it.
- Keep `retainedBytes` O(1). Do not walk entries in the getter: `boundHistory` calls it on
  every keystroke.
- `pin()` for a revision that already left the chain must fail. Do not resurrect a text.
- Highlight unpinning must not run per event in `onPlaying`. Do it in `tick()`.
- `diagnostics.ts` must keep the 15.3.8.13 C revision-skip rule.
- Do not touch `editor/src/code/{renderer,layout,atlas,syntax,input,accessibility,frame}.ts`
  or the bind panel files. They belong to other plans.

## Verification

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`
and `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.

Record `BASE` and the fresh-read sha256 values in `tmp/canvas-cutover/opt-history/intent.json`.

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/code/history.test.ts test/code/sync.test.ts test/protocol/utf8.test.ts test/canvas/state.test.ts test/canvas/edit-cost.test.ts test/code/highlight.test.ts test/code/diagnostics.test.ts test/bind/write.test.ts test/canvas/input.test.ts` | exit 0; the new rows are listed |
| `cd editor && ./node_modules/.bin/vitest run test/bind test/params test/app test/code test/canvas` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `grep -n "new Utf8Index" editor/src/code/history.ts editor/src/code/sync.ts editor/src/bind/mount.ts editor/src/bind/write.ts` | no match (exit 1 from grep is the expected "no match"; record it in notes, not as a verification record) |
| `grep -n "doc.toString()" editor/src/code/pointer.ts` | no match (record in notes) |
| `wc -l` on every touched `.ts` file | each under 1000 (record in notes) |
| `git diff --name-only $BASE` | a subset of this plan's paths |

Outside the sandbox: `cd editor && npm run test:perf` (alone); `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`;
`NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`;
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`;
`CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml`. Each must
exit 0.

Use the session-286 record format: `{command, exitStatus: 0, testsRun > 0, testsPassed,
failureCount: 0, outcome: "passed", log}` for tests, details in `notes`. The grep and wc
checks go in `notes` only. Run no mutation or negative-control command.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/opt-history/intent.json`
and `receipt.json`. If a file drifted without an edit from this plan, stop editing it and
report. Edit only this plan's progress log.

## Completion Criteria

- [ ] `line-bytes.ts` with `LineBytes`, `LineTable` and `starts()`; byte-identical to `Utf8Index` on the test corpus
- [ ] Delta charging with an O(1) total; the 300-edit growth bound passes
- [ ] Pins: owners, composed sets, the 32-owner LRU, refusal and restart rules; the eval-survives-churn row passes
- [ ] After 300 edits, `mapWireSpan(evalSpan, evalRev)` is non-null and equals the sequential reference
- [ ] A single-character edit makes 0 `Utf8Index` builds and 0 `LineTable` builds; no whole-document `toString` in history, sync, bind or pointer paths
- [ ] `CodeApi.toWireSpan` added; the bind consumers use it; the test fakes compile
- [ ] Highlight per-revision cache: 0 `map` calls on ticks with an unchanged revision
- [ ] `state.test.ts` ceiling rows ported to `lineTable()` with identical `expect()` lines (`indexByteLimit` 400 -> 128 at :235; 1,048,576-empty-line fixture at :249), each port recorded; every other existing assertion unchanged
- [ ] The pin-refusal row (a@A, b@A shared, c@B refused, c@B succeeds after unpin) and the on-demand-evicted-before-refusal row pass; fixtures derived from `128 + 4 * (lines + 1)`
- [ ] Default vitest, `npm run check`, `test:perf`, clippy, nextest, wasm32 build and src-tauri check pass
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-06 (session 286 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 2.
**Plan repair (step5 S286-PLAN-HISTORY-TABLE-ARITH, mid)**:

- Stated the table formula `128 + 4 * (lines + 1)`.
- Rewrote the pin-refusal row (a@A, b@A shared, c@B refused, then accepted after unpin).
- Added the on-demand-evicted-before-refusal row.
- Ported the `state.test.ts` ceiling rows: `indexByteLimit` 400 -> 128, and the
  1,048,576-empty-line fixture.
- Defined charge removal for the oldest entry.
- `LineTable.build` iterates lines linearly.

**Notes**: Wave 9, after HARNESS. It fixes the highlight-death product bug that also causes
the mass stale flip of the bind panel.

# Context-Aware Completion (engine, LSP, wasm, editor service)

This document designs one completion engine for `.vact` sources and its
three consumers: the LSP, a raw wasm export, and a UI-agnostic editor
service with a small DOM popup. It answers the user request of
2026-09-30, which was written in Japanese: build an auto-completion and
suggestion engine that shows next-candidate suggestions while typing.

Issue reference: workflow input of `opus-luna-design-and-implement-review-loop-session-226`
(no GitHub issue number was supplied). Status: Implemented and verified
2026-09-30 (session 234; 1,672 plain nextest, 1,696 `--features lsp`
nextest, and 432 vitest tests passed); plans are in `impl-plans/completed/`,
branch `wf/syntax-fmt`. The open choices are in
`design-docs/user-qa/pending-completion-questions.md`. Their
recommendations are followed by default.

Companion changes of the same run, (B) the CodeMirror-free tree-sitter
span core, (C) the formatter space-indent repair, and (D) the wasm
format test loader, are specified in `design-formatter-and-syntax.md`
sections 3.9, 5.5 and 5.4. Section 8 of this document owns the delivery
waves for all four parts.

## Overview

| Part | Where | Role |
|------|-------|------|
| Engine core | `src/complete/` (new) | A pure `(text, cursor, snapshot, limit) -> Completion` function. It is wasm32-clean, never panics, and does bounded work. |
| LSP consumer | `src/lsp/analysis.rs`, `src/lsp/server.rs` | `textDocument/completion` delegates to the engine. The analysis adds package names and checked types to the snapshot. |
| Wasm consumer | `src/host/wasm/complete_abi.rs` (new) | Pinned exports `complete_source`, `complete_out_ptr` and `complete_out_len`, in the style of `fmt_abi.rs`, returning JSON |
| Editor service | `editor/src/code/completion.ts` (new) | Calls the export and maps byte ranges to UTF-16. It has no DOM and no CodeMirror. |
| Editor popup | `editor/src/code/completion-popup.ts` (new) | A DOM listbox driven only through the `CompletionSurface` interface |
| EditorView adapter | `editor/src/code/completion-view.ts` (new) | The only completion file that imports `@codemirror/view`. It implements `CompletionSurface` for today's EditorView. |

### Baseline this document revises

- design-implementation 14.5 "Features" lists LSP completion as
  "prelude names from `NativeTable`, document top-level names, manifest
  keywords, package prefixes and qualified names". Today that is
  `Analyzer::complete` (`src/lsp/analysis.rs:337`). It filters every
  source with `starts_with` on the word before the cursor. It is not
  scope-aware, position-aware or ranked, and only `vactr lsp` can reach it.
  This document keeps every one of those sources and moves them into the
  engine (section 4 defines the superset rule).
- The web editor (design 15.1.5) has no completion.
  `@codemirror/autocomplete` is not installed and is NOT added (section 6).
- design-implementation 15.3 (on `origin/main`, commit `8ee36f2`)
  replaces the visible EditorView with a WebGL2 canvas. It keeps
  EditorState as headless machinery and allows DOM panels and input
  bridges. Section 6.5 maps the popup's surface onto the 15.3.2
  code-surface contract, so completion survives the cutover.

---

## 1. Scope

In scope:

- the engine core with the seven cursor contexts of 3.3 (`None`
  included), scope-aware
  locals (3.4), the candidate sources of 3.5, and the ranking and cap of
  3.6;
- LSP completion on the engine;
- the wasm export and a node test against the real artifact;
- the editor service, the DOM popup, the EditorView adapter, and minimal
  wiring in `mount.ts`, `main.ts` and `deps.ts`.

Out of scope:

- `textDocument/signatureHelp` (deferred, user-QA C2);
- snippets, with parameter placeholders or otherwise;
- completion of names that exist only in a live session (console
  `import`s, definitions evaluated from other files);
- package names in the wasm tier (the wasm snapshot has no package
  cache, user-QA C3);
- unqualified names of `open` imports;
- `@codemirror/autocomplete`, and any change to `app/apis.ts` or
  `code/language.ts`.

## 2. Data flow

```
LSP:    didChange -> Analyzer (analysis thread) -- text, cursor, Snapshot{manifest, packages, doc extras}
                                              -> complete::complete -> CompletionList (UTF-16 TextEdits)
Editor: keystroke -> CompletionSurface.onChange -> CompletionPopup -> CompletionService
                  -> WasmCompletionEngine -> complete_source (tool wasm instance, Snapshot::builtin)
                  -> JSON (UTF-8 byte ranges) -> service maps to UTF-16 -> popup -> surface.replace
```

The engine reads only the document text and a snapshot. It never
expands, type-checks, executes code or does I/O. That keeps it cheap
enough for every keystroke, and it keeps it identical in both tiers.

---

## 3. Engine core (`src/complete/`)

### 3.1 API

- `complete::complete(text: &str, cursor: usize, snap: &Snapshot, limit: usize) -> Completion`.
  - `cursor` is a UTF-8 byte offset. It is clamped to `text.len()` and
    moved back to the nearest char boundary.
  - `limit` is clamped to `1..=MAX_LIMIT` (500). `DEFAULT_LIMIT` is 100.
- `Completion`:
  - `context: ContextKind`;
  - `from, to: usize`: the replace range, in UTF-8 bytes. `to` is always
    the cursor. Every item replaces the same range;
  - `items: Vec<Candidate>`, in rank order;
  - `incomplete: bool`: true when the cap cut the list.
- `Candidate`:
  - `label`: the display text;
  - `kind: CandidateKind`;
  - `detail`: a signature, type or origin;
  - `insert`: the text that replaces `from..to`.
- `CandidateKind`: `Local`, `Function`, `Value`, `Variable`, `Type`,
  `Keyword`, `Control`, `Key`, `Module`, `Qualified`. JSON uses the
  lowercase names.
- `complete::complete_bytes(input: &[u8], cursor: u32, limit: u32) -> (u32, Vec<u8>)`
  is the byte-level seam for the wasm export, testable on native targets,
  like `fmt::format_bytes`. It uses `Snapshot::builtin()`. Its statuses
  are:
  - `STATUS_OK` = 0, with the JSON of 5.2 as output;
  - `STATUS_NOT_UTF8` = 2, with empty output;
  - `STATUS_BAD_CURSOR` = 3, when `cursor > len`, with empty output.
  A `limit` of 0 means `DEFAULT_LIMIT`.

### 3.2 Document model: the layout skeleton, not the node tree

The reader turns a statement with any error into ONE childless `Error`
node and drops the nodes of its block (`reader/line.rs` `statement`).
While the user is typing, the statement under the cursor usually has an
error, such as an unclosed `{`, a trailing `->` or a half-typed pair. So
the engine does NOT use `read()` nodes. It uses the reader's own layout
skeleton instead:

- `reader::layout::split_lines` and `reader::layout::statements`. These
  are crate-internal `pub(crate)` APIs, and the engine uses them as they
  are. There are no reader changes, so parse behavior and render digests
  cannot move.
- The skeleton gives each statement's tokens (continuation lines
  included), its block of statements and its spans. It exists for every
  statement, whether it has errors or not. Tokens are the reader's
  `lexer::Token`, so identifiers, keywords, pair colons, block colons and
  `>` follow the language exactly.
- `reader::prescan_imports(text)` gives the document's import prefixes
  and paths.

A small byte scan of the cursor's physical line, from the line start to
the cursor, decides whether the cursor is in string text or in a comment,
and where the word under the cursor starts (3.3). The lexer drops the
content of an unterminated string, so the scan tracks `"`, `\` escapes,
`{`/`}` interpolation depth inside strings, and `#` outside strings. An
interpolation `"{...}"` counts as code.

### 3.3 Cursor contexts

The WORD is the text before the cursor that is still part of the current
token. Identifier bytes are the lexer's (`[A-Za-z0-9-]`; identifiers are
ASCII, `lexer::is_identifier`). The word may start with a `:`, and it may
contain one `.` after an identifier prefix. `from` is the start of the
word.

The ROLE comes from the previous significant token before the word. That
token is found in the cursor statement's tokens, keeping a bracket stack
(`{`, `[` and string interpolation). The SEGMENT is the run of tokens
since the last statement start, `{`, pipe `>` or `->`. The segment's HEAD
is the segment's first token, when it is an identifier or a qualified
name.

| Context | Detection (first rule that matches) | Candidates (tier, 3.6) |
|---------|--------------------------------------|------------------------|
| `None` | The cursor is in string text or a comment. The word starts with a digit, or with `-` and a digit (a number). The word is an identifier in a DEFINITION-NAME position: right after `let var fn inst struct enum bus look`, or a header parameter name of a `fn`/`inst` statement before its block colon. A default value after a header pair colon is not a definition name, and a word that starts with `:` never is. The cursor directly follows a pair colon (`name:|`, with no space). | none |
| `Keyword` | The word starts with `:`, and whitespace, the line start, `[` or `{` comes before that `:`. | Manifest `sounds`, `synths` and `controls` (0). `synths` also holds `dsp::ugen::catalog::TEMPLATE_NAMES` and the document's `inst` names, because `s :name` plays them. When the segment head is `s` or `sound`, sounds and synths come before controls. |
| `Qualified` | The word is `prefix.rest`, and `prefix` is an identifier. | The names of the package that `prefix` is bound to, labeled `prefix.name` (0). An unbound prefix gives none. |
| `PipeTarget` | The previous token is `>`, and it is a pipe. A `>` that starts a line is a continuation pipe. A `>` at the head of a `{}` group is greater-than, so that case is an `Argument` (lang-reference 1). | Controls of the statement's SEED sound (0). Then locals (1), document (2), prelude (3) and packages (4). |
| `Head` | There is no previous token (the statement line start), or the previous token is `{` or `->`. | locals (1), document (2), prelude (3), packages (4) |
| `PairKey` | This is an argument position (the fallback below), and the head has a non-empty KEY SET. Also, a pair (`key:`) already comes earlier in this segment, or the head's maximum positional arity is known and already used. | The head's keys, as `name:` (0), then the `Argument` sources |
| `Argument` | Any other code position | locals (1), document (2), prelude (3), packages (4) |

Definitions used above:

- **Seed sound.** When the statement's first segment is `s` or `sound`
  followed by a keyword `:name`, the seed sound is `name`. Its controls
  are the parameter names of `dsp::meta::decl_for(name)` when that
  exists. For a document `inst name ...:` they are the header pair keys
  of that `inst` instead. So `s :analog > wave :saw > cu|` offers
  `cutoff` first.
- **Key set** of a head:
  - `NativeSig::keywords` for a native head (for example `kit` of `s`);
  - the header pair keys of a document `fn` or `inst` with that name;
  - the parameter names of `dsp::meta::decl_for(head)` for a ugen,
    effect or template head.
- **Maximum positional arity:**
  - a native head: `NativeSig::max_args`;
  - a document `fn`: its count of positional header parameters;
  - any other head: unknown.

  When the segment follows a pipe `>`, the piped subject counts as the
  first positional argument.
- `Keyword` wins over `PairKey`. After the colon of a pair followed by a
  space (`gain: |`), the position is an `Argument` (the pair value).
- Manual and automatic requests are classified the same way. The ABI
  (5.1) carries no mode, so `name:|` stays `None` for `Ctrl-Space` too.
  The user types the space that starts the value first.

### 3.4 Scope resolution

The scope chain is the one decided in architecture.md (2026-09-25 row)
and lang-reference 1: prelude -> session (the document's top level) ->
fn and block scopes. A child may shadow a parent. The inner binding wins,
and completion shows only the innermost binding of a name.

1. **Document names.** Take every top-level statement whose first token
   is `let var fn inst struct enum bus look` and whose second token is an
   identifier. They are visible everywhere in the document, because
   top-level bindings are namespace bindings. The detail is the head
   word. The LSP may replace it with a checked type (4.1).
2. **Enclosing blocks.** Let `L` be the number of tabs that lead the
   cursor's line. Start at the top-level body at level 0 and repeat:
   - Pick the LAST statement `S` in the body that starts at or before
     the cursor.
   - If `S` has a block with inner level `b`, and `L >= b`, descend into
     that block. The block becomes a frame, and `S` is the frame's owner.
   - Otherwise stop.

   This matches `layout.rs`: a shallower line would have been picked at
   the parent level. It also works on a blank or tab-only cursor line,
   which the skeleton omits.
3. **Frame binders, from the owner `S`:**
   - `fn` or `inst`: the header identifiers after the name up to the
     block colon. For a pair, the key is bound, and everything after
     the pair colon is skipped up to the next key followed by a pair
     colon or up to the block colon. That covers `gain: 0.8` and the
     prelude form `cutoff: float = 1200` (`src/prelude/templates.vact`).
   - A statement whose last top-level `->` directly precedes its block
     colon (`x ->:`, `if {d :gain} g ->:`, a `match` clause
     `voice [note: p] ->:`): the BINDERS of that arrow's left side.
   - `for pattern iterable:`: the binders of the pattern (the first item
     after `for`).
   - Anything else (`struct`, `enum`, `match v:`, `let x:`) binds
     nothing.
4. **Block locals.** In each frame's body, take the `let`, `var`, `fn`
   and `inst` statements that START BEFORE the statement that holds the
   cursor. A block local is visible only after its definition, and never
   outside its block. A statement that starts after the cursor is not
   visible.
5. **Inline lambdas.** Walk the cursor statement's tokens before the
   cursor with a bracket stack. For each enclosing `{` group, and for the
   statement level itself, a top-level `->` before the cursor makes the
   binders of its left side visible. Examples:
   `map xs {x -> * x |}` and `x b -> + x |`.
6. **Binders.** The identifiers of a left side at bracket depth 0 or
   inside `[...]` patterns. Excluded are pair keys (an identifier before
   a pair colon), the tokens inside `{...}` groups, and reserved heads
   (`if elif else match for fn let var upd`, `expand::kernel`). A leading
   struct or variant name in a clause (`voice [note: p]`) is bound too.
   That extra local is harmless, because the same name is also a
   document name.
7. **Order.** Frames go from innermost to outermost: inline lambdas,
   then blocks from the deepest. Within a frame, names are in source
   order. The first occurrence of a name wins (shadowing).

### 3.5 Candidate sources and the snapshot

`Snapshot` is the context the host supplies:

| Field | `Snapshot::builtin()` (wasm, tests) | LSP (4.1) |
|-------|--------------------------------------|-----------|
| `manifest: HostManifest` | `HostManifest::spec_default()` (the session's default too, `session.rs`) | the analysis manifest, with fetched package banks |
| `packages: Vec<PackageNames { prefix, path, names }>` | empty | every bound prefix with `Analyzer::package_names(path)` |
| `document: Vec<DocName { label, kind, detail }>` | empty | top-level definitions of the checked forms, with the type as detail |

Fixed sources, which the engine reads directly because they are static
and wasm-clean:

- `NativeTable::global()` gives the prelude. The kind is `Function` or
  `Value`, and the detail is `sig.ty[0]`, or `prelude` when that is
  missing, as today.
- `dsp::meta::decl_for` gives template, ugen and effect parameters.
- `dsp::ugen::catalog::TEMPLATE_NAMES` gives template names.

From the text, the engine takes the import prefixes (`prescan_imports`,
kind `Module`, detail = path), plus locals and document names (3.4).
Qualified names outside a `Qualified` context get the package tier, as
today's LSP does. `snap.document` entries merge into the document tier.
They override the detail and add names that the skeleton scan cannot see.

### 3.6 Matching and ranking

Let `p` be the word without a leading `:`. In a `Qualified` context, `p`
is the part after the `.`. Matching is ASCII case-insensitive, and every
candidate gets a MATCH CLASS:

| Class | Rule |
|-------|------|
| 0 `prefix` | The label (without `:`) starts with `p`. An empty `p` matches everything with class 0. |
| 1 `subword` | `p` starts a later `-`- or `.`-separated segment (`dec` matches `lpg-decay`), or `p` is a run of segment initials (`ea` matches `env-adsr`). |
| 2 `fuzzy` | `p` is a subsequence of the label, and its first character matches at a segment start. |
| none | The candidate is dropped. |

The TIERS are:

- 0 `context` (keys, seed controls, keyword sets, the qualified names of
  the typed prefix);
- 1 `local`;
- 2 `document`;
- 3 `prelude`;
- 4 `package`.

Candidates are ranked as follows:

1. Duplicate labels are removed first. The occurrence with the lowest
   `(tier, frame depth)` stays, so an inner local shadows an outer one,
   and a local shadows a document or prelude name.
2. The sort key is `(class, tier, frame depth, sub-order, label bytes)`.
   The sub-order is the `Keyword` set order of 3.3. The sort is total,
   so the order is stable and deterministic.
3. The list is truncated to `limit`. `incomplete` is set when the list
   was cut.

This gives "exact prefix before subword or fuzzy", and within a class
"locals before document before prelude".

### 3.7 Robustness and performance budget

- **No panic** on any input. Every slice uses a char boundary or a lexer
  offset. The cursor is clamped (3.1). Unknown or partial token shapes
  give `None` or fewer candidates, never an error. The skeleton walk is
  bounded by the reader's `MAX_NESTING` (128).
- **Bounded work** for each call:
  - one `split_lines` and `statements` pass, O(n) in the text;
  - a scan of the cursor statement's tokens;
  - candidate generation, O(|natives| + |manifest| + |document names| + |locals| + |package names|);
  - one sort of the matched candidates.
- **Budget.** Under 5 ms per call natively, in a release build, for a
  2000-line (about 80 KB) document with the builtin snapshot. An ignored
  test `complete::tests::budget` builds that document and asserts that
  the median of 20 calls is under 5 ms. The plan runs it with
  `cargo nextest run --release --run-ignored only -E 'test(budget)'` and
  records the measured median as evidence. The default debug nextest
  keeps a tripwire version: under 250 ms in a debug build.
- The editor makes at most one call in flight, and later keystrokes
  coalesce (6.3). So the wasm cost is one call per settled keystroke.

### 3.8 Module layout

| File | Content |
|------|---------|
| `src/complete/mod.rs` | the API types, `complete`, `complete_bytes`, the statuses, and the JSON encoding (`serde_json`, already a core dependency) |
| `src/complete/context.rs` | the line scan, word extraction and context classification (3.3) |
| `src/complete/scope.rs` | the skeleton walk, frames, binders and document names (3.4) |
| `src/complete/sources.rs` | `Snapshot` and candidate generation for each context (3.5) |
| `src/complete/rank.rs` | match classes, deduplication, the sort and the cap (3.6) |
| `src/complete/tests/{mod,context,scope,rank,robust}.rs` | the tests of 7.1 |

Every file stays under 1000 lines. `src/lib.rs` gains `pub mod complete;`,
which is built for every target. The module adds no crate dependency.

---

## 4. LSP consumer

### 4.1 Behavior

- `Analyzer::complete_list(uri, pos) -> CompletionList` builds the
  snapshot from the open document's `Analysis`:
  - `manifest` is `analysis.manifest`;
  - `packages` comes from `alias_env.prefixes` with the cached
    `package_names`;
  - `document` comes from the checked forms, with the type as detail
    exactly as today.

  It then calls `complete::complete` with `DEFAULT_LIMIT`.
- Each candidate maps to a `CompletionItem`:
  - `label` and `detail`;
  - `kind`: `Local`/`Variable` -> VARIABLE, `Function` -> FUNCTION,
    `Value` -> CONSTANT, `Type` -> STRUCT, `Keyword` -> KEYWORD,
    `Control` -> PROPERTY, `Key` -> FIELD, `Module` -> MODULE,
    `Qualified` -> FIELD;
  - `text_edit` replaces `from..to`, converted to UTF-16 by the existing
    `lsp::convert::range`, with `insert`;
  - `filter_text` is the label;
  - `sort_text` is the zero-padded rank, so clients keep the engine
    order.
- `Analyzer::complete(uri, pos) -> Vec<CompletionItem>` stays as a
  wrapper that returns `complete_list(..).items`, so existing callers
  compile.
- `AnalysisReq::Complete` replies with the `CompletionList`. The
  `completion` handler in `server.rs` returns
  `CompletionResponse::List { is_incomplete, items }`. The trigger
  characters stay `:` and `.`.

### 4.2 Superset rule and changed expectations

- **Superset.** For a cursor that is not in a string, a comment or a
  definition-name position, the UNCAPPED engine result (`limit =
  MAX_LIMIT`, and the documents in the test have fewer candidates than
  that) includes every label that today's `Analysis::complete` returns
  AND that fits the context:
  - `:`-labels in `Keyword`;
  - `prefix.`-labels in `Qualified`;
  - all other labels in `Head`, `PipeTarget`, `Argument` and `PairKey`.

  When the cap cuts the list, the LSP sets `isIncomplete`, and the
  client asks again as the user types. A new test keeps a copy of
  today's filter as a test helper, and asserts the rule over a table of
  documents and cursors (7.2).
- **Intentional changes** to `completion_offers_prelude_names_keywords_and_document_names`
  (`src/lsp/tests/analysis.rs:122`):
  1. At an empty line (`Head`), `:bd` and `:analog` are no longer
     offered. Keywords are offered in the `Keyword` context, which
     typing `:` opens (a trigger character). The test moves those
     assertions to a cursor after `:`.
  2. `sine` is not guaranteed in an empty-prefix `Head` list. There are
     more than 100 prelude natives, and document names rank first. The
     test asserts `tempo-x` and `incomplete == true` there, and `sine`
     for the prefix `si`.
  3. The `:b` half of that test changes too. `any(label == ":bd")` stays.
     The `all(label.starts_with(":b"))` assertion becomes: every item is
     a `:`-keyword (the context is `Keyword`), and every prefix-class
     item (`:bd`, `:bd-haus`, `:bd-tek`, `:break`, `:begin`) ranks before
     every subword or fuzzy match such as `:six-bank-a-voice`. The 3.3
     `Keyword` set holds `dsp::ugen::catalog::TEMPLATE_NAMES`, and the
     subword class of 3.6 intentionally applies in every context, so
     `b` matches the `bank` segment of `six-bank-a-voice`.
- Inside strings and comments, the LSP now returns no items. Today it
  returns the whole list. This is intentional (3.3 `None`).

---

## 5. Wasm consumer

### 5.1 Exports (`src/host/wasm/complete_abi.rs`)

It is declared in `host/wasm/mod.rs`, so it is built only for `wasm32`
with `host-wasm`, like `fmt_abi.rs`.

- `complete_source(ptr: *const u8, len: u32, cursor: u32, limit: u32) -> u32`
  returns the status of `complete::complete_bytes` and stores the output
  in a thread-local buffer.
- `complete_out_ptr() -> *const u8` and `complete_out_len() -> u32`
  return that buffer, which stays valid until the next
  `complete_source`.
- The input goes through the existing `alloc`/`free`. There is no
  outbox and no `*_init`, and the function is pure. `Snapshot::builtin()`
  is built once, in a thread-local.

### 5.2 JSON result (version 1)

```
{"v":1,"context":"head","from":12,"to":14,"incomplete":false,
 "items":[{"label":"alpha","kind":"local","detail":"param of f","insert":"alpha"}, ...]}
```

- `context` is one of `none keyword qualified pipe head pair-key argument`.
- `from` and `to` are UTF-8 byte offsets into the input.
- With `none`, `items` is empty.

---

## 6. Editor: service, popup and surface

### 6.1 Service (`editor/src/code/completion.ts`)

It is pure TypeScript with no DOM and no `@codemirror/*` import.

- `interface CompletionEngine { complete(text: string, byteCursor: number, limit: number): Promise<RawCompletion> }`
  is the JSON of 5.2.
- `WasmCompletionEngine` calls the 5.1 exports on the shared tool
  instance (6.4). Its `load()` rejects when `vactr.wasm` cannot be
  fetched or instantiated, or when `complete_source` is missing.
- `class CompletionService`:
  - `complete(text, cursor16, limit = 100) -> Promise<CompletionResult | null>`
    converts the UTF-16 cursor to a UTF-8 byte offset, calls the engine,
    and maps `from`/`to` back to UTF-16;
  - `CompletionResult` is
    `{ context, from, to, incomplete, items: {label, kind, detail, insert}[] }`;
  - it returns `null` for status 2 or 3 or a `none` context;
  - after the FIRST engine rejection it marks itself `unavailable`,
    returns `null` from then on, and never calls the engine again;
  - it never throws.
- `utf16ToUtf8(text, cursor16)` and `utf8ToUtf16(text, byteOffset)` are
  exported pure helpers. They count 3 bytes for a BMP code unit at
  U+0800 and above, such as Japanese, and 4 bytes for one surrogate pair
  (2 code units). An offset inside a pair clamps to the pair's start.

### 6.2 `CompletionSurface` (types in `completion-types.ts`)

This is the popup's only view of the editor. The shared editor-side
completion types live in `editor/src/code/completion-types.ts`, which
holds types only:

- `CompletionSurface`, `SurfaceChange` and `CompletionKey`;
- the 5.2 JSON shapes and the `CompletionEngine` interface;
- the `CompletionSource` interface: `complete(text, cursor16)` and an
  `available` flag. The popup consumes it, and `CompletionService`
  implements it.

That file lets the service, the popup and the adapter be written in
parallel. `completion-popup.ts` re-exports `CompletionSurface`, so a
canvas surface can import it from either file.

```
interface CompletionSurface {
  text(): string;                                  // whole document, UTF-16
  selection(): { anchor: number; head: number };   // main selection, UTF-16
  replace(from: number, to: number, insert: string): void; // one transaction, cursor at from + insert.length, userEvent 'input.complete'
  caretRect(pos: number): { left: number; top: number; bottom: number } | null; // viewport coordinates
  isComposing(): boolean;
  onChange(listener: (change: SurfaceChange) => void): () => void;
  onKey(handler: (key: CompletionKey) => boolean): () => void; // true = consumed
  onBlur(listener: () => void): () => void;
  onCompositionStart(listener: () => void): () => void;
  popupHost(): HTMLElement;                        // DOM parent for the panel
}
interface SurfaceChange { docChanged: boolean; selectionChanged: boolean; userEvent: string | null; inserted: string }
type CompletionKey = 'ArrowUp' | 'ArrowDown' | 'PageUp' | 'PageDown' | 'Enter' | 'Tab' | 'Escape' | 'Ctrl-Space';
```

### 6.3 Popup behavior (`editor/src/code/completion-popup.ts`)

- **DOM.** The panel is `div.vact-completion[role=listbox]`, positioned
  `fixed` under `caretRect(from)`. Each row is
  `div.vact-completion-item[role=option]`, with label, kind and detail,
  and `aria-selected` on the selected row. At most 12 rows are visible,
  and the rest scroll. The panel sets its own minimal inline styles, so
  no stylesheet is edited.
- **Auto-trigger.** A change triggers a request when all of these hold:
  - `docChanged`;
  - `userEvent === 'input.type'`;
  - `inserted` is one character that is an identifier byte, `:` or `.`;
  - the selection is empty;
  - the surface is not composing.

  Whitespace, paste, undo, redo, format and remote edits never open the
  popup.
- **Manual trigger.** `Ctrl-Space` requests completion at the cursor,
  even with an empty word and without a trigger character. It returns
  `false` when the service is unavailable.
- **While open:**

| Key or event | Effect |
|--------------|--------|
| `ArrowDown` / `ArrowUp` | move the selection (wraps) |
| `PageDown` / `PageUp` | move the selection by 12 rows (clamped) |
| `Enter` / `Tab` | accept the selected item: `replace(from, cursor, insert)`, then close |
| `Escape` | close |
| mouse `mousedown` on a row | `preventDefault` (the editor keeps focus), then accept that row |
| typing (the trigger rule, or any `input.type`/`delete.*` change with the cursor still at or after `from`) | re-request; close on a `null` result or no items |
| selection change without a document change, blur, composition start, or the cursor before `from` | close |

- **When closed**, the popup's key handler returns `false` for every key
  except `Ctrl-Space`. So Enter inserts a newline, Tab and Escape keep
  their current behavior, and `Mod-Enter`, `Mod-Shift-Enter`, `Mod-.`
  and `Shift-Alt-f` are never intercepted, open or closed.
- **Staleness.** Each request gets a sequence number. A result is shown
  only if it is the latest request and the text is unchanged since the
  request. At most one request is in flight. A change during a request
  marks the popup dirty, and one follow-up request runs when the current
  one settles (latest wins).
- **IME.** While `isComposing()` is true, no request starts and no
  result is shown. Composition start closes an open popup.
- **Accept** uses `userEvent 'input.complete'`. `DocumentSync` records
  the change like any edit, and the change does not re-trigger the
  popup.

### 6.4 EditorView adapter and wiring

- `editor/src/code/completion-view.ts` is the only completion file that
  imports `@codemirror/view` and `@codemirror/state`.
  `attachCompletion(view, engine): { dispose(): void }` does two things:
  - It builds an EditorView `CompletionSurface`. It adds its
    extensions to the running view with `StateEffect.appendConfig`:
    - `Prec.highest(keymap.of(...))` for the eight `CompletionKey`s,
      each delegating to the `onKey` handler;
    - an `EditorView.updateListener` that turns each `ViewUpdate` into a
      `SurfaceChange` (`userEvent` from `Transaction.userEvent`,
      `inserted` from `changes.iterChanges`);
    - `EditorView.domEventHandlers` for `blur` and `compositionstart`.

    Its other members read the view: `caretRect` is `view.coordsAtPos`,
    `isComposing` is `view.composing`, `replace` is `view.dispatch`, and
    `popupHost` is the view's owner document body.
  - It creates `CompletionService` and `CompletionPopup` over that
    surface.

  `dispose` removes the panel and detaches the listeners (the
  keymap's `onKey` target becomes inert).
- `editor/src/code/tool-wasm.ts` (new) provides `ToolWasm`, one lazily
  instantiated, import-free `vactr.wasm` instance. It is shared by
  `WasmFormatter` and `WasmCompletionEngine` so that the page holds one
  extra instance, not two. Its promise resets on failure, as
  `WasmFormatter.load` does today.
- Wiring, and nothing else, in files that the canvas plans own:
  - `editor/src/app/deps.ts`: one import and an optional field
    `completion?: CompletionEngine`.
  - `editor/src/app/main.ts`: create one `ToolWasm` for the existing
    `vactr.wasm` URL, pass it to `WasmFormatter`, and set
    `deps.completion = new WasmCompletionEngine(tool)`.
  - `editor/src/code/mount.ts`: one import. After
    `diagnostics.attach(editorView)`, call
    `const completion = deps.completion ? attachCompletion(editorView, deps.completion) : null`,
    and call `completion?.dispose()` in `dispose`.

  Mounts and tests without `deps.completion` are unchanged.
- `app/apis.ts` and `code/language.ts` are not edited.
  `@codemirror/autocomplete` is not added.

### 6.5 Mapping to the 15.3 code-surface contract

| `CompletionSurface` | 15.3 capability (design-implementation 15.3.2, 15.3.3) | EditorView adapter today | Canvas surface later (plan J1) |
|---------------------|------------------------------------------------------|--------------------------|--------------------------------|
| `text()`, `selection()` | state reads | `view.state.doc`, `view.state.selection.main` | headless EditorState reads |
| `replace()` | transaction dispatch (records `DocumentSync.apply` once) | `view.dispatch` | the same dispatch pipeline |
| `onChange()` | change and selection subscriptions | `updateListener` | surface subscriptions |
| `caretRect()` | position-to-rectangle mapping | `coordsAtPos` | the atlas rectangle of the position |
| `onKey()` | input bridge key handling, before shortcut dispatch | `Prec.highest` keymap | the textarea bridge keydown, run before the eval shortcuts |
| `isComposing()`, `onCompositionStart()` | composition state in the input bridge (15.3.3) | `view.composing`, `compositionstart` | bridge composition state |
| `onBlur()` | focus | DOM `blur` | bridge blur |
| `popupHost()` | DOM panels are allowed (15.3.1) | document body | the same DOM panel layer |

Coordinate-to-position mapping is not needed, because the popup's DOM
rows handle their own clicks. At cutover, J1 replaces the single
`attachCompletion(editorView, ...)` line in `mount.ts` with an attach
over its canvas surface. The service and the popup do not change.

### 6.6 Fallback

- If `vactr.wasm` is missing or has no `complete_source`, the service
  becomes `unavailable` after one rejection, and the popup never opens.
- Without `deps.completion`, which is the case in every existing jsdom
  mount, nothing is attached.
- In both cases editing, eval, format, history and highlighting behave
  exactly as they do today.

---

## 7. Tests

Every plan owns committed tests that a gate recognizes: nextest,
`npm test` (vitest) or node. Ad-hoc scripts do not count.

### 7.1 Engine (nextest, `src/complete/tests/`)

- **context.rs.** One or more cases for each context kind: `Head` (the
  line start, after `{`, after `->`); `PipeTarget` (inline `>`, and a
  line-initial `>` continuation); `Argument`; `Keyword`; `PairKey`
  (after an earlier pair, and at a native's arity limit); `Qualified`;
  `None` (a string, a comment, a number, a definition name, directly
  after a pair colon). It also covers group-head `>` as greater-than,
  and interpolation as code.
- **scope.rs:**
  - `fn` positional and keyword params;
  - `->` inline lambda params and `->:` block params;
  - `for` and `match` clause binders;
  - a block `let`/`var` visible after its definition, invisible before
    it, and invisible outside its block;
  - a tab-only cursor line inside a block;
  - shadowing: the inner binding wins, and only one entry per name
    remains.
- **rank.rs:**
  - class order (prefix, then subword, then fuzzy);
  - tier order (local, then document, then prelude) within a class;
  - a stable total order (the same result twice, and ties broken by
    label);
  - the cap: `limit` is honored, `incomplete` is set, and the default
    cap is 100;
  - manifest keywords after `:`, with sounds and synths before controls
    after `s`;
  - seed-sound template controls (`s :analog > cu` gives `cutoff` first);
  - PairKey keys: after an earlier pair in the call, at a native's
    positional arity limit, and the header keys of a document `fn` or
    `inst`;
  - qualified names from a snapshot package.
- **robust.rs:**
  - no panic and no error for every prefix of every committed `.vact`
    at every char boundary;
  - the formatter fixtures;
  - a deterministic xorshift mutation run (about 2000 cases, the
    mutation token set of `fmt/tests/mutation.rs`);
  - a cursor past the end and inside a multi-byte char;
  - an empty text;
  - `complete_bytes` statuses 0, 2 and 3, and JSON that parses;
  - the ignored `budget` test and its debug tripwire (3.7).

### 7.2 LSP (nextest `--features lsp`, `src/lsp/tests/complete.rs`, new)

- A scope-aware local ranks first: in `fn f alpha:\n\tlet beta 1\n\tal`
  at the end, the first item is `alpha`, of kind VARIABLE.
- A context-specific keyword: `s :an` offers `:analog` and no
  non-keyword items. `s :analog > cu` offers `cutoff` first.
- `text_edit` ranges are UTF-16 correct after a line that holds Japanese
  text in a comment.
- `is_incomplete` is set for an empty-prefix `Head`.
- The superset rule of 4.2 holds over a table of documents and cursors.
- The existing tests pass, except the intentional changes documented in
  4.2.
- `src/lsp/tests/mod.rs` declares the new file.

### 7.3 Wasm (vitest node, `editor/test/wasm/complete.test.ts`, new)

- Through `loadVactrWasm()` (which honors `VACTR_WASM`), call
  `complete_source`:
  - for a sample document with the cursor after `\tal` inside
    `fn f alpha:`, the JSON parses, `context` is `head`, the first label
    is `alpha`, and `from`/`to` are the byte offsets;
  - `s :an` gives `:analog`;
  - status 2 for invalid UTF-8, and status 3 for a cursor past the end.
- The session outbox stays empty.

### 7.4 Editor (vitest)

- `editor/test/code/completion.test.ts` (node) uses a fake engine to
  cover:
  - candidate mapping;
  - UTF-16 and UTF-8 conversion with Japanese text before the cursor
    (a comment holding U+65E5 U+672C U+8A9E, and a string holding
    U+97F3) and with a surrogate pair (U+20BB7);
  - `null` for a `none` context;
  - the no-wasm fallback: a rejecting engine gives `null`, the engine is
    called once, and nothing throws.
- `editor/test/code/completion-popup.test.ts` (jsdom) uses a
  `FakeSurface` to cover:
  - auto-trigger on an identifier character, `:` and `.`, and no
    trigger on a space, a paste or `input.complete`;
  - `Ctrl-Space`;
  - arrows and page keys;
  - Enter and Tab accept (the exact `replace` arguments);
  - Escape;
  - mouse accept;
  - keys return `false` when closed;
  - the IME cases: no open while composing, and composition start closes
    the popup;
  - stale results are dropped;
  - closing on selection move and on blur.
- `editor/test/code/completion-view.test.ts` (jsdom) mounts a real
  EditorView with `attachCompletion` and a fake engine, and asserts:
  - typing opens `.vact-completion`;
  - Enter accepts when open and inserts a newline when closed;
  - the adapter binds none of `Mod-Enter`, `Mod-Shift-Enter`, `Mod-.`
    and `Shift-Alt-f`;
  - `dispose` removes the panel;
  - the three completion files other than `completion-view.ts` contain
    no `@codemirror/view` import (a source text check).
- The existing `eval`, `format`, `history`, `syntax` and `highlight`
  tests stay green unchanged.

---

## 8. Delivery waves and path ownership

Fanout manifests list only these tracked paths. They never list
`target/`, `editor/node_modules`, `editor/dist`,
`tree-sitter-vact/node_modules`, `tmp/` or `*.wasm`, and the snapshot
stays under 512 entries. During fanout, each plan scopes its gates to
the files it owns. Workspace-wide gates run at reconcile.

| Wave | Plan | Write paths | Depends on | Owned tests |
|------|------|-------------|------------|-------------|
| 1 | CMP-10 complete-core | `src/complete/**` (3.8), `src/lib.rs` | none | nextest `complete::` |
| 1 | CMP-15 editor-completion-types | `editor/src/code/completion-types.ts`, `editor/test/code/completion-types.test.ts` | none | vitest `completion-types` |
| 1 | FST-50 fmt-space-repair | `src/fmt/{mod.rs,repair.rs}`, `src/fmt/tests/{mod.rs,repair.rs,mutation.rs}`, `src/fmt/tests/fixtures/{space2,space4,space-ambiguous,space-mixed-lines,space-other-error}.{in,out}`, `src/cli/tests/fmt.rs` | none | nextest `fmt::` and `cli::tests::fmt` |
| 1 | EDS-10 syntax-span-core | `editor/src/code/{syntax-core.ts,syntax.ts}`, `editor/test/code/syntax-core.test.ts` | none | vitest `syntax-core` |
| 1 | EDS-11 wasm-format-test-loader | `editor/test/wasm/format.test.ts` | none | vitest `wasm/format` |
| 1 | EDS-12 format-core-tool-wasm | `editor/src/code/{format-core.ts,format.ts,tool-wasm.ts}`, `editor/test/code/{format.test.ts,tool-wasm.test.ts}` | none | vitest `code/format`, `tool-wasm` |
| 2 | CMP-20 complete-lsp | `src/lsp/{analysis.rs,server.rs}`, `src/lsp/tests/{mod.rs,analysis.rs,complete.rs}` | CMP-10 | nextest `--features lsp` `lsp::` |
| 2 | CMP-21 complete-wasm | `src/host/wasm/{mod.rs,complete_abi.rs}`, `editor/test/wasm/complete.test.ts` | CMP-10 | vitest `wasm/complete` |
| 2 | CMP-30 completion-service | `editor/src/code/completion.ts`, `editor/test/code/completion.test.ts` | CMP-15, EDS-12 | vitest `code/completion` |
| 2 | CMP-31 completion-popup | `editor/src/code/completion-popup.ts`, `editor/test/code/completion-popup.test.ts` | CMP-15 | vitest `completion-popup` |
| 3 | CMP-32 completion-view-wiring | `editor/src/code/{completion-view.ts,mount.ts}`, `editor/src/app/{main.ts,deps.ts}`, `editor/test/code/completion-view.test.ts`, `editor/test/wasm/completion-engine.test.ts` | CMP-21, CMP-30, CMP-31, EDS-12 | vitest `completion-view`, `wasm/completion-engine` |
| 4 | CMP-40 closeout | `README.md`, `impl-plans/README.md`, the status lines of this document and `design-formatter-and-syntax.md`, and each of the 12 cmp/eds/fst-50 plan files and `cmp-dispatch.json` in `impl-plans/active/` with its `impl-plans/completed/` destination (files only) | all | the full gate of section 9 |

CMP-32 is the only plan that edits `mount.ts`, `main.ts` and `deps.ts`,
so no two plans share them. CMP-30 and CMP-31 work against fakes of the
CMP-15 interfaces, so they do not wait for CMP-21. The real wasm is
exercised by CMP-21's raw-export test and by CMP-32's
`WasmCompletionEngine` test.

Closeout run (session 234, no design change). Session 226 accepted
every plan except CMP-40, but its CMP-40 dispatch failed because that
manifest entry listed directories, which exceeded the 512-entry fanout
snapshot (tacogips/riela#128). The closeout is dispatched from a new
manifest, `impl-plans/active/cmp-closeout-dispatch.json`:

- `plans` holds CMP-40 only;
- the 11 accepted plans (CMP-10, CMP-15, FST-50, EDS-10, EDS-11,
  EDS-12, CMP-20, CMP-21, CMP-30, CMP-31, CMP-32) are listed in the
  top-level `acceptedDependencies` as `{planId, planPath}`, so they are
  treated as satisfied and are not re-implemented;
- CMP-40's `writePaths` are the concrete files of the row above, and
  its `sharedPaths` are the concrete source, test and fixture files
  changed since `a61fc2e`. No entry is a directory.

`cmp-dispatch.json` is not dispatched again; it is archived with the
plans. `cmp-closeout-dispatch.json` stays in `impl-plans/active/`, like
the earlier `fst-dispatch.json`. The uncommitted working tree is the
accepted implementation. A material integration finding (a gate
failure, a violated rule of this document or of
`design-formatter-and-syntax.md`, or a cross-plan contract mismatch) is
repaired in its owning file with a minimal edit and re-verified; a
finding that needs a design change stops the closeout.

## 9. Verification gate

Every command runs in the foreground and records its exit status and
full log:

- `CARGO_TERM_QUIET=true cargo build`
- `cargo clippy --all-targets -- -D warnings`, with and without
  `--features lsp`
- `cargo fmt --check`
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`,
  with and without `--features lsp` (outside the Codex sandbox)
- `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`,
  then check that the module exports `complete_source`,
  `complete_out_ptr`, `complete_out_len`, `fmt_source` and
  `session_init`
- `cargo nextest run --release --run-ignored only -E 'test(budget)'`
  (the 3.7 evidence; record the median)
- `mise run ts-test`, `mise run lint`
- `cd editor && npm run check && npm test && npm run build`

## References

- design-implementation 14.5 (LSP), 15.1.5 (editor), and 15.3 on
  `origin/main` (the GPU canvas editor).
- `design-formatter-and-syntax.md` 3.7.3 (the wasm ABI style) and 5
  (tree-sitter spans).
- LSP 3.17 `textDocument/completion` (`CompletionList.isIncomplete`,
  `textEdit`, `sortText`), as used through `tower-lsp` 0.20.

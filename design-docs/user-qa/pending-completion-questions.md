# Completion, Span Core and Space-Indent Repair: Pending Decisions

These are the choices that `design-docs/specs/design-completion.md` and
the session-226 amendment of `design-docs/specs/design-formatter-and-syntax.md`
made without an explicit user decision (2026-09-30). The recommendation
for each is followed by default until the user answers.

## C1: Keywords in call-head and argument positions

- **Question**: Today's LSP offers `:bd`, `:analog` and other keywords at
  any position that matches the typed word, even an empty line. Should
  that continue?
- **Options**:
  - (a) Offer keywords only in the `:keyword` context. Typing `:` is a
    trigger character in the LSP and the editor.
  - (b) Also mix keywords into head and argument lists.
- **Recommendation**: (a). A keyword can only be typed after `:`, so
  mixing keywords into other lists is noise. The 100-item cap would
  also push real names out. The one LSP test that expects keywords at an
  empty line is updated with this justification (design-completion
  4.2).

## C2: Signature help

- **Question**: Should this run add `textDocument/signatureHelp`?
- **Recommendation**: Defer it. The engine already resolves the call
  head and the argument index, so signature help can reuse them later.
  Adding it now means a new LSP request variant, a new capability and
  new tests, and the request does not require it. For now, candidate
  `detail` carries the native signature.

## C3: What the wasm tier knows

- **Question**: The browser completion runs in a dedicated wasm
  instance with `Snapshot::builtin()`. That includes the prelude, the
  spec manifest, template names and parameters, plus the document's own
  names, locals and import prefixes. It does not include fetched package
  names or names that exist only in the live session. Is that enough for
  v1?
- **Options**:
  - (a) Builtin snapshot only.
  - (b) Extend the session ABI so the live session pushes its names and
    package names into the completion instance.
- **Recommendation**: (a). It keeps the export pure, and it needs no
  session ABI change. Option (b) can come later as an additive export.

## C4: Space-indent repair gate (refines the requested rule)

- **Question**: The request says that the repair runs "only when every
  reader error is indent-space". However, the reader also reports
  `empty-block` for every block opener whose body lines are
  space-indented (`reader/layout.rs` `block` and `body`). Under the
  literal rule, no space-indented file that contains a block could be
  repaired.
- **Options**:
  - (a) Allow exactly `indent-space` and `empty-block`, require at least
    one `indent-space`, and keep the other safeguards: pure spaces, a
    unit of 2 to 8 that divides every width, and a zero-error re-read.
  - (b) Keep the literal rule, so only files without blocks are
    repaired.
- **Recommendation**: (a). `empty-block` here is caused by the spaces,
  and the zero-error re-read still refuses a file with a real empty
  block. Every other error code still refuses the file
  (design-formatter-and-syntax 3.9).

## C5: Minimum indent unit

- **Question**: Should a file indented with 1 space per level be
  repaired?
- **Recommendation**: No. The unit must be between 2 and 8. A width of 1
  is almost always a stray space, and refusing it means the formatter's
  mutation test (a single inserted space) cannot trigger a repair.

## C6: Popup keys

- **Question**: Which keys drive the completion popup?
- **Recommendation**:
  - `Enter` and `Tab` accept, `Escape` closes, the arrows and PageUp or
    PageDown move the selection, and `Ctrl-Space` opens the popup by
    hand.
  - The popup auto-opens on identifier characters, `:` and `.`.
  - Keys are intercepted only while the popup is open (except
    `Ctrl-Space`), so `Mod-Enter`, `Mod-Shift-Enter`, `Mod-.` and
    `Shift-Alt-f` never change.

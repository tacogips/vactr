# Formatter and Editor Syntax: Pending Decisions

These are the choices that `design-docs/specs/design-formatter-and-syntax.md`
made without an explicit user decision (2026-09-30). The recommendation
for each is followed by default until the user answers.

## F1: Blank-line policy

- **Question**: Should the formatter collapse runs of blank lines, and
  should it drop blank lines at the start of the file?
- **Options**: (a) Keep blank lines inside the file as they are (emptied
  of whitespace) and drop only trailing ones. (b) Collapse each run to
  one blank line and drop leading ones.
- **Recommendation**: (a). The interim LSP formatter already behaves
  this way (design-implementation 14.5.11). It is the minimal change,
  and every committed `.vact` file is already a fixed point under it.

## F2: Missing `tree-sitter-vact.wasm` at `npm run build`

- **Question**: When the grammar wasm has not been built, should the
  editor build fail or continue?
- **Options**: (a) Warn, omit the asset, and let the editor use the
  StreamLanguage fallback. (b) Fail the build, as for `vactr.wasm`.
- **Recommendation**: (a). The fallback is the designed degraded mode,
  and the editor stays buildable without the tree-sitter toolchain. The
  verification gate runs `mise run ts-build-wasm` before the editor
  build, so release builds still carry the asset.

## F3: Format keybinding

- **Question**: Which key formats the document in the editor?
- **Recommendation**: `Shift-Alt-f` (the VS Code convention). It does
  not clash with `defaultKeymap`, `historyKeymap` or the eval keys.

## F4: Files that rely on console-bound import aliases

- **Question**: A document may use a qualified name whose prefix only a
  console `import` in a live session binds. The formatter reads it with
  an empty alias environment (as `ns/load.rs` and the LSP do), so it is
  refused and left unchanged. Is that acceptable?
- **Recommendation**: Yes, for v1. The same document is also a load
  error when read as a file.

## F5: Repository-wide formatting

- **Question**: Should the formatter be applied to the committed `.vact`
  files?
- **Recommendation**: No action. All 10 committed files already appear
  to be fixed points: no trailing blanks, no indented comment lines,
  `>` lines at statement level + 1, and LF with one final newline.
  `mise run fmt-vact-check` confirms this. If it reports a file, the
  file is NOT reformatted in this change. The finding is reported
  instead.

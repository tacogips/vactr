# Pending: code pane font stack, weight and bundling

Decisions for [`design-code-pane-text-rendering.md`](../specs/design-code-pane-text-rendering.md)
(2026-10-16, branch `wf/code-render`).

Both items are open. Until the owner answers, implementation follows the
recommendations: keep the system stack at weight 400 and bundle no font.

## CR1. Font family and whether to bundle a monospace web font

**Question.** The code pane uses `13px ui-monospace, SFMono-Regular,
Menlo, monospace` (`editor/src/code/mount.ts:84`). The "dirty" look was
caused by rendering defects (RC4 premultiplication, RC5 unsnapped and
rescaled quads), not by the font. Should the editor keep the system stack,
or bundle a specific monospace web font?

- (a) **Recommended.** Keep the system stack. Do not bundle a font.
  - On macOS and iPadOS, `ui-monospace` resolves to SF Mono, which is what
    DOM text in the same app already uses. The canvas will match the
    surrounding UI after the fix.
  - No license file, no download weight, no font-loading delay or atlas
    invalidation at startup.
  - The pixel test compares the canvas against DOM text in the same
    resolved font, so it does not need a bundled font to be
    deterministic.
  - Cost: glyph shapes and advances differ across platforms (Menlo or the
    generic `monospace` on systems without SF Mono).
- (b) Bundle JetBrains Mono (SIL Open Font License 1.1), Regular only, as
  woff2.
  - Identical shapes and metrics on every platform and in the native
    shell.
  - Cost: about 90-110 KB per weight; the OFL text and copyright notice
    must be committed and listed in the third-party notices; the font
    load must finish (or invalidate the atlas, as `fontsChanged` already
    does) before the first stable frame.
- (c) Bundle IBM Plex Mono (SIL Open Font License 1.1), Regular only.
  Same trade-offs as (b), with a narrower, more traditional design.

No font is added unless (b) or (c) is chosen, and then only with its
license file and notice recorded in the same change.

## CR2. Font weight

**Question.** Should the code pane change its weight (currently the
default 400) to compensate for the heavy look?

- (a) **Recommended.** Keep 400. The heaviness came from straight-alpha
  output under a premultiplied blend (RC4), which thickened every glyph
  edge. Once R10 is fixed, the canvas weight matches DOM text at 400.
  Revisit only if the after-screenshots in `tmp/code-render/` still look
  heavier than DOM text.
- (b) Use a lighter weight (300). Rejected for now: SF Mono Light and
  Menlo do not have matching weights on every platform, and a lighter
  weight would hide a rendering defect instead of fixing it.

# emaki

A framework-independent canvas editor with virtualized text, incremental languages, decorations and inline widgets.

## Installation

Install `emaki` as `file:../emaki` or `github:tacogips/emaki#<sha>`. Its exports point to TypeScript source, so the consumer's bundler compiles it without a package build step. Install the exact peer versions `@codemirror/state@6.7.6` and `@codemirror/commands@6.11.1`.

Configure Vite with `resolve.dedupe: ['@codemirror/state', '@codemirror/commands']` and allow the package directory in `server.fs.allow` when using a local link. Configure Vitest with the same dedupe and `test.server.deps.inline: ['emaki']`. Set `preserveSymlinks: true` in TypeScript. These settings ensure one CodeMirror instance with local and Git installs.

## Quick start

```ts
import { createEditor } from 'emaki';
import { jsonLanguage } from 'emaki/json';

const editor = createEditor(document.getElementById('editor')!, {
  doc: '{"hello": true}', language: jsonLanguage,
});
// Later: editor.destroy();
```

Give the host a defined width and height. `createEditor` accepts `doc`, `language`, `plugins`, `theme` (palette and token colors), `font` (family, size, lineHeight, baseline), `readOnly`, `lineNumbers` (default true), CodeMirror `extensions`, and `modalKeys`. `gl`, `frameHost`, and `createCanvas` allow test injection. The default font is 13px monospace, with 18px line height and 14px baseline.

The returned handle exposes `state`, `dispatch`, `undo`, `redo`, `setDoc`, `onUpdate`, `onScroll`, `scrollTop`, `scrollTo({top})` or `scrollTo({pos})`, `lineTop`, `lineAtHeight`, `addPlugin`, `focus`, `requestFrame`, `revision`, `stats`, `perf`, and `destroy`. Subscriptions and `addPlugin` return removers. `destroy` is idempotent.

## Document and history

`state` is the current CodeMirror `EditorState`; `dispatch` accepts a transaction spec. `undo` and `redo` use the editor's bounded history. `setDoc` replaces the document without adding that replacement to history. Read-only mode rejects user input and deletion while allowing selection and programmatic dispatch.

## Languages

A line-kind `LanguageProvider` supplies `startState`, `tokenizeLine`, and `sameState`; the engine's `IncrementalHighlighter` uses checkpoints and visible-first tokenization. A span-kind provider supplies `spans(state, from, to, limit)` and may receive changes, an invalidation callback and disposal. Tags map to theme token colors. `emaki/json` exports `jsonLanguage`; `emaki/engine` exports the highlighter and composition blocks.

## Decorations and precedence

Plugins set marks by owner with ranges and styles: color, background, font weight/style, underline, strike, opacity and glow. Animated marks support only quad-based styles (background, glow, underline, strike and labels), keeping text shaping unchanged. Line formats provide background, color, weight/style, opacity, a left bar and gutter marker/background. Both map through edits.

Within a category later entries win. Text color precedence is default, syntax, line, then mark; font weight/style is default, line, then mark. Line and mark opacity multiply. Background order is editor, line, static mark, selection, animated mark, glow. Overlay order is static underline/strike, composition, cursor, static labels, hop labels, touch handles, animated labels. Gutter order is background, line numbers, markers; the left bar sits at the text edge.

## Plugins and widgets

A plugin has `name` and `attach(host)`, returning optional `docChanged`, `viewportChanged`, `frame`, and `destroy` hooks. Hooks run in registration order. A thrown hook is isolated and the plugin is removed with its owners and animated providers. The read-only host supplies `state`, `marks`, `lineFormats`, `animatedMarks`, `widgets`, `requestFrame`, `setActive`, `coordsAtPos`, `lineTop`, `viewport`, `theme`, and `reportError`; it cannot edit the document.

Block widgets anchor above or below a line with a chosen height. End-of-line widgets have a width and do not change line height. Content is either DOM (`mount` returns cleanup) or canvas (`draw` returns whether animation should continue). Anchors map through changes; deleted anchors and removed owners unmount. Widgets mount near the viewport and unmount farther away with hysteresis; canvas draws occur only while visible. Interactive widgets receive pointer events; other widgets let editor hit testing pass through. Caret movement skips block space.

## Composition and diff

`emaki/engine` exposes the core, layout, render, input, view, decoration, plugin and language building blocks for custom hosts. `emaki/diff` exports reusable line and word diff and split-alignment functions.

## Scripts and metrics

`npm run dev` serves the demo pages. `npm run check`, `npm test`, `npm run build`, `npm run perf`, and `npm run e2e` check types, tests, demos, performance and browser behavior respectively.

`perf.onPresent` reports frame start/end, work time, revision and whether visible highlighting is complete. `perf.onInput` reports event timestamp, capture-to-bubble work and revision after each keydown or beforeinput. First paint measures `setDoc` to the first complete visible frame. Scroll measures frame work p95; keystroke metrics record raw keydown-to-present and gate input plus frame work and next-frame presentation. Diff latency measures keydown to both sides presenting the revision. Scaling compares the 1k, 10k and 100k fixtures; rAF intervals are informational.

## License

MIT.

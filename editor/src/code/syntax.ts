import { EditorState, type Extension } from '@codemirror/state';
import { Decoration, type DecorationSet, EditorView, ViewPlugin } from '@codemirror/view';
import { styleSpans, type ParsedVact, type VactSyntax } from './syntax-core';

export { CAPTURE_CLASSES, createVactSyntax, loadVactSyntax } from './syntax-core';
export type { ParsedVact, StyleSpan, SyntaxCapture, SyntaxLoader, VactSyntax } from './syntax-core';

function decorations(view: EditorView, parsed: ParsedVact): DecorationSet {
  const ranges: { from: number; to: number; decoration: Decoration }[] = [];
  const seen = new Set<string>();
  for (const visible of view.visibleRanges) {
    for (const span of styleSpans(parsed, visible.from, visible.to)) {
      const key = `${span.from}:${span.to}`;
      if (seen.has(key)) continue;
      seen.add(key);
      ranges.push({
        from: span.from,
        to: span.to,
        decoration: Decoration.mark({ class: span.cls }),
      });
    }
  }
  ranges.sort((a, b) => a.from - b.from || a.to - b.to);
  return Decoration.set(ranges.map(({ from, to, decoration }) => decoration.range(from, to)), true);
}

export function treeSitterHighlighting(syntax: VactSyntax): Extension {
  class HighlightPlugin {
    parsed: ParsedVact;
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.parsed = syntax.parse(view.state.doc.toString());
      this.decorations = decorations(view, this.parsed);
    }

    update(update: { view: EditorView; docChanged: boolean; viewportChanged: boolean }): void {
      if (update.docChanged) {
        this.parsed.delete();
        this.parsed = syntax.parse(update.view.state.doc.toString());
      }
      if (update.docChanged || update.viewportChanged) {
        this.decorations = decorations(update.view, this.parsed);
      }
    }

    destroy(): void {
      this.parsed.delete();
    }
  }

  const plugin = ViewPlugin.fromClass(HighlightPlugin, {
    decorations: (value) => value.decorations,
  });
  return [plugin, EditorState.languageData.of(() => [{ commentTokens: { line: '#' } }])];
}

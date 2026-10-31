import type { EditorCore } from './core/editor-core';
import type { DecorationStore } from './decor/store';
import type { InputController } from './input/input';
import type { SelectionHandle } from './input/pointer';
import type { IncrementalHighlighter } from './language/incremental';
import type { TextLayout } from './layout/layout';
import type { PluginRuntime } from './plugin/runtime';
import type { WidgetSet } from './plugin/widgets';
import type { FrameScheduler } from './render/frame';
import type { CanvasRenderer } from './render/renderer';
import type { LanguageProvider } from './types';
import type { ViewHost } from './view/view-host';
import type { WidgetLayer } from './view/widget-layer';

export interface FrameParts {
  core: EditorCore; layout: TextLayout; renderer: CanvasRenderer; view: ViewHost; input: InputController;
  widgets: WidgetSet; layer: WidgetLayer; store: DecorationStore; runtime: PluginRuntime;
  language?: LanguageProvider; highlighter?: IncrementalHighlighter<unknown>; scheduler: FrameScheduler;
  handles: readonly SelectionHandle[]; revision: number; frameMs: number; dpr: number; widgetRevision: number;
}
export function renderEditorFrame(p: FrameParts): { rendered: boolean; visibleComplete: boolean; widgetRevision: number } {
  p.input.flushBridge();
  const doc = p.core.state.doc;
  if (p.widgetRevision !== p.widgets.revision) {
    p.layout.heightmap.setBlocks(p.widgets.entries(doc)); p.widgetRevision = p.widgets.revision;
  }
  const view = p.view.viewport;
  const first = p.layout.heightmap.lineAtHeight(Math.max(0, view.scrollTop - view.height)).line;
  const last = Math.min(doc.lines, p.layout.heightmap.lineAtHeight(view.scrollTop + view.height * 2).line + 1);
  p.runtime.viewportChanged({ fromLine: first, toLine: last, top: view.scrollTop, height: view.height });
  const activePlugins = p.runtime.frame(p.frameMs);
  if (p.widgetRevision !== p.widgets.revision) {
    p.layout.heightmap.setBlocks(p.widgets.entries(doc)); p.widgetRevision = p.widgets.revision;
  }
  const decor = p.store.collect(p.frameMs, doc);
  let tokens: readonly import('./types').TokenSpan[] = []; let visibleComplete = true;
  if (p.highlighter) { tokens = p.highlighter.tokens(first, last); visibleComplete = !p.highlighter.approximate; }
  else if (p.language?.kind === 'spans') {
    const result = p.language.spans(p.core.state, doc.line(first + 1).from, doc.line(Math.max(1, last)).to, 16384);
    tokens = result.spans; visibleComplete = !result.truncated;
  }
  const presentation = p.input.presentation; const selection = p.core.state.selection.main;
  p.renderer.setViewport(view, p.dpr);
  const rendered = p.renderer.render({ textRevision: p.revision, decorationsKey: `${decor.key}:${selection.anchor}:${selection.head}:${p.revision}`,
    tokens, selection: selection.empty ? [] : [{ from: selection.from, to: selection.to }],
    composition: presentation.annotations, marks: decor.marks, animatedMarks: decor.animatedMarks,
    lineFormats: decor.lineFormats, cursor: presentation.cursor, cursorVisible: true, handles: p.handles });
  const activeWidgets = p.layer.update({ view, dpr: p.dpr, frameMs: p.frameMs, blocks: p.widgets.blocks(doc), eols: p.widgets.eols(doc) }).animating;
  const catchingUp = p.highlighter?.catchUp() ?? false;
  p.scheduler.setActive('plugins', activePlugins); p.scheduler.setActive('widgets', activeWidgets); p.scheduler.setActive('highlighter', catchingUp);
  if (p.renderer.textPending || (p.highlighter && !visibleComplete && !catchingUp)) p.scheduler.request();
  return { rendered, visibleComplete: visibleComplete && !p.renderer.textPending, widgetRevision: p.widgetRevision };
}

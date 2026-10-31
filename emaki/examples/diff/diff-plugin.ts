import type { BlockWidgetSpec, EditorHandle, LineFormat, Mark, PluginHost, Rgba } from '../../src/index';
import { alignSplit, diffLines, diffWords, type SplitLayout } from '../../src/diff/index';

export interface DiffStats {
  diffRevision: number; rightRevision: number; removed: number; added: number; modified: number;
  fillersLeft: number; fillersRight: number; fillerRowsLeft: number; fillerRowsRight: number;
  wordMarksLeft: number; wordMarksRight: number;
}
export interface DiffPair {
  readonly stats: DiffStats;
  onDecorated(cb: (revision: number) => void): () => void;
  dispose(): void;
}
export function createDiffPair({ left, right, colors }: { left: EditorHandle; right: EditorHandle;
  colors: { add: Rgba; del: Rgba; addStrong: Rgba; delStrong: Rgba } }): DiffPair {
  let leftHost: PluginHost | undefined, rightHost: PluginHost | undefined;
  let dirty = true, syncing = false, layout: SplitLayout = { removed: [], added: [], modified: [], fillers: [] };
  let current: DiffStats = { diffRevision: 0, rightRevision: 0, removed: 0, added: 0, modified: 0,
    fillersLeft: 0, fillersRight: 0, fillerRowsLeft: 0, fillerRowsRight: 0, wordMarksLeft: 0, wordMarksRight: 0 };
  const listeners = new Set<(revision: number) => void>();
  const hatch = (ctx: CanvasRenderingContext2D, info: { width: number; height: number }) => {
    ctx.fillStyle = `rgba(${colors.del[0] * 255},${colors.del[1] * 255},${colors.del[2] * 255},${colors.del[3]})`;
    ctx.fillRect(0, 0, info.width, info.height);
    ctx.strokeStyle = `rgba(${colors.delStrong[0] * 255},${colors.delStrong[1] * 255},${colors.delStrong[2] * 255},${colors.delStrong[3]})`;
    ctx.lineWidth = 1; ctx.beginPath();
    for (let x = -info.height; x < info.width; x += 8) { ctx.moveTo(x, info.height); ctx.lineTo(x + info.height, 0); }
    ctx.stroke(); return false;
  };
  const applyMarks = () => {
    if (!leftHost || !rightHost) return;
    const lm: Mark[] = [], rm: Mark[] = [];
    const lv = leftHost.viewport, rv = rightHost.viewport;
    const visible = (n: number, v: typeof lv) => n >= v.fromLine - (v.toLine - v.fromLine) && n < v.toLine + (v.toLine - v.fromLine);
    for (const pair of layout.modified) {
      if (pair.a >= leftHost.state.doc.lines || pair.b >= rightHost.state.doc.lines) continue;
      if (!visible(pair.a, lv) && !visible(pair.b, rv)) continue;
      const a = leftHost.state.doc.line(pair.a + 1), b = rightHost.state.doc.line(pair.b + 1);
      const words = diffWords(a.text, b.text);
      for (const r of words.a) lm.push({ from: a.from + r.from, to: a.from + r.to, style: { background: colors.delStrong } });
      for (const r of words.b) rm.push({ from: b.from + r.from, to: b.from + r.to, style: { background: colors.addStrong } });
    }
    leftHost.marks('diff').set(lm); rightHost.marks('diff').set(rm);
    current = { ...current, wordMarksLeft: lm.length, wordMarksRight: rm.length };
  };
  const recompute = () => {
    if (!leftHost || !rightHost) return;
    const a = leftHost.state.doc.toJSON(), b = rightHost.state.doc.toJSON();
    layout = alignSplit(diffLines(a, b), a.length, b.length);
    const formats = (host: PluginHost, lines: number[], color: Rgba, marker: string): LineFormat[] => lines.map(n => ({
      pos: host.state.doc.line(n + 1).from, style: { background: color, gutterMarker: { text: marker, color } },
    }));
    leftHost.lineFormats('diff').set(formats(leftHost, [...layout.removed, ...layout.modified.map(p => p.a)], colors.del, '-'));
    rightHost.lineFormats('diff').set(formats(rightHost, [...layout.added, ...layout.modified.map(p => p.b)], colors.add, '+'));
    const fillers = (host: PluginHost, side: 'a' | 'b'): BlockWidgetSpec[] => layout.fillers.filter(f => f.side === side).map(f => ({
      key: `fill:${side}:${f.afterLine}`, pos: host.state.doc.line(Math.max(0, f.afterLine) + 1).from,
      side: f.afterLine < 0 ? 'above' : 'below', height: f.count * 18, content: { kind: 'canvas', draw: hatch },
    }));
    leftHost.widgets('diff').set(fillers(leftHost, 'a')); rightHost.widgets('diff').set(fillers(rightHost, 'b'));
    const count = (side: 'a' | 'b') => layout.fillers.filter(f => f.side === side);
    const lf = count('a'), rf = count('b');
    current = { ...current, diffRevision: current.diffRevision + 1, rightRevision: right.revision,
      removed: layout.removed.length, added: layout.added.length, modified: layout.modified.length,
      fillersLeft: lf.length, fillersRight: rf.length, fillerRowsLeft: lf.reduce((n, f) => n + f.count, 0), fillerRowsRight: rf.reduce((n, f) => n + f.count, 0) };
    applyMarks();
    for (const cb of listeners) cb(current.rightRevision);
  };
  const scroll = (target: EditorHandle) => (top: number) => { if (syncing || target.scrollTop === top) return; syncing = true; try { target.scrollTo({ top }); } finally { syncing = false; } };
  const toRight = scroll(right), toLeft = scroll(left);
  const offLeft = left.addPlugin({ name: 'diff-left', attach(host) { leftHost = host; return { viewportChanged(v) { applyMarks(); toRight(v.top); } }; } });
  const offRight = right.addPlugin({ name: 'diff-right', attach(host) { rightHost = host; return {
    docChanged() { dirty = true; host.requestFrame(); }, viewportChanged(v) { applyMarks(); toLeft(v.top); },
    frame() { if (dirty) { dirty = false; recompute(); } return false; },
  }; } });
  const offScrollLeft = left.onScroll(toRight), offScrollRight = right.onScroll(toLeft);
  right.requestFrame();
  return { get stats() { return current; }, onDecorated(cb) { listeners.add(cb); return () => listeners.delete(cb); },
    dispose() { offScrollLeft(); offScrollRight(); offLeft(); offRight(); listeners.clear(); } };
}

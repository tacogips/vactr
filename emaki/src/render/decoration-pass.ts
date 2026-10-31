import type { TextLayout, LayoutViewport, ShapedLine } from '../layout/layout';
import type { Rect, Range } from '../types';

export function animatedRects(layout: TextLayout, view: LayoutViewport, lines: readonly ShapedLine[], range: Range): Rect[] {
  if (!lines.length) return [];
  const visibleFrom = lines[0]!.from, visibleTo = lines[lines.length - 1]!.to;
  const from = range.from >= visibleFrom && range.from <= visibleTo ? layout.boundary(range.from) : range.from;
  const to = range.to >= visibleFrom && range.to <= visibleTo ? layout.boundary(range.to, 1) : range.to;
  const out: Rect[] = [];
  for (const line of lines) {
    if (line.to < from || line.from > to) continue;
    const start = Math.max(from, line.from), end = Math.min(to, line.to);
    const left = Math.max((view.left ?? 0) + (view.gutter ?? 48), (view.left ?? 0) + (view.gutter ?? 48) + layout.advance(line, start) - view.scrollLeft);
    const endX = (view.left ?? 0) + (view.gutter ?? 48) + layout.advance(line, end) - view.scrollLeft + (to > line.to ? 8 : 0);
    const right = Math.min((view.left ?? 0) + view.width, endX);
    const top = (view.top ?? 0) + layout.heightmap.lineTop(line.number) - view.scrollTop;
    if (right >= left) out.push({ left, right: Math.max(left + 1, right), top, bottom: top + layout.font.lineHeight });
  }
  return out;
}

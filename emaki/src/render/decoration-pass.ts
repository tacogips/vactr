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

import type { RenderInput } from './types';
import type { Palette } from './palette';
import { rgbaCss } from './palette';
import type { GeometryLayer, InstanceWriter } from './geometry';
import type { ShapedRun } from '../layout/layout';
import { cursorColor, cursorRect } from './cursor-shape';

export interface DecorationContext {
  layout: TextLayout; view: LayoutViewport; visibleLines: ShapedLine[]; palette: Palette;
  currentLayer: GeometryLayer; writers: Map<GeometryLayer, InstanceWriter>;
  backgroundStaticCount: number; overlayStaticCount: number; overlayTextStaticCount: number;
  quad(rect: Rect, color: number[]): void; local(rect: Rect): Rect;
  labelRun(text: string, width: number): ShapedRun;
  drawRun(run: ShapedRun, x: number, y: number, styles: readonly import('../types').TokenSpan[], color?: string, gutter?: boolean): void;
}

export function drawDecorations(this: DecorationContext, feedback: RenderInput, backgroundStaticDirty: boolean, overlayDirty: boolean): void {
      this.currentLayer = 'background';
      if (backgroundStaticDirty) {
        for (const mark of feedback.marks ?? []) if (mark.style.background) {
          for (const rect of this.layout.rangeRects(mark, this.view)) {
            const r = mark.from === mark.to && mark.style.minWidth ? { ...rect, right: rect.left + mark.style.minWidth } : rect;
            this.quad(this.local(r), mark.style.background);
          }
        }
        for (const range of feedback.selection ?? []) for (const r of this.layout.rangeRects(range, this.view)) this.quad(this.local(r), this.palette.selection);
        this.backgroundStaticCount = this.writers.get('background')!.count;
      }
      for (const mark of feedback.animatedMarks ?? []) if (mark.style.background) {
        for (const r of animatedRects(this.layout, this.view, this.visibleLines, mark)) this.quad(this.local(r), mark.style.background);
      }
      this.currentLayer = 'overlay';
      if (overlayDirty) {
        const underlines = [
          ...(feedback.marks ?? []).filter(mark => mark.style.underline).map(mark => ({ from: mark.from, to: mark.to, color: mark.style.underline!.color, thickness: mark.style.underline!.thickness, order: 0 })),
          ...(feedback.composition ?? []).map(range => ({ ...range, color: this.palette.composition, thickness: 2, order: 1 })),
        ].sort((a, b) => a.from - b.from || a.order - b.order);
        for (const a of underlines) for (const r of this.layout.rangeRects(a, this.view)) this.quad(this.local({ ...r, top: r.bottom - a.thickness }), a.color);
      }
      if (overlayDirty && feedback.cursorVisible !== false && feedback.cursor != null) {
        const shape = feedback.cursorShape ?? 'bar'; const r = this.layout.coordsAtPos(feedback.cursor, this.view);
        const next = shape !== 'bar' && feedback.cursorEnd != null ? this.layout.coordsAtPos(feedback.cursorEnd, this.view) : null;
        if (r) this.quad(this.local(cursorRect(shape, r, next)), cursorColor(shape, this.palette.cursor));
      }
      if (overlayDirty) for (const mark of feedback.marks ?? []) {
        const label = mark.style.label; if (!label) continue;
        const r = this.layout.coordsAtPos(mark.to, this.view); if (!r) continue;
        const x = r.left - (this.view.left ?? 0) + 3, y = r.top - (this.view.top ?? 0);
        const width = Math.min(256, Math.max(16, label.text.length * 8 + 8));
        this.quad({ left: x, right: x + width, top: y, bottom: y + this.layout.font.lineHeight }, label.fill ?? this.palette.labelFill);
        this.currentLayer = 'overlayText'; this.drawRun(this.labelRun(label.text, width), x + 4, y, [], rgbaCss(label.color ?? this.palette.labelText)); this.currentLayer = 'overlay';
      }
      if (overlayDirty) for (const label of feedback.hopLabels ?? []) {
        const r = this.layout.coordsAtPos(label.pos, this.view); if (!r) continue;
        const x = r.left - (this.view.left ?? 0), y = r.top - (this.view.top ?? 0), width = Math.max(8, label.text.length * 8 + 4);
        this.quad({ left: x, right: x + width, top: y, bottom: y + this.layout.font.lineHeight }, this.palette.labelFill);
        this.currentLayer = 'overlayText'; this.drawRun(this.labelRun(label.text, width), x + 2, y, [], rgbaCss(this.palette.labelText)); this.currentLayer = 'overlay';
      }
      if (overlayDirty) for (const handle of feedback.handles ?? []) {
        const r = this.layout.coordsAtPos(handle.pos, this.view); if (!r) continue;
        const x = r.left - (this.view.left ?? 0), y = (handle.end ? r.bottom : r.top) - (this.view.top ?? 0);
        this.quad({ left: x - 5, right: x + 5, top: y - 4, bottom: y + 6 }, this.palette.handle);
      }
      if (overlayDirty) { this.overlayStaticCount = this.writers.get('overlay')!.count; this.overlayTextStaticCount = this.writers.get('overlayText')!.count; }
      for (const mark of feedback.animatedMarks ?? []) {
        const label = mark.style.label; if (!label) continue;
        const r = this.layout.coordsAtPos(mark.to, this.view); if (!r) continue;
        const x = r.left - (this.view.left ?? 0) + 3, y = r.top - (this.view.top ?? 0);
        const width = Math.min(256, Math.max(16, label.text.length * 8 + 8));
        this.currentLayer = 'overlay'; this.quad({ left: x, right: x + width, top: y, bottom: y + this.layout.font.lineHeight }, label.fill ?? this.palette.labelFill);
        this.currentLayer = 'overlayText'; this.drawRun(this.labelRun(label.text, width), x + 4, y, [], rgbaCss(label.color ?? this.palette.labelText));
      }
}

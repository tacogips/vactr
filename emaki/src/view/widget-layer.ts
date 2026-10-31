import type { TextLayout, LayoutViewport } from '../layout/layout';
import type { ResourceLedger, Reservation } from '../render/resources';
import type { ResolvedBlockWidget, ResolvedEolWidget } from '../plugin/widgets';
import type { WidgetContent } from '../types';

type Widget = ResolvedBlockWidget | ResolvedEolWidget;
interface Mounted { spec: Widget; wrapper: HTMLDivElement; content: WidgetContent; cleanup?: () => void; canvas?: HTMLCanvasElement; ctx?: CanvasRenderingContext2D; reservation?: Reservation; width: number; height: number; dpr: number; animate: boolean; dirty: boolean; transform: string }
export class WidgetLayer {
  private container: HTMLDivElement;
  private mounted = new Map<string, Mounted>();
  constructor(host: HTMLElement, private layout: TextLayout, private ledger: ResourceLedger) {
    this.container = document.createElement('div'); this.container.className = 'emaki-widget-layer';
    Object.assign(this.container.style, { position: 'absolute', pointerEvents: 'none', overflow: 'hidden' }); host.appendChild(this.container);
  }
  private unmount(id: string): void {
    const item = this.mounted.get(id); if (!item) return;
    this.mounted.delete(id); try { item.cleanup?.(); } finally { item.reservation?.release(); item.wrapper.remove(); }
  }
  removeAll(): void { for (const id of [...this.mounted.keys()]) this.unmount(id); }
  dispose(): void { this.removeAll(); this.container.remove(); }
  update({ view, dpr, frameMs, blocks, eols }: { view: LayoutViewport; dpr: number; frameMs: number; blocks: readonly ResolvedBlockWidget[]; eols: readonly ResolvedEolWidget[] }): { animating: boolean } {
    const gutter = view.gutter ?? 48, width = Math.max(0, view.width - gutter), scroll = view.scrollTop;
    Object.assign(this.container.style, { left: `${gutter}px`, top: '0px', width: `${width}px`, height: `${view.height}px`, font: this.layout.font.font });
    const ids = new Set([...blocks, ...eols].map(w => w.id));
    for (const id of this.mounted.keys()) if (!ids.has(id)) this.unmount(id);
    const offsets = new Map<string, number>(); let animating = false;
    const process = (widget: Widget, x: number, top: number, w: number, h: number, eol = false) => {
      const near = top + h >= scroll - view.height && top <= scroll + 2 * view.height;
      const retained = top + h >= scroll - 2 * view.height && top <= scroll + 3 * view.height;
      const visible = top + h > scroll && top < scroll + view.height;
      let item = this.mounted.get(widget.id);
      if (item && (!retained || item.content.kind !== widget.content.kind || (widget.content.kind === 'dom' && item.content !== widget.content) || (item.canvas && (item.width !== w || item.height !== h || item.dpr !== dpr)))) { this.unmount(widget.id); item = undefined; }
      if (!item && near) {
        const wrapper = document.createElement('div'); wrapper.className = 'emaki-widget'; wrapper.style.position = 'absolute';
        item = { spec: { ...widget }, wrapper, content: widget.content, width: w, height: h, dpr, animate: true, dirty: true, transform: '' };
        if (widget.content.kind === 'canvas') {
          const pw = Math.ceil(w * dpr), ph = Math.ceil(h * dpr);
          const reservation = this.ledger.allocate('canvas', pw * ph * 4, { width: pw, height: ph });
          if (!reservation) return;
          const canvas = document.createElement('canvas'); canvas.width = pw; canvas.height = ph; canvas.style.width = `${w}px`; canvas.style.height = `${h}px`;
          const ctx = canvas.getContext('2d'); if (!ctx) { reservation.release(); return; }
          ctx.scale(dpr, dpr); wrapper.appendChild(canvas); item.canvas = canvas; item.ctx = ctx; item.reservation = reservation;
        } else item.cleanup = widget.content.mount(wrapper);
        this.container.appendChild(wrapper); this.mounted.set(widget.id, item);
      }
      if (!item) return;
      if (Object.keys(widget).some(key => (widget as unknown as Record<string, unknown>)[key] !== (item!.spec as unknown as Record<string, unknown>)[key])) item.dirty = true;
      item.spec = { ...widget }; item.content = widget.content;
      item.wrapper.style.pointerEvents = widget.interactive ? 'auto' : 'none';
      item.wrapper.style.width = `${w}px`; item.wrapper.style.height = `${h}px`;
      if (widget.label) { item.wrapper.removeAttribute('aria-hidden'); item.wrapper.setAttribute('role', 'group'); item.wrapper.setAttribute('aria-label', widget.label); }
      else { item.wrapper.setAttribute('aria-hidden', 'true'); item.wrapper.removeAttribute('role'); item.wrapper.removeAttribute('aria-label'); }
      const transform = `translate(${eol ? `calc(${x}px + 1ch)` : `${x}px`}, ${top - scroll}px)`; if (item.transform !== transform) { item.wrapper.style.transform = transform; item.transform = transform; }
      if (visible && item.ctx && widget.content.kind === 'canvas' && (item.animate || item.dirty)) {
        item.ctx.clearRect(0, 0, w, h); item.animate = widget.content.draw(item.ctx, { width: w, height: h, dpr, frameMs }); item.dirty = false;
      }
      if (visible && item.canvas && item.animate) animating = true;
    };
    for (const block of blocks) {
      const key = `${block.line}:${block.side}`, offset = offsets.get(key) ?? 0; offsets.set(key, offset + block.height);
      const top = block.side === 'above' ? this.layout.heightmap.lineTop(block.line) - this.layout.heightmap.blockAbove(block.line) + offset : this.layout.heightmap.lineTop(block.line) + this.layout.font.lineHeight + offset;
      process(block, 0, top, width, block.height);
    }
    for (const eol of eols) {
      const line = this.layout.shape(eol.line), rect = this.layout.coordsAtPos(line.to, view);
      if (rect) process(eol, rect.right - (view.left ?? 0) - gutter, this.layout.heightmap.lineTop(eol.line), eol.width, this.layout.font.lineHeight, true);
    }
    return { animating };
  }
}

// The visual output panes o0..o3 (design 9.3, 15.1.8). Four 2D canvases
// show the outputs' latest frames: for each visible pane the render host
// blits the output into its own (offscreen) WebGL canvas, which the pane
// then draws scaled. `render oN` shows one output and `render` tiles all
// four; the session does not publish that choice in v1, so the pane
// selector sets it. A `shader-compile` host diagnostic shows as a banner
// until that output accepts a program again.

import type { OutputIndex } from '../protocol/types';
import { OUTPUTS, type HostDiagnostic } from './render-host';

export type PaneSelection = OutputIndex | 'tile';

export const PANE_WIDTH = 320;
export const PANE_HEIGHT = 180;

/** What a pane needs from the render host. */
export interface Presenter {
  present(out: OutputIndex): void;
}

export interface VisualPanesOptions {
  /** The render host's canvas; absent shows `notice` instead of panes. */
  source?: CanvasImageSource & { width: number; height: number };
  notice?: string;
  selection?: PaneSelection;
}

interface Pane {
  el: HTMLElement;
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D | null;
}

export class VisualPanes {
  readonly el: HTMLElement;
  private readonly panes: Pane[] = [];
  private readonly banners = new Map<OutputIndex, string>();
  private readonly banner: HTMLElement;
  private readonly selector: HTMLSelectElement;
  private readonly source: VisualPanesOptions['source'];
  private sel: PaneSelection;

  constructor(parent: HTMLElement, opts: VisualPanesOptions = {}) {
    const doc = parent.ownerDocument;
    this.source = opts.source;
    this.sel = opts.selection ?? 0;
    this.el = doc.createElement('section');
    this.el.className = 'visual-panes';
    this.el.dataset.area = 'visual';

    const header = doc.createElement('div');
    header.className = 'visual-header';
    const title = doc.createElement('span');
    title.className = 'visual-title';
    title.textContent = 'Visuals';
    this.selector = doc.createElement('select');
    this.selector.className = 'visual-select';
    this.selector.dataset.role = 'pane-select';
    for (const value of ['o0', 'o1', 'o2', 'o3', 'tile']) {
      const opt = doc.createElement('option');
      opt.value = value;
      opt.textContent = value === 'tile' ? 'tile o0..o3' : value;
      this.selector.appendChild(opt);
    }
    this.selector.addEventListener('change', this.onSelect);
    header.append(title, this.selector);

    this.banner = doc.createElement('div');
    this.banner.className = 'visual-banner';
    this.banner.dataset.role = 'diagnostic';
    this.banner.hidden = true;

    const grid = doc.createElement('div');
    grid.className = 'visual-grid';
    this.el.append(header, this.banner, grid);

    if (!this.source) {
      this.selector.disabled = true;
      const notice = doc.createElement('div');
      notice.className = 'visual-notice';
      notice.dataset.role = 'notice';
      notice.textContent = opts.notice ?? 'visuals not available';
      grid.appendChild(notice);
    } else {
      for (const out of OUTPUTS) {
        const el = doc.createElement('div');
        el.className = 'visual-pane';
        el.dataset.output = `o${out}`;
        const canvas = doc.createElement('canvas');
        canvas.width = PANE_WIDTH;
        canvas.height = PANE_HEIGHT;
        const label = doc.createElement('span');
        label.className = 'visual-pane-label';
        label.textContent = `o${out}`;
        el.append(canvas, label);
        grid.appendChild(el);
        this.panes.push({ el, canvas, ctx: canvas.getContext('2d') });
      }
    }
    parent.appendChild(this.el);
    this.select(this.sel);
  }

  get selection(): PaneSelection {
    return this.sel;
  }

  /** The outputs currently shown. */
  visibleOutputs(): OutputIndex[] {
    return this.sel === 'tile' ? [...OUTPUTS] : [this.sel];
  }

  select(sel: PaneSelection): void {
    this.sel = sel;
    this.selector.value = sel === 'tile' ? 'tile' : `o${sel}`;
    this.el.dataset.selection = this.selector.value;
    const shown = new Set(this.visibleOutputs());
    this.panes.forEach((p, i) => {
      p.el.hidden = !shown.has(i as OutputIndex);
    });
  }

  /** Blits every visible output from the render host into its pane. */
  present(host: Presenter): void {
    const src = this.source;
    if (!src) return;
    for (const out of this.visibleOutputs()) {
      const p = this.panes[out];
      if (!p?.ctx) continue;
      host.present(out);
      p.ctx.drawImage(src, 0, 0, src.width, src.height, 0, 0, p.canvas.width, p.canvas.height);
    }
  }

  showDiagnostic(d: HostDiagnostic): void {
    this.banners.set(d.out, `o${d.out}: ${d.code}: ${d.message}`);
    this.renderBanner();
  }

  clearDiagnostic(out: OutputIndex): void {
    if (this.banners.delete(out)) this.renderBanner();
  }

  dispose(): void {
    this.selector.removeEventListener('change', this.onSelect);
    this.el.remove();
  }

  private readonly onSelect = (): void => {
    const v = this.selector.value;
    this.select(v === 'tile' ? 'tile' : (Number(v.slice(1)) as OutputIndex));
  };

  private renderBanner(): void {
    const lines = [...this.banners.entries()].sort((a, b) => a[0] - b[0]).map(([, text]) => text);
    this.banner.textContent = lines.join('\n');
    this.banner.hidden = lines.length === 0;
  }
}

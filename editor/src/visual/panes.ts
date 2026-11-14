// The visual output panes o0..o3 (design 9.3, 15.1.8). Four 2D canvases
// show the outputs' latest frames: for each visible pane the render host
// blits the output into its own (offscreen) WebGL canvas, which the pane
// then draws scaled. `render oN` shows one output and `render` tiles all
// four; the session does not publish that choice in v1, so the pane
// selector sets it. A `shader-compile` host diagnostic shows as a banner
// until that output accepts a program again.

import { createComponent, createSignal, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import { PanesView } from './panes-view';
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
  private readonly setSelection: Setter<PaneSelection>;
  private readonly setBanner: Setter<string>;
  private readonly disposeView: () => void;
  private readonly source: VisualPanesOptions['source'];
  private sel: PaneSelection;

  constructor(parent: HTMLElement, opts: VisualPanesOptions = {}) {
    const doc = parent.ownerDocument;
    this.source = opts.source;
    this.sel = opts.selection ?? 0;
    const [selection, setSelection] = createSignal<PaneSelection>(this.sel);
    const [banner, setBanner] = createSignal('');
    this.setSelection = setSelection;
    this.setBanner = setBanner;
    const holder = doc.createElement('div');
    this.disposeView = render(() => createComponent(PanesView, {
      source: this.source !== undefined,
      notice: opts.notice ?? 'visuals not available',
      selection, banner,
      width: PANE_WIDTH, height: PANE_HEIGHT,
      onSelect: (value) => this.select(value),
    }), holder);
    this.el = holder.firstElementChild as HTMLElement;
    if (this.source) {
      for (const out of OUTPUTS) {
        const pane = this.el.querySelector<HTMLElement>(`[data-output="o${out}"]`)!;
        const canvas = pane.querySelector('canvas')!;
        this.panes.push({ el: pane, canvas, ctx: canvas.getContext('2d') });
      }
    }
    parent.appendChild(this.el);
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
    this.setSelection(sel);
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
    this.disposeView();
    this.el.remove();
  }

  private renderBanner(): void {
    const lines = [...this.banners.entries()].sort((a, b) => a[0] - b[0]).map(([, text]) => text);
    this.setBanner(lines.join('\n'));
  }
}

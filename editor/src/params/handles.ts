// Parameter-editor handles (design 15.1.7 "Handles are sliders", 13.5).
//
// A `Handle` is one parameter of an open editor bound to one numeric site.
// It writes ONLY through `BindApi.writeSite` (the slider's write path, so
// the site's overlay/source-edit mode applies unchanged) and learns ONLY
// through `BindApi.learn`. A handle with no site is disabled ("not in
// code") and sends nothing: the editor never inserts text.
//
// Also here: the shared scaffolding of the kind components (a canvas
// surface, draggable canvas nodes, the handle rows) and the context every
// kind's `render(el, ctx)` receives.

import { createComponent, createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { CanvasSurface, HandleRowsView, type HandleRow, type HandleRowState } from './handle-view';
import type { BindApi } from '../app/apis';
import type { EditorDeps } from '../app/deps';
import type { EditorDecl, ParamCurve, ParamMeta, ParamUnit, WireSite } from '../protocol/types';
import { clamp, fromUnit, quantize, toUnit } from './curves';
import { bindHandles, type CallGroup } from './open';

export const NOT_IN_CODE = 'not in code';

export interface HandleHost {
  bind(): BindApi | undefined;
  site(id: number): WireSite | undefined;
}

export interface HandleSpec {
  param: string;
  siteId: number | null;
  range: [number, number];
  curve: ParamCurve;
  unit: ParamUnit;
}

export class Handle implements HandleSpec {
  readonly param: string;
  readonly siteId: number | null;
  range: [number, number];
  curve: ParamCurve;
  unit: ParamUnit;
  /** A kind-specific value rule applied before quantizing (beat snap, modulo wrap). */
  snap: ((v: number) => number) | null = null;
  /**
   * The last value this handle wrote and the site value it was written
   * over (an overlay leaves the site value unchanged). It is dropped once
   * the site value becomes anything else (the literal was re-evaluated).
   */
  private local: { value: number; base: number | undefined } | undefined;
  private readonly host: HandleHost;

  constructor(spec: HandleSpec, host: HandleHost) {
    this.param = spec.param;
    this.siteId = spec.siteId;
    this.range = spec.range;
    this.curve = spec.curve;
    this.unit = spec.unit;
    this.host = host;
  }

  get site(): WireSite | undefined {
    return this.siteId === null ? undefined : this.host.site(this.siteId);
  }

  get enabled(): boolean {
    return this.site !== undefined;
  }

  get value(): number {
    const s = this.site?.value;
    const l = this.local;
    if (l && (s === undefined || s === l.base || s === l.value)) return l.value;
    return s ?? this.range[0];
  }

  /** The value as a 0..1 position on the range and curve. */
  get unitPos(): number {
    return toUnit(this.value, this);
  }

  /** Writes `value` (quantized) through `BindApi.writeSite`; null when disabled. */
  set(value: number): number | null {
    const bind = this.host.bind();
    if (this.siteId === null || !this.enabled || !bind) return null;
    const v = quantize(this.snap ? this.snap(value) : value, this);
    const base = this.site?.value;
    this.local = { value: v, base: this.local && base === this.local.value ? this.local.base : base };
    bind.writeSite(this.siteId, v);
    return v;
  }

  /** Moves the handle by `delta` of its 0..1 range; returns the written value. */
  drag(delta: number): number | null {
    return this.set(fromUnit(this.unitPos + delta, this));
  }

  /** Sets the handle at a 0..1 position. */
  setUnit(t: number): number | null {
    return this.set(fromUnit(t, this));
  }

  /** MIDI learn through the slider's learn path. */
  learn(): Promise<void> {
    const bind = this.host.bind();
    if (this.siteId === null || !this.enabled || !bind) return Promise.resolve();
    return bind.learn(this.siteId);
  }
}

/** A range for a site with no ParamMeta (the slider panel's rule). */
export function fallbackRange(value: number): [number, number] {
  const mag = Math.max(1, Math.abs(value) * 2);
  return [value < 0 ? -mag : 0, mag];
}

/** The ParamMeta of a site's own call parameter, when declared. */
export function siteMeta(site: WireSite | undefined, editors: readonly EditorDecl[] | undefined): ParamMeta | undefined {
  const c = site?.call;
  if (!c || !editors) return undefined;
  const decl = editors.find((d) => d.name === c.name);
  return decl?.params.find((p) => p.name === c.param) ?? decl?.params[c.arg];
}

/** A handle over one site, ranged by its own ParamMeta (or `fallback`). */
export function siteHandle(
  siteId: number,
  param: string,
  host: HandleHost,
  editors: readonly EditorDecl[] | undefined,
  fallback?: Partial<HandleSpec>,
): Handle {
  const site = host.site(siteId);
  const meta = siteMeta(site, editors);
  return new Handle(
    {
      param,
      siteId,
      range: meta ? [meta.range[0], meta.range[1]] : (fallback?.range ?? fallbackRange(site?.value ?? 0)),
      curve: meta?.curve ?? fallback?.curve ?? 'linear',
      unit: meta?.unit ?? fallback?.unit ?? 'none',
    },
    host,
  );
}

/** One handle per declared parameter (or per site without a declaration). */
export function makeHandles(group: CallGroup, decl: EditorDecl | undefined, host: HandleHost): Handle[] {
  const out: Handle[] = [];
  for (const [param, siteId] of bindHandles(group, decl)) {
    const meta = decl?.params.find((p) => p.name === param);
    const site = siteId === null ? undefined : host.site(siteId);
    out.push(
      new Handle(
        {
          param,
          siteId,
          range: meta ? [meta.range[0], meta.range[1]] : fallbackRange(site?.value ?? 0),
          curve: meta?.curve ?? 'linear',
          unit: meta?.unit ?? 'none',
        },
        host,
      ),
    );
  }
  return out;
}

/** The first handle whose param matches `name` (exact) or `re`. */
export function findHandle(handles: readonly Handle[], name: string | RegExp): Handle | undefined {
  if (typeof name === 'string') return handles.find((h) => h.param === name);
  return handles.find((h) => name.test(h.param));
}

// ------------------------------------------------------- kind context

export interface KindCtx {
  doc: Document;
  deps: EditorDeps;
  file: string;
  group: CallGroup;
  decl?: EditorDecl;
  handles: Handle[];
  /** The other call groups of the same top-level form. */
  chain: CallGroup[];
  host: HandleHost;
  /** The document's current UTF-16 range of a group's form, when mapped. */
  formText(): string | null;
}

export interface KindView {
  update(): void;
  dispose(): void;
}

export type KindRender = (el: HTMLElement, ctx: KindCtx) => KindView;

/** A handle for the first site of a chain call named `name` (sampler begin/end, lfo range). */
export function chainHandle(ctx: KindCtx, name: string, arg = 0, fallback?: Partial<HandleSpec>): Handle | undefined {
  const g = ctx.chain.find((c) => c.name === name);
  const s = g?.sites.find((x) => (x.call?.arg ?? 0) === arg);
  if (!s) return undefined;
  return siteHandle(s.id, name, ctx.host, ctx.deps.store.manifest?.editors, fallback);
}

// ------------------------------------------------------------ surfaces

export interface Surface {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D | null;
  w: number;
  h: number;
  dispose(): void;
}

export function surface(el: HTMLElement, cls: string, w = 240, h = 120): Surface {
  const holder = el.ownerDocument.createElement('div');
  const disposeView = render(() => createComponent(CanvasSurface, { className: cls, width: w, height: h }), holder);
  const canvas = holder.firstElementChild as HTMLCanvasElement;
  el.appendChild(canvas);
  return { canvas, ctx: canvas.getContext('2d'), w, h, dispose: () => { disposeView(); canvas.remove(); } };
}

export function polyline(ctx: CanvasRenderingContext2D, pts: readonly [number, number][], color = '#8fd'): void {
  if (pts.length === 0) return;
  ctx.strokeStyle = color;
  ctx.beginPath();
  pts.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)));
  ctx.stroke();
}

export function dot(ctx: CanvasRenderingContext2D, x: number, y: number, on = true, label?: string): void {
  ctx.fillStyle = on ? '#fc6' : '#555';
  ctx.fillRect(x - 4, y - 4, 8, 8);
  if (label) ctx.fillText(label, x + 6, y - 6);
}

/** A draggable canvas node: x drags `hx`, y drags `hy`, the wheel drags `wheel`. */
export interface Node2D {
  x: number;
  y: number;
  hx?: Handle;
  hy?: Handle;
  wheel?: Handle;
}

export const NODE_RADIUS = 16;

function canvasPos(s: Surface, ev: MouseEvent): [number, number] {
  const r = s.canvas.getBoundingClientRect();
  const sx = r.width > 0 ? s.w / r.width : 1;
  const sy = r.height > 0 ? s.h / r.height : 1;
  return [(ev.clientX - r.left) * sx, (ev.clientY - r.top) * sy];
}

function nearest(nodes: readonly Node2D[], x: number, y: number): Node2D | undefined {
  let best: { n: Node2D; d: number } | undefined;
  for (const n of nodes) {
    const d = Math.hypot(n.x - x, n.y - y);
    if (d <= NODE_RADIUS && (!best || d < best.d)) best = { n, d };
  }
  return best?.n;
}

/** Wires pointer drag and wheel on the surface to the nodes' handles. */
export function attachNodes(s: Surface, nodes: () => Node2D[], after: () => void): () => void {
  // Moves are applied from the drag start, so a snapping handle still moves.
  let active: { n: Node2D; x: number; y: number; ux: number; uy: number } | null = null;
  const down = (ev: Event): void => {
    const [x, y] = canvasPos(s, ev as MouseEvent);
    const n = nearest(nodes(), x, y);
    active = n ? { n, x, y, ux: n.hx?.unitPos ?? 0, uy: n.hy?.unitPos ?? 0 } : null;
  };
  const move = (ev: Event): void => {
    if (!active) return;
    const [x, y] = canvasPos(s, ev as MouseEvent);
    // An axis the pointer did not move is not written.
    if (x !== active.x) active.n.hx?.setUnit(active.ux + (x - active.x) / s.w);
    if (y !== active.y) active.n.hy?.setUnit(active.uy - (y - active.y) / s.h);
    after();
  };
  const up = (): void => {
    active = null;
  };
  const wheel = (ev: Event): void => {
    const w = ev as WheelEvent;
    const [x, y] = canvasPos(s, w);
    const n = nearest(nodes(), x, y);
    if (!n?.wheel) return;
    w.preventDefault();
    n.wheel.drag(w.deltaY > 0 ? -0.05 : 0.05);
    after();
  };
  const c = s.canvas;
  c.addEventListener('pointerdown', down);
  c.addEventListener('pointermove', move);
  c.addEventListener('pointerup', up);
  c.addEventListener('pointerleave', up);
  c.addEventListener('wheel', wheel);
  return () => {
    c.removeEventListener('pointerdown', down);
    c.removeEventListener('pointermove', move);
    c.removeEventListener('pointerup', up);
    c.removeEventListener('pointerleave', up);
    c.removeEventListener('wheel', wheel);
  };
}

// --------------------------------------------------------------- rows

export function formatHandleValue(v: number): string {
  return String(Number(v.toFixed(4)) + 0);
}

/** One row per handle: label, a 0..1 range input on the curve, the value and learn. */
export function handleRows(el: HTMLElement, handles: readonly Handle[], after: () => void): KindView {
  const holder = el.ownerDocument.createElement('div');
  const rows: { row: HandleRow; setState: (state: HandleRowState) => void }[] = handles.map((handle) => {
    const [state, setState] = createSignal(handleState(handle));
    return { row: { handle, state }, setState };
  });
  const update = (): void => {
    for (const { row, setState } of rows) setState(handleState(row.handle));
  };
  const disposeView = render(() => createComponent(HandleRowsView, {
    rows: rows.map(({ row }) => row),
    onInput: (handle, value) => {
      handle.setUnit(clamp(value, 0, 1));
      update();
      after();
    },
  }), holder);
  const box = holder.firstElementChild as HTMLElement;
  el.appendChild(box);
  return { update, dispose: () => { disposeView(); box.remove(); } };
}

function handleState(handle: Handle): HandleRowState {
  const enabled = handle.enabled;
  return {
    enabled,
    position: handle.unitPos,
    value: enabled ? `${formatHandleValue(handle.value)}${handle.unit === 'none' ? '' : ` ${handle.unit}`}` : NOT_IN_CODE,
  };
}

/**
 * The common kind view: a canvas drawn by `draw`, draggable `nodes`, and
 * one row per handle. `update()` repaints both.
 */
export function standardView(
  el: HTMLElement,
  kind: string,
  handles: readonly Handle[],
  draw: (s: Surface) => void,
  nodes: () => Node2D[] = () => [],
): KindView & { surface: Surface } {
  const s = surface(el, `params-${kind}`);
  let rows: KindView | null = null;
  const repaint = (): void => {
    if (s.ctx) {
      s.ctx.clearRect(0, 0, s.w, s.h);
      draw(s);
    }
    rows?.update();
  };
  const off = attachNodes(s, nodes, repaint);
  rows = handleRows(el, handles, repaint);
  repaint();
  return {
    surface: s,
    update: repaint,
    dispose() {
      off();
      rows?.dispose();
      s.dispose();
    },
  };
}

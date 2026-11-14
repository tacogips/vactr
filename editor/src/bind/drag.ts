// Mouse drag on literals and the overlay widget (design 15.1.6).
//
// Drag: a pointer-down on a literal whose current span is a bound site's
// span starts a vertical drag; moving up raises the value. The scale is the
// ParamMeta range and curve of `site.call`'s parameter when the manifest
// declares one (`DRAG_PX` pixels cover the range), otherwise 1% of
// max(|value|, 1) per pixel. Every value goes through `writeSite`; there is
// no other write path. A pointer-down on a number that is not a site does
// nothing, and a click without movement just places the cursor.
//
// Overlay widget: an active overlay value is rendered right after its
// literal; the document text never changes.

import { StateEffect, StateField, type Extension } from '@codemirror/state';
import { Decoration, EditorView, WidgetType, type DecorationSet } from '@codemirror/view';
import type { BindApi } from '../app/apis';
import type { EditorDecl } from '../protocol/types';
import type { SiteEntry, SiteTable } from './sites';
import { fromUnit, isIntegerLiteral, paramMeta, round6, toUnit } from './write';

/** Pixels of vertical travel for the whole ParamMeta range. */
export const DRAG_PX = 200;
/** Movement below this many pixels is a click. */
export const DRAG_SLOP_PX = 3;

// -------------------------------------------------------------- overlays

export interface OverlayMark {
  pos: number;
  text: string;
}

export const setOverlays = StateEffect.define<OverlayMark[]>();

class OverlayWidget extends WidgetType {
  constructor(readonly text: string) {
    super();
  }

  override eq(other: OverlayWidget): boolean {
    return other.text === this.text;
  }

  toDOM(): HTMLElement {
    const el = document.createElement('span');
    el.className = 'bind-overlay';
    el.textContent = this.text;
    return el;
  }
}

export const overlayField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    let next = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (!e.is(setOverlays)) continue;
      const marks = [...e.value].sort((a, b) => a.pos - b.pos);
      next = Decoration.set(
        marks.map((m) => Decoration.widget({ widget: new OverlayWidget(m.text), side: 1 }).range(m.pos)),
      );
    }
    return next;
  },
  provide: (f) => EditorView.decorations.from(f),
});

/** The overlay widgets currently shown (tests and the panel). */
export function overlayMarks(view: EditorView): OverlayMark[] {
  const out: OverlayMark[] = [];
  const deco = view.state.field(overlayField, false);
  if (!deco) return out;
  deco.between(0, view.state.doc.length, (from, _to, value) => {
    const w = value.spec.widget as OverlayWidget | undefined;
    if (w) out.push({ pos: from, text: w.text });
  });
  return out;
}

// ------------------------------------------------------------------ drag

export interface DragHost {
  table: SiteTable;
  bind: BindApi;
  editors(): readonly EditorDecl[] | undefined;
  /** The active overlay value of a binding. */
  overlay?(bindingId: string): number | undefined;
}

interface Active {
  entry: SiteEntry;
  start: number;
  y0: number;
  moved: boolean;
}

export class DragController {
  private readonly host: DragHost;
  private active: Active | null = null;

  constructor(host: DragHost) {
    this.host = host;
  }

  get dragging(): boolean {
    return this.active !== null;
  }

  /** The bound binding whose current literal range contains `pos` (the innermost). */
  siteAt(pos: number): SiteEntry | undefined {
    let best: { e: SiteEntry; len: number } | undefined;
    for (const e of this.host.table.all()) {
      if (e.state !== 'bound') continue;
      const r = this.host.table.currentRange(e);
      if (!r || pos < r.from || pos > r.to) continue;
      if (!best || r.to - r.from < best.len) best = { e, len: r.to - r.from };
    }
    return best?.e;
  }

  /** A pointer-down at document offset `pos`; false when it is not on a site. */
  begin(pos: number, y: number): boolean {
    const entry = this.siteAt(pos);
    if (!entry) return false;
    const start = this.host.bind.mode(entry.site.id) === 'overlay' ? this.shown(entry) : entry.site.value;
    this.active = { entry, start, y0: y, moved: false };
    return true;
  }

  /** The value for a pointer at `y`; null when no drag is active or within the slop. */
  valueAt(y: number): number | null {
    const a = this.active;
    if (!a) return null;
    const dy = a.y0 - y;
    if (!a.moved && Math.abs(dy) < DRAG_SLOP_PX) return null;
    a.moved = true;
    const meta = paramMeta(a.entry.site, this.host.editors());
    let v: number;
    if (meta) v = fromUnit(toUnit(a.start, meta) + dy / DRAG_PX, meta);
    else v = a.start + dy * 0.01 * Math.max(Math.abs(a.start), 1);
    return isIntegerLiteral(a.entry.literalText) ? Math.round(v) + 0 : round6(v);
  }

  /** A pointer move: writes through `writeSite`. */
  move(y: number): void {
    const v = this.valueAt(y);
    const a = this.active;
    if (v === null || !a) return;
    this.host.bind.writeSite(a.entry.site.id, v);
  }

  /** Ends the drag; true when it moved (false: a click). */
  end(): boolean {
    const moved = this.active?.moved ?? false;
    this.active = null;
    return moved;
  }

  /** The CodeMirror extension: pointer events on the editor content. */
  extension(): Extension {
    return EditorView.domEventHandlers({
      mousedown: (ev, view) => {
        if (ev.button !== 0 || ev.shiftKey || ev.altKey || ev.metaKey || ev.ctrlKey) return false;
        const pos = view.posAtCoords({ x: ev.clientX, y: ev.clientY });
        if (pos === null || !this.begin(pos, ev.clientY)) return false;
        ev.preventDefault();
        const win = view.dom.ownerDocument.defaultView;
        if (!win) {
          this.end();
          return false;
        }
        const onMove = (m: MouseEvent): void => this.move(m.clientY);
        const onUp = (): void => {
          win.removeEventListener('mousemove', onMove);
          win.removeEventListener('mouseup', onUp);
          if (!this.end()) view.dispatch({ selection: { anchor: pos } });
          view.focus();
        };
        win.addEventListener('mousemove', onMove);
        win.addEventListener('mouseup', onUp);
        return true;
      },
    });
  }

  private shown(e: SiteEntry): number {
    return this.host.overlay?.(e.bindingId) ?? e.site.value;
  }
}

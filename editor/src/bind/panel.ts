// The right-pane slider panel and the value displays (design 15.1.6
// "Sites and keys", "Reactive displays", 14.5.5).
//
// Sliders are grouped by `origin` (pattern literals, bindings, inst
// defaults). Each row shows its label (the key, else the literal's line
// excerpt), the value (the overlay value when one is active), the tier
// badge, the mode toggle, commit (overlay only), learn with its mapping, the
// STALE/unbound state, and the owning form's badge (`failed` with its
// diagnostic, `blocked on`). The value displays list every name of the
// `bindings` batches with its committed value and badge.
//
// Every row subscribes to the store under its own `site:<id>` / `name:<n>`
// keys, so one batch repaints exactly the affected rows once each. A whole
// `eval-result` (the `forms` key) is a structural rebuild instead, driven
// by the area. Only completed messages are ever stored, so a provisional
// value cannot be rendered.

import type { MidiMapping } from './persistence';
import { createComponent, createSignal, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import { NameRow, SiteRow, type NameRowState, type SiteRowState } from './panel-view';
import { nameKey, siteKey, type Store } from '../protocol/store';
import type { EditorDecl, SiteOrigin } from '../protocol/types';
import type { Refresh, SiteEntry, SiteTable } from './sites';
import { isIntegerLiteral, paramMeta, round6, tierBadge, type SiteWriter } from './write';

export const GROUPS: readonly [SiteOrigin, string][] = [
  ['pattern-literal', 'Pattern literals'],
  ['binding', 'Bindings'],
  ['inst-default', 'Inst defaults'],
];

export const PANEL_ROW_HEIGHT = 32;
export const PANEL_OVERSCAN_ROWS = 8;
export interface PanelViewport { top: number; height: number; headerHeight?: number }

export interface PanelHost {
  store: Store;
  table: SiteTable;
  writer: SiteWriter;
  editors(): readonly EditorDecl[] | undefined;
  /** The name of the form owning a binding (for its badge), when known. */
  ownerName(e: SiteEntry): string | undefined;
  /** The literal's line, trimmed, for an unkeyed label. */
  lineExcerpt(e: SiteEntry): string;
  mappingOf(e: SiteEntry): MidiMapping | undefined;
  learn(e: SiteEntry): void;
  commit(e: SiteEntry): void;
  /** Test seam: called after every row render. */
  onRender?(key: string, el: HTMLElement): void;
  /** Test seam for deterministic viewport virtualization. */
  viewport?(): PanelViewport;
}

interface Row {
  id: string;
  el: HTMLElement;
  setState: Setter<SiteRowState>;
  dispose: () => void;
  keys: string;
  off: () => void;
  range: [number, number] | null;
}

interface ValueRow {
  name: string;
  el: HTMLElement;
  setState: Setter<NameRowState>;
  dispose: () => void;
  off: () => void;
}

export const formatValue = (v: number): string => String(round6(v));

export class SliderPanel {
  readonly el: HTMLElement;
  private readonly host: PanelHost;
  private readonly groups = new Map<SiteOrigin, HTMLElement>();
  private readonly groupHeaders = new Map<SiteOrigin, HTMLElement>();
  private readonly groupTop = new Map<SiteOrigin, HTMLElement>();
  private readonly groupBottom = new Map<SiteOrigin, HTMLElement>();
  private readonly model = new Map<SiteOrigin, string[]>();
  private readonly mountedOrder = new Map<SiteOrigin, string[]>();
  private readonly namesEl: HTMLElement;
  private readonly namesTop: HTMLElement;
  private readonly namesBottom: HTMLElement;
  private modelNames: string[] = [];
  private readonly rows = new Map<string, Row>();
  private readonly nameRows = new Map<string, ValueRow>();
  private readonly renders = new Map<string, number>();
  private readonly scrollContainer: HTMLElement | null;
  private observer: ResizeObserver | null = null;
  private frame: number | null = null;
  private rowHeight = PANEL_ROW_HEIGHT;
  private disposed = false;
  private readonly removeViewportListeners: (() => void)[] = [];

  constructor(parent: HTMLElement, host: PanelHost) {
    this.host = host;
    const doc = parent.ownerDocument;
    this.el = doc.createElement('section');
    this.el.className = 'bind-panel';
    this.el.dataset.area = 'bind';
    for (const [origin, title] of GROUPS) {
      const g = doc.createElement('div');
      g.className = 'bind-group';
      g.dataset.origin = origin;
      const h = doc.createElement('h3');
      h.textContent = title;
      const top = doc.createElement('div'); top.className = 'bind-spacer'; top.dataset.edge = 'top';
      const bottom = doc.createElement('div'); bottom.className = 'bind-spacer'; bottom.dataset.edge = 'bottom';
      g.appendChild(h);
      g.append(top, bottom);
      this.groups.set(origin, g);
      this.groupHeaders.set(origin, h);
      this.groupTop.set(origin, top);
      this.groupBottom.set(origin, bottom);
      this.model.set(origin, []);
      this.mountedOrder.set(origin, []);
      this.el.appendChild(g);
    }
    this.namesEl = doc.createElement('div');
    this.namesEl.className = 'bind-names';
    const h = doc.createElement('h3');
    h.textContent = 'Values';
    this.namesTop = doc.createElement('div'); this.namesTop.className = 'bind-spacer'; this.namesTop.dataset.edge = 'top';
    this.namesBottom = doc.createElement('div'); this.namesBottom.className = 'bind-spacer'; this.namesBottom.dataset.edge = 'bottom';
    this.namesEl.append(h, this.namesTop, this.namesBottom);
    this.el.appendChild(this.namesEl);
    parent.appendChild(this.el);
    this.scrollContainer = this.findScrollContainer();
    if (!host.viewport && this.scrollContainer) {
      const dirty = (): void => this.requestWindow();
      this.scrollContainer.addEventListener('scroll', dirty, { passive: true });
      this.removeViewportListeners.push(() => this.scrollContainer?.removeEventListener('scroll', dirty));
      const view = doc.defaultView;
      if (view?.ResizeObserver) {
        this.observer = new view.ResizeObserver(dirty);
        this.observer.observe(this.scrollContainer);
      }
      doc.fonts?.addEventListener('loadingdone', dirty);
      this.removeViewportListeners.push(() => doc.fonts?.removeEventListener('loadingdone', dirty));
      const coarse = typeof view?.matchMedia === 'function' ? view.matchMedia('(pointer: coarse)') : null;
      coarse?.addEventListener?.('change', dirty);
      this.removeViewportListeners.push(() => coarse?.removeEventListener?.('change', dirty));
    }
  }

  /** Render counts per `binding:<id>` / `name:<n>` (tests). */
  renderCount(key: string): number {
    return this.renders.get(key) ?? 0;
  }

  row(bindingId: string): HTMLElement | undefined {
    return this.rows.get(bindingId)?.el;
  }

  nameRow(name: string): HTMLElement | undefined {
    return this.nameRows.get(name)?.el;
  }

  /** After an `eval-result`: syncs rows with the table and renders each once. */
  rebuild(): void {
    this.syncRows(new Set(this.host.table.all().map((e) => e.bindingId)));
  }

  /**
   * After a `bindings` sites refresh, from inside the store's notification
   * of that batch (`batch`: its changed keys). A row whose subscription
   * keys hold and intersect the batch renders from its own subscription;
   * new, re-keyed and otherwise changed rows render here, once.
   */
  applyRefresh(r: Refresh, batch: ReadonlySet<string>): void {
    const render = new Set<string>(r.added);
    for (const id of r.changed) {
      const row = this.rows.get(id);
      const keys = this.keysOf(id);
      if (!row || row.keys !== keys || !keys.split('\n').some((k) => batch.has(k))) render.add(id);
    }
    this.syncRows(render);
  }

  /** Renders the given rows once (write state, mapping or STALE changes). */
  renderIds(ids: Iterable<string>): void {
    for (const id of ids) {
      const row = this.rows.get(id);
      if (row) this.renderRow(row);
    }
  }

  /** Adds a value display for every stored name not shown yet. */
  addNames(): void {
    this.modelNames = [...this.host.store.names().keys()];
    this.updateWindow();
  }

  dispose(): void {
    for (const r of this.rows.values()) { r.off(); r.dispose(); }
    for (const n of this.nameRows.values()) { n.off(); n.dispose(); }
    this.rows.clear();
    this.nameRows.clear();
    this.disposed = true;
    if (this.frame !== null) this.el.ownerDocument.defaultView?.cancelAnimationFrame(this.frame);
    this.frame = null;
    this.observer?.disconnect(); this.observer = null;
    for (const remove of this.removeViewportListeners) remove();
    this.removeViewportListeners.length = 0;
    this.el.remove();
  }

  // ------------------------------------------------------------ internals

  private keysOf(id: string): string {
    const e = this.host.table.get(id);
    if (!e) return '';
    const owner = this.host.ownerName(e);
    return [siteKey(e.site.id), ...(owner ? [nameKey(owner)] : [])].join('\n');
  }

  private syncRows(render: Set<string>): void {
    const { table } = this.host;
    const next = new Map<SiteOrigin, string[]>();
    for (const [origin] of GROUPS) next.set(origin, []);
    const entries = table.all().map((e) => ({ e, at: table.currentRange(e)?.from ?? Number.MAX_SAFE_INTEGER }));
    entries.sort((a, b) => a.at - b.at);
    for (const { e } of entries) next.get(e.site.origin)?.push(e.bindingId);
    for (const [id, row] of this.rows) {
      if (table.get(id)) continue;
      row.off();
      row.dispose();
      row.el.remove();
      this.rows.delete(id);
    }
    for (const [origin, ids] of next) this.model.set(origin, ids);
    for (const id of render) this.pendingRender.add(id);
    if (this.host.viewport) this.updateWindow();
    else this.requestWindow();
  }

  private pendingRender = new Set<string>();

  private findScrollContainer(): HTMLElement | null {
    const pane = this.el.closest<HTMLElement>('.pane-right');
    if (pane) return pane;
    for (let node = this.el.parentElement; node; node = node.parentElement) {
      const overflow = this.el.ownerDocument.defaultView?.getComputedStyle(node).overflowY;
      if (overflow === 'auto' || overflow === 'scroll') return node;
    }
    return this.el.parentElement;
  }

  private requestWindow(): void {
    if (this.disposed || this.frame !== null) return;
    const view = this.el.ownerDocument.defaultView;
    if (!view) return;
    this.frame = view.requestAnimationFrame(() => { this.frame = null; this.updateWindow(); });
  }

  private viewport(): PanelViewport {
    const seam = this.host.viewport?.();
    if (seam) return seam;
    const container = this.scrollContainer;
    if (!container) return { top: 0, height: 0 };
    const panelRect = this.el.getBoundingClientRect();
    const containerRect = container.getBoundingClientRect();
    return {
      top: containerRect.top + container.clientTop - panelRect.top,
      height: container.clientHeight,
    };
  }

  private updateWindow(): void {
    if (this.disposed) return;
    const viewport = this.viewport();
    const measuredRow = this.rows.values().next().value as Row | undefined;
    if (!this.host.viewport && measuredRow) {
      const measured = measuredRow.el.offsetHeight;
      this.rowHeight = measured > 0 ? measured : PANEL_ROW_HEIGHT;
    }
    const rowHeight = this.rowHeight > 0 ? this.rowHeight : PANEL_ROW_HEIGHT;
    const headerHeight = viewport.headerHeight ?? 0;
    const measuredHeaders = new Map<SiteOrigin, number>();
    if (!this.host.viewport) for (const [origin] of GROUPS) measuredHeaders.set(origin, this.groupHeaders.get(origin)?.offsetHeight ?? 0);
    let groupOrigin = 0;
    for (const [origin] of GROUPS) {
      const ids = this.model.get(origin) ?? [];
      const groupHeader = this.host.viewport ? headerHeight : measuredHeaders.get(origin) ?? 0;
      const contentOrigin = groupOrigin + groupHeader;
      const start = Math.max(0, Math.floor((viewport.top - contentOrigin) / rowHeight) - PANEL_OVERSCAN_ROWS);
      const end = Math.min(ids.length, Math.ceil((viewport.top + viewport.height - contentOrigin) / rowHeight) + PANEL_OVERSCAN_ROWS);
      this.mountGroup(origin, ids, start, Math.max(start, end), rowHeight);
      groupOrigin = contentOrigin + ids.length * rowHeight;
    }
    const nameOrigin = groupOrigin;
    const nameStart = Math.max(0, Math.floor((viewport.top - nameOrigin - headerHeight) / rowHeight) - PANEL_OVERSCAN_ROWS);
    const nameEnd = Math.min(this.modelNames.length, Math.ceil((viewport.top + viewport.height - nameOrigin - headerHeight) / rowHeight) + PANEL_OVERSCAN_ROWS);
    this.mountNames(nameStart, Math.max(nameStart, nameEnd), rowHeight);
    this.pendingRender.clear();
  }

  private mountGroup(origin: SiteOrigin, ids: string[], start: number, end: number, rowHeight: number): void {
    const group = this.groups.get(origin)!;
    const selected = ids.slice(start, end);
    const selectedSet = new Set(selected);
    for (const id of this.mountedOrder.get(origin) ?? []) {
      if (selectedSet.has(id)) continue;
      const row = this.rows.get(id);
      if (row) { row.off(); row.dispose(); row.el.remove(); this.rows.delete(id); }
    }
    const renderIds = this.pendingRender;
    for (const id of selected) {
      let row = this.rows.get(id);
      if (!row) {
        row = this.createRow(id);
        const keys = this.keysOf(id);
        this.subscribe(row, keys);
        renderIds.delete(id);
      } else {
        const keys = this.keysOf(id);
        if (row.keys !== keys) { this.subscribe(row, keys); renderIds.add(id); }
      }
    }
    const prior = this.mountedOrder.get(origin) ?? [];
    const unchanged = prior.length === selected.length && prior.every((id, i) => id === selected[i]);
    const top = this.groupTop.get(origin)!; const bottom = this.groupBottom.get(origin)!;
    top.style.height = `${start * rowHeight}px`;
    bottom.style.height = `${(ids.length - end) * rowHeight}px`;
    const groupEl = group as HTMLElement & { dataset: DOMStringMap };
    if (ids.length === 0) groupEl.dataset.empty = ''; else delete groupEl.dataset.empty;
    if (!unchanged) {
      let anchor: Node = bottom;
      for (let i = selected.length - 1; i >= 0; i -= 1) {
        const row = this.rows.get(selected[i] as string)!;
        if (row.el.parentNode !== group || row.el.nextSibling !== anchor) group.insertBefore(row.el, anchor);
        anchor = row.el;
      }
    }
    if (bottom.parentNode !== group) group.appendChild(bottom);
    this.mountedOrder.set(origin, selected);
    for (const id of [...renderIds]) {
      if (!selectedSet.has(id)) continue;
      const row = this.rows.get(id); if (row) this.renderRow(row);
      renderIds.delete(id);
    }
  }

  private mountNames(start: number, end: number, rowHeight: number): void {
    const selected = this.modelNames.slice(start, end);
    const selectedSet = new Set(selected);
    for (const [name, row] of this.nameRows) {
      if (selectedSet.has(name)) continue;
      row.off(); row.dispose(); row.el.remove(); this.nameRows.delete(name);
    }
    for (const name of selected) if (!this.nameRows.has(name)) this.createNameRow(name);
    this.namesTop.style.height = `${start * rowHeight}px`;
    this.namesBottom.style.height = `${(this.modelNames.length - end) * rowHeight}px`;
    let anchor: Node = this.namesBottom;
    for (let i = selected.length - 1; i >= 0; i -= 1) {
      const row = this.nameRows.get(selected[i] as string)!;
      if (row.el.parentNode !== this.namesEl || row.el.nextSibling !== anchor) this.namesEl.insertBefore(row.el, anchor);
      anchor = row.el;
    }
    if (this.modelNames.length === 0) this.namesEl.dataset.empty = ''; else delete this.namesEl.dataset.empty;
  }

  private subscribe(row: Row, keys: string): void {
    row.off();
    row.keys = keys;
    row.off = this.host.store.subscribe(keys.split('\n'), (changed) => {
      // A whole eval-result is the area's structural rebuild.
      if (changed.has('forms')) return;
      this.renderRow(row);
    });
  }

  private createRow(id: string): Row {
    const holder = this.el.ownerDocument.createElement('div');
    const row: Row = {
      id, el: holder, setState: (() => {}) as Setter<SiteRowState>,
      dispose: () => {}, keys: '', off: () => {}, range: null,
    };
    const [state, setState] = createSignal(this.siteState(row));
    row.setState = setState;
    row.dispose = render(() => createComponent(SiteRow, {
      id, state,
      onInput: (value) => {
        const entry = this.host.table.get(id);
        if (entry) this.host.writer.writeSite(entry.site.id, value);
      },
      onMode: () => this.host.writer.setMode(id, this.host.writer.modeOf(id) === 'overlay' ? 'source-edit' : 'overlay'),
      onCommit: () => {
        const entry = this.host.table.get(id);
        if (entry) this.host.commit(entry);
      },
      onLearn: () => {
        const entry = this.host.table.get(id);
        if (entry) this.host.learn(entry);
      },
    }), holder);
    row.el = holder.firstElementChild as HTMLElement;
    this.rows.set(id, row);
    this.count(`binding:${id}`, row.el);
    return row;
  }

  private siteState(row: Row): SiteRowState {
    const entry = this.host.table.get(row.id)!;
    const overlay = this.host.writer.overlay(entry.bindingId);
    const value = overlay ?? entry.site.value;
    const meta = paramMeta(entry.site, this.host.editors());
    if (!row.range) {
      const magnitude = Math.max(1, Math.abs(entry.site.value) * 2);
      row.range = meta ? [meta.range[0], meta.range[1]] : [entry.site.value < 0 ? -magnitude : 0, magnitude];
    }
    const mode = this.host.writer.modeOf(entry.bindingId);
    const mapping = this.host.mappingOf(entry);
    const owner = this.host.ownerName(entry);
    const name = owner === undefined ? undefined : this.host.store.name(owner);
    return {
      label: entry.key ?? this.host.lineExcerpt(entry),
      value: formatValue(value), rawValue: value,
      tier: tierBadge(entry.site.tier), tierKind: entry.site.tier,
      mode, commit: mode === 'overlay' && overlay !== undefined,
      midi: mapping ? `cc ${mapping.cc}${mapping.ch === undefined ? '' : ` ch ${mapping.ch}`}` : '',
      state: entry.state,
      stateLabel: entry.state === 'stale' ? 'STALE' : entry.state === 'unbound' ? 'unbound' : '',
      form: name?.state === 'failed'
        ? `failed: ${name.diagnostic?.message ?? ''}`
        : name?.state === 'blocked' ? `blocked on ${name.blocked_on ?? '?'}` : '',
      min: row.range[0], max: row.range[1],
      step: isIntegerLiteral(entry.literalText) || meta?.curve === 'stepped' ? '1' : 'any',
      paramLabel: meta?.label,
      paramDefault: meta?.default,
      choices: meta?.choices ?? [],
    };
  }

  private renderRow(row: Row): void {
    row.setState(this.siteState(row));
    this.count(`binding:${row.id}`, row.el);
  }

  private createNameRow(name: string): ValueRow {
    const holder = this.el.ownerDocument.createElement('div');
    const [state, setState] = createSignal(this.nameState(name));
    const dispose = render(() => createComponent(NameRow, { name, state }), holder);
    const el = holder.firstElementChild as HTMLElement;
    const row: ValueRow = { name, el, setState, dispose, off: () => {} };
    row.off = this.host.store.subscribe([nameKey(name)], () => this.renderName(row));
    this.nameRows.set(name, row);
    this.count(`name:${name}`, row.el);
    return row;
  }

  private nameState(name: string): NameRowState {
    const state = this.host.store.name(name);
    return {
      value: state?.value ?? '', state: state?.state ?? 'ok',
      badge: state?.state === 'failed'
        ? `failed: ${state.diagnostic?.message ?? ''}`
        : state?.state === 'blocked' ? `blocked on ${state.blocked_on ?? '?'}` : '',
    };
  }

  private renderName(row: ValueRow): void {
    row.setState(this.nameState(row.name));
    this.count(`name:${row.name}`, row.el);
  }

  private count(key: string, el: HTMLElement): void {
    this.renders.set(key, (this.renders.get(key) ?? 0) + 1);
    this.host.onRender?.(key, el);
  }
}

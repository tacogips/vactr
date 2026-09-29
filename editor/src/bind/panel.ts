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
  private readonly namesEl: HTMLElement;
  private readonly rows = new Map<string, Row>();
  private readonly nameRows = new Map<string, ValueRow>();
  private readonly renders = new Map<string, number>();

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
      g.appendChild(h);
      this.groups.set(origin, g);
      this.el.appendChild(g);
    }
    this.namesEl = doc.createElement('div');
    this.namesEl.className = 'bind-names';
    const h = doc.createElement('h3');
    h.textContent = 'Values';
    this.namesEl.appendChild(h);
    this.el.appendChild(this.namesEl);
    parent.appendChild(this.el);
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
    for (const name of this.host.store.names().keys()) {
      if (this.nameRows.has(name)) continue;
      const nr = this.createNameRow(name);
      this.renderName(nr);
    }
  }

  dispose(): void {
    for (const r of this.rows.values()) { r.off(); r.dispose(); }
    for (const n of this.nameRows.values()) { n.off(); n.dispose(); }
    this.rows.clear();
    this.nameRows.clear();
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
    for (const [id, row] of this.rows) {
      if (table.get(id)) continue;
      row.off();
      row.dispose();
      row.el.remove();
      this.rows.delete(id);
    }
    const entries = table.all().map((e) => ({ e, at: table.currentRange(e)?.from ?? Number.MAX_SAFE_INTEGER }));
    entries.sort((a, b) => a.at - b.at);
    for (const { e } of entries) {
      let row = this.rows.get(e.bindingId);
      if (!row) {
        row = this.createRow(e.bindingId);
        render.delete(e.bindingId); // the Solid root rendered its initial state
      }
      const keys = this.keysOf(e.bindingId);
      if (row.keys !== keys) {
        this.subscribe(row, keys);
        if (this.renderCount(`binding:${e.bindingId}`) > 1) render.add(e.bindingId);
      }
      // Re-appending moves the element into order; it is not a render.
      this.groups.get(e.site.origin)?.appendChild(row.el);
    }
    for (const id of render) {
      const row = this.rows.get(id);
      if (row) this.renderRow(row);
    }
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
    this.namesEl.appendChild(el);
    const row: ValueRow = { name, el, setState, dispose, off: () => {} };
    row.off = this.host.store.subscribe([nameKey(name)], () => this.renderName(row));
    this.nameRows.set(name, row);
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

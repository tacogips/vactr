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
  label: HTMLElement;
  slider: HTMLInputElement;
  value: HTMLElement;
  tier: HTMLElement;
  mode: HTMLButtonElement;
  commit: HTMLButtonElement;
  learn: HTMLButtonElement;
  midi: HTMLElement;
  state: HTMLElement;
  form: HTMLElement;
  keys: string;
  off: () => void;
  range: [number, number] | null;
}

interface NameRow {
  name: string;
  el: HTMLElement;
  value: HTMLElement;
  badge: HTMLElement;
  off: () => void;
}

export const formatValue = (v: number): string => String(round6(v));

export class SliderPanel {
  readonly el: HTMLElement;
  private readonly host: PanelHost;
  private readonly groups = new Map<SiteOrigin, HTMLElement>();
  private readonly namesEl: HTMLElement;
  private readonly rows = new Map<string, Row>();
  private readonly nameRows = new Map<string, NameRow>();
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
    for (const r of this.rows.values()) r.off();
    for (const n of this.nameRows.values()) n.off();
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
      row.el.remove();
      this.rows.delete(id);
    }
    const entries = table.all().map((e) => ({ e, at: table.currentRange(e)?.from ?? Number.MAX_SAFE_INTEGER }));
    entries.sort((a, b) => a.at - b.at);
    for (const { e } of entries) {
      let row = this.rows.get(e.bindingId);
      if (!row) {
        row = this.createRow(e.bindingId);
        render.add(e.bindingId);
      }
      const keys = this.keysOf(e.bindingId);
      if (row.keys !== keys) {
        this.subscribe(row, keys);
        render.add(e.bindingId);
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
    const doc = this.el.ownerDocument;
    const el = doc.createElement('div');
    el.className = 'bind-row';
    el.dataset.binding = id;
    const span = (cls: string): HTMLElement => {
      const s = doc.createElement('span');
      s.className = cls;
      el.appendChild(s);
      return s;
    };
    const button = (cls: string, text: string): HTMLButtonElement => {
      const b = doc.createElement('button');
      b.type = 'button';
      b.className = cls;
      b.textContent = text;
      el.appendChild(b);
      return b;
    };
    const label = span('bind-label');
    const slider = doc.createElement('input');
    slider.type = 'range';
    slider.className = 'bind-slider';
    el.appendChild(slider);
    const row: Row = {
      id,
      el,
      label,
      slider,
      value: span('bind-value'),
      tier: span('bind-tier'),
      mode: button('bind-mode', 'overlay'),
      commit: button('bind-commit', 'commit'),
      learn: button('bind-learn', 'learn'),
      midi: span('bind-midi'),
      state: span('bind-state'),
      form: span('bind-form'),
      keys: '',
      off: () => {},
      range: null,
    };
    const entry = (): SiteEntry | undefined => this.host.table.get(id);
    slider.addEventListener('input', () => {
      const e = entry();
      if (e) this.host.writer.writeSite(e.site.id, Number(slider.value));
    });
    row.mode.addEventListener('click', () => {
      const next = this.host.writer.modeOf(id) === 'overlay' ? 'source-edit' : 'overlay';
      this.host.writer.setMode(id, next);
    });
    row.commit.addEventListener('click', () => {
      const e = entry();
      if (e) this.host.commit(e);
    });
    row.learn.addEventListener('click', () => {
      const e = entry();
      if (e) this.host.learn(e);
    });
    this.rows.set(id, row);
    return row;
  }

  private renderRow(row: Row): void {
    const e = this.host.table.get(row.id);
    if (!e) return;
    const { writer } = this.host;
    const overlay = writer.overlay(e.bindingId);
    const value = overlay ?? e.site.value;
    const meta = paramMeta(e.site, this.host.editors());
    row.label.textContent = e.key ?? this.host.lineExcerpt(e);
    if (!row.range) {
      const mag = Math.max(1, Math.abs(e.site.value) * 2);
      row.range = meta ? [meta.range[0], meta.range[1]] : [e.site.value < 0 ? -mag : 0, mag];
      row.slider.min = String(row.range[0]);
      row.slider.max = String(row.range[1]);
      row.slider.step = isIntegerLiteral(e.literalText) || meta?.curve === 'stepped' ? '1' : 'any';
    }
    row.slider.value = String(value);
    row.slider.disabled = e.state !== 'bound';
    row.value.textContent = formatValue(value);
    row.tier.textContent = tierBadge(e.site.tier);
    row.tier.dataset.tier = e.site.tier;
    const mode = writer.modeOf(e.bindingId);
    row.mode.textContent = mode;
    row.el.dataset.mode = mode;
    row.commit.hidden = mode !== 'overlay' || overlay === undefined;
    const m = this.host.mappingOf(e);
    row.midi.textContent = m ? `cc ${m.cc}${m.ch === undefined ? '' : ` ch ${m.ch}`}` : '';
    row.state.textContent = e.state === 'stale' ? 'STALE' : e.state === 'unbound' ? 'unbound' : '';
    row.el.dataset.state = e.state;
    const owner = this.host.ownerName(e);
    const ns = owner === undefined ? undefined : this.host.store.name(owner);
    row.form.textContent =
      ns?.state === 'failed'
        ? `failed: ${ns.diagnostic?.message ?? ''}`
        : ns?.state === 'blocked'
          ? `blocked on ${ns.blocked_on ?? '?'}`
          : '';
    this.count(`binding:${row.id}`, row.el);
  }

  private createNameRow(name: string): NameRow {
    const doc = this.el.ownerDocument;
    const el = doc.createElement('div');
    el.className = 'bind-name';
    el.dataset.name = name;
    const label = doc.createElement('span');
    label.className = 'bind-name-label';
    label.textContent = name;
    const value = doc.createElement('span');
    value.className = 'bind-name-value';
    const badge = doc.createElement('span');
    badge.className = 'bind-name-badge';
    el.append(label, value, badge);
    this.namesEl.appendChild(el);
    const nr: NameRow = { name, el, value, badge, off: () => {} };
    nr.off = this.host.store.subscribe([nameKey(name)], () => this.renderName(nr));
    this.nameRows.set(name, nr);
    return nr;
  }

  private renderName(nr: NameRow): void {
    const ns = this.host.store.name(nr.name);
    nr.value.textContent = ns?.value ?? '';
    nr.el.dataset.state = ns?.state ?? 'ok';
    nr.badge.textContent =
      ns?.state === 'failed'
        ? `failed: ${ns.diagnostic?.message ?? ''}`
        : ns?.state === 'blocked'
          ? `blocked on ${ns.blocked_on ?? '?'}`
          : '';
    this.count(`name:${nr.name}`, nr.el);
  }

  private count(key: string, el: HTMLElement): void {
    this.renders.set(key, (this.renders.get(key) ?? 0) + 1);
    this.host.onRender?.(key, el);
  }
}

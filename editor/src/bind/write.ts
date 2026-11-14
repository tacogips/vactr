// The ONE site-write path (design 15.1.6 "Two modes per slider", 13).
//
// Slider, drag, learned CC and (ED-PARAMS) parameter-editor handles all
// call `writeSite`. The binding's mode (per slider, default OVERLAY)
// decides the path:
// - OVERLAY sends `set-tweak {file, id, form_gen, value}`; the client
//   flushes the pending `doc-changed` first, stamps the epoch and
//   rate-limits. The text is never changed; the value shows beside the
//   literal as a widget.
// - SOURCE-EDIT and COMMIT make a VALIDATED text edit: the current text at
//   the mapped span must still spell the last-known literal, else the write
//   declines with a notice and sends nothing. The edit (the code sync sends
//   `doc-changed`) is followed by an `eval` of the owning form's span. At
//   most one eval is in flight per form; later values are kept, latest
//   wins, and re-applied when the eval returns.
//
// `stale-binding` replies: `stale-form-gen` keeps the value and re-sends
// it to the re-keyed site once fresh sites arrive; `edit-invalidated` and
// `unreconciled-edit` mark the binding STALE until the next table;
// `superseded-definition` drops the pending write.

import { ChangeSet } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { BindApi, SiteMode } from '../app/apis';
import type { Client } from '../protocol/client';
import type { EditorDecl, ParamMeta, SiteTier, Span, StaleBindingBody, WireSite } from '../protocol/types';
import { Utf8Index } from '../protocol/utf8';
import type { Range16, SiteEntry, SiteTable } from './sites';

// ------------------------------------------------------------ value math

/** The ParamMeta of a site's parameter, when the manifest declares one. */
export function paramMeta(site: WireSite, editors: readonly EditorDecl[] | undefined): ParamMeta | undefined {
  const call = site.call;
  if (!call?.param || !editors) return undefined;
  return editors.find((d) => d.name === call.name)?.params.find((p) => p.name === call.param);
}

const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));

/** A value -> its 0..1 position on the parameter's range and curve. */
export function toUnit(value: number, meta: ParamMeta): number {
  const [lo, hi] = meta.range;
  if (hi === lo) return 0;
  const v = clamp(value, Math.min(lo, hi), Math.max(lo, hi));
  if (meta.curve === 'log' && lo > 0 && hi > 0) return Math.log(v / lo) / Math.log(hi / lo);
  return (v - lo) / (hi - lo);
}

/** A 0..1 position -> a value on the parameter's range and curve. */
export function fromUnit(t: number, meta: ParamMeta): number {
  const [lo, hi] = meta.range;
  const u = clamp(t, 0, 1);
  if (meta.curve === 'log' && lo > 0 && hi > 0) return lo * (hi / lo) ** u;
  const v = lo + u * (hi - lo);
  return meta.curve === 'stepped' ? Math.round(v) : v;
}

/** True when the literal spells an integer (no `.` or exponent). */
export const isIntegerLiteral = (text: string): boolean => !/[.eE]/.test(text);

/** Rounds to at most 6 decimals, never producing `-0`. */
export const round6 = (v: number): number => Number(v.toFixed(6)) + 0;

/**
 * The literal text for `value`: an integer when the original literal had no
 * `.` (or the parameter is stepped); otherwise at most 6 decimals, keeping
 * a `.` so the literal stays a float.
 */
export function formatLiteral(value: number, original: string, meta?: ParamMeta): string {
  if (isIntegerLiteral(original) || meta?.curve === 'stepped') return String(Math.round(value) + 0);
  const s = String(round6(value));
  return /[.eE]/.test(s) ? s : `${s}.0`;
}

/** The tier badge text (design 15.1.6, 14.5.3). */
export function tierBadge(tier: SiteTier): string {
  if (tier === 'manual') return 're-evaluate to hear';
  if (tier === 'reeval') return 'next cycle';
  return '';
}

// ---------------------------------------------------------------- writer

export interface WriterHost {
  client: Client;
  file: string;
  table: SiteTable;
  view: EditorView;
  currentRevision(): number;
  /** A wire span of revision `rev` mapped to now, or null when touched. */
  map(span: Span, rev: number): Range16 | null;
  /** The current range of the form owning `range` (from `eval-result.forms`). */
  formOf(range: Range16): Range16 | null;
  editors(): readonly EditorDecl[] | undefined;
  notice(message: string): void;
  /** Bindings whose write state (overlay, mode, pending) changed. */
  changed(ids: Iterable<string>): void;
}

interface WriteState {
  mode: SiteMode;
  overlay?: number;
  /** The last value written (for a `stale-form-gen` re-send). */
  last?: number;
  /** A value waiting for fresh sites (`stale-form-gen`). */
  resend?: number;
}

interface InFlight {
  /** The evaluated form span and its revision (our own edit touches the eval-result form). */
  span: Span;
  rev: number;
  /** bindingId -> latest value, applied when the eval returns. */
  pending: Map<string, number>;
}

export class SiteWriter implements BindApi {
  private readonly host: WriterHost;
  private readonly states = new Map<string, WriteState>();
  private readonly inflight = new Map<string, InFlight>();
  /** Set by the routing module (`learn` is a routing concern). */
  learnHook: (entry: SiteEntry) => Promise<void> = async () => {};

  constructor(host: WriterHost) {
    this.host = host;
  }

  // -------------------------------------------------------------- BindApi

  writeSite(siteId: number, value: number): void {
    const e = this.host.table.byId(siteId);
    if (!e) {
      this.host.notice(`no site ${siteId}`);
      return;
    }
    this.write(e, value);
  }

  mode(siteId: number): SiteMode {
    const e = this.host.table.byId(siteId);
    return e ? this.state(e.bindingId).mode : 'overlay';
  }

  learn(siteId: number): Promise<void> {
    const e = this.host.table.byId(siteId);
    if (!e) return Promise.resolve();
    return this.learnHook(e);
  }

  siteById(id: number): WireSite | undefined {
    return this.host.table.byId(id)?.site;
  }

  // ------------------------------------------------------------ bindings

  modeOf(bindingId: string): SiteMode {
    return this.state(bindingId).mode;
  }

  setMode(bindingId: string, mode: SiteMode): void {
    this.state(bindingId).mode = mode;
    this.host.changed([bindingId]);
  }

  overlay(bindingId: string): number | undefined {
    return this.states.get(bindingId)?.overlay;
  }

  /** Every active overlay value. */
  overlays(): [string, number][] {
    const out: [string, number][] = [];
    for (const [id, s] of this.states) if (s.overlay !== undefined) out.push([id, s.overlay]);
    return out;
  }

  /** Drops a removed binding's write state. */
  forget(bindingId: string): void {
    this.states.delete(bindingId);
  }

  /** The write every front end reaches through `writeSite`. */
  write(e: SiteEntry, value: number): void {
    if (e.state !== 'bound') {
      this.host.notice(`${e.key ?? e.literalText}: ${e.state}, not written`);
      return;
    }
    const st = this.state(e.bindingId);
    st.last = value;
    if (st.mode === 'overlay') {
      this.host.client.setTweak(this.host.file, e.site.id, e.site.form_gen, value);
      st.overlay = value;
      this.host.changed([e.bindingId]);
      return;
    }
    this.sourceWrites([[e, value]]);
  }

  /** Writes the overlay value into the text (validated edit + form eval). */
  commit(siteId: number): void {
    const e = this.host.table.byId(siteId);
    if (!e) return;
    const st = this.state(e.bindingId);
    if (st.overlay === undefined) {
      this.host.notice('nothing to commit');
      return;
    }
    if (e.state !== 'bound') {
      this.host.notice(`${e.key ?? e.literalText}: ${e.state}, not committed`);
      return;
    }
    if (this.sourceWrites([[e, st.overlay]])) {
      delete st.overlay;
      this.host.changed([e.bindingId]);
    }
  }

  /** A `stale-binding` reply. */
  onStale(body: StaleBindingBody): void {
    if (typeof body.target !== 'number') return;
    const e = this.host.table.byId(body.target);
    if (!e) return;
    const st = this.state(e.bindingId);
    switch (body.reason) {
      case 'stale-form-gen':
        if (st.last !== undefined) st.resend = st.last;
        break;
      case 'edit-invalidated':
      case 'unreconciled-edit':
        this.host.table.markStale(e);
        break;
      case 'superseded-definition':
        delete st.resend;
        for (const f of this.inflight.values()) f.pending.delete(e.bindingId);
        break;
    }
    this.host.changed([e.bindingId]);
  }

  /** After a table refresh: re-send kept `stale-form-gen` values to re-keyed sites. */
  afterRefresh(changed: Iterable<string>): void {
    for (const id of changed) {
      const st = this.states.get(id);
      const e = this.host.table.get(id);
      if (!st || st.resend === undefined || !e || e.state !== 'bound') continue;
      const v = st.resend;
      delete st.resend;
      this.write(e, v);
    }
  }

  // ------------------------------------------------------------ internals

  private state(bindingId: string): WriteState {
    let st = this.states.get(bindingId);
    if (!st) {
      st = { mode: 'overlay' };
      this.states.set(bindingId, st);
    }
    return st;
  }

  /** Validated edits grouped by owning form; true when anything was written or queued. */
  private sourceWrites(list: [SiteEntry, number][]): boolean {
    const groups = new Map<string, { form: Range16; items: [SiteEntry, number][] }>();
    let any = false;
    for (const [e, v] of list) {
      const r = this.host.table.currentRange(e);
      if (!r) {
        this.host.notice(`${e.key ?? e.literalText}: the literal was edited, not written`);
        continue;
      }
      const busy = this.busyFor(r);
      if (busy) {
        busy.pending.set(e.bindingId, v);
        any = true;
        continue;
      }
      const form = this.host.formOf(r);
      if (!form) {
        this.host.notice(`${e.key ?? e.literalText}: its form was edited; re-evaluate first`);
        continue;
      }
      const key = String(form.from);
      const g = groups.get(key) ?? { form, items: [] };
      g.items.push([e, v]);
      groups.set(key, g);
    }
    for (const [key, g] of groups) {
      const busy = this.inflight.get(key);
      if (busy) {
        for (const [e, v] of g.items) busy.pending.set(e.bindingId, v);
        any = true;
      } else if (this.applyEdits(key, g.form, g.items)) any = true;
    }
    return any;
  }

  /** The in-flight eval whose form contains `range`. */
  private busyFor(range: Range16): InFlight | undefined {
    for (const slot of this.inflight.values()) {
      const r = this.host.map(slot.span, slot.rev);
      if (r && r.from <= range.from && range.to <= r.to) return slot;
    }
    return undefined;
  }

  private applyEdits(key: string, form: Range16, items: [SiteEntry, number][]): boolean {
    const { view, table } = this.host;
    const doc = view.state.doc;
    const specs: { e: SiteEntry; from: number; to: number; insert: string }[] = [];
    for (const [e, v] of items) {
      const r = table.currentRange(e);
      if (!r) continue;
      if (doc.sliceString(r.from, r.to) !== e.literalText) {
        this.host.notice(`${e.key ?? e.literalText}: the text changed, write-back declined`);
        continue;
      }
      const insert = formatLiteral(v, e.literalText, paramMeta(e.site, this.host.editors()));
      specs.push({ e, from: r.from, to: r.to, insert });
    }
    if (specs.length === 0) return false;
    specs.sort((a, b) => a.from - b.from);
    const cs = ChangeSet.of(
      specs.map((s) => ({ from: s.from, to: s.to, insert: s.insert })),
      doc.length,
    );
    view.dispatch({ changes: cs });
    const rev = this.host.currentRevision();
    const idx = new Utf8Index(view.state.doc.toString());
    for (const s of specs) {
      const from = cs.mapPos(s.from, -1);
      table.setAnchor(s.e, idx.spanToBytes(from, from + s.insert.length), rev, s.insert);
    }
    this.host.changed(specs.map((s) => s.e.bindingId));
    const span = idx.spanToBytes(cs.mapPos(form.from, -1), cs.mapPos(form.to, 1));
    this.evalForm(key, span, rev);
    return true;
  }

  private evalForm(key: string, span: Span, rev: number): void {
    const slot: InFlight = { span, rev, pending: new Map() };
    this.inflight.set(key, slot);
    const done = (): void => {
      if (this.inflight.get(key) === slot) this.inflight.delete(key);
      if (slot.pending.size === 0) return;
      const next: [SiteEntry, number][] = [];
      for (const [id, v] of slot.pending) {
        const e = this.host.table.get(id);
        if (e) next.push([e, v]);
      }
      this.sourceWrites(next);
    };
    const { client, file, view } = this.host;
    client.eval(file, view.state.doc.toString(), span).then(done, done);
  }
}

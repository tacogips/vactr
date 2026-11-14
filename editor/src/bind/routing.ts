// CC routing and learn (design 15.1.6 "CC routing", "Learn", 13.5, E4).
//
// An incoming CC `(ch, cc)` resolves to bindings, then each value goes
// through `writeSite` (the binding's slider mode decides the path), scaled
// 0..127 onto the ParamMeta range and curve, else onto 0..1.
// - Directive mode: the directive table's `bindings`, matched to a site by
//   `key`, else by span containment plus `site.call.param`, honoring `ch`
//   (default: the file's `#@ midi ch:`); plus mappings learned since the
//   last `eval-result` (the text holds them, the table does not yet).
// - ExternalFile mode: the editor-side set.
// Keys are full BindingKey spellings, so `hats.lpf`/`hats.hpf` and
// `hats.lpf.1`/`hats.lpf.2` never cross-talk.
//
// Learn: Directive mode sends `learn {binding: key | tweak id, cc, ch}` and
// applies the returned `directive-edit` only after verifying `expected`
// against the current text at the mapped span (the code sync then sends
// `doc-changed`); a mismatch is declined. ExternalFile mode records the
// mapping in the set and sends no `learn`.

import type { CodeSurface } from '../app/apis';
import type { MidiApi, MidiCcEvent } from '../app/apis';
import type { Client } from '../protocol/client';
import type { DirectiveEditBody, EditorDecl, Span, WireBinding, WireDirectives } from '../protocol/types';
import { identOf, type EditorBindingEntry, type MidiMapping, type Persistence } from './persistence';
import { contains, type Range16, type SiteEntry, type SiteTable } from './sites';
import { fromUnit, paramMeta, type SiteWriter } from './write';

export interface RouterHost {
  file: string;
  client: Client;
  table: SiteTable;
  writer: SiteWriter;
  persistence: Persistence;
  surface: CodeSurface;
  /** `deps.midi`, read at use time. */
  midi(): MidiApi | undefined;
  directives(): WireDirectives | undefined;
  /** The revision of the directive table's spans (the latest `eval-result`). */
  evalRevision(): number;
  map(span: Span, rev: number): Range16 | null;
  /** A current UTF-16 range -> current-revision bytes. */
  bytes(range: Range16): Span;
  editors(): readonly EditorDecl[] | undefined;
  notice(message: string): void;
  changed(ids: Iterable<string>): void;
  /** A verified `directive-edit` was applied to the text. */
  edited?(): void;
}

/** 0..127 onto the parameter's range and curve, else onto 0..1. */
export function scaleCc(value: number, meta: Parameters<typeof fromUnit>[1] | undefined): number {
  const t = Math.min(127, Math.max(0, value)) / 127;
  return meta ? fromUnit(t, meta) : t;
}

const chMatches = (want: number | undefined, got: number): boolean => want === undefined || want === got;

export class CcRouter {
  private readonly host: RouterHost;
  /** Directive-mode mappings learned since the last `eval-result`. */
  private readonly learned = new Map<string, MidiMapping>();
  private attached: MidiApi | undefined;
  private off: (() => void) | null = null;

  constructor(host: RouterHost) {
    this.host = host;
  }

  /** Subscribes to `deps.midi` when it appeared or changed. */
  sync(): void {
    const midi = this.host.midi();
    if (midi === this.attached) return;
    this.off?.();
    this.off = null;
    this.attached = midi;
    if (midi) this.off = midi.onCc((ev) => this.onCc(ev));
  }

  dispose(): void {
    this.off?.();
    this.off = null;
    this.attached = undefined;
  }

  /** A fresh `eval-result`: the directive table is authoritative again. */
  onEval(): void {
    this.learned.clear();
  }

  learnedOf(bindingId: string): MidiMapping | undefined {
    return this.learned.get(bindingId);
  }

  /** The mapping shown for a binding in the current mode. */
  mappingOf(e: SiteEntry): MidiMapping | undefined {
    if (this.host.persistence.mode === 'external-file') return this.host.persistence.set.get(this.ident(e).id)?.midi;
    const l = this.learned.get(e.bindingId);
    if (l) return l;
    const b = this.host.directives()?.bindings?.find((x) => x.cc !== undefined && this.siteFor(x) === e);
    if (!b || b.cc === undefined) return undefined;
    const ch = b.ch ?? this.host.directives()?.file_level.midi_ch;
    return ch === undefined ? { cc: b.cc } : { cc: b.cc, ch };
  }

  /** One CC event: every matching binding gets a scaled write. */
  onCc(ev: MidiCcEvent): void {
    for (const e of this.route(ev)) {
      const v = scaleCc(ev.value, paramMeta(e.site, this.host.editors()));
      this.host.writer.writeSite(e.site.id, v);
    }
  }

  /** The bindings a CC event reaches. */
  route(ev: MidiCcEvent): SiteEntry[] {
    const out = new Set<SiteEntry>();
    const dirs = this.host.directives();
    const fileCh = dirs?.file_level.midi_ch;
    if (this.host.persistence.mode === 'external-file') {
      for (const e of this.host.table.all()) {
        const m = this.host.persistence.set.get(this.ident(e).id)?.midi;
        if (m && m.cc === ev.cc && chMatches(m.ch ?? fileCh, ev.ch)) out.add(e);
      }
      return [...out];
    }
    for (const b of dirs?.bindings ?? []) {
      if (b.cc !== ev.cc || !chMatches(b.ch ?? fileCh, ev.ch)) continue;
      const e = this.siteFor(b);
      if (e) out.add(e);
    }
    for (const [id, m] of this.learned) {
      if (m.cc !== ev.cc || !chMatches(m.ch, ev.ch)) continue;
      const e = this.host.table.get(id);
      if (e) out.add(e);
    }
    return [...out];
  }

  /** The binding a directive-table binding names: by key, else span containment plus param. */
  siteFor(b: WireBinding): SiteEntry | undefined {
    const { table } = this.host;
    if (b.key !== undefined) return table.byKey(b.key);
    const target = this.host.map(b.span, this.host.evalRevision());
    if (!target) return undefined;
    for (const e of table.all()) {
      if (e.site.call?.param !== b.param) continue;
      const r = table.currentRange(e);
      if (r && contains(target, r)) return e;
      const head = this.host.map(e.site.call.head, e.rev);
      if (head && head.from === target.from && head.to === target.to) return e;
    }
    return undefined;
  }

  /** The persistence ident of a binding: its key, else `{span, param}` in current bytes. */
  ident(e: SiteEntry): { id: string; entry: Pick<EditorBindingEntry, 'key' | 'span' | 'param'> } {
    if (e.key !== undefined) return { id: e.key, entry: { key: e.key } };
    const r = this.host.table.currentRange(e);
    const span = r ? this.host.bytes(r) : e.anchor.span;
    const entry = { span: [span.start, span.end] as [number, number], param: e.site.call?.param ?? 'value' };
    return { id: identOf(entry), entry };
  }

  /** The current panel as ExternalFile entries (directive bindings, learned mappings, overlays). */
  snapshot(): EditorBindingEntry[] {
    const out = new Map<string, EditorBindingEntry>();
    const put = (e: SiteEntry, midi?: MidiMapping): void => {
      const { id, entry } = this.ident(e);
      const prev = out.get(id);
      const next: EditorBindingEntry = { ...entry, panel: true };
      const m = midi ?? prev?.midi;
      if (m) next.midi = m;
      const overlay = this.host.writer.overlay(e.bindingId);
      if (overlay !== undefined) next.overlay = overlay;
      out.set(id, next);
    };
    const dirs = this.host.directives();
    for (const b of dirs?.bindings ?? []) {
      const e = this.siteFor(b);
      const ch = b.ch ?? dirs?.file_level.midi_ch;
      const midi = b.cc === undefined ? undefined : ch === undefined ? { cc: b.cc } : { cc: b.cc, ch };
      if (e) put(e, midi);
      else if (b.key !== undefined) out.set(b.key, { key: b.key, panel: true, ...(midi ? { midi } : {}) });
    }
    for (const [id, m] of this.learned) {
      const e = this.host.table.get(id);
      if (e) put(e, m);
    }
    for (const [id] of this.host.writer.overlays()) {
      const e = this.host.table.get(id);
      if (e) put(e);
    }
    return [...out.values()];
  }

  /** Learn for one binding (the `BindApi.learn` path). */
  async learn(e: SiteEntry): Promise<void> {
    const midi = this.host.midi();
    if (!midi) {
      this.host.notice('MIDI is not enabled');
      return;
    }
    let got: { cc: number; ch: number };
    try {
      got = await midi.learnNext();
    } catch {
      return; // cancelled or superseded
    }
    if (this.host.persistence.mode === 'external-file') {
      const { id, entry } = this.ident(e);
      const prev = this.host.persistence.set.get(id);
      this.host.persistence.set.upsert({ ...(prev ?? { panel: true }), ...entry, midi: { cc: got.cc, ch: got.ch } });
      this.host.changed([e.bindingId]);
      return;
    }
    let reply;
    try {
      reply = await this.host.client.learn(this.host.file, e.key ?? e.site.id, got.cc, got.ch);
    } catch (err) {
      this.host.notice(`learn failed: ${String(err)}`);
      return;
    }
    if (reply.kind === 'directive-edit') {
      if (this.applyDirectiveEdit(reply.body)) {
        this.learned.set(e.bindingId, { cc: got.cc, ch: got.ch });
        this.host.changed([e.bindingId]);
      }
    } else if (reply.kind === 'protocol-error') {
      this.host.notice(`learn failed: ${reply.body.message}`);
    } else if (reply.kind === 'stale-binding') {
      this.host.notice(`learn: binding is stale (${reply.body.reason})`);
    }
  }

  /**
   * Applies a `directive-edit` after verifying `expected`; false when
   * declined. Only Directive mode edits `#@` text (E4).
   */
  applyDirectiveEdit(body: DirectiveEditBody): boolean {
    if (body.file !== this.host.file) return false;
    if (this.host.persistence.mode !== 'directive') {
      this.host.notice('directive edit declined: bindings are kept in the external file');
      return false;
    }
    const r = this.host.map(body.span, body.doc_revision);
    const { surface } = this.host;
    if (!r || surface.state.sliceDoc(r.from, r.to) !== body.expected) {
      this.host.notice('directive edit declined: the text changed');
      return false;
    }
    surface.deferSourceWrite(r, () => {
      if (surface.state.sliceDoc(r.from, r.to) !== body.expected) return;
      surface.dispatch({ changes: { from: r.from, to: r.to, insert: body.text } });
    });
    this.host.edited?.();
    return true;
  }
}

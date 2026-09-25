// The binding site table (design 15.1.6 "Sites and keys", 13, 13.5,
// 14.5.8 "Binding identity").
//
// Sites come only from the session: `eval-result.sites` (the file's whole
// table) and `bindings.sites` (the sites of the recomputed forms). Each
// site the editor has seen is a BINDING with a stable `bindingId`:
// - a site with `key` is keyed by its `BindingKey` spelling;
// - any other site is keyed by its tracked span, mapped through the
//   revision history on every edit.
//
// When a fresh table arrives, every binding whose tracked span maps cleanly
// onto a fresh site's span re-keys to that site's `TweakId` (a keyed
// binding landing on a same-family key with a new ordinal MIGRATES its key).
// A keyed binding whose span did not land re-keys by key. Otherwise the
// binding is `unbound`, or `stale` when its mapping was touched. Duplicate
// literals have distinct spans, hence distinct bindings.

import type { Span, WireSite } from '../protocol/types';

export type BindingState = 'bound' | 'unbound' | 'stale';

/** A current UTF-16 range. */
export interface Range16 {
  from: number;
  to: number;
}

export interface SiteEntry {
  readonly bindingId: string;
  /** The BindingKey spelling, when the site has one. */
  key?: string;
  /** The latest site (kept for display when unbound or stale). */
  site: WireSite;
  /** The revision `site.span` is in. */
  rev: number;
  /** The literal's last known wire span and its revision. */
  anchor: { span: Span; rev: number };
  /** The literal text at the anchor when it was last read. */
  literalText: string;
  state: BindingState;
  /** Consecutive fresh tables that did not match this binding. */
  missed: number;
}

export interface SiteTableHost {
  /** A wire span of revision `rev` -> the current range, or null when touched or gone. */
  map(span: Span, rev: number): Range16 | null;
  /** The current document text in `[from, to)`. */
  text(from: number, to: number): string;
  /** An unmatched binding the owner still needs (a learned mapping, an overlay). */
  retain?(entry: SiteEntry): boolean;
  /** The current range of the form owning `range`, for `bindings` replacement by form. */
  formOf?(range: Range16): Range16 | null;
}

export interface Refresh {
  /** Bindings whose site, key or state changed. */
  changed: Set<string>;
  added: string[];
  removed: string[];
  /** Keyed migrations `[bindingId, oldKey, newKey]`. */
  migrated: [string, string, string][];
}

interface Fresh {
  s: WireSite;
  range: Range16 | null;
}

const sameRange = (a: Range16 | null, b: Range16 | null): boolean =>
  a !== null && b !== null && a.from === b.from && a.to === b.to;

export const contains = (outer: Range16, inner: Range16): boolean =>
  outer.from <= inner.from && inner.to <= outer.to;

export class SiteTable {
  private readonly host: SiteTableHost;
  private readonly entries = new Map<string, SiteEntry>();
  private counter = 0;

  constructor(host: SiteTableHost) {
    this.host = host;
  }

  get(bindingId: string): SiteEntry | undefined {
    return this.entries.get(bindingId);
  }

  all(): SiteEntry[] {
    return [...this.entries.values()];
  }

  /** The binding currently holding tweak id `id` (bound ones first). */
  byId(id: number): SiteEntry | undefined {
    let found: SiteEntry | undefined;
    for (const e of this.entries.values()) {
      if (e.site.id !== id) continue;
      if (e.state === 'bound') return e;
      found ??= e;
    }
    return found;
  }

  byKey(key: string): SiteEntry | undefined {
    for (const e of this.entries.values()) if (e.key === key) return e;
    return undefined;
  }

  /** The binding's literal mapped to now, or null when touched. */
  currentRange(e: SiteEntry): Range16 | null {
    return this.host.map(e.anchor.span, e.anchor.rev);
  }

  /** Records the literal the editor itself just wrote. */
  setAnchor(e: SiteEntry, span: Span, rev: number, literalText: string): void {
    e.anchor = { span, rev };
    e.literalText = literalText;
    e.state = 'bound';
  }

  /** A `stale-binding` reply: STALE until the next table refreshes the site. */
  markStale(e: SiteEntry): void {
    e.state = 'stale';
  }

  /** After an edit: bound bindings whose literal was touched become STALE. */
  refreshMapping(): Set<string> {
    const changed = new Set<string>();
    for (const e of this.entries.values()) {
      if (e.state === 'bound' && this.currentRange(e) === null) {
        e.state = 'stale';
        changed.add(e.bindingId);
      }
    }
    return changed;
  }

  /** `eval-result.sites`: the file's whole table at revision `rev`. */
  applyEval(sites: readonly WireSite[], rev: number): Refresh {
    const r: Refresh = { changed: new Set(), added: [], removed: [], migrated: [] };
    const fresh: Fresh[] = sites.map((s) => ({ s, range: this.host.map(s.span, rev) }));
    const claimed = new Set<WireSite>();
    const matched = new Set<string>();
    const current = new Map<string, Range16 | null>();
    for (const e of this.entries.values()) current.set(e.bindingId, this.currentRange(e));

    // Pass 1: a clean mapping onto a fresh span (migrates keys).
    for (const e of this.entries.values()) {
      const cur = current.get(e.bindingId) ?? null;
      const f = fresh.find((x) => !claimed.has(x.s) && sameRange(x.range, cur));
      if (!f) continue;
      claimed.add(f.s);
      matched.add(e.bindingId);
      this.bind(e, f, rev, r);
    }
    // Pass 2: keyed bindings re-key by key.
    for (const e of this.entries.values()) {
      if (matched.has(e.bindingId) || e.key === undefined) continue;
      const f = fresh.find((x) => !claimed.has(x.s) && x.s.key === e.key);
      if (!f) continue;
      claimed.add(f.s);
      matched.add(e.bindingId);
      this.bind(e, f, rev, r);
    }
    // Unmatched: unbound, or stale when touched; dropped after a second miss.
    for (const e of [...this.entries.values()]) {
      if (matched.has(e.bindingId)) continue;
      this.miss(e, current.get(e.bindingId) ?? null, r);
    }
    for (const f of fresh) if (!claimed.has(f.s)) this.create(f, rev, r);
    return r;
  }

  /**
   * `bindings.sites`: the sites of the recomputed forms, in revision `rev`
   * (the session remaps spans on every `doc-changed`). Replaces by form:
   * a binding of a recomputed form that no fresh site matches is unbound.
   */
  applySites(sites: readonly WireSite[], rev: number): Refresh {
    const r: Refresh = { changed: new Set(), added: [], removed: [], migrated: [] };
    const touchedForms: Range16[] = [];
    const matched = new Set<string>();
    for (const s of sites) {
      const f: Fresh = { s, range: this.host.map(s.span, rev) };
      if (f.range) {
        const form = this.host.formOf?.(f.range);
        if (form) touchedForms.push(form);
      }
      let e = [...this.entries.values()].find((x) => !matched.has(x.bindingId) && x.site.id === s.id);
      e ??= [...this.entries.values()].find(
        (x) => !matched.has(x.bindingId) && sameRange(this.currentRange(x), f.range),
      );
      if (!e && s.key !== undefined) e = [...this.entries.values()].find((x) => !matched.has(x.bindingId) && x.key === s.key);
      if (e) {
        matched.add(e.bindingId);
        this.bind(e, f, rev, r);
      } else {
        matched.add(this.create(f, rev, r).bindingId);
      }
    }
    for (const e of this.entries.values()) {
      if (matched.has(e.bindingId) || e.state === 'unbound') continue;
      const cur = this.currentRange(e);
      if (cur && touchedForms.some((form) => contains(form, cur))) {
        e.state = 'unbound';
        r.changed.add(e.bindingId);
      }
    }
    return r;
  }

  private bind(e: SiteEntry, f: Fresh, rev: number, r: Refresh): void {
    const before = `${e.site.id}|${e.site.form_gen}|${e.site.value}|${e.state}|${e.key ?? ''}`;
    if (f.s.key !== undefined && e.key !== undefined && f.s.key !== e.key) {
      r.migrated.push([e.bindingId, e.key, f.s.key]);
    }
    // A reactive pass publishes sites without keys: keep the known one.
    const key = f.s.key ?? e.key;
    e.site = key === undefined ? f.s : { ...f.s, key };
    if (key !== undefined) e.key = key;
    e.rev = rev;
    e.missed = 0;
    e.anchor = { span: f.s.span, rev };
    if (f.range) {
      e.literalText = this.host.text(f.range.from, f.range.to);
      e.state = 'bound';
    } else {
      // The literal was edited after revision `rev`.
      e.state = 'stale';
    }
    const after = `${e.site.id}|${e.site.form_gen}|${e.site.value}|${e.state}|${e.key ?? ''}`;
    if (before !== after) r.changed.add(e.bindingId);
  }

  private miss(e: SiteEntry, cur: Range16 | null, r: Refresh): void {
    e.missed += 1;
    if (e.missed > 1 && !(this.host.retain?.(e) ?? false)) {
      this.entries.delete(e.bindingId);
      r.removed.push(e.bindingId);
      return;
    }
    const state: BindingState = cur === null ? 'stale' : 'unbound';
    if (e.state !== state) {
      e.state = state;
      r.changed.add(e.bindingId);
    }
  }

  private create(f: Fresh, rev: number, r: Refresh): SiteEntry {
    this.counter += 1;
    const e: SiteEntry = {
      bindingId: `b${this.counter}`,
      site: f.s,
      rev,
      anchor: { span: f.s.span, rev },
      literalText: f.range ? this.host.text(f.range.from, f.range.to) : '',
      state: f.range ? 'bound' : 'stale',
      missed: 0,
    };
    if (f.s.key !== undefined) e.key = f.s.key;
    this.entries.set(e.bindingId, e);
    r.added.push(e.bindingId);
    r.changed.add(e.bindingId);
    return e;
  }
}

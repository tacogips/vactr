// The batch store (design 15.1.6 "Reactive displays", 14.5.5).
//
// `apply(msg)` applies one whole `bindings`, `eval-result`, `diag`,
// `levels`, `tempo` or `manifest` message, then notifies each subscriber
// whose key set intersects the changed keys EXACTLY ONCE. Subscribers never
// observe a partially applied message: state is updated first, callbacks
// run after, and a message applied from inside a callback is queued until
// the current notification round ends. Nothing but completed messages is
// stored, so a provisional value can never be displayed.
//
// Keys: `name:<n>`, `site:<id>`, `slot:<s>`, `sites`, `diag`, `levels`,
// `tempo`, `manifest`, `directives`, `forms`.

import type {
  BindingsBody,
  DiagBody,
  Diagnostic,
  EvalResultBody,
  FormStateKind,
  LevelsBody,
  ManifestBody,
  ServerMsg,
  TempoBody,
  WireDirectives,
  WireForm,
  WireSite,
} from './types';

export interface NameState {
  /** The last completed value (display text). */
  value?: string;
  form_gen?: number;
  state: FormStateKind;
  blocked_on?: string;
  diagnostic?: Diagnostic;
}

export type StoreCallback = (changed: ReadonlySet<string>, store: Store) => void;

interface Sub {
  keys: ReadonlySet<string>;
  cb: StoreCallback;
  active: boolean;
}

export const nameKey = (n: string): string => `name:${n}`;
export const siteKey = (id: number): string => `site:${id}`;
export const slotKey = (s: string): string => `slot:${s}`;

export class Store {
  private readonly siteMap = new Map<number, WireSite>();
  private readonly siteFile = new Map<number, string>();
  private readonly nameMap = new Map<string, NameState>();
  private readonly fileDiags = new Map<string, Diagnostic[]>();
  private readonly slotDiags = new Map<string, Diagnostic[]>();
  private readonly directiveMap = new Map<string, WireDirectives>();
  private readonly formMap = new Map<string, WireForm[]>();
  private tempoBody: TempoBody | null = null;
  private levelsBody: LevelsBody | null = null;
  private manifestBody: ManifestBody | null = null;
  private lastPass = 0;
  private subs: Sub[] = [];
  private queue: ServerMsg[] = [];
  private notifying = false;
  private readonly onError: (e: unknown) => void;

  constructor(opts: { onError?: (e: unknown) => void } = {}) {
    this.onError = opts.onError ?? ((e) => console.error('store subscriber failed', e));
  }

  // -------------------------------------------------------------- reads

  site(id: number): WireSite | undefined {
    return this.siteMap.get(id);
  }

  /** Every site, in id order. */
  sites(): WireSite[] {
    return [...this.siteMap.values()].sort((a, b) => a.id - b.id);
  }

  /** The sites of one file's latest `eval-result` (plus later upserts). */
  sitesOf(file: string): WireSite[] {
    return this.sites().filter((s) => this.siteFile.get(s.id) === file);
  }

  fileOfSite(id: number): string | undefined {
    return this.siteFile.get(id);
  }

  name(n: string): NameState | undefined {
    return this.nameMap.get(n);
  }

  names(): ReadonlyMap<string, NameState> {
    return this.nameMap;
  }

  /** Static diagnostics of `file`'s latest `eval-result`. */
  diagnostics(file: string): Diagnostic[] {
    return this.fileDiags.get(file) ?? [];
  }

  /** Runtime diagnostics per slot (`diag`). */
  runtimeDiagnostics(): ReadonlyMap<string, Diagnostic[]> {
    return this.slotDiags;
  }

  directives(file: string): WireDirectives | undefined {
    return this.directiveMap.get(file);
  }

  forms(file: string): WireForm[] {
    return this.formMap.get(file) ?? [];
  }

  get tempo(): TempoBody | null {
    return this.tempoBody;
  }

  get levels(): LevelsBody | null {
    return this.levelsBody;
  }

  get manifest(): ManifestBody | null {
    return this.manifestBody;
  }

  /** The `pass` of the last applied `bindings` batch. */
  get pass(): number {
    return this.lastPass;
  }

  // ------------------------------------------------------ subscriptions

  subscribe(keys: Iterable<string>, cb: StoreCallback): () => void {
    const sub: Sub = { keys: new Set(keys), cb, active: true };
    this.subs.push(sub);
    return () => {
      sub.active = false;
      this.subs = this.subs.filter((s) => s !== sub);
    };
  }

  // ------------------------------------------------------------ applying

  /** Applies one server message atomically, then notifies. */
  apply(msg: ServerMsg): void {
    this.queue.push(msg);
    if (this.notifying) return;
    while (this.queue.length > 0) {
      const next = this.queue.shift() as ServerMsg;
      const changed = this.applyOne(next);
      if (changed.size > 0) this.notify(changed);
    }
  }

  private notify(changed: ReadonlySet<string>): void {
    this.notifying = true;
    try {
      for (const sub of [...this.subs]) {
        if (!sub.active) continue;
        let hit = false;
        for (const k of sub.keys) {
          if (changed.has(k)) {
            hit = true;
            break;
          }
        }
        if (!hit) continue;
        try {
          sub.cb(changed, this);
        } catch (e) {
          this.onError(e);
        }
      }
    } finally {
      this.notifying = false;
    }
  }

  private applyOne(msg: ServerMsg): Set<string> {
    switch (msg.kind) {
      case 'bindings':
        return this.applyBindings(msg.body);
      case 'eval-result':
        return this.applyEval(msg.body);
      case 'diag':
        return this.applyDiag(msg.body);
      case 'levels':
        this.levelsBody = msg.body;
        return new Set(['levels']);
      case 'tempo':
        this.tempoBody = msg.body;
        return new Set(['tempo']);
      case 'manifest':
        this.manifestBody = msg.body;
        return new Set(['manifest']);
      default:
        return new Set();
    }
  }

  private upsertSite(site: WireSite, file: string | undefined, changed: Set<string>): void {
    const prev = this.siteMap.get(site.id);
    // A reactive pass publishes sites without their BindingKey; the key is
    // a function of the directive table, which a pass never changes.
    const next = prev?.key !== undefined && site.key === undefined ? { ...site, key: prev.key } : site;
    this.siteMap.set(site.id, next);
    if (file !== undefined) this.siteFile.set(site.id, file);
    changed.add(siteKey(site.id));
  }

  private applyBindings(b: BindingsBody): Set<string> {
    const changed = new Set<string>();
    this.lastPass = b.pass;
    for (const c of b.changed) {
      const prev = this.nameMap.get(c.name);
      this.nameMap.set(c.name, { ...(prev ?? { state: 'ok' }), value: c.value, form_gen: c.form_gen });
      changed.add(nameKey(c.name));
    }
    for (const s of b.sites) this.upsertSite(s, this.siteFile.get(s.id), changed);
    if (b.sites.length > 0) changed.add('sites');
    for (const st of b.states) {
      const prev = this.nameMap.get(st.name);
      const next: NameState = { state: st.state, value: st.value };
      const gen = prev?.form_gen;
      if (gen !== undefined) next.form_gen = gen;
      if (st.state === 'blocked' && st.blocked_on !== undefined) next.blocked_on = st.blocked_on;
      if (st.state === 'failed' && st.diagnostic !== undefined) next.diagnostic = st.diagnostic;
      this.nameMap.set(st.name, next);
      changed.add(nameKey(st.name));
    }
    return changed;
  }

  private applyEval(b: EvalResultBody): Set<string> {
    const changed = new Set<string>(['diag', 'forms', 'directives', 'sites']);
    // The eval-result carries the file's whole site table: drop the old one.
    for (const [id, file] of this.siteFile) {
      if (file === b.file) {
        this.siteMap.delete(id);
        this.siteFile.delete(id);
        changed.add(siteKey(id));
      }
    }
    for (const s of b.sites) this.upsertSite(s, b.file, changed);
    this.fileDiags.set(b.file, b.diagnostics);
    this.directiveMap.set(b.file, b.directives);
    this.formMap.set(b.file, b.forms);
    return changed;
  }

  private applyDiag(b: DiagBody): Set<string> {
    const changed = new Set<string>(['diag']);
    for (const c of b.clear) {
      this.slotDiags.delete(c.slot);
      changed.add(slotKey(c.slot));
    }
    for (const d of b.add) {
      const slot = d.slot ?? '';
      // A fresh array: a reader may still hold the previous one.
      this.slotDiags.set(slot, [...(this.slotDiags.get(slot) ?? []), d]);
      changed.add(slotKey(slot));
    }
    return changed;
  }
}

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
  TransportSample,
  WireDirectives,
  WireForm,
  WireSite,
  SongApplied,
  SongCandidateFailed,
  SongSound,
  SongTransportStatus,
} from './types';

export interface SongDocumentState {
  draftRevision: number;
  pending?: { request: number; revision: number; epoch?: string; ready: boolean };
  applied?: SongApplied;
  failure?: SongCandidateFailed;
  transport?: SongTransportStatus;
  instrumentMutes?: { sound: SongSound; muted: boolean; application_frame: string }[];
}

/** Decimal strings have already passed canonical u64 validation. */
function frameOlder(next: string, previous: string): boolean {
  return next.length < previous.length || (next.length === previous.length && next < previous);
}
function soundKey(sound: SongSound): string {
  switch (sound.kind) {
    case 'builtin': return JSON.stringify(['builtin', sound.name]);
    case 'instrument': return JSON.stringify(['instrument', sound.id]);
    case 'sample': return JSON.stringify(['sample', sound.path, sound.file ?? null]);
    case 'buffer': return JSON.stringify(['buffer', sound.id]);
  }
}
export function songSelectorKey(selector: { family: SongSound[] }): string {
  return JSON.stringify(selector.family.map(soundKey).sort());
}
const transportOrder = { prepared: 0, playing: 1, draining: 2, ended: 3, failed: 4 };

type CorrelatedMessage = ServerMsg & { re?: number };
type QueuedChange = CorrelatedMessage | (() => Set<string>);
export const songKey = (file: string): string => `song:${file}`;

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
  seq: number;
}

export const nameKey = (n: string): string => `name:${n}`;
export const siteKey = (id: number): string => `site:${id}`;
export const slotKey = (s: string): string => `slot:${s}`;

// Correlated replies stay outside the telemetry drop policy.
function isTelemetry(msg: QueuedChange): boolean {
  return typeof msg !== 'function' && msg.re === undefined &&
    (msg.kind === 'tempo' || msg.kind === 'levels' || msg.kind === 'playing');
}
function older(next: TransportSample | undefined, prev: TransportSample | null | undefined): boolean {
  return !!next && !!prev && next.epoch === prev.epoch && next.sample_time < prev.sample_time;
}

export class Store {
  private readonly siteMap = new Map<number, WireSite>();
  private readonly siteFile = new Map<number, string>();
  private readonly nameMap = new Map<string, NameState>();
  private readonly fileDiags = new Map<string, Diagnostic[]>();
  private readonly slotDiags = new Map<string, Diagnostic[]>();
  private readonly directiveMap = new Map<string, WireDirectives>();
  private readonly formMap = new Map<string, WireForm[]>();
  private readonly songMap = new Map<string, SongDocumentState>();
  private readonly songRequests = new Map<number, string>();
  private tempoBody: TempoBody | null = null;
  private levelsBody: LevelsBody | null = null;
  private manifestBody: ManifestBody | null = null;
  private lastPass = 0;
  private subs = new Set<Sub>();
  private readonly index = new Map<string, Set<Sub>>();
  private nextSubSeq = 0;
  private queue: QueuedChange[] = [];
  private notifying = false;
  private disposed = false;
  private timedSample: TransportSample | null = null;
  private dropped = 0;
  private coalesced = 0;
  readonly stats = { notifyVisits: 0 };
  get transportSample(): TransportSample | null { return this.timedSample; }
  get synchronized(): boolean { return this.timedSample !== null; }
  get queueStats(): { telemetryQueued: number; dropped: number; coalesced: number } {
    return { telemetryQueued: this.queue.filter(isTelemetry).length, dropped: this.dropped, coalesced: this.coalesced };
  }
  dispose(): void {
    this.disposed = true;
    this.queue.length = 0;
    for (const sub of this.subs) sub.active = false;
    this.subs.clear();
    this.index.clear();
    this.siteMap.clear(); this.siteFile.clear(); this.nameMap.clear();
    this.fileDiags.clear(); this.slotDiags.clear(); this.directiveMap.clear(); this.formMap.clear();
    this.songMap.clear(); this.songRequests.clear();
    this.tempoBody = null; this.timedSample = null; this.levelsBody = null; this.manifestBody = null;
  }
  private readonly onError: (e: unknown) => void;

  constructor(opts: { onError?: (e: unknown) => void } = {}) {
    this.onError = opts.onError ?? ((e) => console.error('store subscriber failed', e));
  }

  // -------------------------------------------------------------- reads

  song(file: string): Readonly<SongDocumentState> | undefined { return this.songMap.get(file); }

  beginSongApply(file: string, revision: number, request: number, notify = true): void {
    this.changeSong(() => {
      const previous = this.songMap.get(file);
      if (previous?.pending) this.songRequests.delete(previous.pending.request);
      this.songRequests.set(request, file);
      this.songMap.set(file, { ...previous, draftRevision: revision, failure: undefined,
        pending: { request, revision, ready: false } });
      return notify ? new Set([songKey(file)]) : new Set();
    });
  }

  publishSongApply(request: number): void {
    this.changeSong(() => {
      const file = this.songRequests.get(request);
      const pending = file === undefined ? undefined : this.songMap.get(file)?.pending;
      return file !== undefined && pending?.request === request && !pending.ready
        ? new Set([songKey(file)]) : new Set();
    });
  }

  cancelSongApply(request: number): void {
    this.changeSong(() => {
      const file = this.songRequests.get(request);
      if (file === undefined) return new Set();
      this.songRequests.delete(request);
      const previous = this.songMap.get(file);
      if (previous?.pending?.request !== request) return new Set();
      const { pending: _, ...next } = previous;
      this.songMap.set(file, next);
      return new Set([songKey(file)]);
    });
  }

  songDocumentChanged(file: string, revision: number): void {
    this.changeSong(() => {
      const previous = this.songMap.get(file);
      if (!previous || revision <= previous.draftRevision) return new Set();
      if (previous.pending) this.songRequests.delete(previous.pending.request);
      const { pending: _, failure: __, ...active } = previous;
      this.songMap.set(file, { ...active, draftRevision: revision });
      return new Set([songKey(file)]);
    });
  }

  private changeSong(change: () => Set<string>): void {
    if (this.disposed) return;
    this.queue.push(change);
    this.drainChanges();
  }

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
    if (this.disposed) return () => {};
    const sub: Sub = { keys: new Set(keys), cb, active: true, seq: this.nextSubSeq++ };
    this.subs.add(sub);
    for (const key of sub.keys) {
      let bucket = this.index.get(key);
      if (!bucket) { bucket = new Set(); this.index.set(key, bucket); }
      bucket.add(sub);
    }
    return () => {
      if (!sub.active) return;
      sub.active = false;
      this.subs.delete(sub);
      for (const key of sub.keys) {
        const bucket = this.index.get(key);
        bucket?.delete(sub);
        if (bucket?.size === 0) this.index.delete(key);
      }
    };
  }

  // ------------------------------------------------------------ applying

  /** Applies one server message atomically, then notifies. */
  apply(msg: CorrelatedMessage): void {
    if (this.disposed) return;
    if (isTelemetry(msg)) {
      if (msg.kind === 'tempo' || msg.kind === 'levels') {
        const i = this.queue.findIndex((m) => typeof m !== 'function' && isTelemetry(m) && m.kind === msg.kind);
        // Do not replace a newer same-epoch transport snapshot with a stale arrival.
        const prev = i >= 0 ? this.queue[i] : undefined;
        if (msg.kind === 'tempo' && typeof prev !== 'function' && prev?.kind === 'tempo' && older(msg.body.transport, prev.body.transport)) return;
        if (i >= 0) { this.queue.splice(i, 1); this.coalesced += 1; }
      }
      if (this.queue.filter(isTelemetry).length >= 64) {
        this.queue.splice(this.queue.findIndex(isTelemetry), 1);
        this.dropped += 1;
      }
    }
    this.queue.push(msg);
    this.drainChanges();
  }

  private drainChanges(): void {
    if (this.notifying) return;
    while (!this.disposed && this.queue.length > 0) {
      const next = this.queue.shift()!;
      const changed = typeof next === 'function' ? next() : this.applyOne(next);
      if (changed.size > 0) this.notify(changed);
    }
  }

  private notify(changed: ReadonlySet<string>): void {
    this.notifying = true;
    try {
      const candidates = new Set<Sub>();
      for (const key of changed) for (const sub of this.index.get(key) ?? []) candidates.add(sub);
      const snapshot = [...candidates].sort((a, b) => a.seq - b.seq);
      for (const sub of snapshot) {
        if (!sub.active) continue;
        this.stats.notifyVisits += 1;
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

  private applyOne(msg: CorrelatedMessage): Set<string> {
    switch (msg.kind) {
      case 'song-instrument-muted':
      case 'song-transport-state': {
        const entry = [...this.songMap.entries()].find(([, state]) => state.applied?.epoch === msg.body.epoch);
        if (!entry) return new Set();
        const [file, previous] = entry;
        if (msg.kind === 'song-transport-state') {
          if (previous.transport && transportOrder[msg.body.state] <= transportOrder[previous.transport.state]) return new Set();
          this.songMap.set(file, { ...previous, transport: msg.body });
        } else {
          const mutes = new Map((previous.instrumentMutes ?? []).map((mute) => [soundKey(mute.sound), mute]));
          let changed = false;
          for (const sound of msg.body.selector.family) {
            const key = soundKey(sound);
            const old = mutes.get(key);
            if (old && (frameOlder(msg.body.application_frame, old.application_frame) ||
              (old.application_frame === msg.body.application_frame && old.muted === msg.body.muted))) continue;
            mutes.set(key, { sound, muted: msg.body.muted, application_frame: msg.body.application_frame });
            changed = true;
          }
          if (!changed) return new Set();
          this.songMap.set(file, { ...previous, instrumentMutes: [...mutes.values()] });
        }
        return new Set([songKey(file)]);
      }
      case 'song-candidate-ready':
      case 'song-candidate-applied':
      case 'song-candidate-failed': {
        const file = msg.re === undefined ? undefined : this.songRequests.get(msg.re);
        const previous = file === undefined ? undefined : this.songMap.get(file);
        const pending = previous?.pending;
        if (file === undefined || !previous || !pending || pending.request !== msg.re) return new Set();
        if (msg.body.doc_revision !== null && msg.body.doc_revision !== pending.revision) return new Set();
        if (pending.epoch !== undefined && msg.body.epoch !== null && msg.body.epoch !== pending.epoch) return new Set();
        if (msg.kind === 'song-candidate-ready') {
          if (pending.ready) return new Set();
          this.songMap.set(file, { ...previous, pending: { ...pending, ready: true, epoch: msg.body.epoch } });
        } else {
          this.songRequests.delete(pending.request);
          const { pending: _, ...next } = previous;
          this.songMap.set(file, msg.kind === 'song-candidate-applied'
            ? { draftRevision: previous.draftRevision, applied: msg.body }
            : { ...next, failure: msg.body });
        }
        return new Set([songKey(file)]);
      }
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
        if (older(msg.body.transport, this.timedSample)) return new Set();
        this.timedSample = msg.body.transport ?? null;
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

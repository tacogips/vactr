// Binding persistence modes (design 15.1.6 "Persistence and saving", 13,
// 14.5.8; pending-editor-questions E4).
//
// `directive` (the default, per document): bindings live in the `#@`
// comments of the text; the editor keeps nothing else.
// `external-file`: the editor keeps an `EditorBindingSet` of entries
// `{key, panel, midi?, overlay?}` and writes `<doc>.bindings.json` in the
// 14.5.8 format `{"v": 1, "bindings": [...]}`, keys spelled
// `label.site.n.param` (or `label.param`), a site without a key as
// `{span, param}` in current-revision bytes, overlays kept. The editor never
// inserts, edits or strips `#@` text in this mode.

export type PersistenceMode = 'directive' | 'external-file';

export interface MidiMapping {
  cc: number;
  ch?: number;
}

/** One entry; either `key`, or `span` plus `param` (positional). */
export interface EditorBindingEntry {
  key?: string;
  span?: [number, number];
  param?: string;
  panel: boolean;
  midi?: MidiMapping;
  overlay?: number;
}

const KEY_RE = /^[^.\s]+(\.[^.\s]+\.[1-9][0-9]*)?\.[^.\s]+$/;

/** True for a `label.param` or `label.site.n.param` spelling. */
export const isKeySpelling = (s: string): boolean => KEY_RE.test(s);

/** The map key of an entry: the key spelling, or `@start:end.param`. */
export function identOf(e: Pick<EditorBindingEntry, 'key' | 'span' | 'param'>): string {
  if (e.key !== undefined) return e.key;
  const [s, t] = e.span ?? [0, 0];
  return `@${s}:${t}.${e.param ?? ''}`;
}

function validMidi(m: unknown): MidiMapping | undefined | null {
  if (m === undefined) return undefined;
  if (typeof m !== 'object' || m === null) return null;
  const { cc, ch } = m as { cc?: unknown; ch?: unknown };
  if (typeof cc !== 'number' || !Number.isInteger(cc) || cc < 0 || cc > 127) return null;
  if (ch === undefined) return { cc };
  if (typeof ch !== 'number' || !Number.isInteger(ch) || ch < 1 || ch > 16) return null;
  return { cc, ch };
}

function parseEntry(raw: unknown): EditorBindingEntry | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const r = raw as Record<string, unknown>;
  if (typeof r.panel !== 'boolean') return null;
  const midi = validMidi(r.midi);
  if (midi === null) return null;
  if (r.overlay !== undefined && typeof r.overlay !== 'number') return null;
  const e: EditorBindingEntry = { panel: r.panel };
  if (typeof r.key === 'string' && r.span === undefined && r.param === undefined) {
    if (!isKeySpelling(r.key)) return null;
    e.key = r.key;
  } else if (
    r.key === undefined &&
    Array.isArray(r.span) &&
    r.span.length === 2 &&
    r.span.every((x) => typeof x === 'number' && Number.isInteger(x) && x >= 0) &&
    typeof r.param === 'string'
  ) {
    e.span = [r.span[0] as number, r.span[1] as number];
    e.param = r.param;
  } else return null;
  if (midi) e.midi = midi;
  if (typeof r.overlay === 'number') e.overlay = r.overlay;
  return e;
}

export class EditorBindingSet {
  private readonly map = new Map<string, EditorBindingEntry>();

  get size(): number {
    return this.map.size;
  }

  get(ident: string): EditorBindingEntry | undefined {
    return this.map.get(ident);
  }

  /** Inserts or replaces the entry of its ident. */
  upsert(e: EditorBindingEntry): void {
    this.map.set(identOf(e), { ...e });
  }

  delete(ident: string): void {
    this.map.delete(ident);
  }

  /**
   * Keyed migrations (reordered same-named sites), applied at once so that
   * `a.1 -> a.2` and `a.2 -> a.3` do not overwrite each other.
   */
  renameAll(pairs: readonly [string, string][]): void {
    const moved: EditorBindingEntry[] = [];
    for (const [from, to] of pairs) {
      const e = this.map.get(from);
      if (!e || from === to) continue;
      this.map.delete(from);
      moved.push({ ...e, key: to });
    }
    for (const e of moved) this.map.set(identOf(e), e);
  }

  /** Entries in ident order. */
  entries(): EditorBindingEntry[] {
    return [...this.map.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map(([, e]) => ({ ...e }));
  }

  /** The 14.5.8 session-file text. */
  toJson(): string {
    const bindings = this.entries().map((e) => {
      const out: Record<string, unknown> = {};
      if (e.key !== undefined) out.key = e.key;
      if (e.span !== undefined) out.span = e.span;
      if (e.param !== undefined) out.param = e.param;
      out.panel = e.panel;
      if (e.midi) out.midi = e.midi.ch === undefined ? { cc: e.midi.cc } : { cc: e.midi.cc, ch: e.midi.ch };
      if (e.overlay !== undefined) out.overlay = e.overlay;
      return out;
    });
    return JSON.stringify({ v: 1, bindings });
  }

  /**
   * Parses session-file text. An unknown `v`, invalid JSON or a malformed
   * entry is ignored with a notice (an unknown `v` yields an empty set).
   */
  static fromJson(text: string): { set: EditorBindingSet; notices: string[] } {
    const set = new EditorBindingSet();
    const notices: string[] = [];
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch (e) {
      notices.push(`bindings file ignored: ${String(e)}`);
      return { set, notices };
    }
    const form = parsed as { v?: unknown; bindings?: unknown };
    if (typeof form !== 'object' || form === null || form.v !== 1 || !Array.isArray(form.bindings)) {
      const v = typeof form === 'object' && form !== null ? String(form.v) : '?';
      notices.push(`bindings file ignored: unsupported version ${v}`);
      return { set, notices };
    }
    form.bindings.forEach((raw, i) => {
      const e = parseEntry(raw);
      if (e) set.upsert(e);
      else notices.push(`bindings file entry ${i} ignored: malformed`);
    });
    return { set, notices };
  }
}

/** The per-document mode and the ExternalFile set. */
export class Persistence {
  private current: PersistenceMode = 'directive';
  set = new EditorBindingSet();
  private readonly listeners: ((mode: PersistenceMode) => void)[] = [];

  get mode(): PersistenceMode {
    return this.current;
  }

  /**
   * Switches the mode. Into ExternalFile, `snapshot` (the current panel:
   * directive bindings, learned mappings, overlays) becomes the set; the
   * text is never touched.
   */
  setMode(mode: PersistenceMode, snapshot: () => EditorBindingEntry[] = () => []): void {
    if (mode === this.current) return;
    if (mode === 'external-file') {
      const set = new EditorBindingSet();
      for (const e of snapshot()) set.upsert(e);
      this.set = set;
    }
    this.current = mode;
    for (const cb of [...this.listeners]) cb(mode);
  }

  /** Loads a sidecar into the set (ExternalFile); returns the notices. */
  load(text: string): string[] {
    const { set, notices } = EditorBindingSet.fromJson(text);
    this.set = set;
    for (const cb of [...this.listeners]) cb(this.current);
    return notices;
  }

  onChange(cb: (mode: PersistenceMode) => void): () => void {
    this.listeners.push(cb);
    return () => {
      const i = this.listeners.indexOf(cb);
      if (i >= 0) this.listeners.splice(i, 1);
    };
  }
}

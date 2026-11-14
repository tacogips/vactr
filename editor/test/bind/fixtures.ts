// ED-BIND fixtures: a bind area over a real CodeMirror view and the real
// code sync, a Client on a RecordingTransport (the "recording host" at the
// protocol boundary), scripted server messages, the 13.5 directive
// examples, and every 14.5.5 `bindings` batch shape as the session tests
// (src/session/tests/publish.rs) expect them.

import { EditorState, Text } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type { CodeApi } from '../../src/app/apis';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { BindArea, type BindOptions } from '../../src/bind/mount';
import { DocumentSync } from '../../src/code/sync';
import { CcStream } from '../../src/midi/learn';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import type { Timers } from '../../src/protocol/document';
import { Store } from '../../src/protocol/store';
import type {
  BindingsBody,
  ClientEnvelope,
  Diagnostic,
  EditorDecl,
  Span,
  WireDirectives,
  WireForm,
  WireFormState,
  WireSite,
} from '../../src/protocol/types';
import { MockClock } from '../support/clock';
import { RecordingTransport, type Scripted } from '../support/recording';

export const FILE = 'main.vact';
const enc = new TextEncoder();

/** The byte span of the `occurrence`-th `needle` in `text`. */
export function spanOf(text: string, needle: string, occurrence = 0): Span {
  let at = -1;
  for (let i = 0; i <= occurrence; i += 1) {
    at = text.indexOf(needle, at + 1);
    if (at < 0) throw new Error(`no ${needle} #${occurrence} in ${text}`);
  }
  const start = enc.encode(text.slice(0, at)).length;
  return { start, end: start + enc.encode(needle).length };
}

/** A site for the `occurrence`-th `literal` of `text`. */
export function site(
  text: string,
  literal: string,
  id: number,
  extra: Partial<WireSite> = {},
  occurrence = 0,
): WireSite {
  return {
    id,
    span: spanOf(text, literal, occurrence),
    tier: 'direct',
    origin: 'pattern-literal',
    value: Number(literal),
    form_gen: 1,
    ...extra,
  };
}

/** One form per line that starts a form (not blank, not `#`). */
export function formsByLine(text: string, gen = 1): WireForm[] {
  const out: WireForm[] = [];
  let at = 0;
  for (const line of text.split('\n')) {
    const start = enc.encode(text.slice(0, at)).length;
    if (line.trim() !== '' && !line.startsWith('#')) {
      out.push({ span: { start, end: start + enc.encode(line).length }, form_gen: gen });
    }
    at += line.length + 1;
  }
  return out;
}

export const noDirectives = (): WireDirectives => ({ file_level: {}, entries: [], labels: [], bindings: [] });

/** A manifest with the lpf/hpf EditorDecls (G2). */
export const EDITORS: EditorDecl[] = [
  {
    name: 'lpf',
    kind: 'filter-response',
    params: [
      { name: 'cutoff', ctl: 1, range: [20, 20000], curve: 'log', unit: 'hz', group: 0 },
      { name: 'q', ctl: 2, range: [0, 1], curve: 'linear', unit: 'none', group: 0 },
    ],
  },
  {
    name: 'hpf',
    kind: 'filter-response',
    params: [
      { name: 'cutoff', ctl: 1, range: [20, 20000], curve: 'log', unit: 'hz', group: 0 },
      { name: 'q', ctl: 2, range: [0, 1], curve: 'linear', unit: 'none', group: 0 },
    ],
  },
  {
    name: 'euclid',
    kind: 'euclid-ring',
    params: [{ name: 'hits', range: [0, 16], curve: 'stepped', unit: 'none', group: 0 }],
  },
];

/** Timers that fire only through `run()` (the `doc-changed` debounce stays pending). */
export class ManualTimers implements Timers {
  private next = 1;
  private readonly fns = new Map<number, () => void>();

  set(fn: () => void, _ms: number): unknown {
    const h = this.next;
    this.next += 1;
    this.fns.set(h, fn);
    return h;
  }

  clear(h: unknown): void {
    this.fns.delete(h as number);
  }

  run(): void {
    const fns = [...this.fns.values()];
    this.fns.clear();
    for (const f of fns) f();
  }
}

export interface EvalOpts {
  rev?: number;
  forms?: WireForm[];
  directives?: WireDirectives;
  diagnostics?: Diagnostic[];
  re?: number;
}

export interface Harness {
  transport: RecordingTransport;
  client: Client;
  store: Store;
  view: EditorView;
  sync: DocumentSync;
  deps: EditorDeps;
  root: HTMLElement;
  area: BindArea;
  files: MemoryFiles;
  timers: ManualTimers;
  /** Every row render: `[key, value text]`. */
  renders: [string, string][];
  text(): string;
  /** Delivers an `eval-result` for the current text (revision: current unless given). */
  evalResult(sites: WireSite[], opts?: EvalOpts): void;
  emit(msg: Scripted): void;
  /** The `seq` of the last client envelope of `kind`. */
  lastSeq(kind: ClientEnvelope['kind']): number;
  /** Replaces the first occurrence of `find` (UTF-16) with `insert`. */
  edit(find: string, insert: string, occurrence?: number): void;
  /** The slider panel row element of the binding holding tweak id `id`. */
  row(id: number): HTMLElement;
  dispose(): void;
}

const harnesses: Harness[] = [];

/** Disposes every harness (call from `afterEach`). */
export function cleanup(): void {
  for (const h of harnesses.splice(0)) h.dispose();
}

export function setup(text: string, opts: BindOptions & { editors?: EditorDecl[] } = {}): Harness {
  const transport = new RecordingTransport();
  const store = new Store();
  const timers = new ManualTimers();
  let t = 0;
  const client = new Client(transport, { store, timers, now: () => (t += 1000) });
  const initial = Text.of(text.split('\n'));
  const sync = new DocumentSync(client.document(opts.file ?? FILE), initial);
  const view = new EditorView({
    parent: document.body,
    state: EditorState.create({ doc: initial, extensions: [sync.extension()] }),
  });
  const code: CodeApi = {
    view,
    mapWireSpan: (span, rev) => sync.mapWireSpan(span, rev),
    currentRevision: () => sync.revision,
    selectedSiteId: () => null,
    samples: { frames: () => null, openBrowser: () => {} },
  };
  const root = document.createElement('div');
  document.body.appendChild(root);
  buildLayout(root);
  const files = new MemoryFiles();
  const deps: EditorDeps = { client, store, clock: new MockClock(), tier: 'browser', files, code };
  if (opts.editors) {
    store.apply({ kind: 'manifest', body: { sounds: [], synths: [], controls: [], editors: opts.editors } });
  }
  const renders: [string, string][] = [];
  const area = new BindArea(root, deps, code, {
    ...opts,
    onRender: (key, el) => {
      const v = el.querySelector('.bind-value, .bind-name-value');
      renders.push([key, v?.textContent ?? '']);
    },
  });
  const h: Harness = {
    transport,
    client,
    store,
    view,
    sync,
    deps,
    root,
    area,
    files,
    timers,
    renders,
    text: () => view.state.doc.toString(),
    evalResult(sites, o = {}) {
      const cur = view.state.doc.toString();
      transport.emit({
        kind: 'eval-result',
        ...(o.re === undefined ? {} : { re: o.re }),
        body: {
          file: opts.file ?? FILE,
          doc_revision: o.rev ?? sync.revision,
          forms: o.forms ?? formsByLine(cur),
          diagnostics: o.diagnostics ?? [],
          sites,
          directives: o.directives ?? noDirectives(),
        },
      });
    },
    emit: (msg) => transport.emit(msg),
    lastSeq(kind) {
      const all = transport.envelopes.filter((e) => e.kind === kind);
      const last = all[all.length - 1];
      if (!last) throw new Error(`no ${kind} sent`);
      return last.seq;
    },
    edit(find, insert, occurrence = 0) {
      const doc = view.state.doc.toString();
      let at = -1;
      for (let i = 0; i <= occurrence; i += 1) at = doc.indexOf(find, at + 1);
      if (at < 0) throw new Error(`no ${find}`);
      view.dispatch({ changes: { from: at, to: at + find.length, insert } });
    },
    row(id) {
      const e = area.table.byId(id);
      const el = e ? area.panel.row(e.bindingId) : undefined;
      if (!el) throw new Error(`no row for site ${id}`);
      return el;
    },
    dispose() {
      area.dispose();
      view.destroy();
      root.remove();
      client.close();
    },
  };
  harnesses.push(h);
  return h;
}

/** Lets pending promise continuations (eval replies, learn) run. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 5; i += 1) await Promise.resolve();
}

/** A MidiApi with real CC parsing (the ED-MIDI `CcStream`). */
export function fakeMidi(): CcStream & { cc(cc: number, value: number, ch?: number): void } {
  const s = new CcStream() as CcStream & { cc(cc: number, value: number, ch?: number): void };
  s.cc = (cc, value, ch = 1) => s.dispatch(new Uint8Array([0xb0 | (ch - 1), cc, value]), 0);
  return s;
}

/** Text of an element's descendant. */
export const q = (el: Element, sel: string): string => el.querySelector(sel)?.textContent ?? '';

// --------------------------------------------------- 13.5 directive examples

/** The design 13.5 / vocabulary.toml examples in one document. */
export const DIRECTIVE_DOC = [
  '#@ midi ch: 1',
  's [:bd :sd] > lpf 800 res: 0.4 > d1        #@ bass-filter: lpf cc: 74 71',
  's [:sd] > hpf 300 > d2',
  '#@ name snare',
  '#@ snare.hpf cc: 9',
  '#@ ghost.lpf cc: 3',
].join('\n');

export function directiveTable(text: string): WireDirectives {
  const d1 = spanOf(text, '#@ bass-filter: lpf cc: 74 71');
  const d2 = spanOf(text, '#@ name snare');
  const d3 = spanOf(text, '#@ snare.hpf cc: 9');
  const d4 = spanOf(text, '#@ ghost.lpf cc: 3');
  const line2 = formsByLine(text)[0] as WireForm;
  const line3 = formsByLine(text)[1] as WireForm;
  return {
    file_level: { midi_ch: 1 },
    entries: [
      { span: spanOf(text, '#@ midi ch: 1'), kind: 'addressed', trailing: false },
      { span: d1, kind: 'positional', target: line2.span, trailing: true },
      { span: d2, kind: 'positional', target: line3.span, trailing: false },
      { span: d3, kind: 'addressed', trailing: false },
      { span: d4, kind: 'addressed', trailing: false },
    ],
    labels: [
      { name: 'bass-filter', spans: [spanOf(text, 'bass-filter')], ambiguous: false },
      { name: 'snare', spans: [spanOf(text, 'snare')], ambiguous: false },
    ],
    bindings: [
      { key: 'bass-filter.lpf.1.cutoff', span: spanOf(text, 'lpf 800 res: 0.4'), param: 'cutoff', cc: 74, directive: d1 },
      { key: 'bass-filter.lpf.1.q', span: spanOf(text, 'lpf 800 res: 0.4'), param: 'q', cc: 71, directive: d1 },
      { key: 'snare.hpf.1.cutoff', span: spanOf(text, 'hpf 300'), param: 'cutoff', cc: 9, directive: d3 },
      { key: 'snare.hpf.1.q', span: spanOf(text, 'hpf 300'), param: 'q', directive: d3 },
    ],
  };
}

export function directiveSites(text: string): WireSite[] {
  const call = (name: string, needle: string, arg: number, param: string) => ({
    name,
    head: spanOf(text, needle),
    ordinal: 1,
    arg,
    param,
  });
  return [
    site(text, '800', 1, { key: 'bass-filter.lpf.1.cutoff', call: call('lpf', 'lpf', 0, 'cutoff') }),
    site(text, '0.4', 2, { key: 'bass-filter.lpf.1.q', call: call('lpf', 'lpf', 1, 'q') }),
    site(text, '300', 3, { key: 'snare.hpf.1.cutoff', call: call('hpf', 'hpf', 0, 'cutoff') }),
  ];
}

// ------------------------------------------------- 14.5.5 batch shapes

const ok = (name: string, value: string): WireFormState => ({ name, state: 'ok', value });
const failed = (name: string, value: string, message: string): WireFormState => ({
  name,
  state: 'failed',
  value,
  diagnostic: { code: 'div-by-zero', severity: 'error', message, span: { start: 0, end: 1 }, file: FILE },
});
const blocked = (name: string, value: string, on: string): WireFormState => ({
  name,
  state: 'blocked',
  value,
  blocked_on: on,
});
const batch = (pass: number, changed: [string, string][], states: WireFormState[]): BindingsBody => ({
  pass,
  changed: changed.map(([name, value]) => ({ name, value, form_gen: pass + 1 })),
  sites: [],
  states,
});

export interface BatchShape {
  name: string;
  /** Batches that establish the starting state. */
  seed: BindingsBody[];
  /** The batch under test. */
  batch: BindingsBody;
  /** Expected display after `batch`: name -> [value, badge substring or ''] */
  expect: Record<string, [string, string]>;
  /** Values that must never be rendered for a name (provisional). */
  never?: Record<string, string[]>;
}

/** Every 14.5.5 shape, one per session publish test. */
export const BATCH_SHAPES: BatchShape[] = [
  {
    name: 'changing edge',
    seed: [batch(1, [['a', '10'], ['b', '11']], [ok('a', '10'), ok('b', '11')])],
    batch: batch(2, [['a', '20'], ['b', '20']], [ok('a', '20'), ok('b', '20')]),
    expect: { a: ['20', ''], b: ['20', ''] },
  },
  {
    name: 'failed diamond',
    seed: [batch(1, [['left', '1'], ['right', '2'], ['total', '3']], [ok('left', '1'), ok('right', '2'), ok('total', '3')])],
    batch: batch(2, [['right', '1']], [failed('left', '1', 'division by zero'), ok('right', '1'), blocked('total', '3', 'left')]),
    expect: { left: ['1', 'failed'], right: ['1', ''], total: ['3', 'blocked on left'] },
  },
  {
    name: 'failed diamond recovery',
    seed: [batch(1, [['right', '1']], [failed('left', '1', 'division by zero'), ok('right', '1'), blocked('total', '3', 'left')])],
    batch: batch(2, [['right', '2']], [ok('left', '1'), ok('right', '2'), ok('total', '3')]),
    expect: { left: ['1', ''], right: ['2', ''], total: ['3', ''] },
  },
  {
    name: 'provisional rollback',
    seed: [batch(1, [['x', '7'], ['z', '3'], ['y', '10']], [ok('x', '7'), ok('z', '3'), ok('y', '10')])],
    batch: batch(2, [['z', '5'], ['y', '12']], [failed('x', '7', 'division by zero'), ok('z', '5'), ok('y', '12')]),
    expect: { x: ['7', 'failed'], z: ['5', ''], y: ['12', ''] },
    never: { x: ['1', '2'] },
  },
  {
    name: 'abort/retry',
    seed: [batch(1, [['a', '2'], ['b', '3']], [ok('a', '2'), ok('b', '3')])],
    batch: batch(2, [['b', '0']], [failed('a', '2', 'division by zero'), ok('b', '0')]),
    expect: { a: ['2', 'failed'], b: ['0', ''] },
    never: { a: ['1/3'] },
  },
  {
    name: 'conditional unblocking',
    seed: [batch(1, [], [failed('broken', '1', 'division by zero'), blocked('selected', '7', 'broken')])],
    batch: batch(2, [['selected', '7']], [ok('selected', '7')]),
    expect: { selected: ['7', ''], broken: ['1', 'failed'] },
  },
  {
    name: 'switch-toward',
    seed: [batch(1, [['selected', '7']], [ok('selected', '7'), failed('broken', '1', 'division by zero')])],
    batch: batch(2, [], [blocked('selected', '7', 'broken')]),
    expect: { selected: ['7', 'blocked on broken'], broken: ['1', 'failed'] },
  },
  {
    name: 'status recovery',
    seed: [batch(1, [], [failed('broken', '1', 'division by zero'), blocked('total', '2', 'broken'), blocked('selected', '1', 'broken')])],
    batch: batch(2, [], [ok('broken', '1'), ok('total', '2'), ok('selected', '1')]),
    expect: { broken: ['1', ''], total: ['2', ''], selected: ['1', ''] },
  },
  {
    name: 'late failure',
    seed: [batch(1, [['x', '7'], ['z', '1'], ['y', '1']], [ok('x', '7'), ok('z', '1'), ok('y', '1')])],
    batch: batch(2, [['z', '0']], [blocked('x', '7', 'y'), failed('y', '1', 'division by zero'), ok('z', '0')]),
    expect: { x: ['7', 'blocked on y'], y: ['1', 'failed'], z: ['0', ''] },
    never: { x: ['2'] },
  },
  {
    name: 'newly discovered selector',
    seed: [batch(1, [], [failed('broken', '1', 'division by zero'), blocked('selected', '5', 'broken')])],
    batch: batch(2, [['selected', '7']], [ok('selected', '7')]),
    expect: { selected: ['7', ''], broken: ['1', 'failed'] },
  },
  {
    name: 'ordinary failure',
    seed: [batch(1, [['selected', '5']], [ok('selected', '5')])],
    batch: batch(2, [], [failed('selected', '5', 'division by zero')]),
    expect: { selected: ['5', 'failed: division by zero'] },
  },
  {
    name: 'upd b 1 recovery',
    seed: [batch(1, [], [failed('selected', '5', 'division by zero')])],
    batch: batch(2, [['selected', '1']], [ok('selected', '1')]),
    expect: { selected: ['1', ''] },
  },
];

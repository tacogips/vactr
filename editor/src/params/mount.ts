// The `params` area (design 15.1.3, 15.1.7): the parameter-editor pane in
// the right column (the call-group list, the open editor, and the step
// grid and piano roll tabs) plus a click affordance on every call head in
// the code view. Editors write only through `deps.bind` (ED-BIND's
// `BindApi`); the grid and roll only display `playing` telemetry.

import { StateEffect, StateField, type Range } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import type { EditorDeps, Mounted } from '../app/deps';
import { buildLayout } from '../app/layout';
import { DOC_FILE } from '../code/mount';
import type { EditorKind, WireSite } from '../protocol/types';
import { render as delay } from './delay';
import { render as dynamics } from './dynamics';
import { render as envelope } from './envelope';
import { render as eq } from './eq';
import { render as euclid } from './euclid';
import { render as filter } from './filter';
import { render as granular } from './granular';
import { StepGrid } from './grid';
import { makeHandles, type Handle, type HandleHost, type KindCtx, type KindRender, type KindView } from './handles';
import { render as length } from './length';
import { render as lfo } from './lfo';
import { callGroups, editorFor, offersSampler, siblings, type CallGroup } from './open';
import { render as probability } from './probability';
import { render as reverb } from './reverb';
import { PianoRoll } from './roll';
import { render as sampler } from './sampler';
import { render as scalar } from './scalar';
import { render as stereo } from './stereo';
import { render as wavetable } from './wavetable';
import { render as xy } from './xy';

export const KINDS: Record<EditorKind, KindRender> = {
  'eq-curve': eq,
  'filter-response': filter,
  'dynamics-transfer': dynamics,
  'envelope-shape': envelope,
  'delay-taps': delay,
  'reverb-room': reverb,
  'sampler-wave': sampler,
  'wavetable-frames': wavetable,
  'granular-region': granular,
  'lfo-shape': lfo,
  'stereo-field': stereo,
  'xy-pad': xy,
  'euclid-ring': euclid,
  'probability-dial': probability,
  'length-handle': length,
  scalar,
};

export type ParamsTab = 'editors' | 'grid' | 'roll';

const STYLESHEET = new URL('./params.css', import.meta.url).href;

function addStylesheet(doc: Document): HTMLLinkElement | null {
  if (doc.head.querySelector('link[data-style="params"]')) return null;
  const link = doc.createElement('link');
  link.rel = 'stylesheet';
  link.href = STYLESHEET;
  link.dataset.style = 'params';
  doc.head.appendChild(link);
  return link;
}

// ---------------------------------------------------- call-head marks

interface HeadMark {
  from: number;
  to: number;
  group: string;
}

const setHeads = StateEffect.define<HeadMark[]>();

const headField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(set, tr) {
    let next = set.map(tr.changes);
    for (const e of tr.effects) {
      if (!e.is(setHeads)) continue;
      const ranges: Range<Decoration>[] = e.value
        .filter((m) => m.to > m.from)
        .map((m) =>
          Decoration.mark({ class: 'params-call-head', attributes: { 'data-group': m.group } }).range(m.from, m.to),
        );
      next = Decoration.set(ranges, true);
    }
    return next;
  },
  provide: (f) => EditorView.decorations.from(f),
});

// ------------------------------------------------------------ the area

export interface ParamsOptions {
  /** The session file name of the document (default: the code pane's). */
  file?: string;
}

export interface OpenEditor {
  kind: EditorKind;
  ctx: KindCtx;
  handles: Handle[];
  view: KindView;
}

const sameIds = (a: readonly WireSite[], b: readonly WireSite[]): boolean =>
  a.length === b.length && a.every((s, i) => s.id === b[i]?.id);

export class ParamsArea {
  readonly file: string;
  readonly el: HTMLElement;
  readonly grid: StepGrid;
  readonly roll: PianoRoll;
  current: OpenEditor | null = null;
  private readonly deps: EditorDeps;
  private readonly host: HandleHost;
  private readonly list: HTMLElement;
  private readonly editorEl: HTMLElement;
  private readonly panes: Record<ParamsTab, HTMLElement>;
  private readonly offs: (() => void)[] = [];
  private readonly stylesheet: HTMLLinkElement | null;
  private evalRev = 0;
  private all: CallGroup[] = [];
  private disposed = false;

  constructor(root: HTMLElement, deps: EditorDeps, opts: ParamsOptions = {}) {
    this.deps = deps;
    this.file = opts.file ?? DOC_FILE;
    const doc = root.ownerDocument;
    this.stylesheet = addStylesheet(doc);
    this.host = {
      bind: () => deps.bind,
      site: (id) => deps.bind?.siteById(id) ?? deps.store.site(id),
    };
    this.el = doc.createElement('section');
    this.el.className = 'params';
    buildLayout(root).right.appendChild(this.el);
    const tabs = doc.createElement('div');
    tabs.className = 'params-tabs';
    this.el.appendChild(tabs);
    const pane = (tab: ParamsTab): HTMLElement => {
      const b = doc.createElement('button');
      b.className = 'params-tab';
      b.dataset.tab = tab;
      b.textContent = tab;
      b.addEventListener('click', () => this.showTab(tab));
      tabs.appendChild(b);
      const p = doc.createElement('div');
      p.className = `params-pane params-pane-${tab}`;
      this.el.appendChild(p);
      return p;
    };
    this.panes = { editors: pane('editors'), grid: pane('grid'), roll: pane('roll') };
    this.list = doc.createElement('div');
    this.list.className = 'params-list';
    this.editorEl = doc.createElement('div');
    this.editorEl.className = 'params-editor';
    this.panes.editors.append(this.list, this.editorEl);
    this.grid = new StepGrid(this.panes.grid);
    this.roll = new PianoRoll(this.panes.roll, () => deps.code);
    this.showTab('editors');

    const { client, store } = deps;
    this.offs.push(
      store.subscribe(['sites', 'manifest'], (changed) => {
        // A whole eval-result is refreshed from the client listener (after its revision is known).
        if (!changed.has('forms')) this.refresh(false);
      }),
      store.subscribe(['levels', 'tempo'], () => this.current?.view.update()),
      client.on('eval-result', (env) => {
        if (env.kind !== 'eval-result' || env.body.file !== this.file) return;
        this.evalRev = env.body.doc_revision;
        this.refresh(true);
      }),
      client.on('playing', (env) => {
        if (env.kind !== 'playing') return;
        const bpc = store.tempo?.beats_per_cycle ?? 4;
        this.grid.update(env.body.events, bpc);
        this.roll.update(env.body.events, bpc);
      }),
    );

    const view = deps.code?.view;
    if (view) {
      view.dispatch({
        effects: StateEffect.appendConfig.of([
          headField,
          EditorView.domEventHandlers({
            click: (ev) => {
              const t = ev.target instanceof Element ? ev.target.closest('.params-call-head') : null;
              const id = t?.getAttribute('data-group');
              if (!id || this.disposed) return false;
              this.showTab('editors');
              this.open(id);
              return false;
            },
          }),
        ]),
      });
    }
    this.refresh(true);
  }

  /** The call groups of the file's current sites. */
  groups(): CallGroup[] {
    return this.all;
  }

  /** Opens the editor of a group (its declared kind unless `kind` is given). */
  open(groupId: string, kind?: EditorKind): OpenEditor | null {
    const group = this.all.find((g) => g.id === groupId);
    if (!group) return null;
    this.close();
    const found = editorFor(group, this.deps.store.manifest?.editors);
    const k = kind ?? found.kind;
    const decl = found.decl && (kind === undefined || kind === found.kind) ? found.decl : undefined;
    const handles = makeHandles(group, decl, this.host);
    const doc = this.el.ownerDocument;
    const title = doc.createElement('div');
    title.className = 'params-title';
    title.textContent = `${group.name} ${group.ordinal} - ${k}`;
    const body = doc.createElement('div');
    body.className = `params-kind params-kind-${k}`;
    body.dataset.kind = k;
    this.editorEl.append(title, body);
    const ctx: KindCtx = {
      doc,
      deps: this.deps,
      file: this.file,
      group,
      handles,
      chain: siblings(group, this.all),
      host: this.host,
      formText: () => this.formText(ctx.group),
      ...(decl ? { decl } : {}),
    };
    const view = (KINDS[k] ?? scalar)(body, ctx);
    this.current = { kind: k, ctx, handles, view };
    return this.current;
  }

  close(): void {
    this.current?.view.dispose();
    this.current = null;
    this.editorEl.textContent = '';
  }

  showTab(tab: ParamsTab): void {
    for (const [name, el] of Object.entries(this.panes)) el.hidden = name !== tab;
    this.el.dataset.tab = tab;
  }

  dispose(): void {
    this.disposed = true;
    for (const off of this.offs) off();
    this.close();
    this.grid.dispose();
    this.roll.dispose();
    this.deps.code?.view.dispatch({ effects: setHeads.of([]) });
    this.el.remove();
    this.stylesheet?.remove();
  }

  // ------------------------------------------------------------ internals

  /** Rebuilds the groups; call heads are re-marked only from an eval-result (its revision is `evalRev`). */
  private refresh(heads: boolean): void {
    const store = this.deps.store;
    this.all = callGroups(store.sitesOf(this.file), store.forms(this.file));
    this.renderList();
    if (heads) this.markHeads();
    const cur = this.current;
    if (!cur) return;
    const next = this.all.find((g) => g.id === cur.ctx.group.id);
    if (!next) {
      this.close();
      return;
    }
    if (!sameIds(cur.ctx.group.sites, next.sites)) {
      this.open(next.id, cur.kind);
      return;
    }
    cur.ctx.group = next;
    cur.ctx.chain = siblings(next, this.all);
    cur.view.update();
  }

  private renderList(): void {
    const doc = this.el.ownerDocument;
    const editors = this.deps.store.manifest?.editors;
    this.list.textContent = '';
    for (const g of this.all) {
      const { kind } = editorFor(g, editors);
      const row = doc.createElement('div');
      row.className = 'params-group';
      row.dataset.group = g.id;
      const label = doc.createElement('span');
      label.className = 'params-group-label';
      label.textContent = `${g.name} ${g.ordinal}`;
      const open = doc.createElement('button');
      open.className = 'params-open';
      open.textContent = kind;
      open.addEventListener('click', () => this.open(g.id));
      row.append(label, open);
      if (kind !== 'sampler-wave' && offersSampler(g, kind)) {
        const wave = doc.createElement('button');
        wave.className = 'params-open-wave';
        wave.textContent = 'waveform';
        wave.addEventListener('click', () => this.open(g.id, 'sampler-wave'));
        row.appendChild(wave);
      }
      this.list.appendChild(row);
    }
  }

  private markHeads(): void {
    const code = this.deps.code;
    if (!code) return;
    const marks: HeadMark[] = [];
    for (const g of this.all) {
      const r = code.mapWireSpan(g.head, this.evalRev);
      if (r) marks.push({ ...r, group: g.id });
    }
    code.view.dispatch({ effects: setHeads.of(marks.sort((a, b) => a.from - b.from)) });
  }

  /** The current text of the group's top-level form (read only; the sampler's bank). */
  private formText(group: CallGroup): string | null {
    const code = this.deps.code;
    const form = this.deps.store.forms(this.file)[group.form];
    if (!code || !form) return null;
    const r = code.mapWireSpan(form.span, this.evalRev);
    return r ? code.view.state.sliceDoc(r.from, r.to) : null;
  }
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: ParamsOptions = {}): Mounted {
  const area = new ParamsArea(root, deps, opts);
  return { dispose: () => area.dispose() };
}

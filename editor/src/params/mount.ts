// The `params` area (design 15.1.3, 15.1.7): the parameter-editor pane in
// the right column (the call-group list, the open editor, and the step
// grid and piano roll tabs) plus a click affordance on every call head in
// the code view. Editors write only through `deps.bind` (ED-BIND's
// `BindApi`); the grid and roll only display `playing` telemetry.

import { createComponent, createSignal, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import type { EditorDeps, Mounted } from '../app/deps';
import { buildLayout } from '../app/layout';
import { DOC_FILE } from '../code/mount';
import type { CodeSurface as HeadlessCodeSurface } from '../code/surface';
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
import { EditorHostView, ParamsView, type GroupView, type ParamsTab } from './params-view';
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

export type { ParamsTab } from './params-view';

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
  private readonly editorEl: HTMLElement;
  private readonly panes: Record<ParamsTab, HTMLElement>;
  private readonly setTab: Setter<ParamsTab>;
  private readonly setGroups: Setter<GroupView[]>;
  private readonly disposeView: () => void;
  private disposeHostView: (() => void) | null = null;
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
    const holder = doc.createElement('div');
    const [tab, setTab] = createSignal<ParamsTab>('editors');
    const [groups, setGroups] = createSignal<GroupView[]>([]);
    this.setTab = setTab;
    this.setGroups = setGroups;
    this.disposeView = render(() => createComponent(ParamsView, {
      tab, groups,
      onTab: (name) => this.showTab(name),
      onOpen: (id, kind) => this.open(id, kind),
    }), holder);
    this.el = holder.firstElementChild as HTMLElement;
    buildLayout(root).right.appendChild(this.el);
    this.panes = {
      editors: this.el.querySelector('.params-pane-editors') as HTMLElement,
      grid: this.el.querySelector('.params-pane-grid') as HTMLElement,
      roll: this.el.querySelector('.params-pane-roll') as HTMLElement,
    };
    this.editorEl = this.el.querySelector('.params-editor') as HTMLElement;
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

    const surface = deps.code?.surface;
    if (surface) {
      let down: { x: number; y: number; pointerId: number; primary: boolean; numeric: boolean } | null = null;
      this.offs.push(surface.onPointer((event) => {
        if (event.type === 'pointerdown') {
          down = { x: event.clientX, y: event.clientY, pointerId: event.pointerId,
            primary: event.button === 0 && event.isPrimary !== false,
            numeric: (event as PointerEvent & { vactrNumericGesture?: boolean }).vactrNumericGesture === true };
          return;
        }
        if (event.type !== 'pointerup' || !down || down.pointerId !== event.pointerId) return;
        const start = down; down = null;
        if (!start.primary || start.numeric || this.disposed || surface.compositionRange || Math.hypot(event.clientX - start.x, event.clientY - start.y) > 5) return;
        const pos = surface.posAtCoords({ x: event.clientX, y: event.clientY });
        if (pos === null) return;
        const head = (surface as HeadlessCodeSurface).annotationRanges().find((range) => range.kind === 'call-head' && range.from <= pos && pos <= range.to);
        if (!head?.label) return;
        this.showTab('editors');
        this.open(head.label);
      }));
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
    this.disposeHostView = render(() => createComponent(EditorHostView, {
      title: `${group.name} ${group.ordinal} - ${k}`, kind: k,
    }), this.editorEl);
    const body = this.editorEl.querySelector('.params-kind') as HTMLElement;
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
    this.disposeHostView?.();
    this.disposeHostView = null;
  }

  showTab(tab: ParamsTab): void {
    this.setTab(tab);
  }

  dispose(): void {
    this.disposed = true;
    for (const off of this.offs) off();
    this.close();
    this.grid.dispose();
    this.roll.dispose();
    this.deps.code?.surface.annotate('params-heads', []);
    this.disposeView();
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
    const editors = this.deps.store.manifest?.editors;
    this.setGroups(this.all.map((g) => {
      const { kind } = editorFor(g, editors);
      return { id: g.id, label: `${g.name} ${g.ordinal}`, kind, waveform: kind !== 'sampler-wave' && offersSampler(g, kind) };
    }));
  }

  private markHeads(): void {
    const code = this.deps.code;
    if (!code) return;
    const marks: { from: number; to: number; kind: 'call-head'; className: string; label: string }[] = [];
    for (const g of this.all) {
      const r = code.mapWireSpan(g.head, this.evalRev);
      if (r && r.to > r.from) marks.push({ ...r, kind: 'call-head', className: 'params-call-head', label: g.id });
    }
    code.surface.annotate('params-heads', marks.sort((a, b) => a.from - b.from || a.to - b.to));
  }

  /** The current text of the group's top-level form (read only; the sampler's bank). */
  private formText(group: CallGroup): string | null {
    const code = this.deps.code;
    const form = this.deps.store.forms(this.file)[group.form];
    if (!code || !form) return null;
    const r = code.mapWireSpan(form.span, this.evalRev);
    return r ? code.surface.state.sliceDoc(r.from, r.to) : null;
  }
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: ParamsOptions = {}): Mounted {
  const area = new ParamsArea(root, deps, opts);
  return { dispose: () => area.dispose() };
}

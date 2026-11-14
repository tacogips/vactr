// The `bind` area (design 15.1.3, 15.1.6): the slider panel and the
// directive control panel in the right pane, the drag and overlay
// extensions on the code view, CC routing and learn, the persistence mode
// and mode-scoped saving. It sets `deps.bind`, the `BindApi` of
// `app/apis.ts`, whose `writeSite` is the one write path every front end
// (slider, drag, CC, ED-PARAMS handles) uses.
//
// Wiring order matters for the one-repaint rule: the table's own store
// subscription is registered before any row's, so a `bindings` batch
// re-keys the table first and each affected row then repaints once. A whole
// `eval-result` is applied from the client listener as one rebuild.

import type { CodeApi } from '../app/apis';
import type { EditorDeps, Mounted } from '../app/deps';
import { buildLayout } from '../app/layout';
import { DOC_FILE } from '../code/mount';
import type { EditorDecl, EvalResultBody, WireForm } from '../protocol/types';
import { Utf8Index } from '../protocol/utf8';
import { ControlPanel } from './directives';
import { DragController } from './drag';
import { formatValue, SliderPanel } from './panel';
import { Persistence, type PersistenceMode } from './persistence';
import { CcRouter } from './routing';
import { save, type Saved } from './save';
import { contains, SiteTable, type Range16, type SiteEntry } from './sites';
import { SiteWriter } from './write';

/** How often `deps.midi` is checked for (it appears after the user enables MIDI). */
export const MIDI_POLL_MS = 500;

const STYLESHEET = new URL('./bind.css', import.meta.url).href;

function addStylesheet(doc: Document): HTMLLinkElement | null {
  if (doc.head.querySelector('link[data-style="bind"]')) return null;
  const link = doc.createElement('link');
  link.rel = 'stylesheet';
  link.href = STYLESHEET;
  link.dataset.style = 'bind';
  doc.head.appendChild(link);
  return link;
}

export interface BindOptions {
  /** The session file name of the document (default: the code pane's). */
  file?: string;
  /** The name `save()` writes (default: the file name). */
  docName?: string;
  /** Test seam: called after every panel row render. */
  onRender?: (key: string, el: HTMLElement) => void;
}

export class BindArea {
  readonly file: string;
  docName: string;
  readonly table: SiteTable;
  readonly writer: SiteWriter;
  readonly drag: DragController;
  readonly router: CcRouter;
  readonly persistence = new Persistence();
  readonly panel: SliderPanel;
  readonly control: ControlPanel;
  readonly notices: string[] = [];
  private readonly deps: EditorDeps;
  private readonly code: CodeApi;
  private readonly surface: CodeApi['surface'];
  private evalRev = 0;
  private forms: WireForm[] = [];
  private owners = new Map<string, string>();
  private indexCache: { rev: number; idx: Utf8Index } | null = null;
  private readonly offs: (() => void)[] = [];
  private readonly stylesheet: HTMLLinkElement | null;
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor(root: HTMLElement, deps: EditorDeps, code: CodeApi, opts: BindOptions = {}) {
    this.deps = deps;
    this.code = code;
    this.surface = code.surface;
    this.file = opts.file ?? DOC_FILE;
    this.docName = opts.docName ?? this.file;
    const { client, store } = deps;
    this.stylesheet = addStylesheet(root.ownerDocument);

    this.table = new SiteTable({
      map: (span, rev) => code.mapWireSpan(span, rev),
      text: (from, to) => this.surface.state.sliceDoc(from, to),
      retain: (e) => this.writer.overlay(e.bindingId) !== undefined || this.hasSetEntry(e),
      formOf: (range) => this.formOf(range),
    });
    this.writer = new SiteWriter({
      client,
      file: this.file,
      table: this.table,
      surface: this.surface,
      currentRevision: () => code.currentRevision(this.file),
      map: (span, rev) => code.mapWireSpan(span, rev),
      formOf: (range) => this.formOf(range),
      editors: () => this.editors(),
      notice: (m) => this.notice(m),
      changed: (ids) => this.changed(ids),
    });
    this.router = new CcRouter({
      file: this.file,
      client,
      table: this.table,
      writer: this.writer,
      persistence: this.persistence,
      surface: this.surface,
      midi: () => deps.midi,
      directives: () => store.directives(this.file),
      evalRevision: () => this.evalRev,
      map: (span, rev) => code.mapWireSpan(span, rev),
      bytes: (r) => this.bytes(r),
      editors: () => this.editors(),
      notice: (m) => this.notice(m),
      changed: (ids) => this.changed(ids),
      edited: () => this.control.render(),
    });
    this.writer.learnHook = (e) => {
      this.router.sync();
      return this.router.learn(e);
    };
    this.drag = new DragController({
      table: this.table,
      bind: this.writer,
      editors: () => this.editors(),
      overlay: (id) => this.writer.overlay(id),
    });

    // The table's subscription first: a `bindings` batch re-keys before rows repaint.
    this.offs.push(
      store.subscribe(['sites'], (changed) => {
        if (changed.has('forms')) return; // an eval-result: the client listener rebuilds
        const sites = [];
        for (const k of changed) {
          if (!k.startsWith('site:')) continue;
          const s = store.site(Number(k.slice(5)));
          const f = s ? store.fileOfSite(s.id) : undefined;
          if (s && (f === undefined || f === this.file)) sites.push(s);
        }
        if (sites.length === 0) return;
        const r = this.table.applySites(sites, client.document(this.file).baseRevision);
        this.panel.applyRefresh(r, changed);
        this.writer.afterRefresh(r.changed);
      }),
    );

    const right = buildLayout(root).right;
    this.panel = new SliderPanel(right, {
      store,
      table: this.table,
      writer: this.writer,
      editors: () => this.editors(),
      ownerName: (e) => this.owners.get(e.bindingId),
      lineExcerpt: (e) => this.lineExcerpt(e),
      mappingOf: (e) => this.router.mappingOf(e),
      learn: (e) => void this.writer.learn(e.site.id),
      commit: (e) => this.writer.commit(e.site.id),
      ...(opts.onRender ? { onRender: opts.onRender } : {}),
    });
    this.control = new ControlPanel(right, {
      store,
      file: this.file,
      persistence: this.persistence,
      evalRevision: () => this.evalRev,
      map: (span, rev) => code.mapWireSpan(span, rev),
      slice: (from, to) => this.surface.state.sliceDoc(from, to),
      lineOf: (pos) => this.surface.state.doc.lineAt(pos).number,
      setMode: (mode) => this.setMode(mode),
      save: () => void this.save().catch((e: unknown) => this.notice(`save failed: ${String(e)}`)),
    });

    this.offs.push(
      client.on('eval-result', (env) => {
        if (env.kind === 'eval-result' && env.body.file === this.file) this.onEval(env.body);
      }),
      client.on('bindings', () => this.panel.addNames()),
      client.on('stale-binding', (env) => {
        if (env.kind === 'stale-binding') this.writer.onStale(env.body);
      }),
      client.on('directive-edit', (env) => {
        // A learn reply is applied by the learn itself; only unsolicited edits here.
        if (env.kind === 'directive-edit' && env.re === undefined) this.router.applyDirectiveEdit(env.body);
      }),
      this.persistence.onChange(() => {
        this.control.render();
        this.panel.renderIds(this.table.all().map((e) => e.bindingId));
      }),
    );

    this.offs.push(this.surface.subscribe((update) => {
      if (update.docChanged) this.panel.renderIds(this.table.refreshMapping());
    }), this.drag.attach(this.surface));

    this.router.sync();
    const win = root.ownerDocument.defaultView;
    if (win) this.timer = win.setInterval(() => this.router.sync(), MIDI_POLL_MS);
    this.control.render();
    this.panel.addNames();
    deps.bind = this.writer;
  }

  /** Switches the document's persistence mode (no text change). */
  setMode(mode: PersistenceMode): void {
    this.persistence.setMode(mode, () => this.router.snapshot());
  }

  /** Saves the buffer (and, in ExternalFile mode, the sidecar with current overlays). */
  async save(name: string = this.docName): Promise<Saved> {
    const set = this.persistence.set;
    if (this.persistence.mode === 'external-file') {
      for (const [id, overlay] of this.writer.overlays()) {
        const e = this.table.get(id);
        if (!e) continue;
        const { id: ident, entry } = this.router.ident(e);
        set.upsert({ ...(set.get(ident) ?? { panel: true }), ...entry, overlay });
      }
    }
    return save({
      files: this.deps.files,
      name,
      text: this.surface.state.doc.toString(),
      mode: this.persistence.mode,
      set,
    });
  }

  notice(message: string): void {
    this.notices.push(message);
    this.control.notice(message);
  }

  dispose(): void {
    if (this.timer !== null) clearInterval(this.timer);
    for (const off of this.offs) off();
    this.router.dispose();
    this.panel.dispose();
    this.control.dispose();
    this.stylesheet?.remove();
    if (this.deps.bind === this.writer) delete this.deps.bind;
  }

  // ------------------------------------------------------------ internals

  private editors(): readonly EditorDecl[] | undefined {
    return this.deps.store.manifest?.editors;
  }

  private onEval(body: EvalResultBody): void {
    this.evalRev = body.doc_revision;
    this.forms = body.forms;
    this.router.onEval();
    const r = this.table.applyEval(body.sites, body.doc_revision);
    this.persistence.set.renameAll(r.migrated.map(([, from, to]) => [from, to]));
    for (const id of r.removed) this.writer.forget(id);
    this.owners = this.computeOwners(body);
    this.panel.rebuild();
    this.control.render();
    this.writer.afterRefresh(r.changed);
    this.updateOverlays();
  }

  /** The owning name of each binding: the first label inside the site's form. */
  private computeOwners(body: EvalResultBody): Map<string, string> {
    const out = new Map<string, string>();
    const labels = body.directives.labels ?? [];
    for (const e of this.table.all()) {
      if (e.rev !== body.doc_revision) continue;
      const s = e.site.span;
      const form = body.forms
        .filter((f) => f.span.start <= s.start && s.end <= f.span.end)
        .sort((a, b) => a.span.end - a.span.start - (b.span.end - b.span.start))[0];
      if (!form) continue;
      let best: { name: string; at: number } | undefined;
      for (const l of labels) {
        for (const sp of l.spans) {
          if (sp.start < form.span.start || sp.end > form.span.end) continue;
          if (!best || sp.start < best.at) best = { name: l.name, at: sp.start };
        }
      }
      if (best) out.set(e.bindingId, best.name);
    }
    return out;
  }

  /** The current range of the innermost `eval-result` form containing `range`. */
  private formOf(range: Range16): Range16 | null {
    let best: Range16 | null = null;
    for (const f of this.forms) {
      const r = this.code.mapWireSpan(f.span, this.evalRev);
      if (!r || !contains(r, range)) continue;
      if (!best || r.to - r.from < best.to - best.from) best = r;
    }
    return best;
  }

  private bytes(r: Range16): { start: number; end: number } {
    const rev = this.code.currentRevision(this.file);
    if (!this.indexCache || this.indexCache.rev !== rev) {
      this.indexCache = { rev, idx: new Utf8Index(this.surface.state.doc.toString()) };
    }
    return this.indexCache.idx.spanToBytes(r.from, r.to);
  }

  private hasSetEntry(e: SiteEntry): boolean {
    return this.persistence.mode === 'external-file' && this.persistence.set.get(this.router.ident(e).id) !== undefined;
  }

  private lineExcerpt(e: SiteEntry): string {
    const r = this.table.currentRange(e);
    if (!r) return e.literalText;
    const line = this.surface.state.doc.lineAt(r.from).text.trim();
    return line.length > 40 ? `${line.slice(0, 39)}…` : line;
  }

  private changed(ids: Iterable<string>): void {
    const list = [...ids];
    this.panel.renderIds(list);
    this.updateOverlays();
    if (this.persistence.mode === 'external-file') this.control.render();
  }

  private updateOverlays(): void {
    const marks: { from: number; to: number; kind: 'binding'; label: string }[] = [];
    for (const [id, v] of this.writer.overlays()) {
      const e = this.table.get(id);
      const r = e ? this.table.currentRange(e) : null;
      if (r) marks.push({ from: r.to, to: r.to, kind: 'binding', label: ` = ${formatValue(v)}` });
    }
    this.surface.annotate('bind-overlays', marks);
  }
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: BindOptions = {}): Mounted {
  const code = deps.code;
  if (!code) return { dispose() {} };
  const area = new BindArea(root, deps, code, opts);
  return { dispose: () => area.dispose() };
}

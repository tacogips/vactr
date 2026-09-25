// The directive-backed control panel (design 15.1.6 "Directive control
// panel", 13.5, 14.5.8).
//
// Directive mode renders `eval-result.directives`: each `#@` entry with the
// bindings it declares (param, `cc`, `ch`, the file default from
// `#@ midi ch:`), grouped by label (the key's first segment) or by line.
// Directive lint diagnostics (`unknown-label`, `duplicate-label`,
// `ambiguous-selector`, ...) are rendered in the editor by the code area's
// lint source; this panel puts a marker next to the affected entry.
// ExternalFile mode renders the editor-side set instead. Panel membership is
// edited only in the text; there is no add-to-panel action.

import type { Store } from '../protocol/store';
import type { Diagnostic, Span, WireBinding, WireDirective } from '../protocol/types';
import type { EditorBindingEntry, Persistence, PersistenceMode } from './persistence';
import type { Range16 } from './sites';

/** The directive lint codes (13.5, 14.5.8). */
export const DIRECTIVE_CODES: ReadonlySet<string> = new Set([
  'unknown-directive-site',
  'unknown-parameter',
  'unknown-label',
  'duplicate-label',
  'ambiguous-selector',
  'cc-out-of-range',
  'reserved-key',
  'duplicate-key',
]);

export interface ControlHost {
  store: Store;
  file: string;
  persistence: Persistence;
  /** The revision of the directive table's spans. */
  evalRevision(): number;
  map(span: Span, rev: number): Range16 | null;
  /** The current text in `[from, to)`. */
  slice(from: number, to: number): string;
  /** The 1-based current line of a UTF-16 offset. */
  lineOf(pos: number): number;
  setMode(mode: PersistenceMode): void;
  save(): void;
}

/** Overlapping spans, or a zero-length `a` inside `b`. */
const overlaps = (a: Span, b: Span): boolean =>
  (a.start < b.end && b.start < a.end) || (a.start === a.end && b.start <= a.start && a.start <= b.end);

const sameSpan = (a: Span, b: Span): boolean => a.start === b.start && a.end === b.end;

export class ControlPanel {
  readonly el: HTMLElement;
  private readonly host: ControlHost;
  private readonly body: HTMLElement;
  private readonly modeSelect: HTMLSelectElement;
  private readonly noticeEl: HTMLElement;
  private renders = 0;

  constructor(parent: HTMLElement, host: ControlHost) {
    this.host = host;
    const doc = parent.ownerDocument;
    this.el = doc.createElement('section');
    this.el.className = 'bind-controls';
    const header = doc.createElement('div');
    header.className = 'bind-controls-header';
    const title = doc.createElement('h3');
    title.textContent = 'Controls';
    this.modeSelect = doc.createElement('select');
    this.modeSelect.className = 'bind-persistence';
    for (const [v, t] of [
      ['directive', 'directives (#@)'],
      ['external-file', 'external file'],
    ] as const) {
      const o = doc.createElement('option');
      o.value = v;
      o.textContent = t;
      this.modeSelect.appendChild(o);
    }
    this.modeSelect.addEventListener('change', () => host.setMode(this.modeSelect.value as PersistenceMode));
    const save = doc.createElement('button');
    save.type = 'button';
    save.className = 'bind-save';
    save.textContent = 'Save';
    save.addEventListener('click', () => host.save());
    header.append(title, this.modeSelect, save);
    this.noticeEl = doc.createElement('div');
    this.noticeEl.className = 'bind-notice';
    this.body = doc.createElement('div');
    this.body.className = 'bind-controls-body';
    this.el.append(header, this.noticeEl, this.body);
    parent.appendChild(this.el);
  }

  get renderCount(): number {
    return this.renders;
  }

  notice(message: string): void {
    this.noticeEl.textContent = message;
  }

  render(): void {
    this.renders += 1;
    this.modeSelect.value = this.host.persistence.mode;
    this.body.replaceChildren();
    if (this.host.persistence.mode === 'external-file') this.renderSet(this.host.persistence.set.entries());
    else this.renderDirectives();
  }

  dispose(): void {
    this.el.remove();
  }

  // ------------------------------------------------------------ internals

  private renderDirectives(): void {
    const { store, file } = this.host;
    const table = store.directives(file);
    if (!table) return;
    const diags = store.diagnostics(file).filter((d) => DIRECTIVE_CODES.has(d.code));
    const rev = this.host.evalRevision();
    const fileCh = table.file_level.midi_ch;
    const groups = new Map<string, HTMLElement>();
    const group = (title: string): HTMLElement => {
      let g = groups.get(title);
      if (!g) {
        g = this.el.ownerDocument.createElement('div');
        g.className = 'bind-ctl-group';
        g.dataset.group = title;
        const h = this.el.ownerDocument.createElement('h4');
        h.textContent = title;
        g.appendChild(h);
        groups.set(title, g);
        this.body.appendChild(g);
      }
      return g;
    };
    if (fileCh !== undefined) group('file').appendChild(this.line('bind-ctl-file', `midi ch ${fileCh}`));
    for (const entry of table.entries) {
      const bindings = (table.bindings ?? []).filter((b) => sameSpan(b.directive, entry.span));
      const r = this.host.map(entry.span, rev);
      const key = bindings.find((b) => b.key !== undefined)?.key;
      const title = key ? (key.split('.')[0] ?? key) : r ? `line ${this.host.lineOf(r.from)}` : 'moved';
      group(title).appendChild(this.entryEl(entry, r, bindings, diags, fileCh));
    }
  }

  private entryEl(
    entry: WireDirective,
    r: Range16 | null,
    bindings: WireBinding[],
    diags: Diagnostic[],
    fileCh: number | undefined,
  ): HTMLElement {
    const doc = this.el.ownerDocument;
    const el = doc.createElement('div');
    el.className = 'bind-ctl-entry';
    el.dataset.kind = entry.kind;
    el.dataset.directive = `${entry.span.start}:${entry.span.end}`;
    el.appendChild(this.line('bind-ctl-text', r ? this.host.slice(r.from, r.to) : '(edited)'));
    const diag = diags.find((d) => overlaps(d.span, entry.span));
    if (diag) {
      const mark = this.line('bind-ctl-marker', diag.code);
      mark.title = diag.message;
      el.dataset.diagnostic = diag.code;
      el.appendChild(mark);
    }
    for (const b of bindings) {
      const ch = b.ch ?? fileCh;
      const text = `${b.key ?? b.param}  ${b.cc === undefined ? 'panel' : `cc ${b.cc}`}${
        b.cc !== undefined && ch !== undefined ? ` ch ${ch}` : ''
      }`;
      const row = this.line('bind-ctl-binding', text);
      row.dataset.param = b.param;
      if (b.key !== undefined) row.dataset.key = b.key;
      el.appendChild(row);
    }
    return el;
  }

  private renderSet(entries: EditorBindingEntry[]): void {
    for (const e of entries) {
      const ident = e.key ?? `${e.param ?? ''} @${e.span?.[0] ?? 0}`;
      const midi = e.midi ? `cc ${e.midi.cc}${e.midi.ch === undefined ? '' : ` ch ${e.midi.ch}`}` : 'panel';
      const overlay = e.overlay === undefined ? '' : `  overlay ${e.overlay}`;
      const row = this.line('bind-ctl-binding', `${ident}  ${midi}${overlay}`);
      if (e.key !== undefined) row.dataset.key = e.key;
      this.body.appendChild(row);
    }
  }

  private line(cls: string, text: string): HTMLElement {
    const el = this.el.ownerDocument.createElement('div');
    el.className = cls;
    el.textContent = text;
    return el;
  }
}

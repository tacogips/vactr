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

import { createComponent, createSignal, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import { ControlView, type ControlBinding, type ControlGroup, type ControlModel } from './control-view';
import type { Store } from '../protocol/store';
import type { Diagnostic, Span } from '../protocol/types';
import type { Persistence, PersistenceMode } from './persistence';
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
  private readonly setModel: Setter<ControlModel>;
  private readonly disposeView: () => void;
  private noticeText = '';
  private renders = 0;

  constructor(parent: HTMLElement, host: ControlHost) {
    this.host = host;
    const holder = parent.ownerDocument.createElement('div');
    const [model, setModel] = createSignal(this.buildModel());
    this.setModel = setModel;
    this.disposeView = render(() => createComponent(ControlView, {
      model,
      onMode: (mode) => host.setMode(mode),
      onSave: () => host.save(),
    }), holder);
    this.el = holder.firstElementChild as HTMLElement;
    parent.appendChild(this.el);
  }

  get renderCount(): number { return this.renders; }

  notice(message: string): void {
    this.noticeText = message;
    this.setModel(this.buildModel());
  }

  render(): void {
    this.renders += 1;
    this.setModel(this.buildModel());
  }

  dispose(): void {
    this.disposeView();
    this.el.remove();
  }

  private buildModel(): ControlModel {
    const mode = this.host.persistence.mode;
    const model: ControlModel = { mode, notice: this.noticeText, groups: [], setEntries: [] };
    if (mode === 'external-file') {
      for (const entry of this.host.persistence.set.entries()) {
        const ident = entry.key ?? `${entry.param ?? ''} @${entry.span?.[0] ?? 0}`;
        const midi = entry.midi ? `cc ${entry.midi.cc}${entry.midi.ch === undefined ? '' : ` ch ${entry.midi.ch}`}` : 'panel';
        const overlay = entry.overlay === undefined ? '' : `  overlay ${entry.overlay}`;
        const binding: ControlBinding = { text: `${ident}  ${midi}${overlay}` };
        if (entry.key !== undefined) binding.key = entry.key;
        model.setEntries.push(binding);
      }
      return model;
    }
    const table = this.host.store.directives(this.host.file);
    if (!table) return model;
    const diagnostics = this.host.store.diagnostics(this.host.file).filter((diag) => DIRECTIVE_CODES.has(diag.code));
    const fileChannel = table.file_level.midi_ch;
    const groups = new Map<string, ControlGroup>();
    const group = (title: string): ControlGroup => {
      let value = groups.get(title);
      if (!value) {
        value = { title, entries: [] };
        groups.set(title, value);
        model.groups.push(value);
      }
      return value;
    };
    if (fileChannel !== undefined) group('file').fileChannel = `midi ch ${fileChannel}`;
    for (const entry of table.entries) {
      const bindings = (table.bindings ?? []).filter((binding) => sameSpan(binding.directive, entry.span));
      const range = this.host.map(entry.span, this.host.evalRevision());
      const key = bindings.find((binding) => binding.key !== undefined)?.key;
      const title = key ? (key.split('.')[0] ?? key) : range ? `line ${this.host.lineOf(range.from)}` : 'moved';
      const diagnostic = diagnostics.find((diag) => overlaps(diag.span, entry.span));
      group(title).entries.push({
        kind: entry.kind,
        span: `${entry.span.start}:${entry.span.end}`,
        text: range ? this.host.slice(range.from, range.to) : '(edited)',
        ...(diagnostic ? { diagnostic: { code: diagnostic.code, message: diagnostic.message } } : {}),
        bindings: bindings.map((binding) => {
          const channel = binding.ch ?? fileChannel;
          const text = `${binding.key ?? binding.param}  ${binding.cc === undefined ? 'panel' : `cc ${binding.cc}`}${
            binding.cc !== undefined && channel !== undefined ? ` ch ${channel}` : ''
          }`;
          return {
            text, param: binding.param,
            ...(binding.key === undefined ? {} : { key: binding.key }),
          };
        }),
      });
    }
    return model;
  }
}

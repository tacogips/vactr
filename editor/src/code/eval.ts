// Eval keys and flash (design 15.1.5 "Eval", 14.5.4, 14.5.6).
//
// - `Mod-Enter` evaluates the form at the cursor: the span runs from the
//   nearest line at or above the cursor that starts at column 0 with a
//   character other than whitespace, `#` or `>`, through the line before
//   the next such line, minus trailing blank lines. The session selects
//   the forms inside it (14.5.4).
// - `Mod-Shift-Enter` evaluates the whole document (no span).
// - `Mod-.` sends `hush`.
//
// `eval` always carries the full text, the revision and the epoch (the
// client flushes the pending `doc-changed` first). The span flashes for
// 200 ms; forms of the reply whose `failure` is set flash the error class.

import { Prec, StateEffect, StateField, type Extension, type Text } from '@codemirror/state';
import { Decoration, EditorView, keymap, type DecorationSet } from '@codemirror/view';
import type { Client } from '../protocol/client';
import { defaultTimers, type Timers } from '../protocol/document';
import type { ServerEnvelope } from '../protocol/types';
import type { Range16 } from './history';
import type { DocumentSync } from './sync';

export const FLASH_MS = 200;
export const FLASH_CLASS = 'vact-flash';
export const FLASH_ERROR_CLASS = 'vact-flash-error';

function isFormStart(line: string): boolean {
  const c = line.charAt(0);
  return c !== '' && c !== '#' && c !== '>' && !/\s/.test(c);
}

function isBlank(line: string): boolean {
  return line.trim() === '';
}

/**
 * The UTF-16 span of the form at `pos`. When no form start is at or above
 * the cursor (leading comments), the span starts at the first line.
 */
export function formSpanAt(doc: Text, pos: number): Range16 {
  const cursor = doc.lineAt(Math.max(0, Math.min(doc.length, pos))).number;
  let first = 1;
  for (let n = cursor; n >= 1; n -= 1) {
    if (isFormStart(doc.line(n).text)) {
      first = n;
      break;
    }
  }
  let last = doc.lines;
  for (let n = Math.max(first, cursor) + 1; n <= doc.lines; n += 1) {
    if (isFormStart(doc.line(n).text)) {
      last = n - 1;
      break;
    }
  }
  while (last > first && isBlank(doc.line(last).text)) last -= 1;
  return { from: doc.line(first).from, to: doc.line(last).to };
}

// ------------------------------------------------------------ flash field

interface FlashSpec {
  id: number;
  from: number;
  to: number;
  cls: string;
}

export const addFlash = StateEffect.define<FlashSpec>();
export const removeFlash = StateEffect.define<number>();

interface FlashState {
  specs: FlashSpec[];
  deco: DecorationSet;
}

function build(specs: FlashSpec[]): DecorationSet {
  const ranges = specs
    .filter((s) => s.from < s.to)
    .map((s) => Decoration.mark({ class: s.cls, attributes: { 'data-flash': String(s.id) } }).range(s.from, s.to));
  return Decoration.set(ranges, true);
}

export const flashField = StateField.define<FlashState>({
  create: () => ({ specs: [], deco: Decoration.none }),
  update(value, tr) {
    let specs = value.specs;
    if (tr.docChanged) {
      specs = specs.map((s) => ({
        ...s,
        from: tr.changes.mapPos(s.from, 1),
        to: tr.changes.mapPos(s.to, -1),
      }));
    }
    let changed = tr.docChanged;
    for (const e of tr.effects) {
      if (e.is(addFlash)) {
        specs = [...specs, e.value];
        changed = true;
      } else if (e.is(removeFlash)) {
        specs = specs.filter((s) => s.id !== e.value);
        changed = true;
      }
    }
    return changed ? { specs, deco: build(specs) } : value;
  },
  provide: (f) => EditorView.decorations.from(f, (v) => v.deco),
});

/** The live flashes of a view (tests). */
export function flashes(view: EditorView): { from: number; to: number; cls: string }[] {
  return view.state.field(flashField).specs.map(({ from, to, cls }) => ({ from, to, cls }));
}

// ------------------------------------------------------------ controller

export interface EvalOptions {
  client: Client;
  sync: DocumentSync;
  timers?: Timers;
  /** Called after `hush` is sent (the highlight scheduler clears). */
  onHush?: () => void;
}

export class EvalController {
  private readonly opts: EvalOptions;
  private readonly timers: Timers;
  private view: EditorView | null = null;
  private nextId = 1;
  private readonly pending = new Set<unknown>();

  constructor(opts: EvalOptions) {
    this.opts = opts;
    this.timers = opts.timers ?? defaultTimers;
  }

  attach(view: EditorView): void {
    this.view = view;
  }

  /** `Mod-Enter`: the form at the main cursor. */
  evalAtCursor(): Promise<ServerEnvelope> | null {
    const view = this.view;
    if (!view) return null;
    const range = formSpanAt(view.state.doc, view.state.selection.main.head);
    const span = this.opts.sync.toWireSpan(range.from, range.to);
    const reply = this.opts.client.eval(this.opts.sync.file, view.state.doc.toString(), span);
    this.flash(range, FLASH_CLASS);
    return this.watch(reply);
  }

  /** `Mod-Shift-Enter`: the whole document. */
  evalAll(): Promise<ServerEnvelope> | null {
    const view = this.view;
    if (!view) return null;
    const reply = this.opts.client.eval(this.opts.sync.file, view.state.doc.toString());
    this.flash({ from: 0, to: view.state.doc.length }, FLASH_CLASS);
    return this.watch(reply);
  }

  /** `Mod-.`. */
  hush(): void {
    this.opts.client.hush();
    this.opts.onHush?.();
  }

  /** Flashes `range` for `FLASH_MS`. */
  flash(range: Range16, cls: string): void {
    const view = this.view;
    if (!view || range.from >= range.to) return;
    const id = this.nextId;
    this.nextId += 1;
    view.dispatch({ effects: addFlash.of({ id, from: range.from, to: range.to, cls }) });
    const handle = this.timers.set(() => {
      this.pending.delete(handle);
      this.view?.dispatch({ effects: removeFlash.of(id) });
    }, FLASH_MS);
    this.pending.add(handle);
  }

  /** The CodeMirror extensions: the keymap and the flash field. */
  extension(): Extension {
    return [
      flashField,
      Prec.high(
        keymap.of([
          { key: 'Mod-Enter', run: () => this.evalAtCursor() !== null },
          { key: 'Mod-Shift-Enter', run: () => this.evalAll() !== null },
          { key: 'Mod-.', run: () => (this.hush(), true) },
        ]),
      ),
    ];
  }

  dispose(): void {
    for (const h of this.pending) this.timers.clear(h);
    this.pending.clear();
    this.view = null;
  }

  /** Flashes the failed forms of the reply. */
  private watch(reply: Promise<ServerEnvelope>): Promise<ServerEnvelope> {
    reply.then(
      (env) => {
        if (env.kind !== 'eval-result' || env.body.file !== this.opts.sync.file) return;
        for (const form of env.body.forms) {
          if (!form.failure) continue;
          const r = this.opts.sync.mapWireSpan(form.span, env.body.doc_revision);
          if (r) this.flash(r, FLASH_ERROR_CLASS);
        }
      },
      () => {
        // A closed client: nothing to flash.
      },
    );
    return reply;
  }
}

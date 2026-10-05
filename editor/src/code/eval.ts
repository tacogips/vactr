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

import type { Text } from '@codemirror/state';
import type { CodeRange, CodeSurface } from '../app/apis';
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

// ------------------------------------------------------------ controller

export interface EvalOptions {
  client: Client;
  sync: DocumentSync;
  timers?: Timers;
  /** Called after `hush` is sent (the highlight scheduler clears). */
  onHush?: () => void;
  /** Called with every eval reply, right after the request is sent (design 15.2: the outcome is shown, audio is started). */
  onEval?: (reply: Promise<ServerEnvelope>) => void;
}

export class EvalController {
  private readonly opts: EvalOptions;
  private readonly timers: Timers;
  private surface: CodeSurface | null = null;
  private removeKeymap: (() => void) | null = null;
  private readonly flashListeners = new Set<(range: CodeRange, kind: string | null) => void>();
  private readonly pending = new Set<unknown>();

  constructor(opts: EvalOptions) {
    this.opts = opts;
    this.timers = opts.timers ?? defaultTimers;
  }

  attach(surface: CodeSurface): void {
    this.surface = surface;
    this.removeKeymap = surface.addKeymap([
      { key: 'Mod-Enter', run: () => this.evalAtCursor() !== null },
      { key: 'Mod-Shift-Enter', run: () => this.evalAll() !== null },
      { key: 'Mod-.', run: () => (this.hush(), true) },
    ], 'default');
  }

  onFlash(cb: (range: CodeRange, kind: string | null) => void): () => void {
    this.flashListeners.add(cb); return () => this.flashListeners.delete(cb);
  }

  /** `Mod-Enter`: the form at the main cursor. */
  evalAtCursor(): Promise<ServerEnvelope> | null {
    const surface = this.surface;
    if (!surface) return null;
    const range = formSpanAt(surface.state.doc, surface.state.selection.main.head);
    const span = this.opts.sync.toWireSpan(range.from, range.to);
    const reply = this.opts.client.eval(this.opts.sync.file, surface.state.doc.toString(), span);
    this.flash(range, FLASH_CLASS);
    return this.watch(reply);
  }

  /** `Mod-Shift-Enter`: the whole document. */
  evalAll(): Promise<ServerEnvelope> | null {
    const surface = this.surface;
    if (!surface) return null;
    const reply = this.opts.client.eval(this.opts.sync.file, surface.state.doc.toString());
    this.flash({ from: 0, to: surface.state.doc.length }, FLASH_CLASS);
    return this.watch(reply);
  }

  /** `Mod-.`. */
  hush(): void {
    this.opts.client.hush();
    this.opts.onHush?.();
  }

  /** Flashes `range` for `FLASH_MS`. */
  flash(range: Range16, cls: string): void {
    if (!this.surface || range.from >= range.to) return;
    for (const listener of this.flashListeners) listener(range, cls);
    const handle = this.timers.set(() => {
      this.pending.delete(handle);
      for (const listener of this.flashListeners) listener(range, null);
    }, FLASH_MS);
    this.pending.add(handle);
  }

  dispose(): void {
    for (const h of this.pending) this.timers.clear(h);
    this.pending.clear();
    this.removeKeymap?.(); this.removeKeymap = null;
    this.flashListeners.clear(); this.surface = null;
  }

  /** Flashes the failed forms of the reply. */
  private watch(reply: Promise<ServerEnvelope>): Promise<ServerEnvelope> {
    this.opts.onEval?.(reply);
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

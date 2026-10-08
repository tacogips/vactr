import { Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EvalController, FLASH_CLASS, FLASH_ERROR_CLASS, FLASH_MS, formSpanAt } from '../../src/code/eval';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { ClientEnvelope } from '../../src/protocol/types';
import { RecordingTransport, type Scripted } from '../support/recording';

const enc = new TextEncoder();
const bytes = (s: string): number => enc.encode(s).length;

const CHAIN = ['# header', 'd1 "bd sd"', '  > fast 2', '\t> gain 0.8', '', '', 'let x 1', '> + x 2', '', '#@ name lead'].join('\n');

function spanText(doc: string, line: number): string {
  const t = Text.of(doc.split('\n'));
  const r = formSpanAt(t, t.line(line).from);
  return t.sliceString(r.from, r.to);
}

describe('formSpanAt', () => {
  it('spans a multi-line chain of indented and > lines, minus trailing blank lines', () => {
    const form = 'd1 "bd sd"\n  > fast 2\n\t> gain 0.8';
    expect(spanText(CHAIN, 2)).toBe(form);
    expect(spanText(CHAIN, 3)).toBe(form);
    expect(spanText(CHAIN, 4)).toBe(form);
    // A blank line after the form belongs to it (nearest start above).
    expect(spanText(CHAIN, 5)).toBe(form);
    // `>` at column 0 continues; the trailing comment-only form stays in.
    expect(spanText(CHAIN, 8)).toBe('let x 1\n> + x 2\n\n#@ name lead');
    // Leading comments before any form: from the first line to the next form.
    expect(spanText(CHAIN, 1)).toBe('# header');
  });

  it('handles the last line and a one-line document', () => {
    expect(spanText('d1 "bd"', 1)).toBe('d1 "bd"');
    expect(spanText('let a 1\nlet b 2', 2)).toBe('let b 2');
  });
});

const views: CodeSurface[] = [];

function setup(text: string, respond?: (env: ClientEnvelope) => Scripted[]) {
  const transport = new RecordingTransport();
  if (respond) transport.respond(respond);
  const client = new Client(transport, { store: new Store() });
  const initial = Text.of(text.split('\n'));
  const sync = new DocumentSync(client.document('main.vact'), initial);
  const onHush = vi.fn();
  const ctl = new EvalController({ client, sync, onHush });
  const view = new CodeSurface({ sync });
  views.push(view);
  ctl.attach(view);
  const currentFlashes = new Map<string, { from: number; to: number; kind: 'eval'; className?: string }>();
  ctl.onFlash((range, kind) => {
    const key = `${range.from}:${range.to}`;
    if (kind) currentFlashes.set(key, { ...range, kind: 'eval', className: kind }); else currentFlashes.delete(key);
    view.annotate('eval-test', [...currentFlashes.values()]);
  });
  return { transport, client, sync, ctl, view, onHush };
}

function key(view: CodeSurface, k: string, mods: { shift?: boolean } = {}): boolean {
  const ev = new KeyboardEvent('keydown', { key: k, ctrlKey: true, shiftKey: mods.shift === true, bubbles: true });
  return view.runKeymaps(ev);
}
const flashes = (surface: CodeSurface) => surface.annotationRanges().filter((range) => range.kind === 'eval').map((range) => ({ from: range.from, to: range.to, cls: range.className }));

describe('EvalController', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    for (const v of views.splice(0)) v.dispose();
    vi.useRealTimers();
  });

  it('Mod-Enter evaluates the form at the cursor as a byte span with the full text, revision and epoch', () => {
    const text = '# über\nd1 "bd"\n  > fast 2\n\nlet ä 1';
    const { transport, view } = setup(text);
    view.dispatch({ selection: { anchor: text.indexOf('fast') } });
    expect(key(view, 'Enter')).toBe(true);
    const [ev] = transport.of('eval');
    const start = bytes('# über\n');
    expect(ev?.body).toEqual({
      file: 'main.vact',
      code: text,
      doc_revision: 1,
      edit_epoch: 0,
      span: { start, end: start + bytes('d1 "bd"\n  > fast 2') },
    });
  });

  it('flushes the pending doc-changed before the eval', () => {
    const { transport, view } = setup('d1 "bd"');
    view.dispatch({ changes: { from: 0, insert: '# ö\n' } });
    key(view, 'Enter');
    expect(transport.kinds()).toEqual(['doc-changed', 'eval']);
    const [ev] = transport.of('eval');
    expect(ev?.body.doc_revision).toBe(2);
    expect(ev?.body.edit_epoch).toBe(1);
    expect(ev?.body.code).toBe('# ö\nd1 "bd"');
  });

  it('Mod-Shift-Enter evaluates the whole document without a span', () => {
    const { transport, view } = setup('let a 1\nd1 "bd"');
    expect(key(view, 'Enter', { shift: true })).toBe(true);
    const [ev] = transport.of('eval');
    expect(ev?.body.span).toBeUndefined();
    expect(ev?.body.code).toBe('let a 1\nd1 "bd"');
    expect(flashes(view)).toEqual([{ from: 0, to: 15, cls: FLASH_CLASS }]);
  });

  it('Mod-. is not a code keymap binding (handled app-level)', () => {
    const { transport, view, onHush } = setup('d1 "bd"');
    expect(key(view, '.')).toBe(false);
    expect(transport.kinds()).toEqual([]);
    expect(onHush).not.toHaveBeenCalled();
  });

  it('flashes the eval span for 200 ms', () => {
    const { view } = setup('let a 1\nd1 "bd"');
    view.dispatch({ selection: { anchor: 9 } });
    key(view, 'Enter');
    expect(flashes(view)).toEqual([{ from: 8, to: 15, cls: FLASH_CLASS }]);
    const flash = view.annotationRanges().find((range) => range.kind === 'eval');
    expect(view.state.doc.sliceString(flash?.from ?? 0, flash?.to ?? 0)).toBe('d1 "bd"');
    vi.advanceTimersByTime(FLASH_MS - 1);
    expect(flashes(view)).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(flashes(view)).toEqual([]);
  });

  it('flashes the failed forms of the reply with the error class', async () => {
    const text = 'let a 1\nd1 "bd"';
    const { view, ctl } = setup(text, (env) =>
      env.kind === 'eval'
        ? [
            {
              kind: 'eval-result',
              re: env.seq,
              body: {
                file: 'main.vact',
                doc_revision: env.body.doc_revision,
                forms: [
                  { span: { start: 0, end: 7 }, value: '1', form_gen: 1 },
                  {
                    span: { start: 8, end: 15 },
                    failure: {
                      code: 'type',
                      severity: 'error',
                      message: 'bad',
                      span: { start: 11, end: 15 },
                      file: 'main.vact',
                    },
                    form_gen: 1,
                  },
                ],
                diagnostics: [],
                sites: [],
                directives: { file_level: {}, entries: [] },
              },
            },
          ]
        : [],
    );
    const reply = ctl.evalAll();
    await reply;
    await Promise.resolve();
    expect(flashes(view)).toEqual([
      { from: 0, to: text.length, cls: FLASH_CLASS },
      { from: 8, to: 15, cls: FLASH_ERROR_CLASS },
    ]);
    vi.advanceTimersByTime(FLASH_MS);
    expect(flashes(view)).toEqual([]);
  });
});

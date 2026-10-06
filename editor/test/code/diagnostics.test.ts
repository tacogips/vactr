import { Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CHECK_DEBOUNCE_MS, DiagnosticsController } from '../../src/code/diagnostics';
import { DocumentSync } from '../../src/code/sync';
import { CodeSurface } from '../../src/code/surface';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { Diagnostic } from '../../src/protocol/types';
import { RecordingTransport } from '../support/recording';

const enc = new TextEncoder();

const TEXT = 'let ü 1\nd1 "bd sd"\nd2 "hh"';

function span(text: string, literal: string): { start: number; end: number } {
  const at = text.indexOf(literal);
  const start = enc.encode(text.slice(0, at)).length;
  return { start, end: start + enc.encode(literal).length };
}

function diag(literal: string, message: string, extra: Partial<Diagnostic> = {}, text = TEXT): Diagnostic {
  return { code: 'x', severity: 'error', message, span: span(text, literal), file: 'main.vact', ...extra };
}

const views: CodeSurface[] = [];

function setup(tier: 'browser' | 'native', check?: (text: string) => Diagnostic[]) {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store() });
  const initial = Text.of(TEXT.split('\n'));
  const sync = new DocumentSync(client.document('main.vact'), initial);
  let view: CodeSurface | null = null;
  const core = check ? { check: vi.fn(check) } : undefined;
  const ctl = new DiagnosticsController({
    client,
    sync,
    tier,
    ...(core ? { core } : {}),
    text: () => view?.state.doc.toString() ?? '',
  });
  view = new CodeSurface({ sync });
  views.push(view);
  ctl.attach(view);
  const evalResult = (diagnostics: Diagnostic[], rev = 1) =>
    transport.emit({
      kind: 'eval-result',
      body: { file: 'main.vact', doc_revision: rev, forms: [], diagnostics, sites: [], directives: { file_level: {}, entries: [] } },
    });
  const shown = (_v: CodeSurface) => ctl.current().map((d) => ({ text: view!.state.doc.sliceString(d.from, d.to), message: d.message, source: d.source }));
  const annotated = () => view!.annotationRanges().filter((range) => range.kind === 'diagnostic').map((range) => ({
    text: view!.state.doc.sliceString(range.from, range.to), label: range.label, className: range.className,
  }));
  return { transport, client, sync, ctl, view, core, evalResult, shown, annotated };
}

describe('DiagnosticsController', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    for (const v of views.splice(0)) v.dispose();
    vi.useRealTimers();
  });

  it('merges static, check and runtime diagnostics', () => {
    const { transport, view, ctl, evalResult, shown, annotated } = setup('browser', (t) =>
      t.includes('# ok') ? [diag('hh', 'check: unknown sound', {}, t)] : [],
    );
    evalResult([diag('bd sd', 'static: bad pattern', { severity: 'warning' })]);
    transport.emit({ kind: 'diag', body: { add: [diag('d2', 'late event', { slot: 'd2', beat: [3, 4] })], clear: [] } });
    // A typing-time check runs 300 ms after the last edit.
    view.dispatch({ changes: { from: 0, insert: '# ok\n' } });
    vi.advanceTimersByTime(CHECK_DEBOUNCE_MS - 1);
    expect(shown(view).map((d) => d.source?.split(':')[0])).toEqual(['static', 'runtime']);
    vi.advanceTimersByTime(1);
    expect(shown(view)).toEqual([
      { text: 'bd sd', message: 'static: bad pattern', source: 'static:x' },
      { text: 'd2', message: 'late event (slot d2, beat 3/4)', source: 'runtime:x' },
      { text: 'hh', message: 'check: unknown sound', source: 'check:x' },
    ]);
    expect(ctl.current()).toHaveLength(3);
    expect(annotated()).toEqual([
      { text: 'bd sd', label: 'static: bad pattern', className: 'vact-diag-warning' },
      { text: 'd2', label: 'late event (slot d2, beat 3/4)', className: 'vact-diag-error' },
      { text: 'hh', label: 'check: unknown sound', className: 'vact-diag-error' },
    ]);
  });

  it('removes a slot runtime markers on clear', () => {
    const { transport, sync, ctl, view, evalResult, shown } = setup('native');
    const pin = vi.spyOn(sync, 'pin'), unpin = vi.spyOn(sync, 'unpin');
    evalResult([]);
    transport.emit({
      kind: 'diag',
      body: { add: [diag('d1', 'd1 late', { slot: 'd1' }), diag('d2', 'd2 late', { slot: 'd2' })], clear: [] },
    });
    expect(shown(view).map((d) => d.text)).toEqual(['d1', 'd2']);
    expect(pin).toHaveBeenCalledWith('diag:static', 1);
    expect(pin).toHaveBeenCalledWith('diag:runtime:d1', 1);
    transport.emit({ kind: 'diag', body: { add: [], clear: [{ slot: 'd1' }] } });
    expect(shown(view).map((d) => d.text)).toEqual(['d2']);
    expect(unpin).toHaveBeenCalledWith('diag:runtime:d1');
    ctl.dispose();
    expect(unpin).toHaveBeenCalledWith('diag:static');
  });

  it('drops a diagnostic whose span no longer maps', () => {
    const { view, evalResult, shown, ctl } = setup('native');
    evalResult([diag('bd sd', 'gone soon'), diag('hh', 'kept')]);
    expect(ctl.current()).toHaveLength(2);
    // Edit inside the first span, then a newer reply re-publishes the old revision's list.
    const at = view.state.doc.toString().indexOf('bd sd') + 1;
    view.dispatch({ changes: { from: at, insert: 'x' } });
    evalResult([diag('bd sd', 'gone soon'), diag('hh', 'kept')], 1);
    expect(shown(view).map((d) => d.message)).toEqual(['kept']);
    // A revision outside the history maps nothing.
    evalResult([diag('hh', 'future')], 42);
    expect(shown(view)).toEqual([]);
  });

  it('never runs session_check on the native tier', () => {
    const { view, core, ctl } = setup('native', () => [diag('hh', 'x')]);
    expect(ctl.checksEnabled).toBe(false);
    view.dispatch({ changes: { from: 0, insert: '# a\n' } });
    vi.advanceTimersByTime(CHECK_DEBOUNCE_MS * 2);
    expect(core?.check).not.toHaveBeenCalled();
  });

  it('debounces the check to 300 ms after the LAST edit', () => {
    const { view, core } = setup('browser', () => []);
    view.dispatch({ changes: { from: 0, insert: 'a' } });
    vi.advanceTimersByTime(200);
    view.dispatch({ changes: { from: 0, insert: 'b' } });
    vi.advanceTimersByTime(200);
    expect(core?.check).not.toHaveBeenCalled();
    vi.advanceTimersByTime(100);
    expect(core?.check).toHaveBeenCalledTimes(1);
    expect(core?.check).toHaveBeenCalledWith(`ba${TEXT}`);
  });

  it('runs one check after 20 edits spaced 50 ms apart', () => {
    const { view, core } = setup('browser', () => []);
    for (let i = 0; i < 20; i += 1) {
      view.dispatch({ changes: { from: 0, insert: String(i % 10) } });
      vi.advanceTimersByTime(50);
    }
    vi.advanceTimersByTime(CHECK_DEBOUNCE_MS);
    expect(core?.check).toHaveBeenCalledTimes(1);

    view.dispatch({ changes: { from: 0, insert: 'x' } });
    vi.advanceTimersByTime(CHECK_DEBOUNCE_MS);
    expect(core?.check).toHaveBeenCalledTimes(2);
  });

  it('checks an unchanged revision once and checks again after an edit', () => {
    const { view, sync, core, ctl } = setup('browser', () => []);
    const pin = vi.spyOn(sync, 'pin');
    ctl.runCheck();
    ctl.runCheck();
    expect(core?.check).toHaveBeenCalledTimes(1);
    expect(pin).toHaveBeenCalledWith('diag:check', 1);

    view.dispatch({ changes: { from: 0, insert: 'x' } });
    ctl.runCheck();
    expect(core?.check).toHaveBeenCalledTimes(2);
  });

  it('does not run checks from refreshes or elapsed frame time', () => {
    const { core, ctl } = setup('browser', () => []);
    ctl.runCheck();
    vi.advanceTimersByTime(10_000);
    for (let i = 0; i < 20; i += 1) ctl.refresh();
    expect(core?.check).toHaveBeenCalledTimes(1);
  });

  it('retries a check that threw at the same revision', () => {
    let attempts = 0;
    const { core, ctl } = setup('browser', () => {
      attempts += 1;
      if (attempts === 1) throw new Error('transient check failure');
      return [];
    });
    ctl.runCheck();
    ctl.runCheck();
    expect(core?.check).toHaveBeenCalledTimes(2);
    ctl.runCheck();
    expect(core?.check).toHaveBeenCalledTimes(2);
  });
});

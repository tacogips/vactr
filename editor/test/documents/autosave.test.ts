import { Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';
import type { Draft, DraftStore, StorageResult } from '../../src/documents/types';
import { DocumentSession } from '../../src/documents/session';
import { DraftAutosave } from '../../src/documents/autosave';

function makeSurface(): CodeSurface {
  const client = new Client(new RecordingTransport(), { store: new Store() });
  return new CodeSurface({ sync: new DocumentSync(client.document('main.vact'), Text.of([''])) });
}
function draftStore() {
  const writes: Draft[] = [];
  let result: StorageResult = { ok: true };
  let clears = 0;
  const store: DraftStore = {
    readDraft: () => null,
    writeDraft: (draft) => { writes.push(draft); return result; },
    clearDraft: () => { clears++; },
  };
  return { store, writes, get clears() { return clears; }, failWith(value: StorageResult) { result = value; } };
}
function setup() {
  const surface = makeSurface();
  const session = new DocumentSession(surface);
  const drafts = draftStore();
  const notice = vi.fn();
  const autosave = new DraftAutosave({ surface, session, drafts: drafts.store, win: window, now: () => 1234, notice });
  return { surface, session, drafts, notice, autosave };
}

describe('DraftAutosave', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('writes only dirty buffers after a trailing one second delay', () => {
    const { surface, session, drafts } = setup();
    session.loadInitial('base', { kind: 'example', id: 'showcase' }, { restored: false });
    expect(drafts.clears).toBe(1);
    surface.dispatch({ changes: { from: 4, insert: '!' } });
    vi.advanceTimersByTime(999);
    expect(drafts.writes).toHaveLength(0);
    vi.advanceTimersByTime(1);
    expect(drafts.writes).toEqual([{ text: 'base!', source: { kind: 'example', id: 'showcase' }, savedAt: 1234 }]);
    surface.dispose(); session.dispose();
  });

  it('restarts the full debounce for repeated edits', () => {
    const { surface, session, drafts } = setup();
    session.loadInitial('', { kind: 'new' }, { restored: false });
    for (let i = 0; i < 10; i++) {
      surface.dispatch({ changes: { from: surface.state.doc.length, insert: 'x' } });
      vi.advanceTimersByTime(300);
      expect(drafts.writes).toHaveLength(0);
    }
    vi.advanceTimersByTime(699);
    expect(drafts.writes).toHaveLength(0);
    vi.advanceTimersByTime(1);
    expect(drafts.writes).toHaveLength(1);
    expect(drafts.writes[0]?.text).toBe('x'.repeat(10));
    surface.dispose(); session.dispose();
  });

  it('clears drafts for clean replacement and initial load without writing', () => {
    const { surface, session, drafts } = setup();
    surface.dispatch({ changes: { from: 0, insert: 'pending' } });
    session.replace('clean', { kind: 'example', id: 'showcase' });
    expect(drafts.clears).toBe(1);
    vi.advanceTimersByTime(1000);
    expect(drafts.writes).toHaveLength(0);
    session.loadInitial('ignored', { kind: 'new' }, { restored: false });
    expect(drafts.clears).toBe(1);
    surface.dispose(); session.dispose();
  });

  it('flushes pending work on pagehide and hidden visibilitychange', () => {
    const first = setup();
    first.session.loadInitial('', { kind: 'new' }, { restored: false });
    first.surface.dispatch({ changes: { from: 0, insert: 'page' } });
    window.dispatchEvent(new Event('pagehide'));
    expect(first.drafts.writes).toHaveLength(1);
    first.autosave.dispose(); first.surface.dispose(); first.session.dispose();

    const second = setup();
    second.session.loadInitial('', { kind: 'new' }, { restored: false });
    second.surface.dispatch({ changes: { from: 0, insert: 'hidden' } });
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' });
    document.dispatchEvent(new Event('visibilitychange'));
    expect(second.drafts.writes).toHaveLength(1);
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
    second.autosave.dispose();
    second.surface.dispose(); second.session.dispose();
  });

  it('shows too-large and quota notices once until a successful write', () => {
    const { surface, session, drafts, notice } = setup();
    session.loadInitial('', { kind: 'new' }, { restored: false });
    drafts.failWith({ ok: false, reason: 'too-large' });
    surface.dispatch({ changes: { from: 0, insert: 'a' } });
    vi.advanceTimersByTime(1000);
    surface.dispatch({ changes: { from: 1, insert: 'b' } });
    vi.advanceTimersByTime(1000);
    expect(notice).toHaveBeenCalledOnce();
    expect(notice).toHaveBeenLastCalledWith('Document too large for the browser draft; use Save file');

    drafts.failWith({ ok: true });
    surface.dispatch({ changes: { from: 2, insert: 'c' } });
    vi.advanceTimersByTime(1000);
    drafts.failWith({ ok: false, reason: 'quota' });
    surface.dispatch({ changes: { from: 3, insert: 'd' } });
    vi.advanceTimersByTime(1000);
    expect(notice).toHaveBeenCalledTimes(2);
    surface.dispatch({ changes: { from: 4, insert: 'e' } });
    vi.advanceTimersByTime(1000);
    expect(notice).toHaveBeenCalledTimes(2);
    expect(notice).toHaveBeenLastCalledWith('Browser storage is full; draft not saved');
    surface.dispose(); session.dispose();
  });

  it('disposal cancels timers and removes lifecycle listeners without writing', () => {
    const { surface, session, drafts, autosave } = setup();
    session.loadInitial('', { kind: 'new' }, { restored: false });
    surface.dispatch({ changes: { from: 0, insert: 'pending' } });
    autosave.dispose();
    window.dispatchEvent(new Event('pagehide'));
    document.dispatchEvent(new Event('visibilitychange'));
    vi.advanceTimersByTime(1000);
    expect(drafts.writes).toHaveLength(0);
    surface.dispose(); session.dispose();
  });
});

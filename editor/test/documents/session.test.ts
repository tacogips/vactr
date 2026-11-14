import { Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';
import { DocumentSession } from '../../src/documents/session';
import { createVimController } from 'emaki/vim';
import { createVimHost } from '../../src/code/vim-mount';
import { isFileBoundary } from '../../src/code/file-boundary';

function surfaceWith(text = ''): CodeSurface {
  const client = new Client(new RecordingTransport(), { store: new Store() });
  return new CodeSurface({ sync: new DocumentSync(client.document('main.vact'), Text.of(text.split('\n'))) });
}

describe('DocumentSession', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('resets undo and redo on file replacement and preserves new-file editing history', () => {
    const surface = surfaceWith();
    const session = new DocumentSession(surface);
    surface.dispatch({ changes: { from: 0, insert: 'abc' }, userEvent: 'input.type' });
    session.replace('X', { kind: 'example', id: 'acid-techno' });
    expect(surface.state.doc.toString()).toBe('X');
    expect(surface.undo()).toBe(false);
    expect(surface.redo()).toBe(false);
    expect(surface.state.doc.toString()).toBe('X');
    expect(session.dirty()).toBe(false);
    surface.dispatch({ changes: { from: 1, insert: '!' }, userEvent: 'input.type' });
    expect(surface.undo()).toBe(true);
    expect(surface.state.doc.toString()).toBe('X');
    expect(surface.redo()).toBe(true);
    expect(surface.state.doc.toString()).toBe('X!');
    surface.dispose(); session.dispose();
  });

  it.each(['different', 'old'])('prevents Vim redo across a file load containing %s text', loaded => {
    const surface = surfaceWith('old');
    const session = new DocumentSession(surface);
    const viewHost = { viewport: { scrollTop: 0, height: 180, width: 600 }, scrollBy: () => undefined, scrollCaret: () => undefined };
    const layout = { font: { lineHeight: 18 }, coordsAtPos: () => null };
    const vim = createVimController(createVimHost({ surface, viewHost: viewHost as never, layout: layout as never }), { hooks: { isDocumentBoundary: isFileBoundary } });
    const stopUpdates = surface.subscribe((update) => vim.noteUpdate(update));
    const key = (value: string) => vim.handleKey({ key: value === '<C-r>' ? 'r' : value,
      ctrlKey: value === '<C-r>', metaKey: false, altKey: false, shiftKey: false });
    key('x'); key('u');
    expect(surface.state.doc.toString()).toBe('old');
    expect(surface.retentionStatus.redoDepth).toBeGreaterThan(0);
    session.replace(loaded, { kind: 'file', name: 'new.vact' });
    key('<C-r>'); key('u');
    expect(surface.state.doc.toString()).toBe(loaded);
    key('x'); key('u');
    expect(surface.state.doc.toString()).toBe(loaded);
    key('<C-r>');
    expect(surface.state.doc.toString()).toBe(loaded.slice(1));
    stopUpdates(); vim.dispose(); session.dispose(); surface.dispose();
  });

  it('tracks clean and restored startup baselines without adding initial history', () => {
    const surface = surfaceWith();
    const session = new DocumentSession(surface);
    expect(session.loadInitial('S', { kind: 'example', id: 'showcase' }, { restored: false })).toBe(true);
    expect(surface.state.doc.toString()).toBe('S');
    expect(surface.retentionStatus.undoDepth).toBe(0);
    expect(session.dirty()).toBe(false);
    session.replace('D', { kind: 'new' });
    surface.dispose(); session.dispose();

    const restoredSurface = surfaceWith();
    const restoredSession = new DocumentSession(restoredSurface);
    expect(restoredSession.loadInitial('draft', { kind: 'new' }, { restored: true })).toBe(true);
    expect(restoredSession.dirty()).toBe(true);
    restoredSurface.dispose(); restoredSession.dispose();
  });

  it('refuses to load over a user edit and retains source and dirty state semantics', () => {
    const surface = surfaceWith();
    const session = new DocumentSession(surface);
    surface.dispatch({ changes: { from: 0, insert: 'user' }, userEvent: 'input.type' });
    expect(session.loadInitial('startup', { kind: 'template' }, { restored: false })).toBe(false);
    expect(surface.state.doc.toString()).toBe('user');
    session.markSaved({ kind: 'library', id: 'id-1', name: 'Saved' });
    expect(session.dirty()).toBe(false);
    expect(session.source).toEqual({ kind: 'library', id: 'id-1', name: 'Saved' });
    surface.dispatch({ changes: { from: 4, insert: '!' } });
    session.setSource({ kind: 'new' });
    expect(session.source).toEqual({ kind: 'new' });
    expect(session.dirty()).toBe(true);
    const baseline = vi.fn();
    const stop = session.onBaseline(baseline);
    session.markSaved({ kind: 'new' });
    expect(baseline).toHaveBeenCalledOnce();
    stop();
    surface.dispose(); session.dispose();
  });

  it('compares an untouched empty document with Text.empty and unsubscribes on dispose', () => {
    const surface = surfaceWith();
    const session = new DocumentSession(surface);
    expect(session.dirty()).toBe(false);
    session.dispose();
    surface.dispatch({ changes: { from: 0, insert: 'later' } });
    expect(session.loadInitial('ignored', { kind: 'new' }, { restored: false })).toBe(false);
    surface.dispose();
  });
});

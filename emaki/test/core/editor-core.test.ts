import { describe, expect, it, vi } from 'vitest';
import { EditorCore } from '../../src/core/editor-core';
import { InputController } from '../../src/input/input';
import { TextLayout } from '../../src/layout/layout';
import { ViewHost } from '../../src/view/view-host';

describe('EditorCore', () => {
  it('calls transaction hooks once before publishing', () => {
    const core = new EditorCore({ doc: 'abc' });
    const events: string[] = [];
    core.onTransaction(() => events.push('transaction'));
    core.subscribe(() => events.push('subscriber'));
    core.dispatch({ changes: { from: 1, insert: 'x' } });
    expect(events).toEqual(['transaction', 'subscriber']);
  });

  it('rejects reentrant and stale transactions', () => {
    const core = new EditorCore({ doc: 'abc' });
    const stale = core.state.update({ changes: { from: 0, insert: 'x' } });
    core.subscribe(() => expect(() => core.dispatch({})).toThrow(/Reentrant/));
    core.dispatch({ changes: { from: 1, insert: 'y' } });
    expect(() => core.dispatch(stale)).toThrow(/Stale or mixed/);
  });

  it('trims external history during dispatch', () => {
    const trimToBytes = vi.fn();
    const core = new EditorCore({ doc: 'abc', externalHistory: { retainedBytes: 0, reducedDepth: false, trimToBytes } });
    core.dispatch({ changes: { from: 0, insert: 'x' } });
    expect(trimToBytes).toHaveBeenCalledOnce();
  });

  it('defers source writes intersecting composition until cleared', () => {
    const core = new EditorCore({ doc: 'abc' });
    const write = vi.fn();
    core.setCompositionRange({ from: 1, to: 2 });
    core.deferSourceWrite({ from: 2, to: 3 }, write);
    expect(write).not.toHaveBeenCalled();
    core.setCompositionRange(null);
    expect(write).toHaveBeenCalledOnce();
  });

  it('reveals the caret after a block-only heightmap change', () => {
    const core = new EditorCore({ doc: 'a\nb' });
    core.dispatch({ selection: { anchor: 2 } });
    const layout = new TextLayout({ font: '', measureText: text => ({ width: text.length * 8 }) }, { font: '16px mono', lineHeight: 18, baseline: 14 });
    layout.setDocument('a\nb');
    const host = new ViewHost(document.createElement('div'), core, layout, { requestAnimationFrame: () => 0, cancelAnimationFrame: () => {}, devicePixelRatio: 1, innerHeight: 100 }, () => {});
    host.setViewport({ width: 200, height: 40, dpr: 1, keyboardInset: 0 });
    expect(host.scrollPosition.top).toBe(0);
    layout.heightmap.setBlocks([{ line: 0, above: 0, below: 40 }]);
    host.scrollCaret();
    expect(host.scrollPosition.top).toBe(36);
    host.dispose(); core.dispose();
  });

  it('allows subclasses to publish and attach input', () => {
    class Sub extends EditorCore {
      emit(): void {
        this.publish({ state: this.state, changes: this.state.changes(), docChanged: false, selectionSet: false, userEvent: null });
      }
    }
    const core = new Sub({ doc: '' });
    const received = vi.fn();
    core.subscribe(received);
    core.emit();
    expect(received).toHaveBeenCalledOnce();
    const input = new InputController(core, document.createElement('div'));
    input.dispose();
  });
});

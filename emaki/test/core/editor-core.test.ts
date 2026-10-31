import { describe, expect, it, vi } from 'vitest';
import { EditorCore } from '../../src/core/editor-core';
import { InputController } from '../../src/input/input';

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

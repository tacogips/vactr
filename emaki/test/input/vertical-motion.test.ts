import { describe, expect, it } from 'vitest';
import { EditorCore } from '../../src/core/editor-core';
import { TextLayout } from '../../src/layout/layout';
import { KeyboardController } from '../../src/input/keyboard';
describe('vertical motion', () => {
  it.each([false, true])('moves by text rows and keeps the column (block=%s)', block => {
    const core = new EditorCore({ doc: 'abcdef\nabcdef\nabcdef' });
    const layout = new TextLayout({ font: '', measureText: text => ({ width: text.length * 8 }) }, { font: '16px mono', lineHeight: 20, baseline: 15 });
    layout.setDocument(core.state.doc.toString()); if (block) layout.heightmap.setBlocks([{ line: 1, above: 0, below: 60 }]);
    const view = { width: 300, height: 200, scrollLeft: 0, scrollTop: 0 };
    core.attachBridge({ focus() {}, coordsAtPos: pos => layout.coordsAtPos(pos, view), posAtCoords: coords => layout.posAtCoords(coords, view) });
    const keyboard = new KeyboardController(core); core.dispatch({ selection: { anchor: 10 } });
    keyboard.handle(new KeyboardEvent('keydown', { key: 'ArrowDown' })); expect(core.state.selection.main.head).toBe(17);
    keyboard.handle(new KeyboardEvent('keydown', { key: 'ArrowUp' })); expect(core.state.selection.main.head).toBe(10);
    core.dispose();
  });
});

import { ChangeSet, EditorState } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import { DecorationStore } from '../../src/decor/store';
import { WidgetSet } from '../../src/plugin/widgets';
import { PluginRuntime } from '../../src/plugin/runtime';
import { FALLBACK_PALETTE } from '../../src/render/palette';
import type { PluginHost } from '../../src/types';

function setup() {
  let state = EditorState.create({ doc: 'abcd' });
  const store = new DecorationStore(() => state.doc.length);
  const widgets = new WidgetSet(state.doc.length);
  const onError = vi.fn();
  const runtime = new PluginRuntime({ store, widgets, getState: () => state, requestFrame: () => {},
    coordsAtPos: () => null, lineTop: () => 0, viewport: () => ({ fromLine: 0, toLine: 1, top: 0, height: 18 }),
    theme: () => ({ palette: FALLBACK_PALETTE, tokens: {} }), onError });
  const update = (changes: ChangeSet) => {
    state = state.update({ changes }).state;
    runtime.docChanged({ state, changes, docChanged: true, selectionSet: false });
  };
  return { runtime, store, widgets, onError, update, state: () => state };
}
const mark = { from: 1, to: 2, style: { opacity: 0.5 } };
const content = { kind: 'dom' as const, mount: () => () => {} };

describe('PluginRuntime', () => {
  it('namespaces owners across plugins', () => {
    const s = setup();
    for (const name of ['one', 'two']) s.runtime.add({ name, attach: host => { host.marks('x').set([mark]); } });
    expect(s.store.collect(0, s.state().doc).marks).toHaveLength(2);
  });
  it('isolates a throwing hook and continues other plugins', () => {
    const s = setup(); const other = vi.fn(() => false); const destroy = vi.fn();
    s.runtime.add({ name: 'bad', attach: host => { host.marks('x').set([mark]); return { frame: () => { throw new Error('failure'); }, destroy }; } });
    s.runtime.add({ name: 'good', attach: () => ({ frame: other }) });
    s.runtime.frame(1); s.runtime.frame(2);
    expect(s.onError).toHaveBeenCalledTimes(1); expect(destroy).toHaveBeenCalledTimes(1);
    expect(s.store.collect(0, s.state().doc).marks).toHaveLength(0); expect(other).toHaveBeenCalledTimes(2);
  });
  it('removes widgets and animated providers and destroys once', () => {
    const s = setup(); const destroy = vi.fn();
    const remove = s.runtime.add({ name: 'owner', attach: host => {
      host.widgets('x').set([{ key: 'x', pos: 0, side: 'below', height: 4, content }]);
      host.animatedMarks(() => [{ from: 1, to: 2, style: { background: [1, 0, 0, 1] } }]); return { destroy };
    } });
    expect(s.store.collect(0, s.state().doc).animatedMarks).toHaveLength(1);
    remove(); remove();
    expect(destroy).toHaveBeenCalledTimes(1); expect(s.widgets.blocks(s.state().doc)).toHaveLength(0);
    expect(s.store.collect(0, s.state().doc).animatedMarks).toHaveLength(0);
  });
  it('suppresses identical viewports', () => {
    const s = setup(); const changed = vi.fn(); s.runtime.add({ name: 'a', attach: () => ({ viewportChanged: changed }) });
    const view = { fromLine: 0, toLine: 1, top: 0, height: 18 };
    s.runtime.viewportChanged(view); s.runtime.viewportChanged({ ...view });
    expect(changed).toHaveBeenCalledTimes(1);
  });
  it('maps decorations before docChanged', () => {
    const s = setup(); let seen = -1;
    s.runtime.add({ name: 'a', attach: (host: PluginHost) => { host.marks('x').set([mark]); return {
      docChanged: () => { seen = s.store.collect(0, s.state().doc).marks[0]!.from; },
    }; } });
    s.update(ChangeSet.of({ from: 0, insert: 'z' }, 4)); expect(seen).toBe(2);
  });
});

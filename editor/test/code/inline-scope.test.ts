import { expect, it, vi } from 'vitest';
import { EditorState } from '@codemirror/state';
import type { BlockWidgetSpec, PluginHost } from 'emaki';
import { createScopePlugin } from '../../src/code/inline/scope';

vi.mock('../../src/visual/scopes', async (importOriginal) => ({ ...await importOriginal<typeof import('../../src/visual/scopes')>(), drawScope: vi.fn() }));
import { drawScope } from '../../src/visual/scopes';

it('draws master waveform and stops animating after telemetry expires', () => {
  let now = 0; vi.spyOn(performance, 'now').mockImplementation(() => now);
  let blocks: readonly BlockWidgetSpec[] = [];
  const host = { state: EditorState.create({ doc: '#@ editor {"visualizers":[{"kind":"oscilloscope"}]}' }), widgets: () => ({ set: (b: readonly BlockWidgetSpec[]) => { blocks = b; }, clear: () => { blocks = []; } }), requestFrame: vi.fn() } as unknown as PluginHost;
  const scope = createScopePlugin({ time: () => 0, latestOnReceipt: () => true, enabled: () => true }); scope.plugin.attach(host);
  scope.onLevels({ levels: [{ source: 'master', rms: 0, waveform: Array(256).fill(0) }] });
  const content = blocks[0].content; expect(content.kind).toBe('canvas');
  if (content.kind === 'canvas') {
    const ctx = { canvas: document.createElement('canvas'), fillRect: vi.fn() } as unknown as CanvasRenderingContext2D;
    expect(content.draw(ctx, { width: 100, height: 64, dpr: 1, frameMs: 0 })).toBe(true); expect(drawScope).toHaveBeenCalled();
    now = 3000; expect(content.draw(ctx, { width: 100, height: 64, dpr: 1, frameMs: now })).toBe(false);
  }
  vi.restoreAllMocks();
});

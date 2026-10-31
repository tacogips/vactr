import { expect, it, vi } from 'vitest';
import { EditorState } from '@codemirror/state';
import type { BlockWidgetSpec, PluginHost } from 'emaki';
import { createSequencerPlugin } from '../../src/code/inline/sequencer';

it('renders and clears current cycle steps at their source form', () => {
  let blocks: readonly BlockWidgetSpec[] = [];
  const host = { state: EditorState.create({ doc: '(foo)' }), widgets: () => ({ set: (b: readonly BlockWidgetSpec[]) => { blocks = b; }, clear: () => { blocks = []; } }), requestFrame: vi.fn() } as unknown as PluginHost;
  let enabled = true;
  const seq = createSequencerPlugin({ file: 'doc', beatsPerCycle: () => 4, mapSpan: (s) => ({ from: s.start, to: s.end }), forms: () => [{ from: 0, to: 5 }], cycles: () => 0.3, running: () => true, enabled: () => enabled });
  seq.plugin.attach(host);
  seq.onPlaying([0, 1, 2, 3].map((n) => ({ slot: 'a', beat: [n, 1], dur: [1, 1], time: 0, src: { file: 'doc', span: { start: 1, end: 2 }, doc_revision: 1, form_gen: 0 } })));
  expect(blocks).toHaveLength(1); expect(blocks[0].height).toBe(20);
  const colors: string[] = []; const canvas = document.createElement('canvas');
  const recording = { canvas, fillStyle: '', fillRect() { colors.push(recording.fillStyle); } };
  const ctx = recording as unknown as CanvasRenderingContext2D;
  const content = blocks[0].content;
  expect(content.kind).toBe('canvas');
  if (content.kind === 'canvas') expect(content.draw(ctx, { width: 100, height: 20, dpr: 1, frameMs: 0 })).toBe(true);
  expect(colors[2]).not.toBe(colors[1]); expect(colors[2]).not.toBe(colors[3]);
  enabled = false; seq.refresh(); expect(blocks).toHaveLength(0);
  enabled = true; seq.clear(); expect(blocks).toHaveLength(0);
});

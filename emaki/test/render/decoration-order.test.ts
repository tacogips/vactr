import { describe, expect, it } from 'vitest';
import { TextLayout } from '../../src/layout/layout';
import { CanvasRenderer } from '../../src/render/renderer';
import { ResourceLedger } from '../../src/render/resources';
import { recordingGL, rasterizer, width } from '../support/recording-gl';

function fixture(text: string) {
  const gl = recordingGL(); const raster = rasterizer();
  const layout = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument(text);
  const renderer = new CanvasRenderer(document.createElement('canvas'), layout, { gl: gl.gl, ledger: new ResourceLedger(), createCanvas: raster.createCanvas });
  renderer.setViewport({ width: 320, height: 100, scrollLeft: 0, scrollTop: 0, gutter: 48 }, 1);
  return { gl, layout, renderer };
}

describe('decoration order', () => {
  it('keeps backgrounds, animated marks and overlays in order', () => {
    const f = fixture('let x = 1');
    expect(f.renderer.render({ selection: [{ from: 0, to: 3 }],
      animatedMarks: [{ from: 4, to: 5, style: { background: [0.7, 0.58, 0.26, 0.5] } }, { from: 0, to: 9, style: { background: [0.4, 0.64, 0.72, 0.5] } }],
      tokens: [{ from: 0, to: 3, tag: 't' }],
      marks: [{ from: 4, to: 5, style: { underline: { color: [0.75, 0.2, 0.25, 1], thickness: 2 } } }, { from: 4, to: 5, style: { label: { text: 'CC 1' } } }],
      composition: [{ from: 6, to: 9 }], cursor: 5, handles: [{ pos: 0 }, { pos: 9, end: true }] })).toBe(true);
    const colors = f.gl.draws.map(d => d.color); const q = (v: number[]) => v.map(x => Math.round(x * 255) / 255);
    expect(colors.slice(0, 4)).toEqual([[0.047, 0.059, 0.082, 0.85], [0.218, 0.4, 0.582, 0.55], [0.7, 0.58, 0.26, 0.5], [0.4, 0.64, 0.72, 0.5]].map(q));
    const underline = colors.findIndex(c => Math.abs(c[0]! - 0.75) < 0.01);
    const cursor = colors.findIndex(c => Math.abs(c[0]! - 0.9) < 0.01);
    const label = colors.findIndex(c => Math.abs(c[0]! - 0.15) < 0.01);
    expect(underline).toBeGreaterThan(3); expect(cursor).toBeGreaterThan(underline); expect(label).toBeGreaterThan(cursor);
    expect(f.gl.draws.some(d => d.layer === 'text' && Math.abs(d.color[0]! - 0.7) < 0.01)).toBe(false);
    f.renderer.dispose();
  });
});

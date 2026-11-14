import { describe, expect, it } from 'vitest';
import { TextLayout, type LayoutFont } from 'emaki/engine';
import { CanvasRenderer } from '../../src/code/renderer';
import { CLIP_GUTTER, SNAP_GLYPH } from 'emaki/engine';
import { ResourceLedger } from 'emaki/engine';
import { rasterizer, recordingGL, width } from './recording-gl';

const font: LayoutFont = { font: '16px test-mono', fallback: 'test-ja', lineHeight: 20, baseline: 16 };
const view = { width: 320, height: 100, scrollLeft: 0, scrollTop: 0, left: 10, top: 20, gutter: 48 };

describe('renderer layer placement', () => {
  it.each([1, 2])('keeps renderer layers and glyph quads aligned at DPR %i', dpr => {
    const text = '# c\nuse-bpm 108\ns :kalimba > note :c5';
    const doc = new TextLayout({ font: '', measureText: value => ({ width: width(value) }) }, font);
    doc.setDocument(text);
    const gl = recordingGL(); const raster = rasterizer(); const canvas = document.createElement('canvas');
    const renderer = new CanvasRenderer(canvas, doc, { gl: gl.gl, ledger: new ResourceLedger(), createCanvas: raster.createCanvas });
    renderer.setViewport(view, dpr);
    const diagnosticFrom = text.indexOf('note');

    try {
      expect(renderer.render({ cursor: text.length, annotations: [
        { kind: 'selection', from: 0, to: 3 },
        { kind: 'diagnostic', from: diagnosticFrom, to: diagnosticFrom + 4 },
      ] })).toBe(true);

      const nonText = gl.draws.filter(draw => draw.layer !== 'text');
      expect(nonText.length).toBeGreaterThan(0);
      expect(nonText.every(draw => draw.slot === 0)).toBe(true);
      expect(gl.draws.some(draw => draw.layer === 'background' && draw.absTop === 0 && draw.rect[0] === 0 && draw.rect[1] === 0 && draw.rect[2] === view.width && draw.rect[3] === view.height)).toBe(true);
      expect(gl.draws.some(draw => draw.layer === 'overlay' && draw.absTop === 2 * font.lineHeight && draw.rect[2] === 1)).toBe(true);
      expect(gl.draws.some(draw => draw.layer === 'overlay' && draw.absTop === 3 * font.lineHeight - 2 && draw.rect[3] === 2)).toBe(true);

      const glyphs = gl.draws.filter(draw => draw.layer === 'text' && (draw.flags & SNAP_GLYPH) !== 0);
      expect(glyphs.length).toBeGreaterThan(0);
      const activeText = gl.draws.filter(draw => draw.layer === 'text' && draw.rect[2] > 0 && draw.rect[3] > 0);
      expect(activeText.every(draw => (draw.flags & SNAP_GLYPH) !== 0)).toBe(true);
      expect(glyphs.every(draw => Number.isInteger(draw.rect[2] * dpr) && Number.isInteger(draw.rect[0] * dpr))).toBe(true);

      const gutterOne = glyphs.find(draw => (draw.flags & CLIP_GUTTER) === 0 && draw.absLeft < view.gutter);
      const documentOne = glyphs.find(draw => (draw.flags & CLIP_GUTTER) !== 0 && draw.rect[2] < 20 && Math.abs(draw.rect[0] - (view.gutter + 62)) < 0.01);
      expect(gutterOne).toBeDefined();
      expect(documentOne).toBeDefined();
      expect(documentOne!.uv).not.toEqual(gutterOne!.uv);

      const deviceCall = gl.calls.find(call => call.name === 'uniform2f' && (call.args[0] as { name?: string })?.name === 'u_device');
      expect(deviceCall?.args.slice(1)).toEqual([canvas.width / view.width, canvas.height / view.height]);
      const backdrop = gl.draws.find(draw => draw.layer === 'background' && draw.rect[2] === view.width && draw.rect[3] === view.height);
      expect(backdrop?.color.map(channel => Math.round(channel * 255) / 255)).toEqual([0.047, 0.059, 0.082, 0.85].map(channel => Math.round(channel * 255) / 255));
    } finally {
      renderer.dispose();
    }
  });
});

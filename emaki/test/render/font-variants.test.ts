import { expect, it } from 'vitest';
import { TextLayout } from '../../src/layout/layout';
import { CanvasRenderer } from '../../src/render/renderer';
import { ResourceLedger } from '../../src/render/resources';
import { recordingGL, rasterizer, width } from '../support/recording-gl';

it('reshapes only the line whose font variant changes', () => {
  const gl = recordingGL(), raster = rasterizer();
  const layout = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument('one\ntwo\nthree');
  const renderer = new CanvasRenderer(document.createElement('canvas'), layout, { gl: gl.gl, ledger: new ResourceLedger(), createCanvas: raster.createCanvas });
  renderer.setViewport({ width: 320, height: 100, scrollLeft: 0, scrollTop: 0, gutter: 48 }, 1);
  expect(renderer.render({ decorationsKey: '1' })).toBe(true);
  const before = layout.stats.builds;
  expect(renderer.render({ decorationsKey: '2', marks: [{ from: 8, to: 13, style: { fontWeight: 'bold' } }] })).toBe(true);
  expect(layout.stats.builds - before).toBe(1);
  expect(raster.text.some(item => item.font.includes('bold'))).toBe(true);
  expect(renderer.render({ decorationsKey: '3' })).toBe(true);
  expect(layout.stats.builds - before).toBe(2);
  renderer.dispose();
});

it('measures variant advances and keeps them during later hit testing', () => {
  const metrics = { font: '', measureText(text: string) { return { width: text.length * (this.font.includes('bold') ? 12 : 8) }; } };
  const layout = new TextLayout(metrics, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument('abcd\nwxyz');
  layout.setFontVariants(new Map([[0, [{ from: 1, to: 3, variant: 'bold' }]]]));
  const line = layout.shape(0);
  layout.shape(1);
  expect(line.width).toBe(40);
  expect(layout.advance(line, 2)).toBe(20);
});

it('falls back to regular advances for non-finite variant metrics', () => {
  const metrics = { font: '', measureText(text: string) { return { width: this.font.includes('italic') ? NaN : text.length * 8 }; } };
  const layout = new TextLayout(metrics, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument('abcd');
  layout.setFontVariants(new Map([[0, [{ from: 0, to: 4, variant: 'italic' }]]]));
  expect(layout.shape(0).width).toBe(32);
});

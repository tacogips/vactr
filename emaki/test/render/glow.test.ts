import { expect, it } from 'vitest';
import { TextLayout } from '../../src/layout/layout';
import { CanvasRenderer } from '../../src/render/renderer';
import { ResourceLedger } from '../../src/render/resources';
import { recordingGL, rasterizer, width } from '../support/recording-gl';

it('draws glow halos after animated backgrounds without tinting text', () => {
  const gl = recordingGL(), raster = rasterizer();
  const layout = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument('let x = 1');
  const renderer = new CanvasRenderer(document.createElement('canvas'), layout, { gl: gl.gl, ledger: new ResourceLedger(), createCanvas: raster.createCanvas });
  renderer.setViewport({ width: 320, height: 100, scrollLeft: 0, scrollTop: 0, gutter: 48 }, 1);
  const mark = { from: 4, to: 5, style: { background: [0.7, 0.58, 0.26, 0.5] as [number, number, number, number], glow: { color: [1, 0, 0, 0.5] as [number, number, number, number], radius: 6 } } };
  expect(renderer.render({ selection: [{ from: 0, to: 3 }], animatedMarks: [mark, { from: 0, to: 9, style: { background: [0.4, 0.64, 0.72, 0.5] } }] })).toBe(true);
  const background = gl.draws.filter(draw => draw.layer === 'background');
  expect(background.slice(0, 4).map(draw => draw.color.map(c => Math.round(c * 255)))).toEqual([[12, 15, 21, 217], [56, 102, 148, 140], [179, 148, 66, 128], [102, 163, 184, 128]]);
  expect(background.slice(4).map(draw => draw.color[3])).toEqual([57 / 255, 32 / 255, 15 / 255]);
  expect(background.slice(4).map(draw => draw.rect)).toEqual([[78, -2, 12, 22], [76, -4, 16, 26], [74, -6, 20, 30]]);
  expect(gl.draws.some(draw => draw.layer === 'text' && draw.color[0] === 1 && draw.color[1] === 0)).toBe(false);
  renderer.dispose();
});

it('re-emits static glow after animated backgrounds on cached frames', () => {
  const gl = recordingGL(), raster = rasterizer();
  const layout = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument('abc');
  const renderer = new CanvasRenderer(document.createElement('canvas'), layout, { gl: gl.gl, ledger: new ResourceLedger(), createCanvas: raster.createCanvas });
  renderer.setViewport({ width: 320, height: 100, scrollLeft: 0, scrollTop: 0, gutter: 48 }, 1);
  const input = { decorationsKey: '1', marks: [{ from: 0, to: 0, style: { minWidth: 5, glow: { color: [1, 0, 0, 1] as [number, number, number, number], radius: 3 } } }], animatedMarks: [{ from: 0, to: 1, style: { background: [0, 1, 0, 1] as [number, number, number, number] } }] };
  expect(renderer.render(input)).toBe(true);
  const start = gl.draws.length;
  expect(renderer.render(input)).toBe(true);
  const background = gl.draws.slice(start).filter(draw => draw.layer === 'background');
  expect(background[1]!.color).toEqual([0, 1, 0, 1]);
  expect(background.slice(2).map(draw => draw.rect[2])).toEqual([7, 9, 11]);
  renderer.dispose();
});

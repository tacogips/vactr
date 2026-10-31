import { expect, it } from 'vitest';
import { TextLayout } from '../../src/layout/layout';
import { CanvasRenderer } from '../../src/render/renderer';
import { ResourceLedger } from '../../src/render/resources';
import { recordingGL, rasterizer, width } from '../support/recording-gl';

function fixture() {
  const gl = recordingGL(), raster = rasterizer();
  const layout = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument('abc\ndef\nghi');
  const renderer = new CanvasRenderer(document.createElement('canvas'), layout, { gl: gl.gl, ledger: new ResourceLedger(), createCanvas: raster.createCanvas });
  renderer.setViewport({ width: 320, height: 100, scrollLeft: 0, scrollTop: 0, gutter: 48 }, 1);
  return { gl, layout, renderer };
}

it('uses mark color over line and token and multiplies opacity', () => {
  const f = fixture();
  expect(f.renderer.render({ decorationsKey: '1', tokens: [{ from: 0, to: 3, tag: 'missing' }],
    lineFormats: [{ line: 0, style: { color: [0, 1, 0, 1], opacity: 0.5 } }],
    marks: [{ from: 0, to: 3, style: { color: [1, 0, 0, 1], opacity: 0.5 } }] })).toBe(true);
  expect(f.gl.draws.some(draw => draw.layer === 'text' && draw.color[0] === 1 && draw.color[1] === 0 && draw.color[3] === 64 / 255)).toBe(true);
  f.renderer.dispose();
});

it('retints only the changed line without reshaping and ignores glow-only changes', () => {
  const f = fixture();
  expect(f.renderer.render({ decorationsKey: '1' })).toBe(true);
  const builds = f.layout.stats.builds, textBuilds = f.renderer.stats.textBuilds;
  const marks = [{ from: 4, to: 7, style: { color: [1, 0, 0, 1] as [number, number, number, number] } }];
  expect(f.renderer.render({ decorationsKey: '2', marks })).toBe(true);
  expect(f.layout.stats.builds).toBe(builds);
  expect(f.renderer.stats.textBuilds - textBuilds).toBe(1);
  const afterTint = f.renderer.stats.textBuilds;
  expect(f.renderer.render({ decorationsKey: '3', marks: [{ ...marks[0]!, style: { ...marks[0]!.style, glow: { color: [1, 0, 0, 0.5], radius: 6 } } }] })).toBe(true);
  expect(f.renderer.stats.textBuilds).toBe(afterTint);
  expect(f.layout.stats.builds).toBe(builds);
  f.renderer.dispose();
});

it('draws line backgrounds before mark backgrounds and selection', () => {
  const f = fixture();
  expect(f.renderer.render({ decorationsKey: '1', lineFormats: [{ line: 0, style: { background: [1, 0, 0, 1] } }],
    marks: [{ from: 0, to: 2, style: { background: [0, 1, 0, 1] } }], selection: [{ from: 0, to: 1 }] })).toBe(true);
  const colors = f.gl.draws.filter(draw => draw.layer === 'background').map(draw => draw.color);
  expect(colors[1]).toEqual([1, 0, 0, 1]);
  expect(colors[2]).toEqual([0, 1, 0, 1]);
  expect(colors[3]).not.toEqual(colors[2]);
  f.renderer.dispose();
});

it('draws gutter markers after line numbers', () => {
  const f = fixture();
  expect(f.renderer.render({ decorationsKey: '1', lineFormats: [{ line: 0, style: { gutterMarker: { text: '!', color: [1, 0, 0, 1] } } }] })).toBe(true);
  const marker = f.gl.draws.findIndex(draw => draw.layer === 'overlayText' && draw.color[0] === 1 && draw.color[1] === 0);
  const number = f.gl.draws.findIndex(draw => draw.layer === 'text' && (draw.flags & 1) === 0);
  expect(number).toBeGreaterThanOrEqual(0);
  expect(marker).toBeGreaterThan(number);
  f.renderer.dispose();
});

it('does not rebuild text when a glow-only mark is inserted before a color mark', () => {
  const f = fixture();
  const colorMark = { from: 4, to: 7, style: { color: [1, 0, 0, 1] as [number, number, number, number] } };
  expect(f.renderer.render({ decorationsKey: '1', marks: [colorMark] })).toBe(true);
  const builds = f.renderer.stats.textBuilds;
  expect(f.renderer.render({ decorationsKey: '2', marks: [{ from: 0, to: 2, style: { glow: { color: [0, 1, 0, 1], radius: 2 } } }, colorMark] })).toBe(true);
  expect(f.renderer.stats.textBuilds).toBe(builds);
  f.renderer.dispose();
});

it('applies text opacity when rasterizing untinted color glyphs', () => {
  const gl = recordingGL(), raster = rasterizer(), alphas: number[] = [];
  const createCanvas = () => {
    const canvas = raster.createCanvas(), context = canvas.getContext('2d')!;
    const fill = context.fillText.bind(context);
    context.fillText = (text, x, y, maxWidth) => { if (text === '😀') alphas.push(context.globalAlpha); fill(text, x, y, maxWidth); };
    return canvas;
  };
  const layout = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, { font: '16px mono', lineHeight: 18, baseline: 14 });
  layout.setDocument('😀');
  const renderer = new CanvasRenderer(document.createElement('canvas'), layout, { gl: gl.gl, ledger: new ResourceLedger(), createCanvas });
  renderer.setViewport({ width: 320, height: 100, scrollLeft: 0, scrollTop: 0, gutter: 48 }, 1);
  expect(renderer.render({ decorationsKey: '1', marks: [{ from: 0, to: 2, style: { opacity: 0.5 } }] })).toBe(true);
  expect(alphas).toContain(0.5);
  renderer.dispose();
});

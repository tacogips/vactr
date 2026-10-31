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

describe('renderer heightmap geometry', () => {
  it('moves text and decorations and invalidates caches only for changed blocks', () => {
    const f = fixture('a\nb\nc');
    const input = { textRevision: 1, selection: [{ from: 2, to: 3 }], animatedMarks: [{ from: 2, to: 3, style: { background: [0.7, 0.58, 0.26, 0.5] as [number, number, number, number] } }], cursor: 2 };
    f.layout.heightmap.setBlocks([{ line: 0, above: 0, below: 40 }]);
    expect(f.renderer.render(input)).toBe(true);
    expect(f.gl.draws.some(d => d.layer === 'text' && d.absTop === 58)).toBe(true);
    expect(f.gl.draws.some(d => d.layer === 'text' && d.absTop === 76)).toBe(true);
    expect(f.gl.draws.some(d => d.layer === 'background' && d.rect[1] === 58 && Math.abs(d.color[0]! - 0.7) < 0.01)).toBe(true);
    expect(f.gl.draws.some(d => d.layer === 'background' && d.rect[1] === 58 && Math.abs(d.color[0]! - 0.218) < 0.01)).toBe(true);
    const uploads = () => f.gl.calls.filter(c => c.name === 'texSubImage2D' && c.args[8] instanceof Float32Array).length;
    const buffer = (layer: string) => f.gl.calls.find(c => c.name === 'drawArraysInstanced' && c.args[4] === layer)!.buffer;
    const background = buffer('background'), overlay = buffer('overlay'), text = buffer('text');
    const staticWrites = (target: object | null | undefined) => f.gl.calls.filter(c => c.name === 'bufferSubData' && c.buffer === target && c.args[1] === 0).length;
    const beforeBackground = staticWrites(background), beforeOverlay = staticWrites(overlay);
    const before = uploads(); const rev = f.layout.heightmap.revision;
    f.layout.heightmap.setBlocks([{ line: 0, above: 0, below: 20 }]);
    expect(f.layout.heightmap.revision).toBeGreaterThan(rev); expect(f.renderer.render(input)).toBe(true);
    expect(uploads()).toBeGreaterThan(before);
    expect(staticWrites(background)).toBeGreaterThan(beforeBackground);
    expect(staticWrites(overlay)).toBeGreaterThan(beforeOverlay);
    expect(f.gl.draws.some(d => d.layer === 'text' && d.absTop === 38)).toBe(true);
    const stable = uploads(), stableBackground = staticWrites(background), stableOverlay = staticWrites(overlay), stableText = staticWrites(text); f.layout.heightmap.setBlocks([{ line: 0, above: 0, below: 20 }]);
    expect(f.renderer.render(input)).toBe(true); expect(uploads()).toBe(stable);
    expect(staticWrites(background)).toBe(stableBackground); expect(staticWrites(overlay)).toBe(stableOverlay); expect(staticWrites(text)).toBe(stableText);
    f.renderer.dispose();
  });
  it('does not place glyphs in a large block band', () => {
    const f = fixture('a\nb\nc');
    f.layout.heightmap.setBlocks([{ line: 0, above: 0, below: 1000 }]);
    f.renderer.setViewport({ width: 320, height: 600, scrollLeft: 0, scrollTop: 500, gutter: 48 }, 1);
    expect(f.renderer.render()).toBe(true);
    const rows = f.gl.draws.filter(d => d.layer === 'text' && d.color[3]! > 0).map(d => d.absTop);
    expect(rows.some(y => y >= 0 && y < 518)).toBe(false); expect(rows).toContain(518); expect(rows).toContain(536);
    f.renderer.dispose();
  });
  it('matches the complete fixed-height draw list without blocks', () => {
    const actual = fixture('abc\ndef\nghi'), control = fixture('abc\ndef\nghi');
    control.layout.heightmap.lineTop = line => line * 18;
    control.layout.heightmap.lineAtHeight = y => ({ line: Math.max(0, Math.min(2, Math.floor(y / 18))), region: 'text' });
    const input = { textRevision: 1, selection: [{ from: 0, to: 3 }],
      animatedMarks: [{ from: 4, to: 5, style: { background: [0.7, 0.58, 0.26, 0.5] as [number, number, number, number] } },
        { from: 8, to: 9, style: { background: [0.4, 0.64, 0.72, 0.5] as [number, number, number, number] } }],
      tokens: [{ from: 0, to: 3, tag: 't' }],
      marks: [{ from: 4, to: 5, style: { underline: { color: [0.75, 0.2, 0.25, 1] as [number, number, number, number], thickness: 2 as const } } },
        { from: 4, to: 5, style: { label: { text: 'CC 1' } } }],
      composition: [{ from: 6, to: 9 }], cursor: 5, handles: [{ pos: 0 }, { pos: 9, end: true }] };
    expect(actual.renderer.render(input)).toBe(true); expect(control.renderer.render(input)).toBe(true);
    const projection = (f: ReturnType<typeof fixture>) => f.gl.draws.map(({ rect, color, layer }) => ({ rect, color, layer }));
    expect(projection(actual)).toEqual(projection(control));
    for (const y of [0, 18, 36]) expect(actual.gl.draws.some(d => d.layer === 'text' && d.color[3]! > 0 && d.absTop === y)).toBe(true);
    for (const y of [18, 36]) expect(actual.gl.draws.some(d => d.layer === 'background' && d.rect[1] === y && (Math.abs(d.color[0]! - 0.7) < 0.01 || Math.abs(d.color[0]! - 0.4) < 0.01))).toBe(true);
    actual.renderer.dispose(); control.renderer.dispose();
  });
});

import { describe, expect, it } from 'vitest';
import { GlyphAtlas } from '../../src/render/atlas';
import type { LayoutFont } from '../../src/layout/layout';
import { ResourceLedger } from '../../src/render/resources';
import { rasterizer, recordingGL } from '../support/recording-gl';

const font: LayoutFont = { font: '16px test-mono', fallback: 'test-ja', lineHeight: 20, baseline: 16 };

function createAtlas() {
  const recorder = recordingGL();
  const raster = rasterizer();
  const atlas = new GlyphAtlas(recorder.gl, new ResourceLedger(), raster.createCanvas);
  return { atlas, recorder, raster };
}

describe('glyph atlas cells', () => {
  it.each([1, 2])('keys cells by texel width at DPR %i', dpr => {
    const { atlas } = createAtlas();
    const wide = atlas.cell({ kind: 'mask', text: '1', width: 40, dpr, font });
    const narrow = atlas.cell({ kind: 'mask', text: '1', width: 8, dpr, font });

    expect(wide?.width).toBe(Math.ceil(40 * dpr) + 2 * Math.ceil(2 * dpr));
    expect(narrow?.width).toBe(Math.ceil(8 * dpr) + 2 * Math.ceil(2 * dpr));
    expect(narrow).not.toBe(wide);
    expect(atlas.stats.uploads).toBe(2);
    atlas.dispose();
  });

  it('shares requests with the same texel width and phase', () => {
    const { atlas } = createAtlas();
    const first = atlas.cell({ kind: 'mask', text: '1', width: 7.8, dpr: 1, font });
    const second = atlas.cell({ kind: 'mask', text: '1', width: 7.8000001, dpr: 1, font });

    expect(second).toBe(first);
    expect(atlas.stats.hits).toBe(1);
    expect(atlas.stats.uploads).toBe(1);
    atlas.dispose();
  });

  it('uses and validates the four raster phases', () => {
    const { atlas, raster } = createAtlas();
    const base = atlas.cell({ kind: 'mask', text: '1', width: 8, dpr: 2, font, phase: 0 });
    const shifted = atlas.cell({ kind: 'mask', text: '1', width: 8, dpr: 2, font, phase: 0.5 });

    expect(shifted).not.toBe(base);
    expect(raster.text[1].x - raster.text[0].x).toBe(0.5 / 2);
    expect(() => atlas.cell({ kind: 'mask', text: '1', width: 8, dpr: 1, font, phase: 0.3 })).toThrow(RangeError);
    atlas.dispose();
  });

  it('shifts run clipping and raster position by phase over DPR', () => {
    const { atlas, raster } = createAtlas();
    const request = {
      kind: 'run' as const,
      text: '0123456789',
      run: { text: '0123456789', from: 0, to: 10, x: 0, width: 80 },
      piece: { from: 2, to: 3 },
      chunkX: 2,
      width: 8,
      dpr: 2,
      font,
    };
    atlas.cell({ ...request, phase: 0 });
    atlas.cell({ ...request, phase: 0.25 });

    expect(raster.crops[1][0] - raster.crops[0][0]).toBe(0.25 / 2);
    expect(raster.text[1].x - raster.text[0].x).toBe(0.25 / 2);
    atlas.dispose();
  });

  it('configures nearest minification and magnification filters', () => {
    const { atlas, recorder } = createAtlas();
    const filters = recorder.calls
      .filter(call => call.name === 'texParameteri')
      .map(call => call.args.slice(1));

    expect(filters).toContainEqual([recorder.gl.TEXTURE_MIN_FILTER, recorder.gl.NEAREST]);
    expect(filters).toContainEqual([recorder.gl.TEXTURE_MAG_FILTER, recorder.gl.NEAREST]);
    atlas.dispose();
  });
});

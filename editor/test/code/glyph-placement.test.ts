import { describe, expect, it } from 'vitest';
import {
  CLIP_GUTTER, FRAGMENT_SOURCE, NO_HSCROLL, SNAP_GLYPH, TEXT_SPACE, UNTINTED,
  VERTEX_SOURCE, glyphPhase, glyphQuad,
} from '../../src/code/geometry';

describe('glyph placement helpers', () => {
  it('quantizes device positions to four phases and carries into the next pixel', () => {
    expect(glyphPhase(10.3, 1)).toEqual({ devicePixel: 10, phase: 0.25 });
    expect(glyphPhase(10.9, 1)).toEqual({ devicePixel: 11, phase: 0 });
    expect(glyphPhase(7.8, 2)).toEqual({ devicePixel: 15, phase: 0.5 });
    expect(glyphPhase(0, 1)).toEqual({ devicePixel: 0, phase: 0 });
    expect(glyphPhase(3.9999999, 1)).toEqual({ devicePixel: 4, phase: 0 });
  });

  it('keeps the quantized coordinate within half a phase step', () => {
    const result = glyphPhase(5.2, 1.5);
    expect(Number.isInteger(result.devicePixel)).toBe(true);
    expect([0, 0.25, 0.5, 0.75]).toContain(result.phase);
    expect(result.devicePixel + result.phase).toBeCloseTo(7.8, 1);
  });

  it('rejects invalid positions and scales', () => {
    expect(() => glyphPhase(1, 0)).toThrow(RangeError);
    expect(() => glyphPhase(1, Number.NaN)).toThrow(RangeError);
    expect(() => glyphPhase(Number.NaN, 1)).toThrow(RangeError);
    expect(() => glyphQuad(1, 0, { width: 8, height: 16, pad: 2 })).toThrow(RangeError);
  });

  it('maps cell texels to exact device-pixel dimensions', () => {
    const cell = { width: 21, height: 36, pad: 4 };
    expect(glyphQuad(15, 2, cell)).toEqual({ left: 5.5, width: 10.5, height: 18 });
    for (const scale of [1, 1.5, 2]) {
      const quad = glyphQuad(15, scale, cell);
      expect(quad.width * scale).toBeCloseTo(cell.width, 9);
      expect(quad.left * scale + cell.pad).toBeCloseTo(15, 9);
      expect(quad.height * scale).toBeCloseTo(cell.height, 9);
    }
  });

  it('reserves a distinct snap flag', () => {
    expect(SNAP_GLYPH).toBe(16);
    expect([CLIP_GUTTER, UNTINTED, TEXT_SPACE, NO_HSCROLL]).not.toContain(SNAP_GLYPH);
  });

  it('snaps flagged vertex origins and emits premultiplied fragment color', () => {
    expect(VERTEX_SOURCE).toMatch(/uniform vec2 u_device;/);
    expect(VERTEX_SOURCE).toMatch(/& 16u/);
    expect(VERTEX_SOURCE).toMatch(/floor\(origin \* u_device \+ 0\.5\) \/ u_device/);
    expect(FRAGMENT_SOURCE).toMatch(/out_color = vec4\(c\.rgb \* c\.a, c\.a\)/);
    expect(FRAGMENT_SOURCE).not.toContain('texel * v_color');
  });
});

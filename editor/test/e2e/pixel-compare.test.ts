import { describe, expect, it } from 'vitest';
// @ts-expect-error The helper is plain injectable JavaScript by design.
import { bandRows, cellInk, columnUniform, compareLine, modeColor, selectDiagnosticRect, verticalRuns } from './pixel-compare.mjs';

function image(width: number, height: number, color: [number, number, number] = [0, 0, 0]) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let i = 0; i < width * height; i++) data.set([...color, 255], i * 4);
  return { data, width, height };
}
function rect(img: ReturnType<typeof image>, left: number, top: number, right: number, bottom: number, color: [number, number, number]) {
  for (let y = top; y < bottom; y++) for (let x = left; x < right; x++) img.data.set([...color, 255], (y * img.width + x) * 4);
}

describe('pixel comparison helpers', () => {
  it('measures a 3 by 10 bar and returns null extents for backdrop only', () => {
    const img = image(12, 12); rect(img, 4, 1, 7, 11, [255, 255, 255]);
    expect(cellInk(img, { left: 0, top: 0, right: 12, bottom: 12 }, [0, 0, 0])).toMatchObject({ ink: 30, inkLeft: 4, inkRight: 6 });
    expect(cellInk(image(2, 2), { left: 0, top: 0, right: 2, bottom: 2 }, [0, 0, 0])).toEqual({ ink: 0, inkLeft: null, inkRight: null });
  });
  it('detects missing, squashed, heavy and whitespace ink', () => {
    const ref = { ink: 20, inkLeft: 1, inkRight: 6 }; const empty = { ink: 0, inkLeft: null, inkRight: null };
    expect(compareLine([empty], [ref], 'a').failures.join()).toContain('P1 char 0');
    expect(compareLine([{ ink: 20, inkLeft: 2, inkRight: 3 }], [ref], 'a').failures.join()).toContain('P3 char 0');
    expect(compareLine([{ ink: 32, inkLeft: 1, inkRight: 6 }], [ref], 'a').failures.join()).toContain('P4');
    expect(compareLine([{ ink: 20, inkLeft: 1, inkRight: 6 }], [ref], 'a').failures).toEqual([]);
    expect(compareLine([{ ink: 4 }, { ink: 20 }], [{ ink: 0 }, ref], ' a').failures.join()).toContain('P2 char 0');
  });
  it('does not apply line ink checks when no non-whitespace cell is comparable', () => {
    expect(compareLine([], [], []).failures).toEqual([]);
    expect(compareLine([], [], []).inkRatio).toBeNull();
    expect(compareLine([{ ink: 4 }], [{ ink: 0 }], ' ').failures).toEqual([]);
  });
  it('finds vertical runs, matching bands, uniform columns and majority color', () => {
    const img = image(5, 20); rect(img, 3, 1, 4, 19, [255, 255, 255]); rect(img, 0, 17, 3, 19, [220, 0, 0]);
    expect(verticalRuns(img, 3, [255, 255, 255], 0)).toEqual([[1, 19]]);
    expect(bandRows(img, { left: 0, top: 15, right: 3, bottom: 20 }, [220, 0, 0], 0)).toEqual([17, 18]);
    expect(columnUniform(img, 0, 0, 10, 2)).toBe(true);
    img.data.set([5, 0, 0, 255], 5 * img.width * 4);
    expect(columnUniform(img, 0, 0, 10, 2)).toBe(false);
    expect(modeColor(img, { left: 0, top: 0, right: 5, bottom: 20 })).toEqual([0, 0, 0]);
  });
  it('ignores disconnected neighbor ink for P3 while detecting a squashed glyph', () => {
    const canvas = image(12, 10); const reference = image(12, 10);
    rect(canvas, 3, 2, 4, 8, [255, 255, 255]); rect(canvas, 5, 2, 9, 8, [255, 255, 255]);
    rect(reference, 4, 2, 8, 8, [255, 255, 255]);
    const actual = cellInk(canvas, { left: 3, top: 0, right: 10, bottom: 10 }, [0, 0, 0]);
    const expected = cellInk(reference, { left: 3, top: 0, right: 10, bottom: 10 }, [0, 0, 0]);
    expect(compareLine([actual], [expected], 'b').failures).toEqual([]);

    const squashed = image(12, 10); rect(squashed, 5, 2, 7, 8, [255, 255, 255]);
    const narrow = cellInk(squashed, { left: 3, top: 0, right: 10, bottom: 10 }, [0, 0, 0]);
    expect(compareLine([narrow], [expected], '1').failures.join()).toContain('P3 char 0');
  });
  it('selects only visible diagnostic spans and clamps partial ends', () => {
    const row = [{ left: 48, right: 56 }, { left: 56, right: 64 }, null];
    expect(selectDiagnosticRect([row], 0, 2, 3, 2)).toEqual({ skipped: true });
    expect(selectDiagnosticRect([row], 0, 1, 3, 2)).toEqual({ skipped: false, rect: { left: 56, right: 64, top: 0, bottom: 36 } });
    expect(selectDiagnosticRect([row], 0, 0, 2, 2)).toEqual({ skipped: false, rect: { left: 48, right: 64, top: 0, bottom: 36 } });
  });
});

import { describe, expect, it } from 'vitest';
import { EVAL_ERROR_MARK, EVAL_MARK, PLAYING_MARK, toRenderInput } from '../../src/code/decorations';
import { FALLBACK_PALETTE } from '../../src/code/theme';
import { DEFAULT_UNAPPLIED_COLOR, unappliedRgba } from '../../src/code/unapplied-settings';

describe('vactr decoration adapter', () => {
  it('maps all annotation kinds to their renderer representations', () => {
    const result = toRenderInput({ annotations: [
      { kind: 'syntax', from: 0, to: 1, className: 'name' },
      { kind: 'selection', from: 1, to: 2 },
      { kind: 'composition', from: 2, to: 3 },
      { kind: 'unapplied', from: 3, to: 3 },
      { kind: 'diagnostic', from: 4, to: 5 },
      { kind: 'call-head', from: 5, to: 6 },
      { kind: 'binding', from: 6, to: 7, label: 'binding' },
      { kind: 'playing', from: 7, to: 8 },
      { kind: 'eval', from: 8, to: 9 },
      { kind: 'eval', from: 9, to: 10, className: 'eval-error' },
      { kind: 'momentary', from: 10, to: 11, label: 'drag' },
    ] }, FALLBACK_PALETTE);
    expect(result.tokens).toEqual([{ from: 0, to: 1, tag: 'name' }]);
    expect(result.selection).toEqual([{ from: 1, to: 2 }]);
    expect(result.composition).toEqual([{ from: 2, to: 3 }]);
    expect(result.marks).toEqual([
      { from: 3, to: 3, style: { background: unappliedRgba(DEFAULT_UNAPPLIED_COLOR), minWidth: 6 } },
      { from: 4, to: 5, style: { underline: { color: FALLBACK_PALETTE.diagnostic, thickness: 2 } } },
      { from: 5, to: 6, style: { underline: { color: FALLBACK_PALETTE.callHead, thickness: 1 } } },
      { from: 6, to: 7, style: { label: { text: 'binding' } } },
    ]);
    expect(result.animatedMarks).toEqual([
      { from: 7, to: 8, style: PLAYING_MARK },
      { from: 8, to: 9, style: EVAL_MARK },
      { from: 9, to: 10, style: EVAL_ERROR_MARK },
      { from: 10, to: 11, style: { label: { text: 'drag' } } },
    ]);
  });

  it('preserves animated ordering, appends plugin entries and forwards feedback', () => {
    const mark = { from: 4, to: 5, style: { opacity: 0.5 } };
    const animated = { from: 5, to: 6, style: PLAYING_MARK };
    const lineFormats = [{ line: 1, style: { opacity: 0.5 } }];
    const result = toRenderInput({
      annotations: [{ kind: 'playing', from: 0, to: 1 }, { kind: 'binding', from: 1, to: 2, label: 'x' }],
      animated: [{ kind: 'playing', from: 2, to: 3 }],
      plugin: { marks: [mark], animatedMarks: [animated], lineFormats, key: '|plugin' },
      annotationsRevision: 3, unappliedColor: '#112233', cursor: 2, cursorShape: 'block', cursorEnd: 3,
      cursorVisible: true, handles: [{ pos: 2 }], hopLabels: [{ pos: 1, text: 'a' }], hopRevision: 4, textRevision: 5,
    }, FALLBACK_PALETTE);
    expect(result.marks).toEqual([{ from: 1, to: 2, style: { label: { text: 'x' } } }, mark]);
    expect(result.animatedMarks).toEqual([{ from: 0, to: 1, style: PLAYING_MARK }, { from: 2, to: 3, style: PLAYING_MARK }, animated]);
    expect(result.lineFormats).toEqual(lineFormats);
    expect(result).toMatchObject({ decorationsKey: '3|#112233|plugin', cursor: 2, cursorShape: 'block', cursorEnd: 3,
      cursorVisible: true, handles: [{ pos: 2 }], hopLabels: [{ pos: 1, text: 'a' }], hopRevision: 4, textRevision: 5 });
  });
});

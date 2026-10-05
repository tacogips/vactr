import { Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { HEADS, tokenize, tokenizerSpans, type LineToken } from '../../src/code/language';

const types = (line: string): LineToken[] => tokenize([line])[0] ?? [];
const typeOf = (line: string, text: string): string | null | undefined => types(line).find((t) => t.text === text)?.type;

describe('.vact tokenizer', () => {
  it('distinguishes #@ directive lines from # comments', () => {
    expect(types('# a comment')).toEqual([{ text: '# a comment', type: 'comment' }]);
    expect(types('#@ name lead cc 21')).toEqual([{ text: '#@ name lead cc 21', type: 'directive' }]);
    // A trailing comment after code.
    expect(typeOf('d1 "bd sd" # note', '# note')).toBe('comment');
    expect(typeOf('lpf 800 #@ cc 74', '#@ cc 74')).toBe('directive');
  });

  it('classifies keywords, numbers, strings and path/url literals', () => {
    expect(typeOf('synth :saw', ':saw')).toBe('keyword');
    expect(typeOf('lpf 800', '800')).toBe('number');
    expect(typeOf('gain 0.75', '0.75')).toBe('number');
    expect(typeOf('fast 3/4', '3/4')).toBe('number');
    expect(typeOf('amp -6', '-6')).toBe('number');
    expect(typeOf('d1 "bd*2 sd"', '"bd*2 sd"')).toBe('string');
    expect(typeOf('d1 "a \\"q\\" b"', '"a \\"q\\" b"')).toBe('string');
    expect(typeOf('let kick sample ./soundpack/bd/1.wav', './soundpack/bd/1.wav')).toBe('path');
    expect(typeOf('let up ../x/y', '../x/y')).toBe('path');
    expect(typeOf('let home ~/s/a.wav', '~/s/a.wav')).toBe('path');
    expect(typeOf('let remote https://example.org/packs/x.vact', 'https://example.org/packs/x.vact')).toBe('path');
    // `/ a b` with a space is division, not a path.
    expect(types('/ a b').some((t) => t.type === 'path')).toBe(false);
    // A url ends at a comment.
    expect(typeOf('import https://e.org/p#frag', 'https://e.org/p')).toBe('path');
  });

  it('marks each definition head only at the start of its line', () => {
    for (const h of HEADS) expect(typeOf(`${h} x 1`, h)).toBe('head');
    expect(HEADS).toEqual(['let', 'var', 'upd', 'fn', 'inst', 'bus', 'look', 'master', 'import', 'slot', 'if']);
    // Indented heads still count; a head word mid-line is a plain name.
    expect(typeOf('\tif {> a 10}', 'if')).toBe('head');
    expect(typeOf('print let', 'let')).toBe('name');
  });

  it('continues an unterminated string on the next line and resets at a blank line', () => {
    const [a, b, , d] = tokenize(['d1 "bd', 'sd" 1', '', 'x']);
    expect(a?.at(-1)).toEqual({ text: '"bd', type: 'string' });
    expect(b?.[0]).toEqual({ text: 'sd"', type: 'string' });
    expect(d?.[0]?.type).toBe('name');
  });

  it('produces equivalent bounded document spans for the canvas renderer', () => {
    const text = '#@ cc 1\nlet a 1';
    expect(tokenizerSpans(Text.of(text.split('\n')))).toEqual([
      expect.objectContaining({ from: 0, to: 7, type: 'directive', className: 'vact-tok-directive' }),
      expect.objectContaining({ from: 8, to: 11, type: 'head', className: 'vact-tok-head' }),
      expect.objectContaining({ from: 12, to: 13, type: 'name', className: 'vact-tok-name' }),
      expect.objectContaining({ from: 14, to: 15, type: 'number', className: 'vact-tok-number' }),
    ]);
  });
});

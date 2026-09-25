import { describe, expect, it } from 'vitest';
import { Utf8Index, utf8Length } from '../../src/protocol/utf8';

const enc = new TextEncoder();
const bytesOf = (s: string): number => enc.encode(s).length;

describe('Utf8Index', () => {
  const text = 'aéあ\u{1f3b5}z\n(s "bd")'; // ASCII, 2-, 3- and 4-byte characters

  it('round-trips every UTF-16 boundary', () => {
    const idx = new Utf8Index(text);
    expect(idx.byteLength).toBe(bytesOf(text));
    expect(utf8Length(text)).toBe(bytesOf(text));
    for (let i = 0; i <= text.length; i += 1) {
      const code = text.charCodeAt(i - 1);
      const midPair = i > 0 && code >= 0xd800 && code <= 0xdbff;
      if (midPair) continue;
      const b = idx.toByte(i);
      expect(b).toBe(bytesOf(text.slice(0, i)));
      expect(idx.toUtf16(b)).toBe(i);
    }
  });

  it('maps known offsets of each width', () => {
    const idx = new Utf8Index(text);
    expect(idx.toByte(1)).toBe(1); // after 'a'
    expect(idx.toByte(2)).toBe(3); // after e-acute (2 bytes)
    expect(idx.toByte(3)).toBe(6); // after hiragana a (3 bytes)
    expect(idx.toByte(5)).toBe(10); // after the note (4 bytes, 2 units)
    expect(idx.toUtf16(10)).toBe(5);
  });

  it('maps a byte inside a character to its start', () => {
    const idx = new Utf8Index(text);
    expect(idx.toUtf16(2)).toBe(1);
    expect(idx.toUtf16(4)).toBe(2);
    expect(idx.toUtf16(5)).toBe(2);
    expect(idx.toUtf16(8)).toBe(3);
    expect(idx.toByte(4)).toBe(6); // inside the surrogate pair: the pair's start
  });

  it('clamps out-of-range offsets', () => {
    const idx = new Utf8Index(text);
    expect(idx.toByte(-3)).toBe(0);
    expect(idx.toByte(999)).toBe(idx.byteLength);
    expect(idx.toUtf16(999)).toBe(text.length);
  });

  it('converts spans in both directions', () => {
    const idx = new Utf8Index(text);
    expect(idx.spanToBytes(2, 5)).toEqual({ start: 3, end: 10 });
    expect(idx.spanToUtf16({ start: 3, end: 10 })).toEqual({ from: 2, to: 5 });
  });

  it('converts a UTF-16 change list over non-ASCII text', () => {
    const base = 'ありがとう\u{1f3b5} (n 0)';
    const idx = new Utf8Index(base);
    const { changes, dirty } = idx.changes([
      { from: 2, to: 3, insert: 'X' }, // replace a 3-byte char with 1 byte
      { from: 5, to: 7, insert: 'éé' }, // replace the 4-byte note with 4 bytes
      { from: 9, to: 10, insert: '' }, // delete 'n'
    ]);
    expect(changes).toEqual([
      { from: 6, to: 9, insert_len: 1 },
      { from: 15, to: 19, insert_len: 4 },
      { from: 21, to: 22, insert_len: 0 },
    ]);
    expect(dirty).toEqual([
      { start: 6, end: 7 },
      { start: 13, end: 17 },
      { start: 19, end: 19 },
    ]);
    const next = 'ありXとうéé ( 0)';
    expect(bytesOf(next)).toBe(bytesOf(base) - 2 + 0 - 1);
  });
});

// UTF-16 <-> UTF-8 offset conversion (design 15.1.4 "Offsets").
//
// The protocol speaks UTF-8 byte offsets and CodeMirror speaks UTF-16 code
// units. `Utf8Index` holds the byte offset of every UTF-16 boundary of one
// text revision: UTF-16 -> bytes is O(1), bytes -> UTF-16 a binary search.
// Byte lengths match `TextEncoder` (a lone surrogate encodes as U+FFFD, 3
// bytes). The offset between the two halves of a surrogate pair maps to
// the pair's start.

import type { ByteChange, Span } from './types';

/** A change in UTF-16 units against the base text. */
export interface Utf16Change {
  from: number;
  to: number;
  insert: string;
}

/** UTF-8 length of `s` without allocating. */
export function utf8Length(s: string): number {
  let n = 0;
  for (let i = 0; i < s.length; i += 1) {
    const c = s.charCodeAt(i);
    if (c < 0x80) n += 1;
    else if (c < 0x800) n += 2;
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length && isLow(s.charCodeAt(i + 1))) {
      n += 4;
      i += 1;
    } else n += 3;
  }
  return n;
}

function isLow(c: number): boolean {
  return c >= 0xdc00 && c <= 0xdfff;
}

export class Utf8Index {
  static builds = 0;
  readonly text: string;
  /** `bytes[i]`: the byte offset of UTF-16 offset `i` (length + 1 entries). */
  private readonly bytes: Uint32Array;

  constructor(text: string) {
    Utf8Index.builds += 1;
    this.text = text;
    const n = text.length;
    const bytes = new Uint32Array(n + 1);
    let b = 0;
    for (let i = 0; i < n; i += 1) {
      bytes[i] = b;
      const c = text.charCodeAt(i);
      if (c < 0x80) b += 1;
      else if (c < 0x800) b += 2;
      else if (c >= 0xd800 && c <= 0xdbff && i + 1 < n && isLow(text.charCodeAt(i + 1))) {
        bytes[i + 1] = b;
        b += 4;
        i += 1;
      } else b += 3;
    }
    bytes[n] = b;
    this.bytes = bytes;
  }

  /** The UTF-8 length of the text. */
  get byteLength(): number {
    return this.bytes[this.text.length] ?? 0;
  }

  /** UTF-16 offset -> byte offset (clamped to the text). */
  toByte(utf16: number): number {
    const i = Math.max(0, Math.min(this.text.length, Math.floor(utf16)));
    return this.bytes[i] ?? 0;
  }

  /**
   * Byte offset -> UTF-16 offset (clamped). A byte offset inside a
   * multi-byte character maps to that character's start.
   */
  toUtf16(byte: number): number {
    const b = Math.max(0, Math.min(this.byteLength, Math.floor(byte)));
    let lo = 0;
    let hi = this.text.length;
    // Largest i with bytes[i] <= b.
    while (lo < hi) {
      const mid = (lo + hi + 1) >>> 1;
      if ((this.bytes[mid] ?? 0) <= b) lo = mid;
      else hi = mid - 1;
    }
    // The first of equal entries (a surrogate pair's inner offset).
    while (lo > 0 && this.bytes[lo - 1] === this.bytes[lo]) lo -= 1;
    return lo;
  }

  /** A byte span -> a UTF-16 `{from, to}`. */
  spanToUtf16(span: Span): { from: number; to: number } {
    return { from: this.toUtf16(span.start), to: this.toUtf16(span.end) };
  }

  /** A UTF-16 range -> a byte span. */
  spanToBytes(from: number, to: number): Span {
    return { start: this.toByte(from), end: this.toByte(to) };
  }

  /**
   * Converts UTF-16 changes (sorted, non-overlapping, base-text offsets) to
   * byte changes against this base text, plus the new-revision dirty spans
   * (the inserted ranges; a pure deletion gives a zero-length span).
   */
  changes(list: readonly Utf16Change[]): { changes: ByteChange[]; dirty: Span[] } {
    const changes: ByteChange[] = [];
    const dirty: Span[] = [];
    let delta = 0;
    for (const c of list) {
      const from = this.toByte(c.from);
      const to = this.toByte(c.to);
      const insertLen = utf8Length(c.insert);
      changes.push({ from, to, insert_len: insertLen });
      const start = from + delta;
      dirty.push({ start, end: start + insertLen });
      delta += insertLen - (to - from);
    }
    return { changes, dirty };
  }
}

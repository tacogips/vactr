import type { Text, ChangeSet } from '@codemirror/state';
import { utf8Length } from '../protocol/utf8';
import type { Span } from '../protocol/types';

const BYTE_SCAN_CHUNK = 16 * 1024;

export function utf8LengthRange(doc: Text, from: number, to: number): number {
  let bytes = 0;
  let pos = from;
  let limit = to;
  if (limit > from && limit < doc.length) {
    const last = doc.sliceString(limit - 1, limit).charCodeAt(0);
    const next = doc.sliceString(limit, limit + 1).charCodeAt(0);
    if (last >= 0xd800 && last <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) limit -= 1;
  }
  while (pos < limit) {
    let end = Math.min(limit, pos + BYTE_SCAN_CHUNK);
    if (end < limit) {
      const last = doc.sliceString(end - 1, end).charCodeAt(0);
      const next = doc.sliceString(end, end + 1).charCodeAt(0);
      if (last >= 0xd800 && last <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) end -= 1;
    }
    bytes += utf8Length(doc.sliceString(pos, end));
    pos = end;
  }
  return bytes;
}

/** UTF-8 line lengths for the edit path; only changed line text is rescanned. */
export class LineBytes {
  private readonly lengths: number[];
  private prefix: number[];
  constructor(private current: Text) {
    this.lengths = Array.from({ length: current.lines }, (_, index) => this.lineLength(current, index + 1));
    this.prefix = [];
    this.rebuildPrefix(0);
  }

  private lineLength(doc: Text, number: number): number {
    const line = doc.line(number);
    return utf8LengthRange(doc, line.from, line.to);
  }

  matches(doc: Text): boolean { return doc === this.current; }

  private rebuildPrefix(from: number): void {
    if (from === 0) this.prefix = [0];
    else this.prefix.length = from + 1;
    for (let i = from; i < this.lengths.length; i += 1) {
      this.prefix[i + 1] = (this.prefix[i] ?? 0) + (this.lengths[i] ?? 0) + 1;
    }
    if (this.lengths.length > 0) this.prefix[this.lengths.length] = (this.prefix[this.lengths.length] ?? 0) - 1;
  }

  starts(): Uint32Array { return Uint32Array.from(this.prefix); }

  toByte(doc: Text, pos: number): number {
    if (doc !== this.current) throw new Error('Line byte index does not match document');
    const bounded = Math.max(0, Math.min(doc.length, Math.floor(pos)));
    const line = doc.lineAt(bounded);
    const index = line.number - 1;
    return (this.prefix[index] ?? 0) + utf8LengthRange(doc, line.from, bounded);
  }

  insertLength(inserted: Text): number {
    let total = Math.max(0, inserted.lines - 1);
    for (let number = 1; number <= inserted.lines; number += 1) total += this.lineLength(inserted, number);
    return total;
  }

  update(changes: ChangeSet, base: Text, next: Text): void {
    const edits: { fromA: number; toA: number; fromB: number; toB: number }[] = [];
    changes.iterChanges((fromA, toA, fromB, toB) => edits.push({ fromA, toA, fromB, toB }));
    for (const edit of edits.reverse()) {
      const startA = base.lineAt(edit.fromA).number - 1;
      const endA = base.lineAt(edit.toA).number;
      const startB = next.lineAt(edit.fromB).number - 1;
      const endB = next.lineAt(edit.toB).number;
      const replacement = Array.from({ length: endB - startB }, (_, offset) => this.lineLength(next, startB + offset + 1));
      this.lengths.splice(startA, endA - startA, ...replacement);
      this.rebuildPrefix(startA);
    }
    this.current = next;
  }
}

/** Line starts and line-local scans replace a whole-document UTF-8 index. */
export class LineTable {
  static builds = 0;
  readonly byteLength: number;
  readonly bytes: number;

  constructor(private readonly text: Text, private readonly starts: Uint32Array) {
    this.byteLength = starts[starts.length - 1] ?? 0;
    this.bytes = 128 + starts.byteLength;
  }

  static build(text: Text): LineTable {
    this.builds += 1;
    const starts = new Uint32Array(text.lines + 1);
    let byte = 0;
    let line = 0;
    for (const value of text.iterLines()) {
      starts[line++] = byte;
      byte += utf8Length(value) + 1;
    }
    if (line > 0) byte -= 1;
    starts[text.lines] = byte;
    return new LineTable(text, starts);
  }

  toByte(pos: number): number {
    const bounded = Math.max(0, Math.min(this.text.length, Math.floor(pos)));
    const line = this.text.lineAt(bounded);
    return (this.starts[line.number - 1] ?? 0) + utf8LengthRange(this.text, line.from, bounded);
  }

  toUtf16(byte: number): number {
    const target = Math.max(0, Math.min(this.byteLength, Math.floor(byte)));
    let lo = 0;
    let hi = this.text.lines - 1;
    while (lo < hi) {
      const mid = Math.ceil((lo + hi) / 2);
      if ((this.starts[mid] ?? 0) <= target) lo = mid;
      else hi = mid - 1;
    }
    const line = this.text.line(lo + 1);
    const startByte = this.starts[lo] ?? 0;
    const localTarget = target - startByte;
    let offset = 0;
    let consumed = 0;
    while (offset < line.length) {
      let end = Math.min(line.length, offset + BYTE_SCAN_CHUNK);
      if (end < line.length) {
        const before = this.text.sliceString(line.from + end - 1, line.from + end).charCodeAt(0);
        const after = this.text.sliceString(line.from + end, line.from + end + 1).charCodeAt(0);
        if (before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff) end -= 1;
      }
      const chunk = this.text.sliceString(line.from + offset, line.from + end);
      for (let i = 0; i < chunk.length;) {
        const code = chunk.charCodeAt(i);
        const width = code >= 0xd800 && code <= 0xdbff && i + 1 < chunk.length && chunk.charCodeAt(i + 1) >= 0xdc00 && chunk.charCodeAt(i + 1) <= 0xdfff ? 2 : 1;
        const size = utf8Length(chunk.slice(i, i + width));
        if (localTarget < consumed + size) return line.from + offset + i;
        consumed += size;
        i += width;
      }
      offset = end;
    }
    return line.to;
  }

  spanToUtf16(span: Span): { from: number; to: number } {
    return { from: this.toUtf16(span.start), to: this.toUtf16(span.end) };
  }

  spanToBytes(from: number, to: number): Span {
    return { start: this.toByte(from), end: this.toByte(to) };
  }
}

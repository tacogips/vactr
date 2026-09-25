import { describe, expect, it } from 'vitest';
import { buildZip, crc32 } from './zip';

const dec = new TextDecoder();

interface Parsed {
  name: string;
  crc: number;
  data: Uint8Array;
}

// A small independent reader: end record -> central directory -> local headers.
function readZip(zip: Uint8Array): Parsed[] {
  const v = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
  const eocd = zip.length - 22;
  expect(v.getUint32(eocd, true)).toBe(0x06054b50);
  const count = v.getUint16(eocd + 10, true);
  let at = v.getUint32(eocd + 16, true);
  const out: Parsed[] = [];
  for (let i = 0; i < count; i += 1) {
    expect(v.getUint32(at, true)).toBe(0x02014b50);
    expect(v.getUint16(at + 10, true)).toBe(0); // STORED
    const crc = v.getUint32(at + 16, true);
    const size = v.getUint32(at + 20, true);
    const nameLen = v.getUint16(at + 28, true);
    const local = v.getUint32(at + 42, true);
    const name = dec.decode(zip.subarray(at + 46, at + 46 + nameLen));
    expect(v.getUint32(local, true)).toBe(0x04034b50);
    expect(v.getUint32(local + 14, true)).toBe(crc);
    const localName = v.getUint16(local + 26, true);
    const extra = v.getUint16(local + 28, true);
    const start = local + 30 + localName + extra;
    out.push({ name, crc, data: zip.slice(start, start + size) });
    at += 46 + nameLen;
  }
  return out;
}

describe('zip writer', () => {
  it('computes the standard CRC-32 check value', () => {
    expect(crc32(new TextEncoder().encode('123456789'))).toBe(0xcbf43926);
    expect(crc32(new Uint8Array())).toBe(0);
  });

  it('writes entries that parse back with valid CRCs', () => {
    const bin = new Uint8Array([0, 1, 2, 255, 128]);
    const zip = buildZip([
      { name: 'pkg/vactrol.toml', data: '[package]\npath = "a/b"\n' },
      { name: 'pkg/src/lib.vact', data: 'let x = 1 # é\n' },
      { name: 'pkg/bin', data: bin },
    ]);
    const entries = readZip(zip);
    expect(entries.map((e) => e.name)).toEqual(['pkg/vactrol.toml', 'pkg/src/lib.vact', 'pkg/bin']);
    for (const e of entries) expect(crc32(e.data)).toBe(e.crc);
    expect(dec.decode(entries[1]?.data)).toBe('let x = 1 # é\n');
    expect([...(entries[2]?.data ?? [])]).toEqual([...bin]);
  });

  it('is deterministic', () => {
    const a = buildZip([{ name: 'x', data: 'y' }]);
    const b = buildZip([{ name: 'x', data: 'y' }]);
    expect([...a]).toEqual([...b]);
  });
});

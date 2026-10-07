import fs from 'node:fs';
import { createHash } from 'node:crypto';

const MAGIC = [0x00, 0x61, 0x73, 0x6d];

function readLeb(bytes, state, end) {
  let value = 0;
  for (let i = 0; i < 5; i++) {
    if (state.offset >= end) throw new Error('truncated wasm LEB128');
    const byte = bytes[state.offset++];
    if (i === 4 && (byte & 0xf0) !== 0) throw new Error('invalid wasm LEB128');
    value |= (byte & 0x7f) << (i * 7);
    if ((byte & 0x80) === 0) return value >>> 0;
  }
  throw new Error('invalid wasm LEB128');
}

export function customSectionNames(bytes) {
  if (bytes.length < 8 || !MAGIC.every((byte, index) => bytes[index] === byte)) throw new Error('bad wasm magic');
  const names = [], state = { offset: 8 };
  while (state.offset < bytes.length) {
    const id = bytes[state.offset++], size = readLeb(bytes, state, bytes.length), end = state.offset + size;
    if (end > bytes.length) throw new Error('truncated wasm section');
    if (id === 0) {
      const nameLength = readLeb(bytes, state, end), nameEnd = state.offset + nameLength;
      if (nameEnd > end) throw new Error('truncated wasm custom section name');
      names.push(new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(state.offset, nameEnd)));
    }
    state.offset = end;
  }
  return names;
}

export function classifyWasm({ sha256, dwarf }, { releaseSha256, debugSha256 }) {
  if (releaseSha256 !== null && sha256 === releaseSha256) return 'release';
  if ((debugSha256 !== null && sha256 === debugSha256) || dwarf) return 'debug';
  return 'unknown';
}

export function gatingRefusal({ profile, nameSection, dwarf }) {
  if (profile === 'release' && nameSection && !dwarf) return null;
  return `gating evidence requires the release wasm with name section and no DWARF; run mise run build-wasm-release`;
}

async function digest(file) {
  try { return createHash('sha256').update(await fs.promises.readFile(file)).digest('hex'); }
  catch (error) { if (error?.code === 'ENOENT') return null; throw error; }
}

export async function inspectWasm(file, { releasePath, debugPath }) {
  const bytes = await fs.promises.readFile(file), sha256 = createHash('sha256').update(bytes).digest('hex');
  const sections = customSectionNames(bytes), releaseSha256 = await digest(releasePath), debugSha256 = await digest(debugPath);
  const dwarf = sections.some((name) => name.startsWith('.debug_'));
  return { path: file, bytes: bytes.byteLength, sha256, nameSection: sections.includes('name'), dwarf,
    profile: classifyWasm({ sha256, dwarf }, { releaseSha256, debugSha256 }), releasePath, releaseSha256, debugSha256 };
}

export async function gatingPreflight({ distWasm, releasePath, debugPath, writeEvidence }) {
  const wasm = { ...await inspectWasm(distWasm, { releasePath, debugPath }), gating: writeEvidence };
  return { wasm, refusal: writeEvidence ? gatingRefusal(wasm) : null };
}

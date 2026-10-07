// @vitest-environment node

import { afterEach, describe, expect, it } from 'vitest';

interface FsModule {
  promises: { mkdtemp(path: string): Promise<string>; writeFile(path: string, data: Uint8Array): Promise<void>; rm(path: string, options: { recursive: boolean; force: boolean }): Promise<void> };
}
interface OsModule { tmpdir(): string }
interface PathModule { join(...parts: string[]): string }
interface WasmProfile {
  customSectionNames(bytes: Uint8Array): string[];
  classifyWasm(input: { sha256: string; dwarf: boolean }, refs: { releaseSha256: string | null; debugSha256: string | null }): string;
  gatingRefusal(input: { profile: string; nameSection: boolean; dwarf: boolean }): string | null;
  inspectWasm(file: string, refs: { releasePath: string; debugPath: string }): Promise<{ bytes: number; sha256: string; nameSection: boolean; dwarf: boolean; profile: string; releaseSha256: string | null }>;
  gatingPreflight(input: { distWasm: string; releasePath: string; debugPath: string; writeEvidence: boolean }): Promise<{ wasm: { gating: boolean; profile: string; nameSection: boolean }; refusal: string | null }>;
}
const fsSpec = 'node:fs', osSpec = 'node:os', pathSpec = 'node:path', wasmSpec = '../../test/e2e/wasm-profile.mjs';
const fs = await import(/* @vite-ignore */ fsSpec) as FsModule;
const os = await import(/* @vite-ignore */ osSpec) as OsModule;
const path = await import(/* @vite-ignore */ pathSpec) as PathModule;
const wasm = await import(/* @vite-ignore */ wasmSpec) as WasmProfile;
const dirs: string[] = [];
afterEach(async () => { for (const dir of dirs.splice(0)) await fs.promises.rm(dir, { recursive: true, force: true }); });

function leb(value: number): number[] {
  const bytes: number[] = [];
  do { let byte = value & 0x7f; value >>>= 7; if (value) byte |= 0x80; bytes.push(byte); } while (value);
  return bytes;
}
function moduleBytes(names: string[], typeSection = true): Uint8Array {
  const bytes = [0, 97, 115, 109, 1, 0, 0, 0];
  for (const name of names) {
    const encoded = [...new TextEncoder().encode(name)], payload = [...leb(encoded.length), ...encoded];
    bytes.push(0, ...leb(payload.length), ...payload);
  }
  if (typeSection) bytes.push(1, 1, 0);
  return new Uint8Array(bytes);
}

describe('gating wasm profile', () => {
  it('reads custom section names and rejects bad or truncated modules', () => {
    expect(wasm.customSectionNames(moduleBytes(['name', '.debug_info']))).toEqual(['name', '.debug_info']);
    expect(() => wasm.customSectionNames(new Uint8Array([1, 2, 3]))).toThrow('bad wasm magic');
    expect(() => wasm.customSectionNames(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 0, 5, 1]))).toThrow('truncated wasm section');
  });

  it('classifies by artifact identity and DWARF presence', () => {
    const refs = { releaseSha256: 'release', debugSha256: 'debug' };
    expect(wasm.classifyWasm({ sha256: 'release', dwarf: false }, refs)).toBe('release');
    expect(wasm.classifyWasm({ sha256: 'debug', dwarf: false }, refs)).toBe('debug');
    expect(wasm.classifyWasm({ sha256: 'other', dwarf: true }, refs)).toBe('debug');
    expect(wasm.classifyWasm({ sha256: 'other', dwarf: false }, refs)).toBe('unknown');
    expect(wasm.classifyWasm({ sha256: 'release', dwarf: false }, { releaseSha256: null, debugSha256: null })).toBe('unknown');
  });

  it('requires release identity, name section and no DWARF for gating', () => {
    expect(wasm.gatingRefusal({ profile: 'release', nameSection: true, dwarf: false })).toBeNull();
    expect(wasm.gatingRefusal({ profile: 'release', nameSection: false, dwarf: false })).toContain('build-wasm-release');
    expect(wasm.gatingRefusal({ profile: 'release', nameSection: true, dwarf: true })).not.toBeNull();
    expect(wasm.gatingRefusal({ profile: 'debug', nameSection: true, dwarf: false })).not.toBeNull();
    expect(wasm.gatingRefusal({ profile: 'unknown', nameSection: true, dwarf: false })).not.toBeNull();
  });

  it('inspects artifacts and covers release, debug, nameless, and non-gating preflight branches', async () => {
    const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'vactr-wasm-profile-')); dirs.push(dir);
    const releasePath = path.join(dir, 'release.wasm'), debugPath = path.join(dir, 'debug.wasm'), distWasm = path.join(dir, 'dist.wasm');
    const release = moduleBytes(['name']), debug = moduleBytes(['name', '.debug_info']), nameless = moduleBytes([]);
    await fs.promises.writeFile(releasePath, release); await fs.promises.writeFile(debugPath, debug);
    await fs.promises.writeFile(distWasm, release);
    const inspected = await wasm.inspectWasm(distWasm, { releasePath, debugPath });
    expect(inspected).toMatchObject({ bytes: release.byteLength, profile: 'release', nameSection: true, dwarf: false, sha256: inspected.releaseSha256 });
    expect(inspected.releaseSha256).not.toBeNull();
    expect((await wasm.gatingPreflight({ distWasm, releasePath, debugPath, writeEvidence: true })).refusal).toBeNull();
    await fs.promises.writeFile(distWasm, debug);
    const debugResult = await wasm.gatingPreflight({ distWasm, releasePath, debugPath, writeEvidence: true });
    expect(debugResult.wasm.profile).toBe('debug'); expect(debugResult.refusal).toContain('build-wasm-release');
    await fs.promises.writeFile(distWasm, nameless);
    expect((await wasm.gatingPreflight({ distWasm, releasePath, debugPath, writeEvidence: true })).refusal).not.toBeNull();
    await fs.promises.writeFile(distWasm, debug);
    expect(await wasm.gatingPreflight({ distWasm, releasePath, debugPath, writeEvidence: false })).toMatchObject({ wasm: { gating: false, profile: 'debug' }, refusal: null });
  });
});

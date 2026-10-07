// @vitest-environment node
import { describe, expect, it } from 'vitest';
interface Wasm { call(name: string, ...args: number[]): number; callStr(name: string, text: string): number; drainRecords(): Array<{tag:number;bytes:Uint8Array}>; }
interface SessionCheckRecord { kind: string; diagnostics: Array<{ severity: string; code: string }> }
interface Helpers { loadVactrWasm(): Promise<Wasm>; }
interface FixtureModule { createLargeDocument(): { text:string; controlText:string; lines:number; bytes:number; longLines:number; voices:number; visualOutputs:string[] }; }
const fixtureSpec: string = './fixtures/large-doc.mjs';
const helperSpec: string = '../support/wasm.ts';
const fixture = (await import(/* @vite-ignore */ fixtureSpec)) as FixtureModule;
const helpers = (await import(/* @vite-ignore */ helperSpec)) as Helpers;
const encoder = new TextEncoder();

describe('large browser evidence workload', () => {
  it('is deterministic and 20,000 lines near one MiB with multilingual and long-line coverage', () => {
    const a = fixture.createLargeDocument(); const b = fixture.createLargeDocument();
    expect(a.text).toBe(b.text); expect(a.lines).toBe(20_000); expect(a.bytes).toBeGreaterThanOrEqual(1_048_576 * 0.95);
    expect(a.bytes).toBeLessThanOrEqual(1_048_576 * 1.05); expect(a.text).toContain('日本語'); expect(a.text).toContain('🎹');
    expect(a.longLines).toBeGreaterThanOrEqual(50); expect(a.voices).toBe(64); expect(a.visualOutputs).toEqual(['o0','o1','o2','o3']);
    expect(a.text).toContain('inst pad freq: float = 440 amp: float = 0.005:');
    expect(a.controlText).toContain('stack ['); expect(a.controlText).toContain('] > d1');
    expect(a.controlText).not.toContain('s :pad > note [60 64] > d1');
  });
  // CANVAS-EVIDENCE-RUNSTART TASK-204: focused duration 1.99s; allow suite-load contention.
  it('validates the complete evaluable document with the real host-wasm session_check', async () => {
    const wasm = await helpers.loadVactrWasm();
    expect(wasm.call('session_init', 48_000, 0)).toBe(1);
    wasm.drainRecords();
    const workload = fixture.createLargeDocument();
    wasm.callStr('session_check', JSON.stringify({ file: 'main.vact', code: workload.text }));
    const records = wasm.drainRecords().filter((record) => record.tag === 0x71);
    const checks = records.map((record) => JSON.parse(new TextDecoder().decode(record.bytes)) as SessionCheckRecord);
    expect(checks).toHaveLength(1);
    expect(checks[0]?.kind).toBe('check');
    expect(checks[0]?.diagnostics.filter((diagnostic) => diagnostic.severity === 'error')).toEqual([]);
    wasm.callStr('session_check', JSON.stringify({ file: 'main.vact', code: workload.controlText }));
    const controlRecords = wasm.drainRecords().filter((record) => record.tag === 0x71);
    const controlChecks = controlRecords.map((record) => JSON.parse(new TextDecoder().decode(record.bytes)) as SessionCheckRecord);
    expect(controlChecks).toHaveLength(1);
    expect(controlChecks[0]?.diagnostics.filter((diagnostic) => diagnostic.severity === 'error')).toEqual([]);
    expect(encoder.encode(workload.text).length).toBe(workload.bytes);
  }, 60_000);
});

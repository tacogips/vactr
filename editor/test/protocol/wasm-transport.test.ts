import { describe, expect, it } from 'vitest';
import { VactrolHost } from '../../worklet/host.js';
import { Client } from '../../src/protocol/client';
import { WasmCore, WasmTransport } from '../../src/protocol/wasm';
import type { PkgReply, RenderRecord } from '../../src/protocol/types';
import { FakeCore, fakeNode } from '../support/fake-core';

function setup(): { fake: FakeCore; core: WasmCore; node: ReturnType<typeof fakeNode> } {
  const fake = new FakeCore();
  const node = fakeNode();
  const core = new WasmCore();
  core.attach(new VactrolHost(null, node, fake.exports, {
    wasmUrl: '',
    processorUrl: '',
    init: 'session',
    onRecord: core.onRecord,
  }));
  return { fake, core, node };
}

const manifestReply = (re: number): string =>
  JSON.stringify({ v: 1, seq: 1, re, kind: 'manifest', body: { sounds: ['bd'], synths: [], controls: [] } });

describe('WasmCore / WasmTransport', () => {
  it('routes 0x71 to the transport, 0x72 to render, 0x73 to pkg', async () => {
    const { fake, core, node } = setup();
    fake.on('session_apply', (c) => {
      const env = JSON.parse(c.text ?? '{}') as { seq: number };
      fake.emit(0x71, manifestReply(env.seq));
    });
    const client = new Client(new WasmTransport(core));
    const renders: RenderRecord[] = [];
    const pkgs: PkgReply[] = [];
    core.onRender((r) => renders.push(r));
    core.onPkg((r) => pkgs.push(r));

    const reply = await client.manifest();
    expect(reply.kind).toBe('manifest');
    expect(fake.callsOf('session_apply')[0]?.text).toContain('"kind":"manifest?"');

    const program: RenderRecord = { op: 'program', out: 0, source: 'void main(){}', uniform_names: ['u0'], assets: [] };
    fake.emit(0x72, JSON.stringify(program));
    fake.emit(0x73, JSON.stringify({ status: 'need', url: 'https://proxy/x' }));
    core.frame(1.5);
    expect(fake.callsOf('session_frame')[0]?.args).toEqual([1.5]);
    expect(renders).toEqual([program]);
    expect(pkgs).toEqual([{ status: 'need', url: 'https://proxy/x' }]);
    // No editor record reached the worklet.
    expect(node.posted).toEqual([]);
  });

  it('returns session_check diagnostics without reaching the transport', () => {
    const { fake, core } = setup();
    const texts: string[] = [];
    new WasmTransport(core).onText((t) => texts.push(t));
    const d = { code: 'unbound', severity: 'error', message: 'x', span: { start: 0, end: 1 }, file: 'a.vact' };
    fake.on('session_check', () => fake.emit(0x71, JSON.stringify({ kind: 'check', diagnostics: [d] })));
    expect(core.check('(x)')).toEqual([d]);
    expect(fake.callsOf('session_check')[0]?.text).toBe('(x)');
    expect(texts).toEqual([]);
  });

  it('passes MIDI, samples and package bodies through the session exports', () => {
    const { fake, core } = setup();
    core.midiIn(new Uint8Array([0xb0, 74, 100]), 2.25);
    const midi = fake.callsOf('session_midi_in')[0];
    expect([...(midi?.bytes ?? [])]).toEqual([0xb0, 74, 100]);
    expect(midi?.args[2]).toBe(2.25);

    core.samplePut('bd:0', new Float32Array([0, 0.5]), 48000, 1);
    expect(fake.callsOf('session_sample_put')[0]?.text).toBe('bd:0');
    expect(fake.callsOf('sample_put')).toHaveLength(0);

    core.pkgResolve({ proxy: 'https://proxy', requirements: { 'a/b': '1.0.0' } });
    expect(JSON.parse(fake.callsOf('pkg_resolve')[0]?.text ?? 'null')).toEqual({
      proxy: 'https://proxy',
      requirements: { 'a/b': '1.0.0' },
    });
    core.pkgSupply('https://proxy/a/b/@v/list', 200, new TextEncoder().encode('1.0.0\n'));
    const supply = fake.callsOf('pkg_supply')[0];
    expect(supply?.text).toBe('https://proxy/a/b/@v/list');
    expect(supply?.args[2]).toBe(200);
    expect(new TextDecoder().decode(supply?.bytes)).toBe('1.0.0\n');
  });

  it('stops delivery after close', () => {
    const { fake, core } = setup();
    const t = new WasmTransport(core);
    const texts: string[] = [];
    t.onText((x) => texts.push(x));
    t.close();
    t.send('{"v":1}');
    fake.emit(0x71, manifestReply(1));
    core.host.flush();
    expect(fake.callsOf('session_apply')).toHaveLength(0);
    expect(texts).toEqual([]);
  });
});

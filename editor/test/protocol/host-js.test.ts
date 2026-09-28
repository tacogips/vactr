import { afterEach, describe, expect, it, vi } from 'vitest';
import { VactrolHost, startHost } from '../../worklet/host.js';
import { FakeCore, fakeNode } from '../support/fake-core';

const bytes = (b: unknown): number[] => [...new Uint8Array(b as ArrayBuffer)];

function host(fake: FakeCore, opts: Record<string, unknown> = {}) {
  const node = fakeNode();
  const h = new VactrolHost(null, node, fake.exports, { wasmUrl: '', processorUrl: '', ...opts });
  return { h, node };
}

describe('worklet/host.js', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('default options: records reach the port exactly as before', () => {
    const fake = new FakeCore();
    const lines: string[] = [];
    const { h, node } = host(fake, { onConsole: (l: string) => lines.push(l) });
    fake.emit(0x01, new Uint8Array([9, 8, 7]));
    fake.emit(0x70, 'hello');
    fake.emit(0x16, new Uint8Array([1]));
    h.flush();
    expect(node.posted.map(bytes)).toEqual([
      [0x01, 9, 8, 7],
      [0x16, 1],
    ]);
    expect(lines).toEqual(['hello']);
    expect(fake.pendingRecords).toBe(0);

    h.onWorklet({ o: new Uint8Array([0x40, 1]).buffer, t: 0.5 });
    expect(fake.callsOf('inbox')).toHaveLength(1);
    expect(fake.callsOf('tick')[0]?.args).toEqual([0.5]);
    expect(fake.callsOf('session_inbox')).toHaveLength(0);
    expect(fake.callsOf('session_tick')).toHaveLength(0);

    h.putSample('bd:0', new Float32Array([0.25]), 48000, 1);
    expect(fake.callsOf('sample_put')).toHaveLength(1);
  });

  it('connects an external Web Audio source to the input-enabled node', () => {
    const { h, node } = host(new FakeCore());
    const connect = vi.fn();
    h.connectInput({ connect } as unknown as AudioNode);
    expect(connect).toHaveBeenCalledWith(node);
  });

  it("init 'session': session exports, and 0x71-0x73 never reach the port", () => {
    const fake = new FakeCore();
    const records: [number, string][] = [];
    const { h, node } = host(fake, {
      init: 'session',
      onRecord: (tag: number, p: Uint8Array) => records.push([tag, new TextDecoder().decode(p)]),
    });
    h.onWorklet({ o: new Uint8Array([0x40, 2]).buffer, t: 1.25 });
    expect(fake.callsOf('session_inbox')).toHaveLength(1);
    expect(fake.callsOf('session_tick')[0]?.args).toEqual([1.25]);
    expect(fake.callsOf('inbox')).toHaveLength(0);
    expect(fake.callsOf('tick')).toHaveLength(0);

    fake.emit(0x71, '{"a":1}');
    fake.emit(0x01, new Uint8Array([5]));
    fake.emit(0x72, '{"b":2}');
    fake.emit(0x73, '{"c":3}');
    h.flush();
    expect(node.posted.map(bytes)).toEqual([[0x01, 5]]);
    expect(records).toEqual([
      [0x71, '{"a":1}'],
      [0x72, '{"b":2}'],
      [0x73, '{"c":3}'],
    ]);

    h.putSample('bd:0', new Float32Array([0.25]), 48000, 1);
    expect(fake.callsOf('session_sample_put')).toHaveLength(1);
    expect(fake.callsOf('sample_put')).toHaveLength(0);
  });

  it('delivers each editor record once when onRecord re-enters wasm', () => {
    const fake = new FakeCore();
    const seen: string[] = [];
    let h: VactrolHost | null = null;
    fake.on('session_apply', () => fake.emit(0x71, 'reply'));
    ({ h } = host(fake, {
      init: 'session',
      onRecord: (_t: number, p: Uint8Array) => {
        const s = new TextDecoder().decode(p);
        seen.push(s);
        if (s === 'first') h?.callStr('session_apply', '{}');
      },
    }));
    fake.emit(0x71, 'first');
    fake.emit(0x71, 'second');
    h.flush();
    expect(seen).toEqual(['first', 'reply', 'second']);
  });

  function stubPlatform(fake: FakeCore): { connections: Array<[string, string, number, number]>; nodeOptions: Array<Record<string, unknown>> } {
    const connections: Array<[string, string, number, number]> = [];
    const nodeOptions: Array<Record<string, unknown>> = [];
    const part = (name: string) => ({
      name,
      connect(to: { name: string }, from = 0, input = 0) {
        connections.push([name, to.name, from, input]);
      },
    });
    vi.stubGlobal('fetch', async () => new Response(new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0])));
    vi.spyOn(WebAssembly, 'instantiate').mockResolvedValue({
      instance: { exports: fake.exports as unknown as WebAssembly.Exports },
      module: {} as WebAssembly.Module,
    } as unknown as WebAssembly.Instance & WebAssembly.WebAssemblyInstantiatedSource);
    vi.stubGlobal(
      'AudioContext',
      class {
        sampleRate = 48000;
        destination = part('destination');
        audioWorklet = { addModule: async () => {} };
        createChannelSplitter(channels: number) {
          expect(channels).toBe(4);
          return part('splitter');
        }
        createChannelMerger(channels: number) {
          expect(channels).toBe(2);
          return part('merger');
        }
      },
    );
    vi.stubGlobal(
      'AudioWorkletNode',
      class {
        port = { postMessage: () => {}, onmessage: null };
        constructor(_ctx: unknown, _name: string, options: Record<string, unknown>) {
          nodeOptions.push(options);
        }
        connect(to: { name: string }, from = 0, input = 0) {
          connections.push(['node', to.name, from, input]);
        }
      },
    );
    return { connections, nodeOptions };
  }

  it('startHost calls main_init by default', async () => {
    const fake = new FakeCore();
    stubPlatform(fake);
    await startHost({ wasmUrl: 'x.wasm', processorUrl: 'p.js' });
    expect(fake.callsOf('main_init')[0]?.args).toEqual([48000, 0]);
    expect(fake.callsOf('session_init')).toHaveLength(0);
  });

  it("startHost calls session_init with init 'session'", async () => {
    const fake = new FakeCore();
    stubPlatform(fake);
    await startHost({ wasmUrl: 'x.wasm', processorUrl: 'p.js', init: 'session', arenaBytes: 1024 });
    expect(fake.callsOf('session_init')[0]?.args).toEqual([48000, 1024]);
    expect(fake.callsOf('main_init')).toHaveLength(0);
  });

  it('routes opt-in quad worklet outputs through a splitter while session init stays available', async () => {
    const fake = new FakeCore();
    const platform = stubPlatform(fake);
    const h = await startHost({ wasmUrl: 'x.wasm', processorUrl: 'p.js', init: 'session', outputChannels: 4 });
    expect(fake.callsOf('session_init')[0]?.args).toEqual([48000, 0]);
    expect(fake.callsOf('main_init')).toHaveLength(0);
    expect(platform.nodeOptions[0]?.outputChannelCount).toEqual([4]);
    expect(platform.nodeOptions[0]?.numberOfInputs).toBe(1);
    expect(platform.nodeOptions[0]?.channelCount).toBe(2);
    expect(platform.nodeOptions[0]?.processorOptions).toMatchObject({ outputChannels: 4 });
    expect(h.quadSplitter).not.toBeNull();
    expect(platform.connections).toEqual([
      ['node', 'splitter', 0, 0],
      ['splitter', 'merger', 0, 0],
      ['splitter', 'merger', 1, 1],
      ['merger', 'destination', 0, 0],
    ]);
    expect(platform.connections.some(([from, , output]) => from === 'splitter' && output >= 2)).toBe(false);
  });

  it('keeps the main-mode eval helper working', () => {
    const fake = new FakeCore();
    const { h } = host(fake);
    fake.on('eval', () => 0);
    expect(h.eval('(s "bd")')).toBe(0);
    expect(fake.callsOf('eval')[0]?.text).toBe('(s "bd")');
  });
});

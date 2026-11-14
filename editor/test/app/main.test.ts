import { describe, expect, it, vi } from 'vitest';

const { order, area, socketMessages, tauriConnect, wasmStart, invokeMock } = vi.hoisted(() => {
  const order: string[] = [];
  const socketMessages: string[] = [];
  const tauriConnect = vi.fn();
  const wasmStart = vi.fn();
  const invokeMock = vi.fn(async (..._args: unknown[]) => false);
  const area = (name: string) => ({
    mount: () => {
      order.push(name);
      return { dispose: () => void order.push(`dispose:${name}`) };
    },
  });
  return { order, area, socketMessages, tauriConnect, wasmStart, invokeMock };
});
vi.mock('../../src/code/mount', () => ({ ...area('code'), DOC_FILE: 'main.vact' }));
vi.mock('../../src/midi/mount', () => area('midi'));
vi.mock('../../src/visual/mount', () => area('visual'));
vi.mock('../../src/bind/mount', () => area('bind'));
vi.mock('../../src/params/mount', () => area('params'));
vi.mock('../../src/pkg/mount', () => area('pkg'));
vi.mock('../../src/protocol/socket', () => ({
  SocketTransport: class {
    send(text: string): void { socketMessages.push(text); }
    onText(_callback: (text: string) => void): void {}
    close(): void {}
  },
}));
vi.mock('../../src/protocol/tauri', () => ({ TauriTransport: { connect: tauriConnect } }));
vi.mock('../../src/protocol/wasm', () => ({
  WasmCore: { start: wasmStart },
  WasmTransport: class {
    send(_text: string): void {}
    onText(_callback: (text: string) => void): void {}
    close(): void {}
  },
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

import { MOUNT_ORDER, boot, createEditor, tierFromUrl, reportSelfCheck } from '../../src/app/main';
import { audibleFor } from '../../src/app/clock';
import { PANES } from '../../src/app/layout';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

describe('createEditor', () => {
  it('mounts all six areas in the fixed order', () => {
    const root = document.createElement('div');
    document.body.appendChild(root);
    const store = new Store();
    const transport = new RecordingTransport();
    const deps = {
      client: new Client(transport, { store }),
      store,
      clock: new MockClock(),
      tier: 'browser' as const,
      files: new MemoryFiles(),
    };
    order.length = 0;
    const editor = createEditor(root, deps);
    expect(MOUNT_ORDER).toEqual(['code', 'midi', 'visual', 'bind', 'params', 'pkg']);
    expect(order).toEqual(['code', 'midi', 'visual', 'bind', 'params', 'pkg']);
    for (const p of PANES) expect(root.querySelector(`[data-pane="${p}"]`)).not.toBeNull();
    editor.dispose();
    expect(order.slice(6)).toEqual(['pkg', 'params', 'bind', 'visual', 'midi', 'code'].map((a) => `dispose:${a}`));
    // Mounting sends nothing on its own.
    expect(transport.sent).toEqual([]);
  });
});

describe('tierFromUrl', () => {
  it('selects the native tier only with ?session=', () => {
    expect(tierFromUrl('?session=ws://127.0.0.1:7777/session?token=ab')).toEqual({
      tier: 'native',
      url: 'ws://127.0.0.1:7777/session?token=ab',
    });
    expect(tierFromUrl('')).toEqual({ tier: 'browser' });
    expect(tierFromUrl('?other=1')).toEqual({ tier: 'browser' });
  });
});

describe('audible tier wiring', () => {
  it('uses output timestamps in browser mode', () => {
    const result = audibleFor('browser', { ctx: {
      currentTime: 3, sampleRate: 48000,
      getOutputTimestamp: () => ({ contextTime: 2, performanceTime: performance.now() }),
    } });
    expect(result.audible.sample(performance.now()).provenance).toBe('measured');
    result.dispose();
  });

  it('starts the native probe immediately, repeats, and stops on disposal', async () => {
    vi.useFakeTimers();
    let result: ReturnType<typeof audibleFor> | undefined;
    try {
      let count = 0;
      result = audibleFor('native', { client: {
        clockProbe: async (pageSend: number) => {
          count += 1;
          return { v: 1, seq: count, kind: 'clock-probe', body: {
            page_send: pageSend, engine_receive: pageSend / 1000, engine_send: pageSend / 1000,
            epoch: 'e1', latency_seconds: null, latency_kind: 'unavailable', uncertainty_seconds: null,
          } };
        },
      } as never });
      result.start();
      await Promise.resolve();
      expect(count).toBe(1);
      expect(result.audible.sample(performance.now()).valid).toBe(true);
      expect(result.audible.sample(performance.now()).provenance).toBe('unavailable');
      await vi.advanceTimersByTimeAsync(1000);
      expect(count).toBe(2);
      result.dispose();
      await vi.advanceTimersByTimeAsync(5000);
      expect(count).toBe(2);
      expect(result.audible.sample(performance.now()).valid).toBe(false);
    } finally {
      result?.dispose();
      vi.useRealTimers();
    }
  });

  it('wires and disposes the native audible clock when booting with ?session=', async () => {
    vi.useFakeTimers();
    socketMessages.length = 0;
    const root = document.createElement('div');
    document.body.appendChild(root);
    const win = {
      location: { search: '?session=ws://127.0.0.1:7777/session' },
      document: root.ownerDocument,
    } as Window;
    try {
      const editor = await boot(root, win);
      expect(editor.deps.audible).toBeDefined();
      const probes = () => socketMessages.filter((text) => JSON.parse(text).kind === 'clock-probe');
      expect(probes()).toHaveLength(1);
      editor.dispose();
      await vi.advanceTimersByTimeAsync(5000);
      expect(probes()).toHaveLength(1);
    } finally {
      vi.useRealTimers();
      root.remove();
    }
  });

  it('selects Tauri IPC as the native tier and starts clock probes', async () => {
    const root = document.createElement('div');
    document.body.appendChild(root);
    const transport = new RecordingTransport();
    tauriConnect.mockResolvedValue(transport);
    const win = {
      location: { search: '' },
      document: root.ownerDocument,
      __TAURI_INTERNALS__: {},
    } as unknown as Window;
    try {
      const editor = await boot(root, win);
      expect(editor.deps.tier).toBe('native');
      expect(editor.deps.audible).toBeDefined();
      expect(tauriConnect).toHaveBeenCalledOnce();
      expect(transport.sent.map((text) => JSON.parse(text).kind)).toContain('clock-probe');
      editor.dispose();
    } finally {
      root.remove();
    }
  });

  it('falls back to browser Wasm and shows the native connection error', async () => {
    const root = document.createElement('div');
    document.body.appendChild(root);
    tauriConnect.mockRejectedValue(new Error('IPC unavailable'));
    const ctx = { currentTime: 1, sampleRate: 48000, resume: vi.fn(async () => {}) };
    wasmStart.mockResolvedValue({ host: { ctx } });
    const win = {
      location: { search: '' },
      document: root.ownerDocument,
      __TAURI_INTERNALS__: {},
    } as unknown as Window;
    try {
      const editor = await boot(root, win);
      expect(editor.deps.tier).toBe('browser');
      expect(wasmStart).toHaveBeenCalledOnce();
      expect(root.querySelector('[data-tauri-fallback="true"]')?.textContent).toContain('IPC unavailable');
      editor.dispose();
    } finally {
      root.remove();
    }
  });
});

describe('native self-check', () => {
  async function runSelfCheck(playingCount:number):Promise<string> {
    invokeMock.mockReset();invokeMock.mockResolvedValueOnce(true).mockResolvedValueOnce(false);
    const listeners=new Map<string,Array<(event:unknown)=>void>>();
    const client={on:(kind:string,callback:(event:unknown)=>void)=>{const entries=listeners.get(kind)??[];entries.push(callback);listeners.set(kind,entries);return()=>listeners.set(kind,(listeners.get(kind)??[]).filter((entry)=>entry!==callback));}};
    const root=document.createElement('div');
    const pending=reportSelfCheck(root,{devicePixelRatio:2} as unknown as Window,{client,store:{transportSample:null},tier:'native'} as never);
    for(let i=0;i<playingCount;i+=1)for(const callback of listeners.get('playing')??[])callback({});
    for(const callback of listeners.get('tempo')??[])callback({});
    await pending;
    const args=invokeMock.mock.calls.find((call)=>call[0]==='self_check')?.[1] as {report:string}|undefined;
    if(!args)throw new Error('self_check report was not invoked');
    return args.report;
  }
  it('reports zero playing envelopes when the simulator self-check starts silent',async()=>{
    const report = await runSelfCheck(0);
    expect(report).toContain('"playingEvents":0');
    expect(report).toContain('"syntax":"absent"');
  });
  it('reports playing envelopes observed before the self-check report',async()=>{
    expect(await runSelfCheck(1)).toContain('"playingEvents":1');
  });

  it('waits for tree-sitter-worker syntax and reports that it resolved', async () => {
    vi.useFakeTimers();
    invokeMock.mockReset();invokeMock.mockResolvedValueOnce(true).mockResolvedValueOnce(false);
    const listeners = new Map<string, Array<(event: unknown) => void>>();
    const client = { on: (kind: string, callback: (event: unknown) => void) => {
      const entries = listeners.get(kind) ?? [];
      entries.push(callback);
      listeners.set(kind, entries);
      return () => listeners.set(kind, (listeners.get(kind) ?? []).filter((entry) => entry !== callback));
    } };
    const root = document.createElement('div');
    const codePane = document.createElement('section');
    codePane.dataset.pane = 'code';
    codePane.dataset.syntax = 'fallback';
    root.append(codePane);
    const pending = reportSelfCheck(root, { devicePixelRatio: 2 } as unknown as Window,
      { client, store: { transportSample: null }, tier: 'native' } as never);
    try {
      await Promise.resolve();
      await Promise.resolve();
      for (const callback of listeners.get('tempo') ?? []) callback({});
      setTimeout(() => { codePane.dataset.syntax = 'tree-sitter-worker'; }, 500);
      await vi.advanceTimersByTimeAsync(500);
      await pending;
      const args = invokeMock.mock.calls.find((call) => call[0] === 'self_check')?.[1] as { report: string };
      expect(JSON.parse(args.report)).toMatchObject({
        syntax: 'tree-sitter-worker',
        syntaxWaitMs: 500,
        syntaxTimedOut: false,
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it('reports fallback syntax when the syntax readiness bound expires', async () => {
    vi.useFakeTimers();
    invokeMock.mockReset();invokeMock.mockResolvedValueOnce(true).mockResolvedValueOnce(false);
    const listeners = new Map<string, Array<(event: unknown) => void>>();
    const client = { on: (kind: string, callback: (event: unknown) => void) => {
      const entries = listeners.get(kind) ?? [];
      entries.push(callback);
      listeners.set(kind, entries);
      return () => listeners.set(kind, (listeners.get(kind) ?? []).filter((entry) => entry !== callback));
    } };
    const root = document.createElement('div');
    const codePane = document.createElement('section');
    codePane.dataset.pane = 'code';
    codePane.dataset.syntax = 'fallback';
    root.append(codePane);
    const pending = reportSelfCheck(root, { devicePixelRatio: 2 } as unknown as Window,
      { client, store: { transportSample: null }, tier: 'native' } as never);
    try {
      await Promise.resolve();
      await Promise.resolve();
      for (const callback of listeners.get('tempo') ?? []) callback({});
      await vi.advanceTimersByTimeAsync(9999);
      expect(invokeMock).not.toHaveBeenCalledWith('self_check', expect.anything());
      await vi.advanceTimersByTimeAsync(1);
      await pending;
      const args = invokeMock.mock.calls.find((call) => call[0] === 'self_check')?.[1] as { report: string };
      expect(JSON.parse(args.report)).toMatchObject({
        syntax: 'fallback',
        syntaxWaitMs: 10_000,
        syntaxTimedOut: true,
      });
    } finally {
      vi.useRealTimers();
    }
  });
});

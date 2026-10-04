import { describe, expect, it, vi } from 'vitest';

const { order, area } = vi.hoisted(() => {
  const order: string[] = [];
  const area = (name: string) => ({
    mount: () => {
      order.push(name);
      return { dispose: () => void order.push(`dispose:${name}`) };
    },
  });
  return { order, area };
});
vi.mock('../../src/code/mount', () => ({ ...area('code'), DOC_FILE: 'main.vact' }));
vi.mock('../../src/midi/mount', () => area('midi'));
vi.mock('../../src/visual/mount', () => area('visual'));
vi.mock('../../src/bind/mount', () => area('bind'));
vi.mock('../../src/params/mount', () => area('params'));
vi.mock('../../src/pkg/mount', () => area('pkg'));

import { MOUNT_ORDER, createEditor, tierFromUrl } from '../../src/app/main';
import { audibleFor } from '../../src/app/clock';
import { PANES } from '../../src/app/layout';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { SimulatedTime } from '../support/clock';
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
    const time = new SimulatedTime();
    const result = audibleFor('native', { timers: time.timers, client: {
      clockProbe: async (pageSend: number) => {
        count += 1;
        return { v: 1, seq: count, kind: 'clock-probe', body: {
          page_send: pageSend, engine_receive: pageSend / 1000, engine_send: pageSend / 1000,
          epoch: 'e1', latency_seconds: null, latency_kind: 'unavailable', uncertainty_seconds: null,
        } };
      },
    } as never });
    let count = 0;
    result.start(); await Promise.resolve();
    expect(count).toBe(1);
    expect(result.audible.sample(performance.now()).valid).toBe(true);
    expect(result.audible.sample(performance.now()).provenance).toBe('unavailable');
    time.advance(1000); await Promise.resolve();
    expect(count).toBe(2);
    result.dispose();
    time.advance(5000); await Promise.resolve();
    expect(count).toBe(2);
    expect(result.audible.sample(time.pageMs).valid).toBe(false);
  });
});

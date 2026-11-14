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
vi.mock('../../src/code/mount', () => area('code'));
vi.mock('../../src/midi/mount', () => area('midi'));
vi.mock('../../src/visual/mount', () => area('visual'));
vi.mock('../../src/bind/mount', () => area('bind'));
vi.mock('../../src/params/mount', () => area('params'));
vi.mock('../../src/pkg/mount', () => area('pkg'));

import { MOUNT_ORDER, createEditor, tierFromUrl } from '../../src/app/main';
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

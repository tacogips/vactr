import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TransportBar } from '../../src/code/transport';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

const cleanup: (() => void)[] = [];
afterEach(() => { for (const dispose of cleanup.splice(0)) dispose(); document.body.innerHTML = ''; });

function bar(vim?: { enabled: () => boolean; toggle: () => void }) {
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const b = new TransportBar(parent, {
    client: new Client(new RecordingTransport(), { store: new Store() }),
    clock: new MockClock(0),
    ...(vim ? { vim } : {}),
  });
  cleanup.push(() => b.dispose());
  return { parent, bar: b };
}

describe('Vim transport toggle', () => {
  it('is absent when the optional transport option is omitted', () => {
    const { parent } = bar();
    expect(parent.querySelector('.vact-vim-toggle')).toBeNull();
  });
  it('renders off with an accessible name and pressed state', () => {
    const [enabled] = createSignal(false);
    const { parent } = bar({ enabled, toggle: vi.fn() });
    const button = parent.querySelector('.vact-vim-toggle');
    expect(button?.getAttribute('aria-pressed')).toBe('false');
    expect(button?.getAttribute('aria-label')).toBe('Vim mode: off');
    expect(button?.textContent).toBe('vim');
  });
  it('calls toggle on click', () => {
    const toggle = vi.fn();
    const [enabled] = createSignal(false);
    const { parent } = bar({ enabled, toggle });
    parent.querySelector<HTMLButtonElement>('.vact-vim-toggle')?.click();
    expect(toggle).toHaveBeenCalledOnce();
  });
  it('reacts to the enabled signal', () => {
    const [enabled, setEnabled] = createSignal(false);
    const { parent } = bar({ enabled, toggle: vi.fn() });
    setEnabled(true);
    expect(parent.querySelector('.vact-vim-toggle')?.getAttribute('aria-pressed')).toBe('true');
    expect(parent.querySelector('.vact-vim-toggle')?.getAttribute('aria-label')).toBe('Vim mode: on');
    expect(parent.querySelector('.vact-vim-toggle')?.getAttribute('title')).toBe('Vim mode: on');
  });
});

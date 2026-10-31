import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TransportBar } from '../../src/code/transport';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

const cleanup: (() => void)[] = [];
afterEach(() => { for (const dispose of cleanup.splice(0)) dispose(); document.body.innerHTML = ''; });

describe('inline widgets transport toggle', () => {
  it('renders on, calls toggle, and reflects the off state', () => {
    const [enabled, setEnabled] = createSignal(true);
    const toggle = vi.fn(() => setEnabled(!enabled()));
    const parent = document.createElement('div');
    document.body.appendChild(parent);
    const bar = new TransportBar(parent, {
      client: new Client(new RecordingTransport(), { store: new Store() }),
      clock: new MockClock(0), inline: { enabled, toggle },
    });
    cleanup.push(() => bar.dispose());
    const button = parent.querySelector<HTMLButtonElement>('.vact-inline-toggle');
    expect(button?.getAttribute('aria-pressed')).toBe('true');
    expect(button?.getAttribute('aria-label')).toBe('Inline widgets: on');
    button?.click();
    expect(toggle).toHaveBeenCalledOnce();
    expect(button?.getAttribute('aria-pressed')).toBe('false');
    expect(button?.getAttribute('aria-label')).toBe('Inline widgets: off');
  });
});

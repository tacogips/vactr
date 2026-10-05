import { describe, expect, it, vi } from 'vitest';
import { TauriTransport, type TauriApi } from '../../src/protocol/tauri';

describe('TauriTransport', () => {
  it('connects a Channel and routes text, sends and closes by session id', async () => {
    const calls: { command: string; args?: Record<string, unknown> }[] = [];
    const channelRef: { value?: { onmessage: (text: string) => void } } = {};
    class FakeChannel {
      onmessage: (text: string) => void;
      constructor(onmessage: (text: string) => void) {
        this.onmessage = onmessage;
        channelRef.value = this;
      }
    }
    const api = {
      Channel: FakeChannel,
      invoke: async (command: string, args?: Record<string, unknown>) => {
        calls.push({ command, args });
        return command === 'session_connect' ? 17 : undefined;
      },
    } as unknown as TauriApi;

    const transport = await TauriTransport.connect(api);
    expect(calls[0]).toMatchObject({ command: 'session_connect' });
    expect(calls[0]?.args?.channel).toBe(channelRef.value);
    const received: string[] = [];
    transport.onText((text) => received.push(text));
    channelRef.value?.onmessage('reply');
    expect(received).toEqual(['reply']);

    transport.send('request');
    transport.close();
    channelRef.value?.onmessage('late');
    expect(received).toEqual(['reply']);
    expect(calls.slice(1)).toEqual([
      { command: 'session_send', args: { id: 17, text: 'request' } },
      { command: 'session_close', args: { id: 17 } },
    ]);
  });

  it('rejects when session_connect fails', async () => {
    const api = {
      Channel: class { constructor(_onmessage: (text: string) => void) {} },
      invoke: vi.fn(async () => { throw new Error('native unavailable'); }),
    } as unknown as TauriApi;
    await expect(TauriTransport.connect(api)).rejects.toThrow('native unavailable');
  });
});

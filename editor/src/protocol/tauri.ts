// Session Protocol transport for the Tauri IPC channel.

import { Channel, invoke } from '@tauri-apps/api/core';
import type { Transport } from './transport';

export interface TauriApi {
  invoke: typeof invoke;
  Channel: typeof Channel;
}

export class TauriTransport implements Transport {
  private readonly id: number;
  private readonly channel: Channel<string>;
  private readonly offs: (() => void)[] = [];
  private closed = false;

  private constructor(id: number, channel: Channel<string>, private readonly api: TauriApi) {
    this.id = id;
    this.channel = channel;
  }

  static async connect(api: TauriApi = { invoke, Channel }): Promise<TauriTransport> {
    let deliver: ((text: string) => void) | null = null;
    const channel = new api.Channel<string>((text) => deliver?.(text));
    const id = await api.invoke<number>('session_connect', { channel });
    const transport = new TauriTransport(id, channel, api);
    deliver = (text) => {
      if (!transport.closed) for (const cb of transport.listeners) cb(text);
    };
    return transport;
  }

  private readonly listeners: ((text: string) => void)[] = [];

  send(text: string): void {
    if (!this.closed) void this.api.invoke('session_send', { id: this.id, text });
  }

  onText(cb: (text: string) => void): void {
    if (!this.closed) this.listeners.push(cb);
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.channel.onmessage = () => {};
    this.listeners.length = 0;
    for (const off of this.offs) off();
    this.offs.length = 0;
    void this.api.invoke('session_close', { id: this.id });
  }
}

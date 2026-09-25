import { afterEach, describe, expect, it, vi } from 'vitest';
import { Client } from '../../src/protocol/client';
import { SocketTransport, type SocketLike } from '../../src/protocol/socket';

class FakeSocket implements SocketLike {
  readyState = 0;
  readonly sent: string[] = [];
  closed = false;
  onopen: ((ev: Event) => void) | null = null;
  onmessage: ((ev: MessageEvent) => void) | null = null;
  onclose: ((ev: CloseEvent) => void) | null = null;
  onerror: ((ev: Event) => void) | null = null;
  constructor(readonly url: string) {}
  send(data: string): void {
    this.sent.push(data);
  }
  close(): void {
    this.closed = true;
  }
  open(): void {
    this.readyState = 1;
    this.onopen?.(new Event('open'));
  }
  receive(data: unknown): void {
    this.onmessage?.(new MessageEvent('message', { data }));
  }
}

const URL_WITH_TOKEN = 'ws://127.0.0.1:7777/session?token=00112233445566778899aabbccddeeff';

describe('SocketTransport', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
    sessionStorage.clear();
  });

  it('queues until open, then sends and receives text frames', () => {
    let sock: FakeSocket | null = null;
    const t = new SocketTransport(URL_WITH_TOKEN, (u) => (sock = new FakeSocket(u)));
    const s = sock as unknown as FakeSocket;
    expect(s.url).toBe(URL_WITH_TOKEN);
    const got: string[] = [];
    t.onText((x) => got.push(x));
    t.send('{"early":1}');
    expect(s.sent).toEqual([]);
    s.open();
    expect(s.sent).toEqual(['{"early":1}']);
    t.send('{"late":2}');
    expect(s.sent).toEqual(['{"early":1}', '{"late":2}']);
    s.receive('{"v":1}');
    s.receive(new ArrayBuffer(2)); // binary frames are not protocol frames
    expect(got).toEqual(['{"v":1}']);
    t.close();
    expect(s.closed).toBe(true);
    t.send('{"after":3}');
    expect(s.sent).toHaveLength(2);
  });

  it('never writes the token or anything else to web storage', async () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem');
    let sock: FakeSocket | null = null;
    const t = new SocketTransport(URL_WITH_TOKEN, (u) => (sock = new FakeSocket(u)));
    const client = new Client(t);
    const s = sock as unknown as FakeSocket;
    s.open();
    const reply = client.manifest();
    s.receive(JSON.stringify({ v: 1, seq: 1, re: 1, kind: 'manifest', body: { sounds: [], synths: [], controls: [] } }));
    await reply;
    client.close();
    expect(setItem).not.toHaveBeenCalled();
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });

  it('reports connection loss', () => {
    let sock: FakeSocket | null = null;
    const t = new SocketTransport(URL_WITH_TOKEN, (u) => (sock = new FakeSocket(u)));
    const reasons: string[] = [];
    t.onClose((r) => reasons.push(r));
    (sock as unknown as FakeSocket).onclose?.(new CloseEvent('close', { code: 1006 }));
    expect(reasons).toEqual(['closed (1006)']);
  });
});

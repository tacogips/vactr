// The transport seam (design 15.1.4): JSON text frames both ways. The
// implementations are `WasmTransport` (wasm.ts), `SocketTransport`
// (socket.ts) and, in tests, `RecordingTransport`. Only `app/main.ts`
// chooses one; no UI component imports a concrete transport.

export interface Transport {
  /** Sends one client envelope (JSON text). */
  send(text: string): void;
  /** Registers a listener for every server frame (JSON text). */
  onText(cb: (text: string) => void): void;
  /** Stops delivery and releases the connection. */
  close(): void;
}

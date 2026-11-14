// Forwarding raw MIDI to the browser session (design 15.1.9, 11.7;
// command.md `session_midi_in`). Browser tier only: on the native tier the
// session's own midir input serves the language, so nothing is forwarded.
// The forwarded kinds mirror the native `parse_midi`: note on/off, control
// change, clock, start, continue and stop. Event timestamps (page clock,
// ms) are converted to the audio clock with `getOutputTimestamp()`.

/** The `WasmCore` surface forwarding needs. */
export interface MidiSink {
  midiIn(bytes: Uint8Array, time: number): void;
}

/** The `AudioContext` surface the time conversion needs. */
export interface OutputClock {
  readonly currentTime: number;
  getOutputTimestamp?(): { contextTime?: number; performanceTime?: number };
}

/** Audio-clock seconds of an event stamped `timeStamp` ms on the page clock. */
export function audioTime(ctx: OutputClock, timeStamp: number): number {
  const ts = typeof ctx.getOutputTimestamp === 'function' ? ctx.getOutputTimestamp() : undefined;
  const contextTime = ts?.contextTime;
  const performanceTime = ts?.performanceTime;
  if (
    Number.isFinite(timeStamp) &&
    typeof contextTime === 'number' &&
    typeof performanceTime === 'number' &&
    performanceTime > 0
  ) {
    return contextTime + (timeStamp - performanceTime) / 1000;
  }
  // No output timestamp yet (for example a suspended context): receipt time.
  return ctx.currentTime;
}

const has2 = (b: Uint8Array): boolean => b.length >= 3 && b[1] < 0x80 && b[2] < 0x80;

/** Whether the session consumes this message (the native `parse_midi` set). */
export function forwardable(bytes: Uint8Array): boolean {
  if (bytes.length === 0) return false;
  const status = bytes[0];
  switch (status & 0xf0) {
    case 0x80:
    case 0x90:
    case 0xb0:
      return has2(bytes);
    default:
      return status === 0xf8 || status === 0xfa || status === 0xfb || status === 0xfc;
  }
}

export class Forwarder {
  private readonly sink: MidiSink;
  private readonly ctx: OutputClock;

  constructor(sink: MidiSink, ctx: OutputClock) {
    this.sink = sink;
    this.ctx = ctx;
  }

  /** Converts the time of one active-input message. */
  time(timeStamp: number): number {
    return audioTime(this.ctx, timeStamp);
  }

  /** Forwards one active-input message at audio-clock `time`. */
  forward(bytes: Uint8Array, time: number): void {
    if (forwardable(bytes)) this.sink.midiIn(bytes, time);
  }
}

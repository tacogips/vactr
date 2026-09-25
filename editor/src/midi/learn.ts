// The CC event stream and learn capture (design 15.1.6, 15.1.9). `CcStream`
// implements `MidiApi`: the device picker feeds it every message from an
// active input; control changes become `MidiCcEvent`s for `onCc`
// subscribers, and the next one resolves a pending `learnNext`. Notes,
// clock and every other message are not CC events.

import type { MidiApi, MidiCcEvent } from '../app/apis';

/** A control change `0xBn cc value` with its channel as 1..16; null otherwise. */
export function parseCc(bytes: Uint8Array, time: number): MidiCcEvent | null {
  if (bytes.length < 3) return null;
  const status = bytes[0];
  const cc = bytes[1];
  const value = bytes[2];
  if ((status & 0xf0) !== 0xb0 || cc > 0x7f || value > 0x7f) return null;
  return { cc, ch: (status & 0x0f) + 1, value, time };
}

/** The rejection of a pending `learnNext` that was cancelled or superseded. */
export class LearnCancelled extends Error {
  constructor() {
    super('MIDI learn cancelled');
    this.name = 'LearnCancelled';
  }
}

interface PendingLearn {
  resolve(v: { cc: number; ch: number }): void;
  reject(e: Error): void;
}

export class CcStream implements MidiApi {
  private readonly listeners: ((ev: MidiCcEvent) => void)[] = [];
  private readonly learnListeners: ((learning: boolean) => void)[] = [];
  private pending: PendingLearn | null = null;

  /** One message from an active input, `time` in seconds on the editor clock. */
  dispatch(bytes: Uint8Array, time: number): void {
    const ev = parseCc(bytes, time);
    if (!ev) return;
    const learn = this.pending;
    if (learn) {
      this.pending = null;
      learn.resolve({ cc: ev.cc, ch: ev.ch });
      this.notifyLearn(false);
    }
    for (const cb of this.listeners.slice()) {
      try {
        cb(ev);
      } catch (e) {
        console.error('midi cc listener failed', e);
      }
    }
  }

  onCc(cb: (ev: MidiCcEvent) => void): () => void {
    this.listeners.push(cb);
    return () => {
      const i = this.listeners.indexOf(cb);
      if (i >= 0) this.listeners.splice(i, 1);
    };
  }

  /** Resolves with the next CC; a learn already pending is cancelled. */
  learnNext(): Promise<{ cc: number; ch: number }> {
    this.pending?.reject(new LearnCancelled());
    return new Promise((resolve, reject) => {
      this.pending = { resolve, reject };
      this.notifyLearn(true);
    });
  }

  cancelLearn(): void {
    const learn = this.pending;
    if (!learn) return;
    this.pending = null;
    learn.reject(new LearnCancelled());
    this.notifyLearn(false);
  }

  get learning(): boolean {
    return this.pending !== null;
  }

  /** Learn state changes, for the pane's indicator. */
  onLearnState(cb: (learning: boolean) => void): () => void {
    this.learnListeners.push(cb);
    return () => {
      const i = this.learnListeners.indexOf(cb);
      if (i >= 0) this.learnListeners.splice(i, 1);
    };
  }

  private notifyLearn(learning: boolean): void {
    for (const cb of this.learnListeners.slice()) cb(learning);
  }
}

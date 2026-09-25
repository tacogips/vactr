import { afterEach, describe, expect, it } from 'vitest';
import type { MidiCcEvent } from '../../src/app/apis';
import { CcStream, LearnCancelled, parseCc } from '../../src/midi/learn';
import { FakeMIDIAccess, MemoryStorage, fakeNavigator, midiDeps, mountMidi, settle, type MountedMidi } from '../support/midi';

const bytes = (...b: number[]): Uint8Array => Uint8Array.from(b);

let mounted: MountedMidi | null = null;

afterEach(() => {
  mounted?.handle.dispose();
  mounted?.root.remove();
  mounted = null;
});

describe('parseCc', () => {
  it('maps the channel nibble to 1..16', () => {
    expect(parseCc(bytes(0xb0, 74, 100), 1)).toEqual({ cc: 74, ch: 1, value: 100, time: 1 });
    expect(parseCc(bytes(0xb9, 1, 0), 2)).toEqual({ cc: 1, ch: 10, value: 0, time: 2 });
    expect(parseCc(bytes(0xbf, 127, 127), 3)).toEqual({ cc: 127, ch: 16, value: 127, time: 3 });
  });

  it('rejects notes, clock, short and malformed messages', () => {
    expect(parseCc(bytes(0x90, 60, 100), 0)).toBeNull();
    expect(parseCc(bytes(0x80, 60, 0), 0)).toBeNull();
    expect(parseCc(bytes(0xf8), 0)).toBeNull();
    expect(parseCc(bytes(0xb0, 74), 0)).toBeNull();
    expect(parseCc(bytes(0xb0, 0x80, 1), 0)).toBeNull();
    expect(parseCc(bytes(0xe0, 0, 64), 0)).toBeNull();
  });
});

describe('CcStream', () => {
  it('delivers CC events to subscribers until unsubscribed', () => {
    const s = new CcStream();
    const got: MidiCcEvent[] = [];
    const off = s.onCc((ev) => got.push(ev));
    s.dispatch(bytes(0x90, 60, 100), 0.5);
    s.dispatch(bytes(0xb1, 10, 20), 1);
    off();
    s.dispatch(bytes(0xb1, 10, 21), 2);
    expect(got).toEqual([{ cc: 10, ch: 2, value: 20, time: 1 }]);
  });

  it('learnNext resolves with the next CC only', async () => {
    const s = new CcStream();
    let learned: { cc: number; ch: number } | null = null;
    const p = s.learnNext().then((v) => (learned = v));
    expect(s.learning).toBe(true);
    s.dispatch(bytes(0x90, 60, 100), 0);
    s.dispatch(bytes(0x80, 60, 0), 0);
    s.dispatch(bytes(0xf8), 0);
    await settle();
    expect(learned).toBeNull();
    s.dispatch(bytes(0xb4, 21, 99), 0);
    s.dispatch(bytes(0xb0, 22, 1), 0);
    await p;
    expect(learned).toEqual({ cc: 21, ch: 5 });
    expect(s.learning).toBe(false);
  });

  it('cancelLearn rejects the pending learn', async () => {
    const s = new CcStream();
    const p = s.learnNext();
    s.cancelLearn();
    await expect(p).rejects.toBeInstanceOf(LearnCancelled);
    expect(s.learning).toBe(false);
    // Nothing pending: a later CC and a second cancel are harmless.
    s.dispatch(bytes(0xb0, 1, 1), 0);
    s.cancelLearn();
  });

  it('a new learn supersedes a pending one', async () => {
    const s = new CcStream();
    const first = s.learnNext();
    const second = s.learnNext();
    s.dispatch(bytes(0xb2, 30, 5), 0);
    await expect(first).rejects.toBeInstanceOf(LearnCancelled);
    await expect(second).resolves.toEqual({ cc: 30, ch: 3 });
  });

  it('isolates a failing subscriber', () => {
    const s = new CcStream();
    const got: number[] = [];
    const orig = console.error;
    console.error = () => {};
    try {
      s.onCc(() => {
        throw new Error('boom');
      });
      s.onCc((ev) => got.push(ev.cc));
      s.dispatch(bytes(0xb0, 9, 9), 0);
    } finally {
      console.error = orig;
    }
    expect(got).toEqual([9]);
  });
});

describe('midi mount learn', () => {
  it('shows the learn indicator and learns from an active input only', async () => {
    const access = new FakeMIDIAccess();
    const keys = access.addInput('in-1', 'Keys', false);
    const pads = access.addInput('in-2', 'Pads', false);
    mounted = mountMidi(midiDeps('native'), { navigator: fakeNavigator(access), storage: new MemoryStorage() });
    await mounted.click();
    mounted.box('in-1')?.click();
    const indicator = mounted.root.querySelector<HTMLElement>('.midi-learn');
    expect(indicator?.hidden).toBe(true);

    const midi = mounted.deps.midi;
    if (!midi) throw new Error('no MidiApi');
    const p = midi.learnNext();
    expect(indicator?.hidden).toBe(false);
    pads.emit([0xb0, 50, 1], 0);
    keys.emit([0x90, 60, 100], 0);
    keys.emit([0xbc, 51, 2], 0);
    await expect(p).resolves.toEqual({ cc: 51, ch: 13 });
    expect(indicator?.hidden).toBe(true);

    const q = midi.learnNext();
    indicator?.querySelector('button')?.click();
    await expect(q).rejects.toBeInstanceOf(LearnCancelled);
    expect(indicator?.hidden).toBe(true);
  });
});

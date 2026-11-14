import { afterEach, describe, expect, it } from 'vitest';
import { VactrHost } from '../../worklet/host.js';
import type { MidiCcEvent } from '../../src/app/apis';
import { Forwarder, audioTime, forwardable, type OutputClock } from '../../src/midi/forward';
import { WasmCore } from '../../src/protocol/wasm';
import { FakeCore, fakeNode } from '../support/fake-core';
import { FakeMIDIAccess, MemoryStorage, fakeNavigator, midiDeps, mountMidi, type MountedMidi } from '../support/midi';

/** An output clock: context time 2 s at page time 1000 ms. */
const clock = (currentTime = 9): OutputClock => ({
  currentTime,
  getOutputTimestamp: () => ({ contextTime: 2, performanceTime: 1000 }),
});

function wasmCore(): { fake: FakeCore; core: WasmCore } {
  const fake = new FakeCore();
  const core = new WasmCore();
  core.attach(
    new VactrHost(null, fakeNode(), fake.exports, {
      wasmUrl: '',
      processorUrl: '',
      init: 'session',
      onRecord: core.onRecord,
    }),
  );
  return { fake, core };
}

const forwarded = (fake: FakeCore): [number[], unknown][] =>
  fake.callsOf('session_midi_in').map((c) => [[...(c.bytes ?? [])], c.args[2]]);

let mounted: MountedMidi | null = null;

afterEach(() => {
  mounted?.handle.dispose();
  mounted?.root.remove();
  mounted = null;
});

describe('audioTime', () => {
  it('converts page-clock milliseconds with getOutputTimestamp', () => {
    expect(audioTime(clock(), 1500)).toBeCloseTo(2.5);
    expect(audioTime(clock(), 900)).toBeCloseTo(1.9);
  });

  it('falls back to currentTime at receipt', () => {
    expect(audioTime({ currentTime: 4 }, 1500)).toBe(4);
    expect(audioTime({ currentTime: 4, getOutputTimestamp: () => ({ contextTime: 0, performanceTime: 0 }) }, 1500)).toBe(4);
    expect(audioTime(clock(7), Number.NaN)).toBe(7);
  });
});

describe('forwardable', () => {
  it('matches the session parse set', () => {
    for (const b of [[0x90, 60, 100], [0x81, 60, 0], [0xbf, 1, 2], [0xf8], [0xfa], [0xfb], [0xfc]]) {
      expect(forwardable(Uint8Array.from(b))).toBe(true);
    }
    for (const b of [[], [0xfe], [0xe0, 0, 64], [0xd0, 5], [0xf0, 1, 0xf7], [0x90, 60], [0x3c, 64]]) {
      expect(forwardable(Uint8Array.from(b))).toBe(false);
    }
  });

  it('Forwarder sends only forwardable messages', () => {
    const sent: [number[], number][] = [];
    const f = new Forwarder({ midiIn: (b, t) => sent.push([[...b], t]) }, clock());
    f.forward(Uint8Array.from([0xfe]), 1);
    f.forward(Uint8Array.from([0xf8]), f.time(1250));
    expect(sent).toEqual([[[0xf8], 2.25]]);
  });
});

describe('midi mount forwarding', () => {
  it('forwards CC, note and clock bytes from active inputs with converted times', async () => {
    const { fake, core } = wasmCore();
    const access = new FakeMIDIAccess();
    const keys = access.addInput('in-1', 'Keys', false);
    const pads = access.addInput('in-2', 'Pads', false);
    mounted = mountMidi(midiDeps('browser', core), {
      navigator: fakeNavigator(access),
      storage: new MemoryStorage(),
      ctx: clock(),
    });

    // No forwarding before access is granted.
    keys.emit([0xb0, 1, 1], 1100);
    expect(forwarded(fake)).toEqual([]);

    await mounted.click();
    expect(mounted.status.textContent).toContain('forwarding');
    mounted.box('in-1')?.click();
    const ccs: MidiCcEvent[] = [];
    mounted.deps.midi?.onCc((ev) => ccs.push(ev));

    keys.emit([0xb0, 74, 100], 1100);
    keys.emit([0x90, 60, 100], 1200);
    keys.emit([0x80, 60, 0], 1300);
    keys.emit([0xf8], 1400);
    keys.emit([0xfa], 1500);
    keys.emit([0xfb], 1600);
    keys.emit([0xfc], 1700);
    keys.emit([0xfe], 1800);
    keys.emit([0xe0, 0, 64], 1900);
    pads.emit([0xb0, 75, 1], 2000);

    const got = forwarded(fake);
    expect(got.map(([b]) => b)).toEqual([[0xb0, 74, 100], [0x90, 60, 100], [0x80, 60, 0], [0xf8], [0xfa], [0xfb], [0xfc]]);
    const times = got.map(([, t]) => t as number);
    [2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7].forEach((t, i) => expect(times[i]).toBeCloseTo(t));
    // CC events carry the same audio-clock time.
    expect(ccs).toHaveLength(1);
    expect(ccs[0]?.time).toBeCloseTo(2.1);
  });

  it('forwards nothing on the native tier', async () => {
    const { fake, core } = wasmCore();
    const access = new FakeMIDIAccess();
    const keys = access.addInput('in-1', 'Keys', false);
    mounted = mountMidi(midiDeps('native', core), {
      navigator: fakeNavigator(access),
      storage: new MemoryStorage(),
      ctx: clock(),
    });
    await mounted.click();
    mounted.box('in-1')?.click();
    const ccs: MidiCcEvent[] = [];
    mounted.deps.midi?.onCc((ev) => ccs.push(ev));
    keys.emit([0xb0, 74, 100], 1100);
    keys.emit([0x90, 60, 100], 1200);
    keys.emit([0xf8], 1300);
    expect(forwarded(fake)).toEqual([]);
    // Learn and CC routing still work.
    expect(ccs).toEqual([{ cc: 74, ch: 1, value: 100, time: 1.1 }]);
  });

  it('forwards nothing without a wasm core', async () => {
    const access = new FakeMIDIAccess();
    access.addInput('in-1', 'Keys', false);
    mounted = mountMidi(midiDeps('browser'), { navigator: fakeNavigator(access), storage: new MemoryStorage() });
    await mounted.click();
    expect(mounted.status.textContent).toBe('enabled');
  });
});

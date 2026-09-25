import { afterEach, describe, expect, it } from 'vitest';
import type { MidiCcEvent } from '../../src/app/apis';
import { DevicePicker, STORAGE_KEY, type ActiveMessage } from '../../src/midi/devices';
import { FakeMIDIAccess, MemoryStorage, fakeNavigator, midiDeps, mountMidi, type MountedMidi } from '../support/midi';

const names = (picker: DevicePicker): string[] =>
  [...picker.el.querySelectorAll('li')].map((li) => (li.textContent ?? '').trim());

const mountedList: MountedMidi[] = [];

afterEach(() => {
  for (const m of mountedList.splice(0)) {
    m.handle.dispose();
    m.root.remove();
  }
});

describe('DevicePicker', () => {
  it('updates the list on statechange', () => {
    const access = new FakeMIDIAccess();
    const picker = new DevicePicker(document, access, () => {}, new MemoryStorage());
    expect(names(picker)).toEqual(['no MIDI inputs']);

    const keys = access.addInput('in-1', 'Keys');
    access.addInput('in-2', 'Pads');
    expect(names(picker)).toEqual(['Keys', 'Pads']);

    access.removeInput('in-1');
    expect(names(picker)).toEqual(['Pads']);
    // A removed input no longer carries the picker's listener.
    expect(keys.listenerCount).toBe(0);

    picker.dispose();
    expect(access.inputs.get('in-2')?.listenerCount).toBe(0);
  });

  it('dispatches only the messages of active inputs', () => {
    const access = new FakeMIDIAccess();
    const keys = access.addInput('in-1', 'Keys');
    const pads = access.addInput('in-2', 'Pads');
    const got: ActiveMessage[] = [];
    const picker = new DevicePicker(document, access, (m) => got.push(m), new MemoryStorage());
    // A checkbox fires `change` only while connected.
    document.body.appendChild(picker.el);

    // Nothing is active until selected.
    keys.emit([0xb0, 1, 10], 5);
    expect(got).toEqual([]);

    const box = picker.el.querySelector<HTMLInputElement>('input[data-input-id="in-1"]');
    box?.click();
    expect(box?.checked).toBe(true);
    keys.emit([0xb0, 1, 11], 6);
    pads.emit([0xb0, 2, 12], 7);
    expect(got.map((m) => [m.input.id, [...m.bytes], m.timeStamp])).toEqual([['in-1', [0xb0, 1, 11], 6]]);

    picker.setActive(keys, false);
    keys.emit([0xb0, 1, 13], 8);
    expect(got).toHaveLength(1);
    picker.dispose();
  });

  it('persists the selection by input name', () => {
    const storage = new MemoryStorage();
    const access = new FakeMIDIAccess();
    const keys = access.addInput('in-1', 'Keys');
    const picker = new DevicePicker(document, access, () => {}, storage);
    picker.setActive(keys, true);
    expect(JSON.parse(storage.getItem(STORAGE_KEY) ?? '[]')).toEqual(['Keys']);
    picker.dispose();

    // The same device under a new port id is still active.
    const again = new FakeMIDIAccess();
    const keys2 = again.addInput('in-9', 'Keys');
    const picker2 = new DevicePicker(document, again, () => {}, storage);
    expect(picker2.isActive(keys2)).toBe(true);
    picker2.dispose();
  });

  it('tolerates unreadable storage', () => {
    const storage = new MemoryStorage();
    storage.setItem(STORAGE_KEY, '{not json');
    const access = new FakeMIDIAccess();
    const keys = access.addInput('in-1', 'Keys');
    const picker = new DevicePicker(document, access, () => {}, storage);
    expect(picker.isActive(keys)).toBe(false);
    picker.dispose();
  });
});

describe('midi mount device selection', () => {
  it('keeps the selection across a remount', async () => {
    const storage = new MemoryStorage();
    const access = new FakeMIDIAccess();
    const keys = access.addInput('in-1', 'Keys', false);
    access.addInput('in-2', 'Pads', false);

    const first = mountMidi(midiDeps('native'), { navigator: fakeNavigator(access), storage });
    mountedList.push(first);
    await first.click();
    first.box('in-1')?.click();
    first.handle.dispose();
    first.root.remove();
    mountedList.pop();

    const second = mountMidi(midiDeps('native'), { navigator: fakeNavigator(access), storage });
    mountedList.push(second);
    await second.click();
    expect(second.box('in-1')?.checked).toBe(true);
    expect(second.box('in-2')?.checked).toBe(false);

    const events: MidiCcEvent[] = [];
    second.deps.midi?.onCc((ev) => events.push(ev));
    keys.emit([0xb3, 7, 64], 2500);
    access.inputs.get('in-2')?.emit([0xb3, 8, 1], 2600);
    // Native tier: CC times are page-clock seconds.
    expect(events).toEqual([{ cc: 7, ch: 4, value: 64, time: 2.5 }]);
  });
});

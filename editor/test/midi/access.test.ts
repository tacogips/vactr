import { afterEach, describe, expect, it } from 'vitest';
import { NOT_AVAILABLE, requestMidi } from '../../src/midi/access';
import { FakeMIDIAccess, MemoryStorage, fakeNavigator, midiDeps, mountMidi, type MountedMidi } from '../support/midi';

let mounted: MountedMidi | null = null;

afterEach(() => {
  mounted?.handle.dispose();
  mounted?.root.remove();
  mounted = null;
});

describe('requestMidi', () => {
  it('requests input access without sysex', async () => {
    const access = new FakeMIDIAccess();
    const nav = fakeNavigator(access);
    expect(await requestMidi(nav)).toBe(access);
    expect(nav.requests).toEqual([{ sysex: false }]);
  });

  it('yields null when the API is absent or the request is refused', async () => {
    expect(await requestMidi(fakeNavigator('absent'))).toBeNull();
    expect(await requestMidi(fakeNavigator('reject'))).toBeNull();
  });
});

describe('midi mount access', () => {
  it('requests nothing before the Enable MIDI click', async () => {
    const access = new FakeMIDIAccess();
    access.addInput('in-1', 'Keys', false);
    const nav = fakeNavigator(access);
    mounted = mountMidi(midiDeps(), { navigator: nav, storage: new MemoryStorage() });
    await Promise.resolve();
    expect(nav.requests).toEqual([]);
    expect(mounted.deps.midi).toBeUndefined();
    expect(mounted.root.querySelector('.midi-devices')).toBeNull();

    await mounted.click();
    expect(nav.requests).toEqual([{ sysex: false }]);
    expect(mounted.deps.midi).toBeDefined();
    expect(mounted.enable.hidden).toBe(true);
    expect(mounted.box('in-1')).not.toBeNull();

    // A second click requests nothing more.
    await mounted.click();
    expect(nav.requests).toHaveLength(1);
  });

  it('shows the not-available text when the API is absent', async () => {
    mounted = mountMidi(midiDeps(), { navigator: fakeNavigator('absent'), storage: new MemoryStorage() });
    await mounted.click();
    expect(mounted.status.textContent).toBe(NOT_AVAILABLE);
    expect(mounted.deps.midi).toBeUndefined();
  });

  it('shows the same text when the request is refused', async () => {
    const nav = fakeNavigator('reject');
    mounted = mountMidi(midiDeps(), { navigator: nav, storage: new MemoryStorage() });
    await mounted.click();
    expect(nav.requests).toEqual([{ sysex: false }]);
    expect(mounted.status.textContent).toBe(NOT_AVAILABLE);
    expect(mounted.deps.midi).toBeUndefined();
    expect(mounted.enable.disabled).toBe(false);
  });

  it('clears deps.midi and the pane on dispose', async () => {
    mounted = mountMidi(midiDeps(), { navigator: fakeNavigator(new FakeMIDIAccess()), storage: new MemoryStorage() });
    await mounted.click();
    expect(mounted.deps.midi).toBeDefined();
    mounted.handle.dispose();
    expect(mounted.deps.midi).toBeUndefined();
    expect(mounted.root.querySelector('[data-area="midi"]')).toBeNull();
    mounted = null;
  });

  it('ignores a grant that arrives after dispose', async () => {
    const deps = midiDeps();
    mounted = mountMidi(deps, { navigator: fakeNavigator(new FakeMIDIAccess()), storage: new MemoryStorage() });
    mounted.enable.click();
    mounted.handle.dispose();
    mounted = null;
    for (let i = 0; i < 5; i++) await Promise.resolve();
    expect(deps.midi).toBeUndefined();
  });
});

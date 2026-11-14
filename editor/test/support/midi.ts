// A fake Web MIDI access for the `midi` area tests. `FakeMIDIAccess` holds
// `FakeMIDIInput`s; `input.emit(bytes, timeStamp)` delivers a `midimessage`
// and `addInput`/`removeInput` change the port list and fire `statechange`.
// `fakeNavigator` records every `requestMIDIAccess` call.

import type { EditorDeps, Mounted, Tier } from '../../src/app/deps';
import type { MidiAccessLike, MidiInputLike, MidiMessageLike, MidiNavigator } from '../../src/midi/access';
import { mount, type MidiMountOptions } from '../../src/midi/mount';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { WasmCore } from '../../src/protocol/wasm';
import { MockClock } from './clock';
import { RecordingTransport } from './recording';

type MessageListener = (ev: MidiMessageLike) => void;

export class FakeMIDIInput implements MidiInputLike {
  readonly id: string;
  readonly name: string | null;
  state = 'connected';
  private readonly listeners: MessageListener[] = [];

  constructor(id: string, name: string | null) {
    this.id = id;
    this.name = name;
  }

  addEventListener(_type: 'midimessage', listener: MessageListener): void {
    this.listeners.push(listener);
  }

  removeEventListener(_type: 'midimessage', listener: MessageListener): void {
    const i = this.listeners.indexOf(listener);
    if (i >= 0) this.listeners.splice(i, 1);
  }

  get listenerCount(): number {
    return this.listeners.length;
  }

  emit(bytes: number[] | Uint8Array, timeStamp = 0): void {
    const ev = { data: Uint8Array.from(bytes), timeStamp };
    for (const l of this.listeners.slice()) l(ev);
  }
}

export class FakeMIDIAccess implements MidiAccessLike {
  readonly inputs = new Map<string, FakeMIDIInput>();
  private readonly stateListeners: (() => void)[] = [];

  addInput(id: string, name: string | null, fire = true): FakeMIDIInput {
    const input = new FakeMIDIInput(id, name);
    this.inputs.set(id, input);
    if (fire) this.statechange();
    return input;
  }

  removeInput(id: string): void {
    this.inputs.delete(id);
    this.statechange();
  }

  addEventListener(_type: 'statechange', listener: () => void): void {
    this.stateListeners.push(listener);
  }

  removeEventListener(_type: 'statechange', listener: () => void): void {
    const i = this.stateListeners.indexOf(listener);
    if (i >= 0) this.stateListeners.splice(i, 1);
  }

  statechange(): void {
    for (const l of this.stateListeners.slice()) l();
  }
}

export interface FakeNavigator extends MidiNavigator {
  requests: { sysex: boolean }[];
}

/** A navigator whose `requestMIDIAccess` grants `access`, rejects, or is absent. */
export function fakeNavigator(result: FakeMIDIAccess | 'reject' | 'absent'): FakeNavigator {
  const nav: FakeNavigator = { requests: [] };
  if (result === 'absent') return nav;
  nav.requestMIDIAccess = (opts) => {
    nav.requests.push(opts);
    return result === 'reject'
      ? Promise.reject(new DOMException('denied', 'SecurityError'))
      : Promise.resolve(result);
  };
  return nav;
}

/** In-memory `localStorage` stand-in. */
export class MemoryStorage {
  readonly items = new Map<string, string>();

  getItem(key: string): string | null {
    return this.items.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.items.set(key, value);
  }
}

/** Lets pending promise callbacks run. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
}

/** Editor dependencies for mounting the `midi` area alone. */
export function midiDeps(tier: Tier = 'browser', core?: WasmCore): EditorDeps {
  const store = new Store();
  return {
    client: new Client(new RecordingTransport(), { store }),
    store,
    clock: new MockClock(),
    tier,
    files: new MemoryFiles(),
    core,
  };
}

export interface MountedMidi {
  root: HTMLElement;
  deps: EditorDeps;
  handle: Mounted;
  enable: HTMLButtonElement;
  status: HTMLElement;
  /** Clicks "Enable MIDI" and lets the request settle. */
  click(): Promise<void>;
  /** The picker checkbox of input `id`. */
  box(id: string): HTMLInputElement | null;
}

export function mountMidi(deps: EditorDeps, opts: MidiMountOptions): MountedMidi {
  const root = document.createElement('div');
  document.body.appendChild(root);
  const handle = mount(root, deps, opts);
  const enable = root.querySelector<HTMLButtonElement>('.midi-enable');
  const status = root.querySelector<HTMLElement>('.midi-status');
  if (!enable || !status) throw new Error('midi pane not rendered');
  return {
    root,
    deps,
    handle,
    enable,
    status,
    async click() {
      enable.click();
      await settle();
    },
    box: (id) => root.querySelector<HTMLInputElement>(`input[data-input-id="${id}"]`),
  };
}

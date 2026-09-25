// The MIDI input device picker (design 15.1.9). It lists the granted
// access's inputs by name, refreshed on `statechange`; a checkbox per
// input selects the ACTIVE inputs, persisted in `localStorage` by input
// name. Only messages from active inputs reach `onMessage`, which feeds
// learn and CC routing (both tiers) and forwarding (browser tier).

import type { MidiAccessLike, MidiInputLike, MidiMessageLike } from './access';

export const STORAGE_KEY = 'vactrol.midi.active-inputs';

export interface ActiveMessage {
  input: MidiInputLike;
  bytes: Uint8Array;
  /** Milliseconds on the page clock. */
  timeStamp: number;
}

export type StorageLike = Pick<Storage, 'getItem' | 'setItem'>;

function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

/** The persistence key of an input: its name, or its id when unnamed. */
export function inputKey(input: MidiInputLike): string {
  return input.name || input.id;
}

interface Attached {
  input: MidiInputLike;
  listener: (ev: MidiMessageLike) => void;
}

export class DevicePicker {
  readonly el: HTMLElement;
  private readonly access: MidiAccessLike;
  private readonly storage: StorageLike | null;
  private readonly onMessage: (msg: ActiveMessage) => void;
  private readonly attached = new Map<string, Attached>();
  private readonly active: Set<string>;
  private readonly onState = (): void => this.refresh();
  private disposed = false;

  constructor(
    doc: Document,
    access: MidiAccessLike,
    onMessage: (msg: ActiveMessage) => void,
    storage: StorageLike | null = defaultStorage(),
  ) {
    this.access = access;
    this.storage = storage;
    this.onMessage = onMessage;
    this.active = new Set(this.load());
    this.el = doc.createElement('ul');
    this.el.className = 'midi-devices';
    access.addEventListener('statechange', this.onState);
    this.refresh();
  }

  /** The connected inputs in the access's order. */
  inputs(): MidiInputLike[] {
    const out: MidiInputLike[] = [];
    this.access.inputs.forEach((input) => {
      if (input.state !== 'disconnected') out.push(input);
    });
    return out;
  }

  isActive(input: MidiInputLike): boolean {
    return this.active.has(inputKey(input));
  }

  setActive(input: MidiInputLike, on: boolean): void {
    const key = inputKey(input);
    if (on) this.active.add(key);
    else this.active.delete(key);
    this.save();
    this.render();
  }

  /** Re-lists the inputs and re-attaches message listeners. */
  refresh(): void {
    if (this.disposed) return;
    const current = new Map(this.inputs().map((i) => [i.id, i]));
    for (const [id, a] of this.attached) {
      if (current.get(id) !== a.input) {
        a.input.removeEventListener('midimessage', a.listener);
        this.attached.delete(id);
      }
    }
    for (const [id, input] of current) {
      if (this.attached.has(id)) continue;
      const listener = (ev: MidiMessageLike): void => this.receive(input, ev);
      input.addEventListener('midimessage', listener);
      this.attached.set(id, { input, listener });
    }
    this.render();
  }

  dispose(): void {
    this.disposed = true;
    this.access.removeEventListener('statechange', this.onState);
    for (const a of this.attached.values()) a.input.removeEventListener('midimessage', a.listener);
    this.attached.clear();
    this.el.remove();
  }

  private receive(input: MidiInputLike, ev: MidiMessageLike): void {
    if (this.disposed || !ev.data || !this.isActive(input)) return;
    this.onMessage({ input, bytes: ev.data, timeStamp: ev.timeStamp });
  }

  private render(): void {
    const doc = this.el.ownerDocument;
    const inputs = this.inputs();
    this.el.replaceChildren();
    if (inputs.length === 0) {
      const li = doc.createElement('li');
      li.className = 'midi-devices-empty';
      li.textContent = 'no MIDI inputs';
      this.el.appendChild(li);
      return;
    }
    for (const input of inputs) {
      const li = doc.createElement('li');
      const label = doc.createElement('label');
      const box = doc.createElement('input');
      box.type = 'checkbox';
      box.dataset.inputId = input.id;
      box.checked = this.isActive(input);
      box.addEventListener('change', () => this.setActive(input, box.checked));
      label.append(box, ` ${inputKey(input)}`);
      li.appendChild(label);
      this.el.appendChild(li);
    }
  }

  private load(): string[] {
    try {
      const raw = this.storage?.getItem(STORAGE_KEY);
      const parsed: unknown = raw ? JSON.parse(raw) : [];
      return Array.isArray(parsed) ? parsed.filter((n): n is string => typeof n === 'string') : [];
    } catch {
      return [];
    }
  }

  private save(): void {
    try {
      this.storage?.setItem(STORAGE_KEY, JSON.stringify([...this.active].sort()));
    } catch {
      // Storage refused (quota or privacy mode): the selection lasts this session.
    }
  }
}

// WebMIDI access (design 15.1.9). `requestMidi` is called only from the
// "Enable MIDI" button handler, never on load: the permission prompt must
// follow a user action. Sysex is never requested. Where the API is absent
// (the Tauri WKWebView, 15.1.1) or the request is refused, the result is
// null and the pane shows `NOT_AVAILABLE`.
//
// The `*Like` interfaces are the structural subset of the DOM Web MIDI
// types the editor uses, so tests drive a `FakeMIDIAccess`.

export const NOT_AVAILABLE = 'MIDI not available on this host';

export interface MidiMessageLike {
  readonly data: Uint8Array | null;
  /** Milliseconds on the page (performance) clock. */
  readonly timeStamp: number;
}

export interface MidiInputLike {
  readonly id: string;
  readonly name: string | null;
  readonly state?: string;
  addEventListener(type: 'midimessage', listener: (ev: MidiMessageLike) => void): void;
  removeEventListener(type: 'midimessage', listener: (ev: MidiMessageLike) => void): void;
}

export interface MidiAccessLike {
  readonly inputs: {
    forEach(cb: (input: MidiInputLike, key: string) => void): void;
  };
  addEventListener(type: 'statechange', listener: () => void): void;
  removeEventListener(type: 'statechange', listener: () => void): void;
}

export type RequestMidiAccess = (opts: { sysex: boolean }) => Promise<MidiAccessLike>;

export interface MidiNavigator {
  requestMIDIAccess?: RequestMidiAccess;
}

function defaultNavigator(): MidiNavigator {
  return typeof navigator === 'undefined' ? {} : (navigator as unknown as MidiNavigator);
}

/** Requests MIDI input access without sysex; null when unavailable or refused. */
export async function requestMidi(nav: MidiNavigator = defaultNavigator()): Promise<MidiAccessLike | null> {
  const request = nav.requestMIDIAccess;
  if (typeof request !== 'function') return null;
  try {
    return await request.call(nav, { sysex: false });
  } catch {
    return null;
  }
}

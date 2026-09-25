// The `midi` area (design 15.1.3, 15.1.9): the MIDI section of the right
// pane with the "Enable MIDI" button, the input device picker and the learn
// indicator. Access is requested only when the button is clicked. Once it
// is granted, `deps.midi` (the `CcStream`) is set for ED-BIND; until then
// it stays undefined, which consumers treat as MIDI unavailable. On the
// browser tier every active-input message the session consumes is also
// forwarded to `core.midiIn` at its audio-clock time; the native tier
// forwards nothing (the session's own midir input serves the language).

import type { EditorDeps, Mounted } from '../app/deps';
import { buildLayout } from '../app/layout';
import { NOT_AVAILABLE, requestMidi, type MidiAccessLike, type MidiNavigator } from './access';
import { DevicePicker, type ActiveMessage, type StorageLike } from './devices';
import { Forwarder, type OutputClock } from './forward';
import { CcStream } from './learn';

// The pane's stylesheet as a Vite asset URL. A side-effect `import
// './midi.css'` fails `tsc` (TS2882) without a CSS module declaration,
// which no editor plan owns.
const STYLESHEET = new URL('./midi.css', import.meta.url).href;

function addStylesheet(doc: Document): HTMLLinkElement | null {
  if (doc.head.querySelector('link[data-style="midi"]')) return null;
  const link = doc.createElement('link');
  link.rel = 'stylesheet';
  link.href = STYLESHEET;
  link.dataset.style = 'midi';
  doc.head.appendChild(link);
  return link;
}

/** Test seams; the defaults are the page's navigator, localStorage and audio context. */
export interface MidiMountOptions {
  navigator?: MidiNavigator;
  storage?: StorageLike | null;
  /** The browser tier's audio context; defaults to `deps.core.host.ctx`. */
  ctx?: OutputClock;
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: MidiMountOptions = {}): Mounted {
  const doc = root.ownerDocument;
  const stylesheet = addStylesheet(doc);
  const section = doc.createElement('section');
  section.className = 'midi-pane';
  section.dataset.area = 'midi';

  const header = doc.createElement('div');
  header.className = 'midi-header';
  const title = doc.createElement('span');
  title.className = 'midi-title';
  title.textContent = 'MIDI';
  const enable = doc.createElement('button');
  enable.type = 'button';
  enable.className = 'midi-enable';
  enable.textContent = 'Enable MIDI';
  const status = doc.createElement('span');
  status.className = 'midi-status';
  header.append(title, enable, status);

  const learn = doc.createElement('div');
  learn.className = 'midi-learn';
  learn.hidden = true;
  const learnText = doc.createElement('span');
  learnText.textContent = 'Learning: move a controller';
  const learnCancel = doc.createElement('button');
  learnCancel.type = 'button';
  learnCancel.textContent = 'Cancel';
  learn.append(learnText, learnCancel);

  section.append(header, learn);
  buildLayout(root).right.appendChild(section);

  let picker: DevicePicker | null = null;
  let stream: CcStream | null = null;
  let offLearn: (() => void) | null = null;
  let requesting = false;
  let disposed = false;

  const forwarder = (): Forwarder | null => {
    if (deps.tier !== 'browser' || !deps.core) return null;
    const ctx = opts.ctx ?? deps.core.host.ctx;
    return new Forwarder(deps.core, ctx);
  };

  const grant = (access: MidiAccessLike): void => {
    const cc = new CcStream();
    const fwd = forwarder();
    const onMessage = (msg: ActiveMessage): void => {
      const time = fwd ? fwd.time(msg.timeStamp) : msg.timeStamp / 1000;
      cc.dispatch(msg.bytes, time);
      fwd?.forward(msg.bytes, time);
    };
    picker = new DevicePicker(doc, access, onMessage, opts.storage);
    stream = cc;
    offLearn = cc.onLearnState((on) => {
      learn.hidden = !on;
    });
    learnCancel.addEventListener('click', () => cc.cancelLearn());
    section.insertBefore(picker.el, learn);
    enable.hidden = true;
    status.textContent = fwd ? 'enabled (forwarding to the session)' : 'enabled';
    deps.midi = cc;
  };

  // The only path to a permission prompt: a click on the button.
  enable.addEventListener('click', () => {
    if (requesting || stream) return;
    requesting = true;
    enable.disabled = true;
    status.textContent = 'requesting access';
    void requestMidi(opts.navigator).then((access) => {
      requesting = false;
      enable.disabled = false;
      if (disposed) return;
      if (access) grant(access);
      else status.textContent = NOT_AVAILABLE;
    });
  });

  return {
    dispose() {
      disposed = true;
      offLearn?.();
      stream?.cancelLearn();
      picker?.dispose();
      if (stream && deps.midi === stream) deps.midi = undefined;
      section.remove();
      stylesheet?.remove();
    },
  };
}

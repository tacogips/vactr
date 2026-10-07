import type { InstrumentSelector, SongSound } from '../protocol/types';
import { songKey, songSelectorKey } from '../protocol/store';
import type { SongControls } from './apis';
import type { EditorDeps } from './deps';
import { pane } from './layout';

function sameSound(a: SongSound, b: SongSound): boolean {
  if (a.kind !== b.kind) return false;
  switch (a.kind) {
    case 'builtin': return b.kind === 'builtin' && a.name === b.name;
    case 'instrument': return b.kind === 'instrument' && a.id === b.id;
    case 'buffer': return b.kind === 'buffer' && a.id === b.id;
    case 'sample': return b.kind === 'sample' && a.path === b.path && (a.file ?? null) === (b.file ?? null);
  }
}
function soundLabel(sound: SongSound, index: number): string {
  switch (sound.kind) {
    case 'builtin': return sound.name;
    case 'instrument': return `Instrument ${index + 1}`;
    case 'sample': return sound.path.split('/').pop() || sound.path;
    case 'buffer': return 'Sample buffer';
  }
}

/** Whole-code song controls. Editing never applies a song automatically. */
export function mount(root: HTMLElement, deps: EditorDeps, file = 'main.vact'): SongControls {
  const doc = root.ownerDocument;
  const panel = doc.createElement('section');
  panel.dataset.songControls = '';
  panel.setAttribute('aria-label', 'Song');
  const apply = doc.createElement('button');
  apply.type = 'button';
  apply.className = 'song-apply vact-primary';
  apply.textContent = 'Apply song';
  const status = doc.createElement('span');
  status.setAttribute('role', 'status');
  const failure = doc.createElement('span');
  failure.setAttribute('role', 'alert');
  const instruments = doc.createElement('div');
  instruments.setAttribute('aria-label', 'Song instruments');
  panel.append(apply, status, failure, instruments);
  pane(root, 'transport').appendChild(panel);
  let disposed = false;
  let error = '';
  const pendingMutes = new Set<string>();

  function refresh(): void {
    if (disposed) return;
    const state = deps.store.song(file);
    apply.disabled = !deps.code || !!state?.pending;
    status.textContent = state?.pending
      ? `${state.pending.ready ? 'Prepared' : 'Preparing'} revision ${state.pending.revision}`
      : state?.applied
        ? `${state.transport?.state ?? 'playing'} revision ${state.applied.doc_revision}${state.draftRevision > state.applied.doc_revision ? ' — draft changed' : ''}`
        : 'No song applied';
    failure.textContent = error || state?.failure?.message || '';
    instruments.replaceChildren();
    for (const [index, selector] of (state?.transport?.instruments ?? []).entries()) {
      const button = doc.createElement('button');
      button.type = 'button';
      const muted = selector.family.every((sound) => state?.instrumentMutes?.some((mute) => sameSound(sound, mute.sound) && mute.muted));
      const label = selector.family.map((sound) => soundLabel(sound, index)).join(', ');
      button.textContent = `${muted ? 'Unmute' : 'Mute'} ${label}`;
      button.setAttribute('aria-pressed', String(muted));
      button.disabled = pendingMutes.has(songSelectorKey(selector)) || !state?.applied ||
        state.transport?.state === 'ended' || state.transport?.state === 'failed';
      button.addEventListener('click', () => void controls.muteInstrument(selector, !muted).catch(() => {}));
      instruments.appendChild(button);
    }
  }

  const controls: SongControls = {
    async applyWholeCode() {
      if (disposed || !deps.code) throw new Error('Code editor unavailable');
      error = '';
      try {
        const state = deps.code.surface.state;
        const response = await deps.client.applySong(file, state.doc.toString());
        if (response.kind === 'song-candidate-failed') throw new Error(response.body.message);
        if (response.kind !== 'song-candidate-ready' && response.kind !== 'song-candidate-applied')
          throw new Error('Song application was not acknowledged');
      } catch (cause) {
        error = cause instanceof Error ? cause.message : String(cause);
        throw cause;
      } finally { refresh(); }
    },
    async muteInstrument(selector, muted) {
      const epoch = deps.store.song(file)?.applied?.epoch;
      if (disposed || !epoch) throw new Error('No active song');
      const key = songSelectorKey(selector);
      pendingMutes.add(key);
      error = '';
      refresh();
      try {
        const response = await deps.client.muteInstrument(epoch, selector, muted);
        if (response.kind === 'song-candidate-failed') throw new Error(response.body.message);
        if (response.kind !== 'song-instrument-muted' || response.body.epoch !== epoch)
          throw new Error('Instrument mute was not acknowledged');
      } catch (cause) {
        error = cause instanceof Error ? cause.message : String(cause);
        throw cause;
      } finally { pendingMutes.delete(key); refresh(); }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      unsubscribe();
      stopObserving();
      panel.remove();
      if (deps.song === controls) delete deps.song;
    },
  };
  apply.addEventListener('click', () => void controls.applyWholeCode().catch(() => {}));
  const unsubscribe = deps.store.subscribe([songKey(file)], refresh);
  function documentChanged(): void {
    const revision = deps.client.document(file).revision;
    const pending = deps.store.song(file)?.pending;
    deps.store.songDocumentChanged(file, revision);
    // Send pending candidate invalidation immediately rather than after debounce.
    if (pending && revision !== pending.revision) deps.client.document(file).flush();
  }
  let stopObserving: () => void = () => {};
  if (deps.code) stopObserving = deps.code.surface.subscribe((update) => { if (update.docChanged) documentChanged(); });
  deps.song = controls;
  refresh();
  return controls;
}

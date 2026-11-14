import { createComponent, createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import type { EditorDeps, Mounted } from '../app/deps';
import { pane } from '../app/layout';
import { FileCancelled } from '../platform/files';
import { PickerView, ensurePickerStylesheet } from '../ui/picker-view';
import { DraftAutosave } from './autosave';
import { DocumentSession } from './session';
import { applyStartup, resolveStartup } from './startup';
import type { DefaultTemplate, DocumentsOptions, LibraryView, PickerActions, StorageResult } from './types';

export interface DocumentsArea extends Mounted { ready: Promise<void> }

function failureMessage(result: StorageResult<unknown>): string {
  if (result.ok) return '';
  if (result.message) return result.message;
  switch (result.reason) {
    case 'name-taken': return 'A saved document with this name already exists';
    case 'invalid-name': return 'Use 1-80 characters without control characters';
    case 'too-large': return 'Document too large for browser storage; use Save file';
    case 'quota': return 'Browser storage is full';
    case 'unavailable': return 'Browser storage is unavailable';
    case 'missing': return 'Saved document not found';
    default: return 'Could not save document';
  }
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: DocumentsOptions): DocumentsArea {
  if (!deps.code) return { ready: Promise.resolve(), dispose() {} };

  const doc = root.ownerDocument;
  const win = doc.defaultView;
  const surface = deps.code.surface;
  const session = new DocumentSession(surface);
  const [open, setOpen] = createSignal(false);
  const [library, setLibrary] = createSignal<LibraryView>(opts.shell === 'web' && opts.store.available
    ? { available: true, ...opts.store.list() }
    : { available: false, entries: [], unreadable: 0 });
  const [template, setTemplate] = createSignal<DefaultTemplate | null>(null);
  const [status, setStatus] = createSignal(opts.shell === 'web' && !opts.store.available
    ? 'Browser storage is unavailable; saving to this browser is disabled' : '');
  const [pendingReplace, setPendingReplace] = createSignal<string | null>(null);
  let pendingLoad: (() => Promise<{ text: string; source: import('./types').DocSource }>) | null = null;
  let disposed = false;
  const refreshLibrary = () => {
    if (opts.shell === 'web' && opts.store.available) setLibrary({ available: true, ...opts.store.list() });
  };
  const autosave = opts.shell === 'web' && opts.store.available
    ? new DraftAutosave({ surface, session, drafts: opts.store, win, notice: setStatus })
    : null;
  const section = doc.createElement('section');
  section.dataset.docPicker = '';
  section.setAttribute('aria-label', 'Documents');
  pane(root, 'transport').appendChild(section);
  ensurePickerStylesheet(doc);

  const runReplacement = async (load: () => Promise<{ text: string; source: import('./types').DocSource }>) => {
    try {
      const result = await load();
      session.replace(result.text, result.source);
      setPendingReplace(null);
      pendingLoad = null;
      setOpen(false);
      surface.focus();
      refreshLibrary();
    } catch (error) {
      if (error instanceof FileCancelled) {
        setPendingReplace(null);
        pendingLoad = null;
        return;
      }
      setStatus(error instanceof Error ? error.message : String(error));
      setPendingReplace(null);
      pendingLoad = null;
    }
  };
  const replaceWith = (load: () => Promise<{ text: string; source: import('./types').DocSource }>) => {
    if (session.dirty()) {
      pendingLoad = load;
      setPendingReplace('Replace the current document? Undo (Mod-Z) restores it.');
      return;
    }
    void runReplacement(load);
  };
  const currentText = () => surface.state.doc.toString();
  const actions: PickerActions = {
    toggle() { const next = !open(); setOpen(next); if (next) refreshLibrary(); },
    close() { setOpen(false); setPendingReplace(null); pendingLoad = null; },
    openExample(id) {
      replaceWith(async () => ({ text: await opts.examples.load(id), source: { kind: 'example', id } }));
    },
    openLibrary(id) {
      replaceWith(async () => {
        const entry = opts.store.read(id);
        if (!entry) throw new Error('Saved document not found');
        return { text: entry.text, source: { kind: 'library', id, name: entry.name } };
      });
    },
    openFile() {
      replaceWith(async () => {
        const file = await deps.files.open();
        return { text: file.text, source: { kind: 'file', name: file.name } };
      });
    },
    newDocument() {
      replaceWith(async () => {
        const selected = template();
        if (selected?.kind === 'snapshot') return { text: selected.text, source: { kind: 'template' } };
        const id = selected?.kind === 'example' ? selected.id : opts.examples.defaultId;
        if (!opts.examples.find(id)) {
          if (selected?.kind === 'example') setStatus(`Default template example ${id} is no longer available`);
          const fallback = opts.examples.defaultId;
          return { text: await opts.examples.load(fallback), source: { kind: 'example', id: fallback } };
        }
        return { text: await opts.examples.load(id), source: { kind: 'example', id } };
      });
    },
    save() {
      const source = session.source;
      if (source.kind !== 'library' || !opts.store.read(source.id)) { setStatus('Saved document not found; use Save as'); return; }
      const result = opts.store.update(source.id, currentText());
      if (result.ok) { session.markSaved(source); setStatus(`Saved ${source.name}`); refreshLibrary(); }
      else setStatus(failureMessage(result));
    },
    saveAs(name) {
      const result = opts.store.create(name, currentText());
      if (!result.ok) return failureMessage(result);
      session.markSaved({ kind: 'library', id: result.value.id, name: result.value.name });
      setStatus(`Saved ${result.value.name}`);
      refreshLibrary();
      return null;
    },
    rename(id, name) {
      const result = opts.store.rename(id, name);
      if (!result.ok) return failureMessage(result);
      if (session.source.kind === 'library' && session.source.id === id)
        session.setSource({ kind: 'library', id, name: result.value.name });
      setStatus(`Renamed ${result.value.name}`);
      refreshLibrary();
      return null;
    },
    remove(id) {
      const result = opts.store.remove(id);
      if (!result.ok) setStatus(failureMessage(result));
      else {
        if (session.source.kind === 'library' && session.source.id === id) session.setSource({ kind: 'new' });
        setStatus('Saved document deleted');
      }
      refreshLibrary();
    },
    useAsTemplate() {
      const source = session.source;
      const value: DefaultTemplate = source.kind === 'example' && !session.dirty()
        ? { kind: 'example', id: source.id }
        : { kind: 'snapshot', name: source.kind === 'library' ? source.name
          : source.kind === 'file' ? source.name.split(/[\\/]/).pop() || source.name
          : source.kind === 'example' ? opts.examples.find(source.id)?.title ?? source.id : 'Untitled', text: currentText() };
      void opts.templates.set(value).then((result) => {
        if (result.ok) { setTemplate(value); setStatus('Default template set'); }
        else setStatus(failureMessage(result));
      }).catch((error: unknown) => setStatus(error instanceof Error ? error.message : String(error)));
    },
    resetTemplate() {
      void opts.templates.reset().then((result) => {
        if (result.ok) { setTemplate(null); setStatus('Default template reset'); }
        else setStatus(failureMessage(result));
      }).catch((error: unknown) => setStatus(error instanceof Error ? error.message : String(error)));
    },
    confirmReplace() { const load = pendingLoad; if (load) void runReplacement(load); },
    cancelReplace() { pendingLoad = null; setPendingReplace(null); },
  };

  const disposeRender = render(() => createComponent(PickerView, {
    shell: opts.shell, open, examples: opts.examples.list(), library,
    templateExampleId: () => template()?.kind === 'example' ? (template() as { kind: 'example'; id: string }).id : null,
    hasTemplate: () => template() !== null,
    canSave: () => opts.shell === 'web' && opts.store.available,
    saveTarget: () => session.source.kind === 'library' && opts.store.read(session.source.id) ? session.source.name : null,
    status, pendingReplace, actions,
  }), section);
  const ready = (async () => {
    try {
      const startup = await resolveStartup({ shell: opts.shell, search: opts.search, examples: opts.examples,
        drafts: opts.shell === 'web' && opts.store.available ? opts.store : null,
        templates: opts.templates, notice: setStatus });
      if (startup) applyStartup(session, startup, setStatus);
      setTemplate(await opts.templates.get());
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  })();
  return {
    ready,
    dispose() {
      if (disposed) return;
      disposed = true;
      autosave?.dispose();
      session.dispose();
      disposeRender();
      section.remove();
    },
  };
}

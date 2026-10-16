import { For, Show, createEffect, createMemo, createSignal, onCleanup, type JSX } from 'solid-js';
import type { ExampleEntry, LibraryEntry, PickerProps } from '../documents/types';

const STYLESHEET = new URL('./picker.css', import.meta.url).href;

export function ensurePickerStylesheet(doc: Document): void {
  if (doc.querySelector('link[data-vact-picker-css]')) return;
  const link = doc.createElement('link');
  link.rel = 'stylesheet';
  link.href = STYLESHEET;
  link.dataset.vactPickerCss = '';
  doc.head.appendChild(link);
}

type PickerRow = { kind: 'library'; entry: LibraryEntry } | { kind: 'example'; entry: ExampleEntry };
type InlineForm = { kind: 'save' } | { kind: 'rename'; id: string } | { kind: 'delete'; id: string } | null;

function rowId(row: PickerRow): string {
  return row.kind === 'library' ? `library:${row.entry.id}` : `example:${row.entry.id}`;
}

function matches(entry: { title: string; description: string; category?: string; name?: string }, query: string): boolean {
  if (!query) return true;
  return [entry.title, entry.description, entry.category ?? '', entry.name ?? '']
    .some((value) => value.toLocaleLowerCase().includes(query));
}

function relativeDate(timestamp: number): string {
  const age = Math.max(0, Date.now() - timestamp);
  const days = Math.floor(age / 86_400_000);
  if (days === 0) return 'Today';
  if (days === 1) return 'Yesterday';
  if (days < 7) return `${days} days ago`;
  return new Date(timestamp).toLocaleDateString();
}

export function PickerView(props: PickerProps): JSX.Element {
  const [filter, setFilter] = createSignal('');
  const [form, setForm] = createSignal<InlineForm>(null);
  const [formValue, setFormValue] = createSignal('');
  const [formError, setFormError] = createSignal('');
  const [activeRow, setActiveRow] = createSignal('');
  let trigger!: HTMLButtonElement;
  let filterInput!: HTMLInputElement;
  let formInput!: HTMLInputElement;
  let replaceButton!: HTMLButtonElement;
  const rowButtons = new Map<string, HTMLButtonElement>();
  const query = () => filter().trim().toLocaleLowerCase();
  const libraryEntries = createMemo(() => [...props.library().entries]
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .filter((entry) => matches({ title: entry.name, name: entry.name, description: '', category: '' }, query())));
  const examples = createMemo(() => props.examples.filter((entry) => matches(entry, query())));
  const categories = createMemo(() => {
    const grouped = new Map<string, ExampleEntry[]>();
    for (const entry of examples()) {
      const group = grouped.get(entry.category) ?? [];
      group.push(entry);
      grouped.set(entry.category, group);
    }
    return [...grouped.entries()];
  });
  const rows = createMemo<PickerRow[]>(() => [
    ...(props.shell === 'web' && props.library().available
      ? libraryEntries().map((entry) => ({ kind: 'library' as const, entry }))
      : []),
    ...categories().flatMap(([, entries]) => entries.map((entry) => ({ kind: 'example' as const, entry }))),
  ]);

  const focusFirstRow = () => {
    const first = rows()[0];
    if (!first) return;
    const id = rowId(first);
    setActiveRow(id);
    queueMicrotask(() => rowButtons.get(id)?.focus());
  };
  const moveRow = (from: string, direction: 'next' | 'previous' | 'first' | 'last') => {
    const visible = rows();
    if (!visible.length) return;
    const index = visible.findIndex((row) => rowId(row) === from);
    const next = direction === 'first' ? 0 : direction === 'last' ? visible.length - 1
      : direction === 'next' ? (index + 1 + visible.length) % visible.length
        : (index - 1 + visible.length) % visible.length;
    const id = rowId(visible[next]);
    setActiveRow(id);
    rowButtons.get(id)?.focus();
  };
  const cancelInline = () => {
    setForm(null);
    setFormValue('');
    setFormError('');
  };
  const startForm = (next: Exclude<InlineForm, null>, initial = '') => {
    setForm(next);
    setFormValue(initial);
    setFormError('');
    queueMicrotask(() => formInput?.focus());
  };
  const submitForm = () => {
    const current = form();
    if (!current || current.kind === 'delete') return;
    const result = current.kind === 'save'
      ? props.actions.saveAs(formValue())
      : props.actions.rename(current.id, formValue());
    if (result === null) cancelInline();
    else setFormError(result);
  };
  const isRenaming = (id: string) => {
    const current = form();
    return current?.kind === 'rename' && current.id === id;
  };
  const isDeleting = (id: string) => {
    const current = form();
    return current?.kind === 'delete' && current.id === id;
  };
  const onPanelKeyDown: JSX.EventHandler<HTMLDivElement, KeyboardEvent> = (event) => {
    if (event.key !== 'Escape') return;
    const target = event.target;
    if (form() || (target instanceof Element && target.closest('.vact-doc-picker-form'))) {
      event.preventDefault();
      event.stopPropagation();
      cancelInline();
      return;
    }
    event.preventDefault();
    props.actions.close();
    queueMicrotask(() => trigger?.focus());
  };
  createEffect(() => {
    if (props.open()) queueMicrotask(() => filterInput?.focus());
  });
  createEffect(() => {
    if (props.pendingReplace()) queueMicrotask(() => replaceButton?.focus());
  });
  createEffect(() => {
    const visible = rows();
    const active = activeRow();
    if (visible.length && !visible.some((row) => rowId(row) === active)) setActiveRow(rowId(visible[0]));
  });
  onCleanup(() => rowButtons.clear());

  return (
    <div class="vact-doc-picker-host">
      <button
        ref={trigger}
        class="vact-doc-picker-toggle"
        type="button"
        aria-haspopup="dialog"
        aria-expanded={props.open()}
        onClick={() => props.actions.toggle()}
      >Documents</button>
      <span class="vact-doc-picker-status" role="status">{props.status()}</span>
      <Show when={props.open()}>
        <div class="vact-doc-picker" role="dialog" aria-label="Documents" onKeyDown={onPanelKeyDown}>
          <input
            ref={filterInput}
            class="vact-doc-picker-filter"
            type="text"
            aria-label="Filter documents"
            value={filter()}
            onInput={(event) => setFilter(event.currentTarget.value)}
            onKeyDown={(event) => { if (event.key === 'ArrowDown') { event.preventDefault(); focusFirstRow(); } }}
          />
          <Show when={props.pendingReplace()}>
            <div class="vact-doc-picker-confirm" role="alertdialog" aria-label="Confirm document replacement">
              <span>Replace the current document? Undo (Mod-Z) restores it.</span>
              <button ref={replaceButton} type="button" onClick={() => props.actions.confirmReplace()}>Replace</button>
              <button type="button" onClick={() => props.actions.cancelReplace()}>Cancel</button>
            </div>
          </Show>
          <div class="vact-doc-picker-actions" aria-label="Document actions">
            <button type="button" onClick={() => props.actions.newDocument()}>New</button>
            <button type="button" onClick={() => props.actions.openFile()}>Open file...</button>
            <Show when={props.shell === 'web' && props.canSave()}>
              <button type="button" title={props.saveTarget() ? `Save to ${props.saveTarget()}` : 'Save'} onClick={() => {
                if (props.saveTarget()) props.actions.save();
                else startForm({ kind: 'save' });
              }}>Save</button>
              <button type="button" onClick={() => startForm({ kind: 'save' })}>Save as...</button>
            </Show>
            <button type="button" onClick={() => props.actions.useAsTemplate()}>Use as default template</button>
            <Show when={props.hasTemplate()}>
              <button type="button" onClick={() => props.actions.resetTemplate()}>Reset default template</button>
            </Show>
          </div>
          <Show when={form()?.kind === 'save'}>
            <form class="vact-doc-picker-form" onSubmit={(event) => { event.preventDefault(); submitForm(); }}>
              <label>Save as <input ref={formInput} type="text" aria-label="Document name" value={formValue()} onInput={(event) => setFormValue(event.currentTarget.value)} onKeyDown={(event) => { if (event.key === 'Escape') { event.preventDefault(); cancelInline(); } }} /></label>
              <button type="submit">Save</button>
              <button type="button" onClick={cancelInline}>Cancel</button>
              <Show when={formError()}><span class="vact-doc-picker-error" role="alert">{formError()}</span></Show>
            </form>
          </Show>
          <Show when={props.shell === 'web'}>
            <section class="vact-doc-picker-library" aria-labelledby="vact-doc-picker-library-heading">
              <h2 id="vact-doc-picker-library-heading">Saved in this browser</h2>
              <Show when={!props.library().available}>
                <p class="vact-doc-picker-notice">Browser storage is unavailable; saving to this browser is disabled</p>
              </Show>
              <Show when={props.library().available}>
                <For each={libraryEntries()}>{(entry) => {
                  const id = `library:${entry.id}`;
                  return <div class="vact-doc-picker-library-row">
                    <button
                      ref={(element) => { rowButtons.set(id, element); }}
                      class="vact-doc-picker-item"
                      type="button"
                      tabindex={activeRow() === id || (!activeRow() && rows()[0] && rowId(rows()[0]) === id) ? 0 : -1}
                      onFocus={() => setActiveRow(id)}
                      onKeyDown={(event) => { if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) { event.preventDefault(); moveRow(id, event.key === 'ArrowDown' ? 'next' : event.key === 'ArrowUp' ? 'previous' : event.key === 'Home' ? 'first' : 'last'); } }}
                      onClick={() => props.actions.openLibrary(entry.id)}
                      >{entry.name}<small>{relativeDate(entry.updatedAt)}</small></button>
                    <button type="button" onClick={() => startForm({ kind: 'rename', id: entry.id }, entry.name)}>Rename</button>
                    <Show when={isRenaming(entry.id)}>
                      <form class="vact-doc-picker-form" onSubmit={(event) => { event.preventDefault(); submitForm(); }}>
                        <label>Rename <input ref={formInput} type="text" aria-label="New document name" value={formValue()} onInput={(event) => setFormValue(event.currentTarget.value)} onKeyDown={(event) => { if (event.key === 'Escape') { event.preventDefault(); cancelInline(); } }} /></label>
                        <button type="submit">Save</button><button type="button" onClick={cancelInline}>Cancel</button>
                        <Show when={formError()}><span class="vact-doc-picker-error" role="alert">{formError()}</span></Show>
                      </form>
                    </Show>
                    <Show when={isDeleting(entry.id)}>
                      <span>Confirm delete?</span>
                      <button type="button" onClick={() => { props.actions.remove(entry.id); cancelInline(); }}>Confirm delete</button>
                      <button type="button" onClick={cancelInline}>Cancel</button>
                    </Show>
                    <Show when={!isDeleting(entry.id)}>
                      <button type="button" onClick={() => startForm({ kind: 'delete', id: entry.id })}>Delete</button>
                    </Show>
                  </div>;
                }}</For>
                <Show when={props.library().unreadable > 0}>
                  <p class="vact-doc-picker-notice">{props.library().unreadable} unreadable saved documents ignored</p>
                </Show>
              </Show>
            </section>
          </Show>
          <section class="vact-doc-picker-examples" aria-labelledby="vact-doc-picker-examples-heading">
            <h2 id="vact-doc-picker-examples-heading">Examples</h2>
            <For each={categories()}>{([category, entries]) => <div class="vact-doc-picker-group">
              <h3>{category}</h3>
              <For each={entries}>{(entry) => {
                const id = `example:${entry.id}`;
                return <button
                  ref={(element) => { rowButtons.set(id, element); }}
                  class="vact-doc-picker-item"
                  type="button"
                  data-example-id={entry.id}
                  tabindex={activeRow() === id || (!activeRow() && rows()[0] && rowId(rows()[0]) === id) ? 0 : -1}
                  onFocus={() => setActiveRow(id)}
                  onKeyDown={(event) => { if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) { event.preventDefault(); moveRow(id, event.key === 'ArrowDown' ? 'next' : event.key === 'ArrowUp' ? 'previous' : event.key === 'Home' ? 'first' : 'last'); } }}
                  onClick={() => props.actions.openExample(entry.id)}
                >
                  <span>{entry.title}</span><small>{entry.description}</small>
                  <Show when={entry.requires.includes('audio-input')}><span class="vact-doc-picker-badge">needs audio input</span></Show>
                  <Show when={entry.requires.includes('midi')}><span class="vact-doc-picker-badge">needs MIDI</span></Show>
                  <Show when={props.templateExampleId() === entry.id}><span class="vact-doc-picker-badge">default</span></Show>
                </button>;
              }}</For>
            </div>}</For>
          </section>
        </div>
      </Show>
    </div>
  );
}

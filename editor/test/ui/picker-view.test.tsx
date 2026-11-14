// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import type { ExampleEntry, LibraryEntry, LibraryView, PickerActions, PickerProps } from '../../src/documents/types';
import { ensurePickerStylesheet, PickerView } from '../../src/ui/picker-view';

const examples: ExampleEntry[] = [
  { id: 'acid', title: 'Acid Techno', description: 'Squelchy bass', category: 'Examples', requires: ['audio-input'] },
  { id: 'fm', title: 'FM Algorithms', description: 'Bright tones', category: 'Synth labs', requires: [] },
  { id: 'wobble', title: 'Wobble Techno', description: 'Moving bass', category: 'Examples', requires: ['midi'] },
];
const entries: LibraryEntry[] = [
  { id: 'saved-1', name: 'My song', text: 'a', createdAt: 1, updatedAt: 2 },
  { id: 'saved-2', name: 'Latest song', text: 'b', createdAt: 2, updatedAt: 4 },
];

function setup(options: { open?: boolean; shell?: 'web' | 'app'; library?: Partial<LibraryView>; canSave?: boolean; saveTarget?: string | null; hasTemplate?: boolean; pendingReplace?: string | null; status?: string } = {}) {
  const [open, setOpen] = createSignal(options.open ?? false);
  const [library, setLibrary] = createSignal<LibraryView>({ available: true, entries, unreadable: 0, ...options.library });
  const [templateId, setTemplateId] = createSignal<string | null>(null);
  const [hasTemplate, setHasTemplate] = createSignal(options.hasTemplate ?? false);
  const [canSave, setCanSave] = createSignal(options.canSave ?? true);
  const [saveTarget, setSaveTarget] = createSignal<string | null>(options.saveTarget ?? null);
  const [status, setStatus] = createSignal(options.status ?? '');
  const [pendingReplace, setPendingReplace] = createSignal<string | null>(options.pendingReplace ?? null);
  const actions: PickerActions = {
    toggle: vi.fn(() => setOpen((value) => !value)), close: vi.fn(() => setOpen(false)),
    openExample: vi.fn(), openLibrary: vi.fn(), openFile: vi.fn(), newDocument: vi.fn(), save: vi.fn(),
    saveAs: vi.fn(() => null), rename: vi.fn(() => null), remove: vi.fn(), useAsTemplate: vi.fn(), resetTemplate: vi.fn(),
    confirmReplace: vi.fn(), cancelReplace: vi.fn(),
  };
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(() => <PickerView
    shell={options.shell ?? 'web'} open={open} examples={examples} library={library}
    templateExampleId={templateId} hasTemplate={hasTemplate} canSave={canSave} saveTarget={saveTarget}
    status={status} pendingReplace={pendingReplace} actions={actions}
  />, host);
  return { host, dispose, actions, setOpen, setLibrary, setTemplateId, setHasTemplate, setCanSave, setSaveTarget, setStatus, setPendingReplace };
}

afterEach(() => { document.body.innerHTML = ''; });

describe('PickerView', () => {
  it('keeps the status visible while closed and toggles from the Documents button', () => {
    const view = setup({ status: 'Saved song' });
    expect(view.host.querySelector('[role="dialog"]')).toBeNull();
    expect(view.host.querySelector('.vact-doc-picker-toggle')?.getAttribute('aria-expanded')).toBe('false');
    expect(view.host.querySelector('[role="status"]')?.textContent).toBe('Saved song');
    (view.host.querySelector('.vact-doc-picker-toggle') as HTMLButtonElement).click();
    expect(view.actions.toggle).toHaveBeenCalledOnce();
    view.dispose();
  });

  it('opens a dialog, focuses the filter and installs the stylesheet once', async () => {
    const view = setup({ open: true });
    await Promise.resolve();
    expect(view.host.querySelector('[role="dialog"]')?.getAttribute('aria-label')).toBe('Documents');
    expect(document.activeElement).toBe(view.host.querySelector('[aria-label="Filter documents"]'));
    ensurePickerStylesheet(document);
    ensurePickerStylesheet(document);
    expect(document.head.querySelectorAll('link[data-vact-picker-css]')).toHaveLength(1);
    view.dispose();
  });

  it('groups examples in input order, shows requirement/default badges and opens the chosen entry', () => {
    const view = setup({ open: true });
    view.setTemplateId('fm');
    const rows = [...view.host.querySelectorAll('[data-example-id]')];
    expect([...view.host.querySelectorAll('.vact-doc-picker-group h3')].map((node) => node.textContent)).toEqual(['Examples', 'Synth labs']);
    expect(rows.map((node) => node.getAttribute('data-example-id'))).toEqual(['acid', 'wobble', 'fm']);
    expect(view.host.textContent).toContain('needs audio input');
    expect(view.host.textContent).toContain('needs MIDI');
    expect(rows[2].textContent).toContain('default');
    (rows[1] as HTMLButtonElement).click();
    expect(view.actions.openExample).toHaveBeenCalledWith('wobble');
    view.dispose();
  });

  it('filters by title, description, category and library name and keeps roving focus in visible rows', async () => {
    const view = setup({ open: true });
    const filter = view.host.querySelector<HTMLInputElement>('.vact-doc-picker-filter')!;
    filter.value = 'acid';
    filter.dispatchEvent(new Event('input', { bubbles: true }));
    expect(view.host.querySelectorAll('[data-example-id]')).toHaveLength(1);
    filter.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await Promise.resolve();
    expect(document.activeElement?.getAttribute('data-example-id')).toBe('acid');
    filter.value = 'Synth labs';
    filter.dispatchEvent(new Event('input', { bubbles: true }));
    expect(view.host.querySelectorAll('[data-example-id]')).toHaveLength(1);
    expect(view.host.querySelector('[data-example-id="fm"]')).not.toBeNull();
    filter.value = 'Bright tones';
    filter.dispatchEvent(new Event('input', { bubbles: true }));
    expect(view.host.querySelector('[data-example-id="fm"]')).not.toBeNull();
    filter.value = 'latest';
    filter.dispatchEvent(new Event('input', { bubbles: true }));
    expect([...view.host.querySelectorAll('.vact-doc-picker-library-row')].map((row) => row.textContent)).toHaveLength(1);
    view.dispose();
  });

  it('moves row focus with arrows, Home and End', () => {
    const view = setup({ open: true, library: { entries: [] } });
    const first = view.host.querySelector<HTMLButtonElement>('[data-example-id="acid"]')!;
    first.focus();
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(document.activeElement?.getAttribute('data-example-id')).toBe('wobble');
    (document.activeElement as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
    expect(document.activeElement?.getAttribute('data-example-id')).toBe('acid');
    (document.activeElement as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    (document.activeElement as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
    expect(document.activeElement?.getAttribute('data-example-id')).toBe('fm');
    (document.activeElement as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'Home', bubbles: true }));
    expect(document.activeElement?.getAttribute('data-example-id')).toBe('acid');
    view.dispose();
  });

  it('closes on Escape and returns focus to the trigger', async () => {
    const view = setup({ open: true });
    const trigger = view.host.querySelector<HTMLButtonElement>('.vact-doc-picker-toggle')!;
    view.host.querySelector('[role="dialog"]')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await Promise.resolve();
    expect(view.actions.close).toHaveBeenCalledOnce();
    expect(document.activeElement).toBe(trigger);
    view.dispose();
  });

  it('hides browser library and save controls in the app shell and navigates only rendered rows', async () => {
    const view = setup({ open: true, shell: 'app', canSave: true });
    expect(view.host.textContent).not.toContain('Saved in this browser');
    expect([...view.host.querySelectorAll('button')].some((button) => button.textContent === 'Save')).toBe(false);
    const filter = view.host.querySelector<HTMLInputElement>('.vact-doc-picker-filter')!;
    filter.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await Promise.resolve();
    expect(document.activeElement?.getAttribute('data-example-id')).toBe('acid');
    view.dispose();
  });

  it('shows the unavailable browser storage notice', () => {
    const view = setup({ open: true, library: { available: false } });
    expect(view.host.textContent).toContain('Browser storage is unavailable; saving to this browser is disabled');
    view.dispose();
  });

  it('submits Save as and keeps the inline form open with action errors', () => {
    const view = setup({ open: true });
    vi.mocked(view.actions.saveAs).mockReturnValueOnce('Name already used').mockReturnValueOnce(null);
    [...view.host.querySelectorAll('button')].find((button) => button.textContent === 'Save as...')!.click();
    const input = view.host.querySelector<HTMLInputElement>('[aria-label="Document name"]')!;
    input.value = 'song';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.closest('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(view.actions.saveAs).toHaveBeenLastCalledWith('song');
    expect(view.host.querySelector('[role="alert"]')?.textContent).toBe('Name already used');
    input.closest('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(view.host.querySelector('[aria-label="Document name"]')).toBeNull();
    view.dispose();
  });

  it('Escape cancels an inline form without closing the picker', () => {
    const view = setup({ open: true });
    [...view.host.querySelectorAll('button')].find((button) => button.textContent === 'Save as...')!.click();
    view.host.querySelector<HTMLInputElement>('[aria-label="Document name"]')!
      .dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(view.host.querySelector('[aria-label="Document name"]')).toBeNull();
    expect(view.actions.close).not.toHaveBeenCalled();
    view.dispose();
  });

  it('renames with an inline form and uses a two-step delete confirmation', () => {
    const view = setup({ open: true });
    const rename = [...view.host.querySelectorAll('.vact-doc-picker-library-row')][0].querySelector<HTMLButtonElement>('button:nth-of-type(2)')!;
    rename.click();
    const input = view.host.querySelector<HTMLInputElement>('[aria-label="New document name"]')!;
    input.value = 'Renamed';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.closest('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(view.actions.rename).toHaveBeenCalledWith('saved-2', 'Renamed');
    const row = [...view.host.querySelectorAll('.vact-doc-picker-library-row')].find((element) => element.textContent?.includes('My song'))!;
    [...row.querySelectorAll('button')].find((button) => button.textContent === 'Delete')!.click();
    expect(row.textContent).toContain('Confirm delete?');
    [...row.querySelectorAll('button')].find((button) => button.textContent === 'Cancel')!.click();
    expect(view.actions.remove).not.toHaveBeenCalled();
    [...row.querySelectorAll('button')].find((button) => button.textContent === 'Delete')!.click();
    [...row.querySelectorAll('button')].find((button) => button.textContent === 'Confirm delete')!.click();
    expect(view.actions.remove).toHaveBeenCalledWith('saved-1');
    view.dispose();
  });

  it('shows and focuses pending replacement controls and resets a configured default', async () => {
    const view = setup({ open: true, pendingReplace: 'acid', hasTemplate: true });
    const confirm = view.host.querySelector('[role="alertdialog"]')!;
    await Promise.resolve();
    expect(confirm.textContent).toContain('Replace the current document?');
    expect(document.activeElement).toBe(confirm.querySelector('button'));
    [...confirm.querySelectorAll('button')].find((button) => button.textContent === 'Replace')!.click();
    expect(view.actions.confirmReplace).toHaveBeenCalledOnce();
    [...view.host.querySelectorAll('button')].find((button) => button.textContent === 'Reset default template')!.click();
    expect(view.actions.resetTemplate).toHaveBeenCalledOnce();
    view.dispose();
  });

  it('hides Reset default template when no default is configured', () => {
    const view = setup({ open: true, hasTemplate: false });
    expect([...view.host.querySelectorAll('button')].some((button) => button.textContent === 'Reset default template')).toBe(false);
    view.dispose();
  });

  it('routes New, Open file and template actions through props.actions', () => {
    const view = setup({ open: true });
    const button = (name: string) => [...view.host.querySelectorAll('button')].find((node) => node.textContent === name)!;
    button('New').click();
    button('Open file...').click();
    button('Use as default template').click();
    expect(view.actions.newDocument).toHaveBeenCalledOnce();
    expect(view.actions.openFile).toHaveBeenCalledOnce();
    expect(view.actions.useAsTemplate).toHaveBeenCalledOnce();
    view.dispose();
  });

  it('saves the current target or opens Save as when no target exists', () => {
    const current = setup({ open: true, saveTarget: 'song' });
    [...current.host.querySelectorAll('button')].find((button) => button.textContent === 'Save')!.click();
    expect(current.actions.save).toHaveBeenCalledOnce();
    current.dispose();
    const fresh = setup({ open: true, saveTarget: null });
    [...fresh.host.querySelectorAll('button')].find((button) => button.textContent === 'Save')!.click();
    expect(fresh.actions.save).not.toHaveBeenCalled();
    expect(fresh.host.querySelector('[aria-label="Document name"]')).not.toBeNull();
    fresh.dispose();
  });
});

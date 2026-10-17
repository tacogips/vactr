// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mountVimKeymapPanel, type KeymapPanelProps } from '../../src/code/vim-keymap-view';

const disposers: (() => void)[] = [];
function setup(open = true) {
  const parent = document.createElement('div'); document.body.append(parent);
  const [isOpen, setOpen] = createSignal(open);
  const [text, setText] = createSignal('{"version":1}');
  const [issues] = createSignal([{ path: 'maps.n[0].lhs', severity: 'warning' as const, message: 'unknown field' }]);
  const [activePreset] = createSignal<string | null>('default');
  const [status] = createSignal<string | null>('Keymap applied');
  const props: KeymapPanelProps = {
    open: isOpen, text, issues, presets: [{ id: 'default', label: 'Default' }, { id: 'mise-darwin', label: 'mise-darwin' }], activePreset, status,
    onPreset: vi.fn(), onInput: vi.fn(), onApply: vi.fn(), onImport: vi.fn(), onExport: vi.fn(), onReset: vi.fn(), onClose: vi.fn(),
  };
  disposers.push(mountVimKeymapPanel(parent, props));
  return { parent, props, setOpen, setText };
}

afterEach(() => { for (const dispose of disposers.splice(0)) dispose(); document.body.replaceChildren(); });

describe('Vim keymap settings view', () => {
  it('renders nothing while closed', () => {
    const view = setup(false);
    expect(view.parent.querySelector('.vact-vim-keymap')).toBeNull();
  });

  it('renders the JSON editor and focuses it while open', () => {
    const view = setup(); const textarea = view.parent.querySelector<HTMLTextAreaElement>('.vact-vim-keymap-text');
    expect(textarea?.value).toBe('{"version":1}');
    expect(textarea?.getAttribute('spellcheck')).toBe('false');
    expect(document.activeElement).toBe(textarea);
  });

  it('shows preset buttons and marks the active preset pressed', () => {
    const view = setup();
    expect(view.parent.querySelectorAll('.vact-vim-keymap-preset')).toHaveLength(2);
    expect(view.parent.querySelector('[aria-pressed="true"]')?.textContent).toBe('Default');
    view.parent.querySelector<HTMLButtonElement>('.vact-vim-keymap-preset:nth-child(2)')?.click();
    expect(view.props.onPreset).toHaveBeenCalledWith('mise-darwin');
  });

  it('sends textarea input to the caller', () => {
    const view = setup(); const textarea = view.parent.querySelector<HTMLTextAreaElement>('textarea')!;
    textarea.value = '{"version":2}'; textarea.dispatchEvent(new Event('input', { bubbles: true }));
    expect(view.props.onInput).toHaveBeenCalledWith('{"version":2}');
  });

  it('calls Apply, Import, Export, Reset and Close handlers', () => {
    const view = setup();
    for (const action of ['apply', 'import', 'export', 'reset', 'close']) view.parent.querySelector<HTMLButtonElement>(`.vact-vim-keymap-${action}`)?.click();
    expect(view.props.onApply).toHaveBeenCalledOnce();
    expect(view.props.onImport).toHaveBeenCalledOnce();
    expect(view.props.onExport).toHaveBeenCalledOnce();
    expect(view.props.onReset).toHaveBeenCalledOnce();
    expect(view.props.onClose).toHaveBeenCalledOnce();
  });

  it('renders status and path-qualified issues', () => {
    const view = setup();
    expect(view.parent.querySelector('[role="status"]')?.textContent).toBe('Keymap applied');
    expect(view.parent.querySelector('.vact-vim-keymap-issues li')?.textContent).toBe('maps.n[0].lhs: unknown field');
    expect(view.parent.querySelector('.vact-vim-keymap-issues li')?.getAttribute('data-severity')).toBe('warning');
  });

  it('closes on Escape and stops every panel keydown from bubbling', () => {
    const view = setup(); const parentListener = vi.fn();
    view.parent.addEventListener('keydown', parentListener);
    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    view.parent.querySelector('.vact-vim-keymap')!.dispatchEvent(event);
    expect(view.props.onClose).toHaveBeenCalledOnce();
    expect(parentListener).not.toHaveBeenCalled();
    const other = new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true });
    view.parent.querySelector('.vact-vim-keymap')!.dispatchEvent(other);
    expect(parentListener).not.toHaveBeenCalled();
  });
});

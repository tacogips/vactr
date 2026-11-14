// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import { createSignal } from 'solid-js';
import { mountVimHints } from '../../src/code/vim-hints-view';
import type { HintView } from '../../src/code/vim/types';

function mount(value: HintView | null) {
  const parent = document.createElement('div'); document.body.append(parent);
  const dispose = mountVimHints(parent, () => value);
  return { parent, dispose };
}

describe('Vim next-key hint view', () => {
  it('renders nothing when hints are absent', () => {
    const view = mount(null);
    expect(view.parent.querySelector('.vact-vim-hints')).toBeNull();
    view.dispose();
  });

  it('shows entries and an accessible summary without focusable descendants', () => {
    const value: HintView = { prefix: 'g', entries: [
      { key: 'g', desc: 'first line', group: false },
      { key: 'j', desc: 'down', group: true },
      { key: 'k', desc: 'up', group: false },
    ], summary: 'g: 3 next keys' };
    const view = mount(value);
    expect(view.parent.querySelector('.vact-vim-hints-prefix')?.textContent).toBe('g');
    expect(view.parent.querySelectorAll('.vact-vim-hints-row')).toHaveLength(3);
    expect(view.parent.querySelector('.vact-vim-hints-grid')?.getAttribute('aria-hidden')).toBe('true');
    expect(view.parent.querySelector('.vact-vim-hints-summary')?.getAttribute('aria-live')).toBe('polite');
    expect(view.parent.querySelector('.vact-vim-hints-summary')?.textContent).toBe('g: 3 next keys');
    expect(view.parent.querySelector('.vact-vim-hints-row[data-group="true"] .vact-vim-hints-key')?.textContent).toBe('j');
    expect(view.parent.querySelectorAll('button,input,textarea,[tabindex]')).toHaveLength(0);
    view.dispose();
  });

  it('collapses entries beyond the row limit', () => {
    const entries = Array.from({ length: 30 }, (_, index) => ({ key: String(index), desc: `item ${index}`, group: index === 0 }));
    const [hints] = createSignal<HintView | null>({ prefix: ' ', entries, summary: 'space: 30 next keys' });
    const parent = document.createElement('div'); document.body.append(parent);
    const dispose = mountVimHints(parent, hints);
    expect(parent.querySelectorAll('.vact-vim-hints-row')).toHaveLength(24);
    expect(parent.querySelector('.vact-vim-hints-more')?.textContent).toContain('+7 more');
    dispose();
  });

  it('keeps the full list when it exactly fits the row limit', () => {
    const entries = Array.from({ length: 24 }, (_, index) => ({ key: String(index), desc: `item ${index}`, group: false }));
    const parent = document.createElement('div'); document.body.append(parent);
    const dispose = mountVimHints(parent, () => ({ prefix: 'g', entries, summary: 'g: 24 next keys' }));
    expect(parent.querySelectorAll('.vact-vim-hints-row')).toHaveLength(24);
    expect(parent.querySelector('.vact-vim-hints-more')).toBeNull();
    dispose();
  });
});

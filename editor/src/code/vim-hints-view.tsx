import { For, Show, type Accessor } from 'solid-js';
import { render } from 'solid-js/web';
import type { HintView } from './vim/types';

export const HINT_MAX_ROWS = 24;

export function mountVimHints(parent: HTMLElement, hints: Accessor<HintView | null>): () => void {
  return render(() => (
    <Show when={hints()}>
      {(view) => {
        const visible = () => view().entries.slice(0, view().entries.length > HINT_MAX_ROWS ? HINT_MAX_ROWS - 1 : HINT_MAX_ROWS);
        const hiddenCount = () => view().entries.length > HINT_MAX_ROWS ? view().entries.length - (HINT_MAX_ROWS - 1) : 0;
        return (
          <aside class="vact-vim-hints" aria-label="vim next keys">
            <div class="vact-vim-hints-prefix">{view().prefix}</div>
            <div class="vact-vim-hints-grid" aria-hidden="true">
              <For each={visible()}>{(entry) => (
                <div class="vact-vim-hints-row" data-group={entry.group ? 'true' : undefined}>
                  <span class="vact-vim-hints-key">{entry.key}</span>
                  <span class="vact-vim-hints-desc">{entry.desc}</span>
                </div>
              )}</For>
              <Show when={hiddenCount() > 0}>
                <div class="vact-vim-hints-row vact-vim-hints-more">
                  <span class="vact-vim-hints-key">+{hiddenCount()} more</span>
                </div>
              </Show>
            </div>
            <span class="vact-vim-hints-summary" aria-live="polite">{view().summary}</span>
          </aside>
        );
      }}
    </Show>
  ), parent);
}

import { createEffect, Show, type Accessor } from 'solid-js';
import { render } from 'solid-js/web';
import type { VimKey, VimKeyInput, VimSnapshot } from './vim/types';

export interface VimStatusHandlers { submit(text: string): void; cancel(): void; tapMode(): void; openKeymap?(): void }

export function mountVimStatus(parent: HTMLElement, snapshot: Accessor<VimSnapshot>, handlers: VimStatusHandlers, resolveCmdline?: (input: VimKeyInput) => VimKey | null): () => void {
  return render(() => {
    let promptActive = false;
    let promptEpoch = 0;
    let finishedEpoch = -1;
    createEffect(() => {
      const prompt = snapshot().prompt;
      const active = prompt !== null;
      if (active && !promptActive) promptEpoch += 1;
      promptActive = active;
      if (prompt) {
        const input = parent.querySelector<HTMLInputElement>('.vact-vim-ex');
        if (input) {
          input.focus();
          input.setSelectionRange(input.value.length, input.value.length);
        }
      }
    });
    const finish = (action: () => void): void => {
      if (finishedEpoch === promptEpoch) return;
      finishedEpoch = promptEpoch;
      action();
    };
    const applyCmdlineKey = (input: HTMLInputElement, key: VimKey): void => {
      if (key === '<Esc>') finish(() => handlers.cancel());
      else if (key === '<CR>') finish(() => handlers.submit(input.value));
      else if (key === '<Home>') input.setSelectionRange(0, 0);
      else if (key === '<End>') input.setSelectionRange(input.value.length, input.value.length);
      else if (key === '<Left>' || key === '<Right>') {
        const start = input.selectionStart ?? input.value.length;
        const end = input.selectionEnd ?? start;
        const caret = start === end ? Math.max(0, Math.min(input.value.length, start + (key === '<Right>' ? 1 : -1))) : (key === '<Right>' ? end : start);
        input.setSelectionRange(caret, caret);
      } else if (key === '<BS>' || key === '<Del>') {
        const start = input.selectionStart ?? input.value.length;
        const end = input.selectionEnd ?? start;
        const from = start === end && key === '<BS>' ? Math.max(0, start - 1) : start;
        const to = start === end && key === '<Del>' ? Math.min(input.value.length, end + 1) : end;
        input.setRangeText('', from, to, 'end');
      } else if (Array.from(key).length === 1 && key >= ' ') {
        const start = input.selectionStart ?? input.value.length;
        const end = input.selectionEnd ?? start;
        input.setRangeText(key, start, end, 'end');
      }
    };
    return (
      <div class="vact-vim-status" data-mode={snapshot().mode}>
        <button
          type="button"
          class="vact-vim-mode"
          data-mode={snapshot().mode}
          aria-label={`vim mode ${snapshot().label}; activate to ${snapshot().mode === 'insert' || snapshot().mode === 'replace' ? 'leave' : 'enter'} insert mode`}
          onClick={() => handlers.tapMode()}
        >{snapshot().label}</button>
        <span class="vact-vim-pending">{snapshot().pending}</span>
        <Show when={handlers.openKeymap}>
          <button type="button" class="vact-vim-keymap-button" aria-label="open vim keymap settings" onClick={() => handlers.openKeymap?.()}>keymap</button>
        </Show>
        <Show when={snapshot().recording}>
          {(reg) => <span class="vact-vim-recording">recording @{reg()}</span>}
        </Show>
        <Show when={snapshot().search}>
          {(search) => <span class="vact-vim-search">/{search()}</span>}
        </Show>
        <Show when={snapshot().prompt} fallback={
          <Show when={snapshot().message !== null}>
            <span class="vact-vim-message" role={snapshot().messageKind === 'error' ? 'alert' : 'status'} data-kind={snapshot().messageKind ?? 'info'}>{snapshot().message}</span>
          </Show>
        }>
          {(activePrompt) => (
            <label class="vact-vim-prompt">
              <span class="vact-vim-prompt-kind">{activePrompt().kind}</span>
              <input
                class="vact-vim-ex"
                type="text"
                aria-label={activePrompt().kind === ':' ? 'vim command' : 'vim search'}
                autocomplete="off"
                spellcheck={false}
                value={activePrompt().initial}
                ref={(input) => {
                  input.setSelectionRange(input.value.length, input.value.length);
                }}
                onKeyDown={(event) => {
                  if (event.isComposing || event.keyCode === 229) return;
                  if (resolveCmdline) {
                    const resolved = resolveCmdline({
                      key: event.key,
                      code: event.code,
                      ctrlKey: event.ctrlKey,
                      metaKey: event.metaKey,
                      altKey: event.altKey,
                      shiftKey: event.shiftKey,
                    });
                    if (resolved !== null) {
                      applyCmdlineKey(event.currentTarget, resolved);
                      event.preventDefault();
                      event.stopPropagation();
                      return;
                    }
                  }
                  if (event.key === 'Enter') finish(() => handlers.submit(event.currentTarget.value));
                  else if (event.key === 'Escape' || (event.ctrlKey && event.key === '[')) finish(() => handlers.cancel());
                  else return;
                  event.preventDefault();
                  event.stopPropagation();
                }}
                onBlur={() => finish(() => handlers.cancel())}
              />
            </label>
          )}
        </Show>
      </div>
    );
  }, parent);
}

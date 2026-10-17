import { createEffect, For, Show, type Accessor } from 'solid-js';
import { render } from 'solid-js/web';
import type { KeymapIssue } from './vim/keymap-schema';

export interface KeymapPanelProps {
  open: Accessor<boolean>;
  text: Accessor<string>;
  issues: Accessor<readonly KeymapIssue[]>;
  presets: readonly { id: string; label: string }[];
  activePreset: Accessor<string | null>;
  status: Accessor<string | null>;
  onPreset(id: string): void;
  onInput(text: string): void;
  onApply(): void;
  onImport(): void;
  onExport(): void;
  onReset(): void;
  onClose(): void;
}

export function mountVimKeymapPanel(parent: HTMLElement, props: KeymapPanelProps): () => void {
  return render(() => {
    createEffect(() => {
      if (!props.open()) return;
      parent.querySelector<HTMLTextAreaElement>('.vact-vim-keymap-text')?.focus();
    });
    return (
      <Show when={props.open()}>
        <section class="vact-vim-keymap" aria-label="vim keymap" on:keydown={(event) => {
          if (event.key === 'Escape') props.onClose();
          event.stopPropagation();
        }}>
          <div class="vact-vim-keymap-presets" role="group" aria-label="vim keymap preset">
            <For each={props.presets}>{(preset) => (
              <button
                type="button"
                class="vact-vim-keymap-preset"
                aria-pressed={props.activePreset() === preset.id ? 'true' : 'false'}
                onClick={() => props.onPreset(preset.id)}
              >{preset.label}</button>
            )}</For>
          </div>
          <textarea
            class="vact-vim-keymap-text"
            spellcheck={false}
            aria-label="vim keymap JSON"
            value={props.text()}
            onInput={(event) => props.onInput(event.currentTarget.value)}
          />
          <div class="vact-vim-keymap-actions">
            <button type="button" class="vact-vim-keymap-apply" onClick={() => props.onApply()}>Apply</button>
            <button type="button" class="vact-vim-keymap-import" onClick={() => props.onImport()}>Import</button>
            <button type="button" class="vact-vim-keymap-export" onClick={() => props.onExport()}>Export</button>
            <button type="button" class="vact-vim-keymap-reset" onClick={() => props.onReset()}>Reset</button>
            <button type="button" class="vact-vim-keymap-close" onClick={() => props.onClose()}>Close</button>
          </div>
          <Show when={props.status()}>{(status) => <div class="vact-vim-keymap-status" role="status">{status()}</div>}</Show>
          <ul class="vact-vim-keymap-issues" role="list">
            <For each={props.issues()}>{(issue) => (
              <li data-severity={issue.severity}>{issue.path}: {issue.message}</li>
            )}</For>
          </ul>
        </section>
      </Show>
    );
  }, parent);
}

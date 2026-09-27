import { For, Show, type Accessor, type JSX } from 'solid-js';
import { Save } from '../ui/icons';
import type { PersistenceMode } from './persistence';

export interface ControlBinding {
  text: string;
  param?: string;
  key?: string;
}

export interface ControlEntry {
  kind: string;
  span: string;
  text: string;
  diagnostic?: { code: string; message: string };
  bindings: ControlBinding[];
}

export interface ControlGroup {
  title: string;
  fileChannel?: string;
  entries: ControlEntry[];
}

export interface ControlModel {
  mode: PersistenceMode;
  notice: string;
  groups: ControlGroup[];
  setEntries: ControlBinding[];
}

export function ControlView(props: {
  model: Accessor<ControlModel>;
  onMode: (mode: PersistenceMode) => void;
  onSave: () => void;
}): JSX.Element {
  return (
    <section class="bind-controls">
      <div class="bind-controls-header">
        <h3>Controls</h3>
        <select class="bind-persistence" value={props.model().mode} on:change={(event) => props.onMode(event.currentTarget.value as PersistenceMode)}>
          <option value="directive">directives (#@)</option>
          <option value="external-file">external file</option>
        </select>
        <button type="button" class="bind-save vact-icon-button" aria-label="Save" title="Save" on:click={props.onSave}><Save /></button>
      </div>
      <div class="bind-notice">{props.model().notice}</div>
      <div class="bind-controls-body">
        <Show when={props.model().mode === 'external-file'} fallback={
          <For each={props.model().groups}>{(group) =>
            <div class="bind-ctl-group" data-group={group.title}>
              <h4>{group.title}</h4>
              <Show when={group.fileChannel}><div class="bind-ctl-file">{group.fileChannel}</div></Show>
              <For each={group.entries}>{(entry) =>
                <div class="bind-ctl-entry" data-kind={entry.kind} data-directive={entry.span} data-diagnostic={entry.diagnostic?.code}>
                  <div class="bind-ctl-text">{entry.text}</div>
                  <Show when={entry.diagnostic}><div class="bind-ctl-marker" title={entry.diagnostic?.message}>{entry.diagnostic?.code}</div></Show>
                  <For each={entry.bindings}>{(binding) =>
                    <div class="bind-ctl-binding" data-param={binding.param} data-key={binding.key}>{binding.text}</div>
                  }</For>
                </div>
              }</For>
            </div>
          }</For>
        }>
          <For each={props.model().setEntries}>{(binding) =>
            <div class="bind-ctl-binding" data-key={binding.key}>{binding.text}</div>
          }</For>
        </Show>
      </div>
    </section>
  );
}

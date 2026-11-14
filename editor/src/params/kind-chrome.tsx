import { For, Show, type Accessor, type JSX } from 'solid-js';
import { Download } from '../ui/icons';

export function EqSpectrumView(): JSX.Element {
  return <div class="params-spectrum" />;
}

export function SamplerChrome(props: {
  hint: Accessor<string>;
  preview: Accessor<string>;
  browse: boolean;
  onBrowse: () => void;
}): JSX.Element {
  return <>
    <div class="params-hint">{props.hint()}</div>
    <div class="params-no-preview">{props.preview()}</div>
    <Show when={props.browse}>
      <button type="button" class="params-browse vact-icon-button" aria-label="browse samples" title="browse samples" on:click={props.onBrowse}><Download /></button>
    </Show>
  </>;
}

export interface AxisOption { id: number; label: string }

export function XyChrome(props: {
  options: Accessor<AxisOption[]>;
  x: Accessor<string>;
  y: Accessor<string>;
  onPick: (x: string, y: string) => void;
}): JSX.Element {
  let xSelect: HTMLSelectElement | undefined;
  let ySelect: HTMLSelectElement | undefined;
  return <>
    <div class="params-xy-pick">
      <select ref={xSelect} class="params-xy-x" value={props.x()} on:change={() => props.onPick(xSelect?.value ?? '', ySelect?.value ?? '')}>
        <For each={props.options()}>{(option) => <option value={String(option.id)}>{option.label}</option>}</For>
      </select>
      <select ref={ySelect} class="params-xy-y" value={props.y()} on:change={() => props.onPick(xSelect?.value ?? '', ySelect?.value ?? '')}>
        <For each={props.options()}>{(option) => <option value={String(option.id)}>{option.label}</option>}</For>
      </select>
    </div>
    <div class="params-xy-body" />
  </>;
}

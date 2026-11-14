import { For, type Accessor, type JSX } from 'solid-js';
import { ClockMidiLocked } from '../ui/icons';
import type { Handle } from './handles';

export interface HandleRowState {
  enabled: boolean;
  position: number;
  value: string;
}

export function CanvasSurface(props: { className: string; width: number; height: number }): JSX.Element {
  return <canvas class={`params-canvas ${props.className}`} width={props.width} height={props.height} />;
}

export interface HandleRow {
  handle: Handle;
  state: Accessor<HandleRowState>;
}

export function HandleRowsView(props: { rows: HandleRow[]; onInput: (handle: Handle, value: number) => void }): JSX.Element {
  return (
    <div class="params-handles">
      <For each={props.rows}>{({ handle, state }) =>
        <div class="params-handle" classList={{ 'params-disabled': !state().enabled }} data-param={handle.param} title={state().enabled ? undefined : 'not in code'}>
          <span class="params-handle-label">{handle.param}</span>
          <input type="range" class="params-handle-input" min="0" max="1" step="any" value={state().position} disabled={!state().enabled}
            on:input={(event) => props.onInput(handle, Number(event.currentTarget.value))} />
          <span class="params-handle-value">{state().value}</span>
          <button type="button" class="params-learn vact-icon-button" disabled={!state().enabled} aria-label={`learn MIDI for ${handle.param}`} title={`learn MIDI for ${handle.param}`}
            on:click={() => void handle.learn()}><ClockMidiLocked /></button>
        </div>
      }</For>
    </div>
  );
}

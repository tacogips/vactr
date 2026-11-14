import { For, Show, type Accessor, type JSX } from 'solid-js';
import { OUTPUTS } from './render-host';
import type { PaneSelection } from './panes';

export function PanesView(props: {
  source: boolean;
  notice: string;
  selection: Accessor<PaneSelection>;
  banner: Accessor<string>;
  width: number;
  height: number;
  onSelect: (selection: PaneSelection) => void;
}): JSX.Element {
  const selected = () => props.selection() === 'tile' ? 'tile' : `o${props.selection()}`;
  return <section class="visual-panes" data-area="visual" data-selection={selected()}>
    <div class="visual-header">
      <span class="visual-title">Visuals</span>
      <select class="visual-select" data-role="pane-select" value={selected()} disabled={!props.source}
        on:change={(event) => {
          const value = event.currentTarget.value;
          props.onSelect(value === 'tile' ? 'tile' : Number(value.slice(1)) as PaneSelection);
        }}>
        <For each={['o0', 'o1', 'o2', 'o3', 'tile']}>{(value) =>
          <option value={value}>{value === 'tile' ? 'tile o0..o3' : value}</option>
        }</For>
      </select>
    </div>
    <div class="visual-banner" data-role="diagnostic" hidden={props.banner() === ''}>{props.banner()}</div>
    <div class="visual-grid">
      <Show when={props.source} fallback={<div class="visual-notice" data-role="notice">{props.notice}</div>}>
        <For each={OUTPUTS}>{(out) =>
          <div class="visual-pane" data-output={`o${out}`} hidden={props.selection() !== 'tile' && props.selection() !== out}>
            <canvas width={props.width} height={props.height} />
            <span class="visual-pane-label">{`o${out}`}</span>
          </div>
        }</For>
      </Show>
    </div>
  </section>;
}

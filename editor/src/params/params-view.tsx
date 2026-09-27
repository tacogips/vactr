import { For, type Accessor, type JSX } from 'solid-js';
import type { EditorKind } from '../protocol/types';

export type ParamsTab = 'editors' | 'grid' | 'roll';

export interface GroupView {
  id: string;
  label: string;
  kind: EditorKind;
  waveform: boolean;
}

export function ParamsView(props: {
  tab: Accessor<ParamsTab>;
  groups: Accessor<GroupView[]>;
  onTab: (tab: ParamsTab) => void;
  onOpen: (id: string, kind?: EditorKind) => void;
}): JSX.Element {
  return (
    <section class="params" data-tab={props.tab()}>
      <div class="params-tabs">
        <For each={['editors', 'grid', 'roll'] as ParamsTab[]}>{(tab) =>
          <button type="button" class="params-tab" data-tab={tab} on:click={() => props.onTab(tab)}>{tab}</button>
        }</For>
      </div>
      <div class="params-pane params-pane-editors" hidden={props.tab() !== 'editors'}>
        <div class="params-list">
          <For each={props.groups()}>{(group) =>
            <div class="params-group" data-group={group.id}>
              <span class="params-group-label">{group.label}</span>
              <button type="button" class="params-open" on:click={() => props.onOpen(group.id)}>{group.kind}</button>
              {group.waveform && <button type="button" class="params-open-wave" on:click={() => props.onOpen(group.id, 'sampler-wave')}>waveform</button>}
            </div>
          }</For>
        </div>
        <div class="params-editor" />
      </div>
      <div class="params-pane params-pane-grid" hidden={props.tab() !== 'grid'} />
      <div class="params-pane params-pane-roll" hidden={props.tab() !== 'roll'} />
    </section>
  );
}

export function EditorHostView(props: { title: string; kind: EditorKind }): JSX.Element {
  return <>
    <div class="params-title">{props.title}</div>
    <div class={`params-kind params-kind-${props.kind}`} data-kind={props.kind} />
  </>;
}

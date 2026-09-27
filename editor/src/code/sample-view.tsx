import { For, Show, type Accessor, type JSX } from 'solid-js';
import type { SampleFrames } from '../app/apis';
import type { Tier } from '../app/deps';
import { Download } from '../ui/icons';

export interface SampleEntryView {
  key: string;
  frames: SampleFrames | null;
}

export interface BankView {
  name: string;
  count: number;
  entries: SampleEntryView[];
}

export function SampleView(props: {
  tier: Tier;
  browserLibrary: boolean;
  url: string;
  sounds: Accessor<readonly string[]>;
  banks: Accessor<BankView[]>;
  status: Accessor<string>;
  onLoadMap: (url: string) => void;
  onLoadBank: (bank: string) => void;
  preview: (canvas: HTMLCanvasElement, frames: SampleFrames) => void;
}): JSX.Element {
  let urlInput: HTMLInputElement | undefined;
  return (
    <details class="vact-samples">
      <summary>samples</summary>
      <ul class="vact-sounds"><For each={props.sounds()}>{(sound) =>
        <li class="vact-sound" data-sound={sound}>{sound}</li>
      }</For></ul>
      <Show when={props.tier === 'browser' && props.browserLibrary}>
        <div class="vact-sample-map">
          <input ref={urlInput} type="url" placeholder="sample map url" value={props.url} />
          <button type="button" class="vact-icon-button" aria-label="load sample map" title="load sample map" on:click={() => props.onLoadMap(urlInput?.value ?? '')}><Download /></button>
        </div>
        <ul class="vact-banks"><For each={props.banks()}>{(bank) =>
          <li class="vact-bank" data-bank={bank.name}>
            <span>{`${bank.name} (${bank.count})`}</span>
            <button type="button" class="vact-icon-button" aria-label={`load ${bank.name}`} title={`load ${bank.name}`} on:click={() => props.onLoadBank(bank.name)}><Download /></button>
            <ul class="vact-entries"><For each={bank.entries}>{(entry) =>
              <li class="vact-entry" data-key={entry.key} data-missing={entry.frames ? undefined : 'true'}>
                {entry.key}
                <Show when={entry.frames}>{(frames) =>
                  <canvas class="vact-preview" width="96" height="24" ref={(canvas) => props.preview(canvas, frames())} />
                }</Show>
              </li>
            }</For></ul>
          </li>
        }</For></ul>
      </Show>
      <div class="vact-samples-status">{props.status()}</div>
    </details>
  );
}

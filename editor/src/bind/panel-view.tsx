// Solid views for individual binding and value rows. Each accessor belongs to
// one row, so a store batch can update one site without touching its peers.
import type { Accessor, JSX } from 'solid-js';
import { Check, ClockMidiLocked } from '../ui/icons';

export interface SiteRowState {
  label: string;
  value: string;
  rawValue: number;
  tier: string;
  tierKind: string;
  mode: string;
  commit: boolean;
  midi: string;
  state: string;
  stateLabel: string;
  form: string;
  min: number;
  max: number;
  step: string;
}

export interface NameRowState {
  value: string;
  state: string;
  badge: string;
}

export function SiteRow(props: {
  id: string;
  state: Accessor<SiteRowState>;
  onInput: (value: number) => void;
  onMode: () => void;
  onCommit: () => void;
  onLearn: () => void;
}): JSX.Element {
  return (
    <div class="bind-row" data-binding={props.id} data-mode={props.state().mode} data-state={props.state().state}>
      <span class="bind-label">{props.state().label}</span>
      <input
        type="range"
        class="bind-slider"
        min={props.state().min}
        max={props.state().max}
        step={props.state().step}
        value={props.state().rawValue}
        disabled={props.state().state !== 'bound'}
        on:input={(event) => props.onInput(Number(event.currentTarget.value))}
      />
      <span class="bind-value">{props.state().value}</span>
      <span class="bind-tier" data-tier={props.state().tierKind}>{props.state().tier}</span>
      <button type="button" class="bind-mode" onClick={props.onMode}>{props.state().mode}</button>
      <button type="button" class="bind-commit vact-icon-button" hidden={!props.state().commit} aria-label="commit overlay" title="commit overlay" onClick={props.onCommit}><Check /></button>
      <button type="button" class="bind-learn vact-icon-button" aria-label="learn MIDI" title="learn MIDI" onClick={props.onLearn}><ClockMidiLocked /></button>
      <span class="bind-midi">{props.state().midi}</span>
      <span class="bind-state">{props.state().stateLabel}</span>
      <span class="bind-form">{props.state().form}</span>
    </div>
  );
}

export function NameRow(props: { name: string; state: Accessor<NameRowState> }): JSX.Element {
  return (
    <div class="bind-name" data-name={props.name} data-state={props.state().state}>
      <span class="bind-name-label">{props.name}</span>
      <span class="bind-name-value">{props.state().value}</span>
      <span class="bind-name-badge">{props.state().badge}</span>
    </div>
  );
}

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
  /** The parameter's editor label (`ParamMeta.label`), when known (DDRUM-006). */
  paramLabel?: string;
  /** The parameter's instrument default, when known (DDRUM-006). */
  paramDefault?: number;
  /** Enum choice names, in index order; empty when the parameter is not an enum (DDRUM-006). */
  choices: string[];
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
  const paramTitle = (): string | undefined => {
    const label = props.state().paramLabel;
    if (!label) return undefined;
    const def = props.state().paramDefault;
    return def === undefined ? label : `${label} (default ${def})`;
  };
  return (
    <div class="bind-row" data-binding={props.id} data-mode={props.state().mode} data-state={props.state().state}>
      <span class="bind-label" title={paramTitle()}>{props.state().label}</span>
      {props.state().choices.length > 0 ? (
        <select
          class="bind-select"
          disabled={props.state().state !== 'bound'}
          value={String(Math.round(props.state().rawValue))}
          on:change={(event) => props.onInput(Number(event.currentTarget.value))}
        >
          {props.state().choices.map((choice, index) => (
            <option value={String(index)}>{choice}</option>
          ))}
        </select>
      ) : (
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
      )}
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

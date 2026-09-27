// The transport bar view (design 15.1.5 "Transport bar", 15.2). Icons
// instead of words: every control has an accessible name and a tooltip
// carrying the text the icon replaces. State arrives as accessors; the view
// owns no protocol logic.

import { For, Match, Show, Switch, type Accessor, type JSX } from 'solid-js';
import type { AudioState } from './audio';
import { audioLabel } from './audio';
import {
  Alert,
  Check,
  ClockInternal,
  ClockMidiLocked,
  ClockMidiLost,
  Hourglass,
  Hush,
  Metronome,
  Play,
  SpeakerOff,
  SpeakerOn,
  StopSquare,
} from './icons';

export type ClockStatus = 'internal' | 'midi locked' | 'midi lost';

export type EvalStatus =
  | { kind: 'idle' }
  | { kind: 'pending' }
  | { kind: 'ok' }
  | { kind: 'diagnostics'; errors: number; warnings: number }
  | { kind: 'not-delivered'; reason: string };

export interface Position {
  cycle: number;
  beat: number;
  beatsPerCycle: number;
}

export interface SlotView {
  name: string;
  lit: Accessor<boolean>;
}

export interface TransportViewProps {
  bpm: Accessor<number>;
  position: Accessor<Position | null>;
  clock: Accessor<ClockStatus>;
  /** Master level in dBFS; null when unknown, -Infinity when silent. */
  level: Accessor<number | null>;
  slots: Accessor<SlotView[]>;
  audio: Accessor<AudioState>;
  audioReason: Accessor<string>;
  evalStatus: Accessor<EvalStatus>;
  onAudio: () => void;
  onRun?: () => void;
  onHush: () => void;
  onStopAll: () => void;
  onStop: (slot: string) => void;
}

function IconButton(props: {
  class: string;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: JSX.Element;
}): JSX.Element {
  return (
    <button
      type="button"
      class={`vact-icon-button ${props.class}`}
      aria-label={props.label}
      title={props.label}
      disabled={props.disabled}
      onClick={() => props.onClick()}
    >
      {props.children}
    </button>
  );
}

function evalLabel(s: EvalStatus): string {
  switch (s.kind) {
    case 'idle':
      return 'not evaluated yet (Run, or Mod-Shift-Enter)';
    case 'pending':
      return 'evaluating';
    case 'ok':
      return 'evaluated: ok';
    case 'diagnostics':
      return `evaluated: ${s.errors} error${s.errors === 1 ? '' : 's'}, ${s.warnings} warning${s.warnings === 1 ? '' : 's'}`;
    case 'not-delivered':
      return `eval not delivered: ${s.reason}`;
  }
}

/** The beat ring: one dot per beat, the current one filled. */
function BeatRing(props: { position: Accessor<Position | null> }): JSX.Element {
  const beats = () => Math.max(1, Math.min(16, props.position()?.beatsPerCycle ?? 4));
  return (
    <svg class="vact-beat-ring" viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
      <For each={Array.from({ length: beats() }, (_, i) => i)}>
        {(i) => {
          const a = () => (2 * Math.PI * i) / beats() - Math.PI / 2;
          return (
            <circle
              cx={12 + 8 * Math.cos(a())}
              cy={12 + 8 * Math.sin(a())}
              r="2.4"
              class="vact-beat-dot"
              data-on={props.position()?.beat === i + 1 ? 'true' : 'false'}
            />
          );
        }}
      </For>
    </svg>
  );
}

export function TransportView(props: TransportViewProps): JSX.Element {
  const positionLabel = () => {
    const p = props.position();
    return p ? `cycle ${p.cycle} beat ${p.beat}` : 'not playing';
  };
  const levelText = () => {
    const l = props.level();
    if (l === null) return '--';
    if (!Number.isFinite(l)) return '-inf';
    return l.toFixed(1);
  };
  return (
    <div class="vact-transport" role="toolbar" aria-label="transport">
      <IconButton
        class="vact-audio"
        label={audioLabel(props.audio(), props.audioReason())}
        onClick={props.onAudio}
        disabled={props.audio() === 'unavailable' || props.audio() === 'starting'}
      >
        <span class="vact-audio-state" data-state={props.audio()}>
          <Switch fallback={<SpeakerOff />}>
            <Match when={props.audio() === 'running' || props.audio() === 'unavailable'}>
              <SpeakerOn />
            </Match>
            <Match when={props.audio() === 'failed'}>
              <Alert />
            </Match>
          </Switch>
        </span>
      </IconButton>
      <Show when={props.onRun}>
        <IconButton class="vact-run" label="run the whole document (Mod-Shift-Enter)" onClick={() => props.onRun?.()}>
          <Play />
        </IconButton>
      </Show>
      <span class="vact-eval-status" data-status={props.evalStatus().kind} role="status" aria-label={evalLabel(props.evalStatus())} title={evalLabel(props.evalStatus())}>
        <Switch>
          <Match when={props.evalStatus().kind === 'pending'}>
            <Hourglass />
          </Match>
          <Match when={props.evalStatus().kind === 'ok'}>
            <Check />
          </Match>
          <Match when={props.evalStatus().kind === 'diagnostics' || props.evalStatus().kind === 'not-delivered'}>
            <Alert />
          </Match>
        </Switch>
        <Show when={props.evalStatus().kind === 'diagnostics'}>
          <span class="vact-eval-count">
            {(() => {
              const s = props.evalStatus();
              return s.kind === 'diagnostics' ? String(s.errors + s.warnings) : '';
            })()}
          </span>
        </Show>
      </span>
      <span class="vact-tempo" aria-label={`tempo ${props.bpm().toFixed(1)} bpm`} title={`${props.bpm().toFixed(1)} bpm`}>
        <Metronome />
        <span class="vact-tempo-value">{props.bpm().toFixed(1)}</span>
      </span>
      <span class="vact-position" aria-label={positionLabel()} title={positionLabel()}>
        <BeatRing position={props.position} />
        <span class="vact-position-value">
          {(() => {
            const p = props.position();
            return p ? `${p.cycle}.${p.beat}` : '-.-';
          })()}
        </span>
      </span>
      <span class="vact-clock" data-status={props.clock().replace(' ', '-')} aria-label={`clock: ${props.clock()}`} title={`clock: ${props.clock()}`}>
        <Switch fallback={<ClockInternal />}>
          <Match when={props.clock() === 'midi locked'}>
            <ClockMidiLocked />
          </Match>
          <Match when={props.clock() === 'midi lost'}>
            <ClockMidiLost />
          </Match>
        </Switch>
      </span>
      <IconButton class="vact-hush" label="hush: silence everything (Mod-.)" onClick={props.onHush}>
        <Hush />
      </IconButton>
      <IconButton class="vact-stop-all" label="stop every slot" onClick={props.onStopAll}>
        <StopSquare />
      </IconButton>
      <span class="vact-level" aria-label={`master level ${levelText()} dB`} title={`master level ${levelText()} dB`}>
        <SpeakerOn />
        <span class="vact-level-value">{levelText()}</span>
      </span>
      <ul class="vact-slots">
        <For each={props.slots()}>
          {(slot) => (
            <li class="vact-slot" data-slot={slot.name}>
              <span class="vact-light" data-lit={slot.lit() ? 'true' : 'false'} aria-hidden="true" />
              <span class="vact-slot-name">{slot.name}</span>
              <IconButton class="vact-mute" label={`stop ${slot.name}`} onClick={() => props.onStop(slot.name)}>
                <StopSquare />
              </IconButton>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}

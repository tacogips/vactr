// Grouped editor transport (design-editor-header.md). State arrives as
// accessors; this view owns disclosure interactions, not protocol logic.

import { For, Match, Show, Switch, onCleanup, onMount, type Accessor, type JSX } from 'solid-js';
import type { AudioState } from './audio';
import { DEFAULT_UNAPPLIED_COLOR } from '../code/unapplied-settings';
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
  Pause,
  Update,
  ChevronDown,
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

export type PlaybackState = 'stopped' | 'playing' | 'paused';

export interface TransportViewProps {
  playback: Accessor<PlaybackState>;
  pauseReason: Accessor<string>;
  onToggle: () => void;
  onPause: () => void;
  bpm: Accessor<number>;
  position: Accessor<Position | null>;
  clock: Accessor<ClockStatus>;
  /** Master level in dBFS; null when unknown, -Infinity when silent. */
  level: Accessor<number | null>;
  output: Accessor<'running' | 'draining' | 'cutting' | 'idle'>;
  slots: Accessor<SlotView[]>;
  audio: Accessor<AudioState>;
  audioReason: Accessor<string>;
  evalStatus: Accessor<EvalStatus>;
  onAudio: () => void;
  onRun?: () => void;
  vim?: { enabled: Accessor<boolean>; toggle: () => void };
  unapplied?: { color: Accessor<string>; setColor: (color: string) => void };
  onHush: () => void;
  onStop: (slot: string) => void;
}

function IconButton(props: {
  class: string;
  label: string | Accessor<string>;
  onClick: () => void;
  disabled?: boolean;
  ariaPressed?: Accessor<'true' | 'false'>;
  children: JSX.Element;
}): JSX.Element {
  const label = () => typeof props.label === 'function' ? props.label() : props.label;
  return (
    <button
      type="button"
      class={`vact-icon-button ${props.class}`}
      aria-label={label()}
      title={label()}
      aria-pressed={props.ariaPressed?.()}
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
      return 'not evaluated yet (Play, Update, or Mod-Shift-Enter)';
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
  let toolbar!: HTMLDivElement;
  onMount(() => {
    const doc = toolbar.ownerDocument;
    const dismiss = (event: PointerEvent | KeyboardEvent) => {
      for (const details of toolbar.querySelectorAll<HTMLDetailsElement>('details[open]')) {
        if (event instanceof KeyboardEvent ? event.key === 'Escape' : !details.contains(event.target as Node)) {
          details.open = false;
          if (event instanceof KeyboardEvent) { details.querySelector('summary')?.focus(); event.preventDefault(); }
        }
      }
    };
    doc.addEventListener('pointerdown', dismiss);
    doc.addEventListener('keydown', dismiss);
    onCleanup(() => { doc.removeEventListener('pointerdown', dismiss); doc.removeEventListener('keydown', dismiss); });
  });
  return (
    <div ref={toolbar} class="vact-transport" role="toolbar" aria-label="transport">
      <div class="vact-transport-group vact-playback-group" role="group" aria-label="Playback">
        <button type="button" class="vact-play-toggle vact-primary"
          classList={{ 'vact-stop-all': props.playback() !== 'stopped' }}
          data-output={props.output()} aria-pressed={props.playback() !== 'stopped'}
          aria-label={props.playback() === 'stopped' ? 'Play document' : 'Stop playback (Mod-.)'}
          title={props.playback() === 'stopped' ? 'Play the whole document' : 'Stop every slot and let effects ring out (Mod-.)'}
          disabled={props.playback() === 'stopped' && !props.onRun}
          onClick={props.onToggle}>
          <Show when={props.playback() !== 'stopped'} fallback={<Play />}><StopSquare /></Show>
          {props.playback() === 'stopped' ? 'Play' : 'Stop'}
        </button>
        <IconButton class="vact-pause"
          label={() => props.pauseReason() || (props.playback() === 'paused' ? 'Resume playback' : 'Pause scheduled music and position; visuals continue')}
          disabled={props.playback() === 'stopped' || !!props.pauseReason()}
          ariaPressed={() => props.playback() === 'paused' ? 'true' : 'false'} onClick={props.onPause}>
          <Show when={props.playback() === 'paused'} fallback={<Pause />}><Play /></Show>
        </IconButton>
        <span class="vact-playback-state" data-state={props.playback()} role="status">
          {props.playback() === 'paused' ? 'Paused' : props.playback() === 'playing' ? 'Playing'
            : props.output() === 'draining' ? 'Effects tail' : props.output() === 'cutting' ? 'Cutting' : 'Stopped'}
        </span>
      </div>
      <div class="vact-transport-group vact-time-group" role="group" aria-label="Position and tempo">
        <span class="vact-tempo" aria-label={`tempo ${props.bpm().toFixed(1)} bpm`} title={`${props.bpm().toFixed(1)} bpm`}>
          <Metronome />
          <span class="vact-tempo-value">{props.bpm().toFixed(1)}</span><span class="vact-unit">BPM</span>
        </span>
        <span class="vact-position" aria-label={positionLabel()} title={positionLabel()}>
          <BeatRing position={props.position} /><span class="vact-unit">CYCLE</span>
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
      </div>
      <div class="vact-transport-group vact-evaluate-group" role="group" aria-label="Live code">
        <Show when={props.onRun}>
          <button type="button" class="vact-run" title="Update the whole document (Mod-Shift-Enter)"
            aria-label="Update document (Mod-Shift-Enter)" onClick={() => props.onRun?.()}>
            <Update /><span>Update</span>
          </button>
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
      </div>
      <div class="vact-transport-group vact-output-group" role="group" aria-label="Audio output">
        <IconButton class="vact-hush" label="cut: silence everything now and clear effects (Mod-Shift-.)" onClick={props.onHush}>
          <Hush />
        </IconButton>
        <IconButton class="vact-audio" label={audioLabel(props.audio(), props.audioReason())} onClick={props.onAudio}
          disabled={props.audio() === 'unavailable' || props.audio() === 'starting'}>
          <span class="vact-audio-state" data-state={props.audio()}>
            <Switch fallback={<SpeakerOff />}>
              <Match when={props.audio() === 'running' || props.audio() === 'unavailable'}><SpeakerOn /></Match>
              <Match when={props.audio() === 'failed'}><Alert /></Match>
            </Switch>
          </span>
        </IconButton>
        <span class="vact-level" aria-label={`master level ${levelText()} dB`} title={`master level ${levelText()} dB`}>
          <SpeakerOn />
          <span class="vact-level-value">{levelText()}</span><span class="vact-unit">dB</span>
        </span>
      </div>
      <EditorOptions vim={props.vim} unapplied={props.unapplied} />
      <Show when={props.slots().length > 0}>
        <details class="vact-slot-disclosure"><summary>Slots <span>{props.slots().length}</span><ChevronDown /></summary>
          <div class="vact-header-popover">
            <ul class="vact-slots">
              <For each={props.slots()}>
                {(slot) => (
                  <li class="vact-slot" data-slot={slot.name}>
                    <span class="vact-light" data-lit={slot.lit() ? 'true' : 'false'} aria-hidden="true" />
                    <span class="vact-slot-name">{slot.name}</span>
                    <IconButton class="vact-mute" label={`stop ${slot.name}: release its notes`} onClick={() => props.onStop(slot.name)}>
                      <StopSquare />
                    </IconButton>
                  </li>
                )}
              </For>
            </ul>
          </div>
        </details>
      </Show>
    </div>
  );
}

function EditorOptions(props: { vim?: TransportViewProps['vim']; unapplied?: TransportViewProps['unapplied'] }): JSX.Element {
  return <details class="vact-editor-options">
    <summary>Editor <ChevronDown /></summary>
    <div class="vact-header-popover">
      <Show when={props.vim}>{(vim) => <div class="vact-editor-option">
        <span>Vim mode</span>
        <IconButton class="vact-vim-toggle" label={() => `Vim mode: ${vim().enabled() ? 'on' : 'off'}`}
          ariaPressed={() => vim().enabled() ? 'true' : 'false'} onClick={vim().toggle}>vim</IconButton>
      </div>}</Show>
      <Show when={props.unapplied}>{(setting) => <>
        <label class="vact-editor-option vact-unapplied-setting">
          <span>Unapplied edit color</span>
          <input type="color" aria-label="Unapplied edit color" value={setting().color()}
            onInput={(event) => setting().setColor(event.currentTarget.value)} />
        </label>
        <div class="vact-editor-option">
          <span class="vact-shortcut-note">Clears after a successful update.</span>
          <button type="button" class="vact-unapplied-reset" onClick={() => setting().setColor(DEFAULT_UNAPPLIED_COLOR)}>Reset color</button>
        </div>
      </>}</Show>
      <p class="vact-popover-heading">Keyboard shortcuts</p>
      <dl class="vact-shortcut-list">
        <dt>Evaluate form</dt><dd>Mod-Enter</dd>
        <dt>Update document</dt><dd>Mod-Shift-Enter</dd>
        <dt>Stop</dt><dd>Mod-.</dd>
        <dt>Cut immediately</dt><dd>Mod-Shift-.</dd>
        <dt>Toggle side pane</dt><dd>Mod-\</dd>
      </dl>
      <p class="vact-shortcut-note">Mod is Command on Mac, Ctrl elsewhere.</p>
    </div>
  </details>;
}

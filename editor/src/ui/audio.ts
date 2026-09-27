// The audio-state model (design 15.2 "Audio start and eval are never
// silent"). Browsers keep an AudioContext `suspended` until a user gesture;
// this module turns that into a visible state and one `start()` action.
//
// States: `off` before any start attempt, `starting` while `resume()` is
// pending, `running`, `suspended` (was running, then paused by the browser
// or the OS), `failed` (resume rejected; `reason` says why), and
// `unavailable` (native tier: audio lives in the engine, not the page).

import { createSignal, type Accessor } from 'solid-js';

export type AudioState = 'off' | 'starting' | 'running' | 'suspended' | 'failed' | 'unavailable';

/** The part of an `AudioContext` the model reads. */
export interface AudioContextLike {
  readonly state: string;
  resume(): Promise<void>;
  addEventListener?(type: 'statechange', listener: () => void): void;
  removeEventListener?(type: 'statechange', listener: () => void): void;
}

export interface AudioModel {
  state: Accessor<AudioState>;
  reason: Accessor<string>;
  /** Resumes the context; resolves after the state settles. */
  start(): Promise<void>;
  dispose(): void;
}

/** A model over `ctx`, or an `unavailable` model when there is none. */
export function audioModel(ctx: AudioContextLike | null): AudioModel {
  const [state, setState] = createSignal<AudioState>(ctx ? (ctx.state === 'running' ? 'running' : 'off') : 'unavailable');
  const [reason, setReason] = createSignal('');
  let everRan = state() === 'running';
  const sync = (): void => {
    if (!ctx) return;
    if (ctx.state === 'running') {
      everRan = true;
      setState('running');
      setReason('');
    } else if (ctx.state === 'closed') {
      setState('failed');
      setReason('the audio context is closed');
    } else if (state() !== 'starting' && state() !== 'failed') {
      setState(everRan ? 'suspended' : 'off');
    }
  };
  ctx?.addEventListener?.('statechange', sync);
  return {
    state,
    reason,
    async start() {
      if (!ctx) return;
      if (ctx.state === 'running') {
        sync();
        return;
      }
      setState('starting');
      try {
        await ctx.resume();
        if (ctx.state === 'running') {
          everRan = true;
          setState('running');
          setReason('');
        } else {
          setState('failed');
          setReason(`the browser kept audio ${ctx.state}`);
        }
      } catch (e) {
        setState('failed');
        setReason(e instanceof Error ? e.message : String(e));
      }
    },
    dispose() {
      ctx?.removeEventListener?.('statechange', sync);
    },
  };
}

/** The tooltip / accessible name of each state. */
export function audioLabel(state: AudioState, reason: string): string {
  switch (state) {
    case 'off':
      return 'audio off: click to start';
    case 'starting':
      return 'audio starting';
    case 'running':
      return 'audio running';
    case 'suspended':
      return 'audio suspended: click to resume';
    case 'failed':
      return `audio failed${reason ? `: ${reason}` : ''} (click to retry)`;
    case 'unavailable':
      return 'audio plays in the native engine';
  }
}

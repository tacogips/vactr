// Design 15.2 / TASK-011: the audio-state control, the eval outcome, the
// Run button, the icon-only transport and the foldable side pane.

import { afterEach, describe, expect, it } from 'vitest';
import { buildLayout } from '../../src/app/layout';
import { EVAL_TIMEOUT_MS, TransportBar, evalOutcome } from '../../src/code/transport';
import { Client } from '../../src/protocol/client';
import type { Timers } from '../../src/protocol/document';
import { Store } from '../../src/protocol/store';
import type { ServerEnvelope } from '../../src/protocol/types';
import { audioLabel, audioModel, type AudioContextLike } from '../../src/ui/audio';
import { SECTION_KEY, SIDE_KEY, mountShell, type StorageLike } from '../../src/ui/shell';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

/** An AudioContext stand-in: `suspended` until `resume()` succeeds. */
class FakeCtx implements AudioContextLike {
  state = 'suspended';
  fail: string | null = null;
  keep = false;
  private readonly listeners = new Set<() => void>();
  resume(): Promise<void> {
    if (this.fail) return Promise.reject(new Error(this.fail));
    if (!this.keep) this.set('running');
    return Promise.resolve();
  }
  set(state: string): void {
    this.state = state;
    for (const l of this.listeners) l();
  }
  addEventListener(_type: 'statechange', l: () => void): void {
    this.listeners.add(l);
  }
  removeEventListener(_type: 'statechange', l: () => void): void {
    this.listeners.delete(l);
  }
}

class ManualTimers implements Timers {
  private next = 1;
  readonly pending = new Map<number, () => void>();
  set(fn: () => void): unknown {
    const id = this.next;
    this.next += 1;
    this.pending.set(id, fn);
    return id;
  }
  clear(h: unknown): void {
    this.pending.delete(h as number);
  }
  fireAll(): void {
    const fns = [...this.pending.values()];
    this.pending.clear();
    for (const f of fns) f();
  }
}

class MemoryStorage implements StorageLike {
  readonly map = new Map<string, string>();
  getItem(k: string): string | null {
    return this.map.get(k) ?? null;
  }
  setItem(k: string, v: string): void {
    this.map.set(k, v);
  }
}

const flush = () => new Promise((r) => setTimeout(r, 0));

const cleanups: (() => void)[] = [];
afterEach(() => {
  for (const c of cleanups.splice(0)) c();
  document.body.innerHTML = '';
});

function bar(opts: { ctx?: FakeCtx | null; onRun?: () => void; timers?: Timers } = {}) {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store() });
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const b = new TransportBar(parent, {
    client,
    clock: new MockClock(0),
    audio: opts.ctx ?? null,
    ...(opts.onRun ? { onRun: opts.onRun } : {}),
    ...(opts.timers ? { timers: opts.timers } : {}),
  });
  cleanups.push(() => b.dispose());
  const q = <T extends Element = HTMLElement>(sel: string) => parent.querySelector<T>(sel);
  return { b, parent, transport, q };
}

describe('audio state (design 15.2)', () => {
  it('shows off before a gesture, running after a click, and a reason when resume fails', async () => {
    const ctx = new FakeCtx();
    const { b, q } = bar({ ctx });
    const state = () => q('.vact-audio-state')?.dataset.state;
    expect(state()).toBe('off');
    expect(q('.vact-audio')?.getAttribute('aria-label')).toBe('audio off: click to start');
    q<HTMLButtonElement>('.vact-audio')?.click();
    await flush();
    expect(state()).toBe('running');
    // The browser suspends the context later: visible as suspended, not off.
    ctx.set('suspended');
    expect(state()).toBe('suspended');
    ctx.fail = 'NotAllowedError: no user gesture';
    await b.audio.start();
    expect(state()).toBe('failed');
    expect(q('.vact-audio')?.getAttribute('aria-label')).toContain('NotAllowedError');
  });

  it('reports a resume that leaves the context suspended as failed', async () => {
    const ctx = new FakeCtx();
    ctx.keep = true;
    const m = audioModel(ctx);
    await m.start();
    expect(m.state()).toBe('failed');
    expect(m.reason()).toBe('the browser kept audio suspended');
    m.dispose();
  });

  it('is unavailable (and the button disabled) on the native tier', () => {
    const { q } = bar({ ctx: null });
    expect(q('.vact-audio-state')?.dataset.state).toBe('unavailable');
    expect(q<HTMLButtonElement>('.vact-audio')?.disabled).toBe(true);
    expect(audioLabel('unavailable', '')).toBe('audio plays in the native engine');
  });
});

describe('eval outcome (design 15.2)', () => {
  const result = (diags: { severity: 'error' | 'warning' | 'hint' }[], failures = 0): ServerEnvelope =>
    ({
      v: 1,
      kind: 'eval-result',
      body: {
        file: 'main.vact',
        doc_revision: 1,
        forms: Array.from({ length: failures }, () => ({ span: { start: 0, end: 1 }, failure: 'x' })),
        diagnostics: diags.map((d) => ({ code: 'c', message: 'm', span: { start: 0, end: 1 }, file: 'main.vact', ...d })),
        sites: [],
        directives: { directives: [], labels: [] },
      },
    }) as unknown as ServerEnvelope;

  it('goes pending, then ok or n diagnostics', async () => {
    const { b, q } = bar({ timers: new ManualTimers() });
    const status = () => q('.vact-eval-status')?.dataset.status;
    expect(status()).toBe('idle');
    b.evalStarted(Promise.resolve(result([])));
    expect(status()).toBe('pending');
    await flush();
    expect(status()).toBe('ok');
    b.evalStarted(Promise.resolve(result([{ severity: 'error' }, { severity: 'warning' }, { severity: 'hint' }], 1)));
    await flush();
    expect(status()).toBe('diagnostics');
    expect(q('.vact-eval-count')?.textContent).toBe('3');
    expect(q('.vact-eval-status')?.getAttribute('aria-label')).toBe('evaluated: 2 errors, 1 warning');
    expect(evalOutcome(result([]))).toEqual({ kind: 'ok' });
  });

  it('shows not delivered when no reply arrives in time or the request fails', async () => {
    const timers = new ManualTimers();
    const { b, q } = bar({ timers });
    b.evalStarted(new Promise<ServerEnvelope>(() => undefined));
    expect(timers.pending.size).toBe(1);
    timers.fireAll();
    expect(q('.vact-eval-status')?.dataset.status).toBe('not-delivered');
    expect(EVAL_TIMEOUT_MS).toBeGreaterThan(0);
    b.evalStarted(Promise.reject(new Error('client closed')));
    await flush();
    expect(q('.vact-eval-status')?.getAttribute('aria-label')).toBe('eval not delivered: client closed');
  });

  it('a newer eval supersedes the pending one', async () => {
    const timers = new ManualTimers();
    const { b, q } = bar({ timers });
    let resolveOld!: (e: ServerEnvelope) => void;
    b.evalStarted(new Promise<ServerEnvelope>((r) => (resolveOld = r)));
    b.evalStarted(Promise.resolve(result([])));
    await flush();
    resolveOld(result([{ severity: 'error' }]));
    await flush();
    expect(q('.vact-eval-status')?.dataset.status).toBe('ok');
  });

  it('the Run button calls onRun', () => {
    let runs = 0;
    const { q } = bar({ onRun: () => (runs += 1) });
    q<HTMLButtonElement>('.vact-run')?.click();
    expect(runs).toBe(1);
    const noRun = bar();
    expect(noRun.q('.vact-run')).toBeNull();
  });
});

describe('icon-only transport (design 15.2)', () => {
  it('shows no words for tempo, clock, hush or stop; every control has a name and a tooltip', () => {
    const { q, parent, b, transport } = bar({ ctx: new FakeCtx(), onRun: () => undefined });
    b.onPlaying([{ slot: 'd1', beat: [0, 1], time: 0, dur: [1, 1] }]);
    for (const sel of ['.vact-audio', '.vact-run', '.vact-hush', '.vact-stop-all', '.vact-clock']) {
      const el = q(sel);
      expect(el, sel).not.toBeNull();
      expect(el?.textContent?.trim(), sel).toBe('');
      expect(el?.getAttribute('aria-label'), sel).toBeTruthy();
      expect(el?.getAttribute('title'), sel).toBeTruthy();
      expect(el?.querySelector('svg'), sel).not.toBeNull();
    }
    // Numbers stay visible; the unit words live in the name.
    expect(q('.vact-tempo')?.textContent).toBe('120.0');
    expect(parent.textContent).not.toMatch(/bpm|internal|hush|mute|master/);
    q<HTMLButtonElement>('.vact-stop-all')?.click();
    expect(transport.of('stop-all').map((m) => m.body)).toEqual([{}]);
  });
});

describe('foldable side pane (design 15.2)', () => {
  function editorRoot(storage: StorageLike | null) {
    const root = document.createElement('div');
    document.body.appendChild(root);
    const layout = buildLayout(root);
    const marker = document.createElement('input');
    marker.className = 'kept';
    layout.right.appendChild(marker);
    const shell = mountShell(root, storage);
    cleanups.push(() => shell.dispose());
    return { root, layout, shell, marker };
  }

  it('folds to a rail by button and Mod-\\, keeping the areas mounted', () => {
    const storage = new MemoryStorage();
    const { root, shell, marker } = editorRoot(storage);
    expect(root.dataset.sideFolded).toBe('false');
    const button = root.querySelector<HTMLButtonElement>('.vact-side-fold');
    expect(button?.getAttribute('aria-expanded')).toBe('true');
    button?.click();
    expect(root.dataset.sideFolded).toBe('true');
    expect(button?.getAttribute('aria-expanded')).toBe('false');
    // Folding hides, it does not remove: slider state and learned bindings survive.
    expect(root.contains(marker)).toBe(true);
    expect(storage.getItem(SIDE_KEY)).toBe('true');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '\\', ctrlKey: true }));
    expect(shell.sideFolded()).toBe(false);
    expect(storage.getItem(SIDE_KEY)).toBe('false');
  });

  it('restores the folded state and section folds from storage', () => {
    const storage = new MemoryStorage();
    storage.setItem(SIDE_KEY, 'true');
    storage.setItem(SECTION_KEY('visual'), 'true');
    const { root, layout } = editorRoot(storage);
    expect(root.dataset.sideFolded).toBe('true');
    expect(layout.visual.dataset.collapsed).toBe('true');
    expect(layout.right.dataset.collapsed).toBe('false');
  });

  it('folds each section independently', () => {
    const storage = new MemoryStorage();
    const { root, layout, shell } = editorRoot(storage);
    const header = root.querySelector<HTMLElement>('.pane-section-header[data-section="analyzers"] button');
    expect(header?.textContent).toBe('analyzers');
    header?.click();
    expect(layout.analyzers.dataset.collapsed).toBe('true');
    expect(layout.right.dataset.collapsed).toBe('false');
    expect(shell.sectionFolded('analyzers')).toBe(true);
    expect(storage.getItem(SECTION_KEY('analyzers'))).toBe('true');
  });

  it('works when storage throws', () => {
    const broken: StorageLike = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    };
    const { root } = editorRoot(broken);
    root.querySelector<HTMLButtonElement>('.vact-side-fold')?.click();
    expect(root.dataset.sideFolded).toBe('true');
  });
});

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps, Mounted } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { AudibleClock } from '../../src/app/clock';
import { DOC_FILE, mount } from '../../src/code/mount';
import { TransportBar, clockStatus, formatLevel, type TransportOptions } from '../../src/code/transport';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { TempoBody, TransportSample, WirePlaying } from '../../src/protocol/types';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

const cleanups: (() => void)[] = [];
afterEach(() => { for (const dispose of cleanups.splice(0).reverse()) dispose(); document.body.innerHTML = ''; });

function track(bar: TransportBar): () => void {
  let active = true;
  const dispose = () => { if (active) { active = false; bar.dispose(); } };
  cleanups.push(dispose);
  return dispose;
}

function setup(options: Pick<TransportOptions, 'platform' | 'shortcuts' | 'sample' | 'onHush'> = {}) {
  const clock = new MockClock(10);
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store() });
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const bar = new TransportBar(parent, { client, clock, ...options });
  const dispose = track(bar);
  const text = (cls: string) => parent.querySelector(`.${cls}`)?.textContent;
  // Design 15.2: icons carry their words as the accessible name.
  const label = (cls: string) => parent.querySelector(`.${cls}`)?.getAttribute('aria-label');
  return { clock, transport, client, parent, bar, text, label, dispose };
}

const tempo = (extra: Partial<TempoBody> = {}): TempoBody => ({ bpm: 120, beats_per_cycle: 4, cycle: [3, 1], ...extra });
const play = (slot: string, time: number): WirePlaying => ({ slot, beat: [0, 1], time, dur: [1, 4] });

describe('TransportBar', () => {
  it('projects the current transport sample at audible time and hides stale positions', () => {
    const clock = new MockClock(10);
    let audibleTime = 10.04;
    let correlationValid = true;
    let sample: TransportSample = {
      epoch: 'run-1', sample_time: 10, cycle: [2, 1], bpm: 120, beats_per_cycle: 4,
      running: true, latency_seconds: null, latency_kind: 'unavailable', uncertainty_seconds: null,
    };
    const client = new Client(new RecordingTransport());
    const parent = document.createElement('div'); document.body.appendChild(parent);
    const bar = new TransportBar(parent, { client, clock,
      audible: new AudibleClock({ at: () => correlationValid
        ? { time: audibleTime, uncertainty: 0, provenance: 'measured' } : null }),
      sample: () => sample });
    const dispose = track(bar);
    const position = parent.querySelector<HTMLElement>('.vact-position');
    bar.tick(0);
    expect(bar.state.cycle).toBeCloseTo(2.02);
    expect(bar.state.beatFlash).toBe(true);
    expect(position?.dataset.sync).toBe('visible');
    expect(position?.dataset.beatFlash).toBe('on');
    audibleTime = 12.01;
    bar.tick(16.7);
    expect(bar.state).toEqual({ cycle: null, beatFlash: false, hidden: true });
    expect(position?.dataset.sync).toBe('hidden');
    sample = { ...sample, sample_time: 12.01, running: false };
    audibleTime = 12.01;
    bar.tick(33.4);
    expect(bar.state.hidden).toBe(false);
    correlationValid = false;
    bar.tick(50.1);
    expect(bar.state).toEqual({ cycle: null, beatFlash: false, hidden: true });
    expect(position?.dataset.beatFlash).toBe('off');
    correlationValid = true;
    audibleTime = 12.02;
    bar.tick(66.8);
    expect(bar.state.hidden).toBe(false);
    dispose();
  });

  it('tracks analytic cycle and beat windows through five minutes of frame drops and stalls', () => {
    const clock = new MockClock();
    const client = new Client(new RecordingTransport());
    const parent = document.createElement('div'); document.body.appendChild(parent);
    const audible = new AudibleClock({ at: (pageMs) => ({
      time: pageMs / 1000, uncertainty: 0, provenance: 'measured',
    }) });
    let sample: TransportSample = {
      epoch: 'run-1', sample_time: 0, cycle: [0, 1], bpm: 120, beats_per_cycle: 4,
      running: true, latency_seconds: null, latency_kind: 'unavailable', uncertainty_seconds: null,
    };
    const bar = new TransportBar(parent, { client, clock, audible, sample: () => sample });
    const dispose = track(bar);
    const beatDuration = 0.5;
    let frameMs = 0;
    let nextStallMs = 10000;
    let random = 0x13579bdf;
    let expectedFlashes = 0;
    let actualFlashes = 0;
    while (frameMs < 300000) {
      random = (random * 1664525 + 1013904223) >>> 0;
      frameMs += (1000 / 60) * (2 + (random % 3));
      if (frameMs >= nextStallMs) {
        frameMs += 250;
        nextStallMs += 10000;
      }
      const targetTime = audible.sample(frameMs).time;
      sample = {
        ...sample,
        sample_time: targetTime - 0.05,
        cycle: [(targetTime - 0.05) / 2, 1],
      };
      bar.tick(frameMs);
      expect(bar.state.cycle).toBeCloseTo(targetTime / 2, 9);
      const beatStart = Math.floor(targetTime / beatDuration) * beatDuration;
      const expectedFlash = targetTime >= beatStart && targetTime < beatStart + 0.08;
      expect(bar.state.beatFlash).toBe(expectedFlash);
      if (expectedFlash) expectedFlashes += 1;
      if (bar.state.beatFlash) actualFlashes += 1;
    }
    expect(actualFlashes).toBe(expectedFlashes);
    dispose();
  });

  it('renders tempo and extrapolates cycle/beat on the clock between messages', () => {
    const { clock, bar, text, label } = setup();
    bar.onTempo(tempo());
    expect(text('vact-tempo-value')).toBe('120.0');
    expect(label('vact-tempo')).toBe('tempo 120.0 bpm');
    expect(label('vact-position')).toBe('cycle 3 beat 1');
    expect(text('vact-position-value')).toBe('3.1');
    // 120 bpm, 4 beats per cycle: 0.5 s is one beat.
    clock.advance(0.5);
    bar.tick();
    expect(bar.cycles()).toBeCloseTo(3.25);
    expect(label('vact-position')).toBe('cycle 3 beat 2');
    clock.advance(1.5);
    bar.tick();
    expect(label('vact-position')).toBe('cycle 4 beat 1');
  });

  it('shows the MIDI clock status, including midi lost (position held)', () => {
    const { clock, bar, label, parent } = setup();
    expect(clockStatus(null)).toBe('internal');
    bar.onTempo(tempo({ clock: { source: 'internal' } }));
    expect(label('vact-clock')).toBe('clock: internal');
    bar.onTempo(tempo({ bpm: 98, clock: { source: 'midi', locked: true } }));
    expect(label('vact-clock')).toBe('clock: midi locked');
    expect(label('vact-tempo')).toBe('tempo 98.0 bpm');
    bar.onTempo(tempo({ clock: { source: 'midi', locked: false } }));
    expect(label('vact-clock')).toBe('clock: midi lost');
    expect(parent.querySelector<HTMLElement>('.vact-clock')?.dataset.status).toBe('midi-lost');
    clock.advance(2);
    bar.tick();
    expect(bar.cycles()).toBe(3);
  });

  it('sends hush from the hush/panic button and stop from a slot mute', () => {
    const { transport, bar, parent } = setup();
    expect(parent.querySelector<HTMLButtonElement>('.vact-hush')?.getAttribute('aria-label')).toBe('cut: silence everything now and clear effects (Mod-Shift-.)');
    parent.querySelector<HTMLButtonElement>('.vact-hush')?.click();
    bar.onPlaying([play('d2', 11), play('d1', 11)]);
    expect(bar.slotNames()).toEqual(['d1', 'd2']);
    expect([...parent.querySelectorAll<HTMLElement>('.vact-slot')].map((li) => li.dataset.slot)).toEqual(['d1', 'd2']);
    expect(parent.querySelector<HTMLButtonElement>('[data-slot="d2"] .vact-mute')?.getAttribute('aria-label')).toBe('stop d2: release its notes');
    parent.querySelector<HTMLButtonElement>('[data-slot="d2"] .vact-mute')?.click();
    expect(transport.kinds()).toEqual(['hush', 'stop']);
    expect(transport.of('stop')[0]?.body).toEqual({ slot: 'd2' });
  });

  it('captures stop shortcuts once from any focus, ignores repeats, and removes the listener on dispose', () => {
    const onHush = vi.fn();
    const { transport, parent, dispose } = setup({ platform: 'Linux', onHush });
    const textarea = document.createElement('textarea');
    const targetKeydown = vi.fn();
    parent.appendChild(textarea);
    textarea.addEventListener('keydown', targetKeydown);
    textarea.focus();

    const stop = new KeyboardEvent('keydown', { key: '.', code: 'Period', ctrlKey: true, bubbles: true, cancelable: true });
    textarea.dispatchEvent(stop);
    expect(stop.defaultPrevented).toBe(true);
    expect(targetKeydown).not.toHaveBeenCalled();
    expect(transport.kinds()).toEqual(['stop-all']);
    expect(transport.of('stop-all')[0]?.body).toEqual({});
    expect(onHush).toHaveBeenCalledOnce();

    textarea.dispatchEvent(new KeyboardEvent('keydown', { key: '.', code: 'Period', ctrlKey: true, repeat: true, bubbles: true, cancelable: true }));
    expect(transport.kinds()).toEqual(['stop-all']);
    textarea.dispatchEvent(new KeyboardEvent('keydown', { key: '>', code: 'Period', ctrlKey: true, shiftKey: true, bubbles: true, cancelable: true }));
    expect(transport.kinds()).toEqual(['stop-all', 'hush']);
    expect(onHush).toHaveBeenCalledTimes(2);

    dispose();
    textarea.dispatchEvent(new KeyboardEvent('keydown', { key: '.', code: 'Period', ctrlKey: true, bubbles: true, cancelable: true }));
    expect(transport.kinds()).toEqual(['stop-all', 'hush']);
  });

  it('uses Meta only on Mac and Ctrl does not trigger there', () => {
    const { transport, parent } = setup({ platform: 'MacIntel' });
    const input = document.createElement('input');
    parent.appendChild(input);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: '.', code: 'Period', ctrlKey: true, bubbles: true, cancelable: true }));
    expect(transport.kinds()).toEqual([]);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: '.', code: 'Period', metaKey: true, bubbles: true, cancelable: true }));
    expect(transport.kinds()).toEqual(['stop-all']);
  });

  it('sends one stop-all for all slots and clears highlights', () => {
    const onHush = vi.fn();
    const { transport, parent, bar } = setup({ onHush });
    bar.onPlaying([play('d1', 11), play('d2', 11)]);
    parent.querySelector<HTMLButtonElement>('.vact-stop-all')?.click();
    expect(transport.of('stop-all').map((env) => env.body)).toEqual([{}]);
    expect(transport.of('stop')).toEqual([]);
    expect(onHush).toHaveBeenCalledOnce();
  });

  it('projects output state only on transitions and bounds output history', () => {
    let sample: TransportSample = {
      epoch: 'run-1', sample_time: 10, cycle: [0, 1], bpm: 120, beats_per_cycle: 4,
      running: true, latency_seconds: null, latency_kind: 'unavailable', uncertainty_seconds: null, output: 'draining',
    };
    const { bar, parent } = setup({ sample: () => sample });
    const stop = parent.querySelector<HTMLButtonElement>('.vact-stop-all');
    bar.tick();
    expect(stop?.dataset.output).toBe('draining');
    expect(stop?.getAttribute('aria-label')).toBe('stop: stop every slot, let effects ring out (Mod-.) - tails ringing out');
    expect(stop?.getAttribute('title')).toBe(stop?.getAttribute('aria-label'));
    expect(bar.outputHistory()).toEqual(['draining']);
    bar.tick();
    expect(bar.outputHistory()).toEqual(['draining']);
    sample = { ...sample, output: 'idle' };
    bar.tick();
    expect(stop?.dataset.output).toBe('idle');
    expect(stop?.getAttribute('aria-label')).toBe('stop: stop every slot, let effects ring out (Mod-.)');
    expect(bar.outputHistory()).toEqual(['draining', 'idle']);
    const states: Array<NonNullable<TransportSample['output']>> = ['running', 'draining', 'cutting', 'idle'];
    for (let index = 0; index < 20; index += 1) {
      sample = { ...sample, output: states[index % states.length] };
      bar.tick();
    }
    expect(bar.outputHistory()).toHaveLength(16);
  });

  it('lights a slot for 150 ms from its event time', () => {
    const { clock, bar } = setup();
    bar.onPlaying([play('d1', 11)]);
    const at = (t: number) => {
      clock.set(t);
      bar.tick();
      return bar.lit('d1');
    };
    expect(at(10.99)).toBe(false);
    expect(at(11)).toBe(true);
    expect(at(11.149)).toBe(true);
    expect(at(11.15)).toBe(false);
  });

  it('adds slots named by diagnostics and reads the master level', () => {
    const { bar, text, label } = setup();
    bar.onDiagnostics([{ code: 'x', severity: 'error', message: 'm', span: { start: 0, end: 1 }, file: 'f', slot: 'bass' }]);
    expect(bar.slotNames()).toEqual(['bass']);
    bar.onLevels({ levels: [{ source: 'master', rms: 0.5 }] });
    expect(text('vact-level-value')).toBe('-6.0');
    expect(label('vact-level')).toBe('master level -6.0 dB');
    expect(formatLevel(0)).toBe('master -inf dB');
    expect(formatLevel(undefined)).toBe('master --');
  });
});

describe('code area mount', () => {
  const mounted: Mounted[] = [];
  afterEach(() => {
    for (const m of mounted.splice(0)) m.dispose();
  });

  function mountArea() {
    const root = document.createElement('div');
    document.body.appendChild(root);
    const layout = buildLayout(root);
    const store = new Store();
    const transport = new RecordingTransport();
    const deps: EditorDeps = {
      client: new Client(transport, { store }),
      store,
      clock: new MockClock(0),
      tier: 'native',
      files: new MemoryFiles(),
    };
    const m = mount(root, deps);
    mounted.push(m);
    return { root, layout, store, transport, deps, m };
  }

  it('sets deps.code, routes store messages to the transport bar and sends nothing on its own', () => {
    const { layout, transport, deps, m } = mountArea();
    const code = deps.code;
    expect(code).toBeDefined();
    expect(transport.sent).toEqual([]);
    expect(layout.code.querySelector('canvas.vact-code-canvas')).not.toBeNull();
    expect(layout.code.querySelector('.vact-samples')).not.toBeNull();
    transport.emit({ kind: 'tempo', body: tempo({ bpm: 140 }) });
    expect(layout.transport.querySelector('.vact-tempo')?.getAttribute('aria-label')).toBe('tempo 140.0 bpm');
    transport.emit({ kind: 'manifest', body: { sounds: ['bd', 'sd'], synths: [], controls: [] } });
    expect([...layout.code.querySelectorAll<HTMLElement>('.vact-sound')].map((e) => e.dataset.sound)).toEqual(['bd', 'sd']);
    m.dispose();
    mounted.length = 0;
    expect(deps.code).toBeUndefined();
    expect(layout.code.querySelector('canvas.vact-code-canvas')).toBeNull();
  });

  it('implements CodeApi: revisions, span mapping and the selected site', () => {
    const { transport, deps } = mountArea();
    const code = deps.code;
    if (!code) throw new Error('no code api');
    const text = 'lpf 800 # ö\ngain 0.5';
    code.surface.dispatch({ changes: { from: 0, insert: text } });
    expect(code.currentRevision(DOC_FILE)).toBe(2);
    expect(code.currentRevision('other.vact')).toBe(0);
    const enc = new TextEncoder();
    const at = (lit: string) => {
      const s = enc.encode(text.slice(0, text.indexOf(lit))).length;
      return { start: s, end: s + enc.encode(lit).length };
    };
    transport.emit({
      kind: 'eval-result',
      body: {
        file: DOC_FILE,
        doc_revision: 2,
        forms: [],
        diagnostics: [],
        sites: [
          { id: 7, span: at('800'), tier: 'direct', origin: 'pattern-literal', value: 800, form_gen: 1 },
          { id: 9, span: at('0.5'), tier: 'direct', origin: 'pattern-literal', value: 0.5, form_gen: 1 },
        ],
        directives: { file_level: {}, entries: [] },
      },
    });
    code.surface.dispatch({ selection: { anchor: text.indexOf('0.5') + 1 } });
    expect(code.selectedSiteId()).toBe(9);
    code.surface.dispatch({ selection: { anchor: text.indexOf('800') } });
    expect(code.selectedSiteId()).toBe(7);
    code.surface.dispatch({ selection: { anchor: text.indexOf('#') } });
    expect(code.selectedSiteId()).toBeNull();
    // An insertion above keeps the mapping; the revision moves on.
    code.surface.dispatch({ changes: { from: 0, insert: '# 日本\n' } });
    const r = code.mapWireSpan(at('0.5'), 2);
    expect(code.surface.state.doc.sliceString(r?.from ?? 0, r?.to ?? 0)).toBe('0.5');
    expect(code.currentRevision(DOC_FILE)).toBe(3);
    expect(code.samples.frames('bd', 0)).toBeNull();
    code.samples.openBrowser('bd');
  });
});

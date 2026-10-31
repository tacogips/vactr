import { describe, expect, it, vi } from 'vitest';
import { EditorState } from '@codemirror/state';
import { installPerfHook, keyMetrics } from '../../examples/json-10k/perf-hook';
import type { EditorHandle, InputSample, PresentSample } from '../../src/editor';

const key: InputSample = { type: 'keydown', eventTimeStamp: 100, start: 100, end: 101, workMs: 1, revisionBefore: 0, revisionAfter: 0 };
const before: InputSample = { type: 'beforeinput', eventTimeStamp: 102, start: 102, end: 103, workMs: 0.5, revisionBefore: 0, revisionAfter: 1 };
const present: PresentSample = { frameStart: 110, frameEnd: 112, workMs: 2, revision: 1, visibleComplete: true };

describe('JSON perf key metrics', () => {
  it('combines the paired input work with its presenting frame', () => {
    expect(keyMetrics([key, before], [present])).toEqual([{ a: 12, b: 3.5, nextFrame: true, missing: false }]);
  });
  it('skips old revisions but counts an intervening frame against nextFrame', () => {
    const old = { ...present, frameStart: 105, frameEnd: 106, revision: 0 };
    expect(keyMetrics([key, before], [old, present])[0]).toEqual({ a: 12, b: 3.5, nextFrame: false, missing: false });
    const early = { ...present, frameStart: 100, frameEnd: 100.5 };
    expect(keyMetrics([key, before], [early, present])[0]).toEqual({ a: 12, b: 3.5, nextFrame: true, missing: false });
  });
  it('reports missing frames and excludes non-editing keys', () => {
    expect(keyMetrics([key, before], [])).toEqual([{ a: null, b: null, nextFrame: false, missing: true }]);
    expect(keyMetrics([key], [present])).toEqual([]);
  });
});


describe('JSON perf hook', () => {
  it('loads only after the matching complete presentation and records editor activity', async () => {
    let state = EditorState.create({ doc: '' });
    let revision = 0, top = 10;
    const presentListeners = new Set<(sample: PresentSample) => void>();
    const inputListeners = new Set<(sample: InputSample) => void>();
    const focus = vi.fn();
    const handle = {
      get state() { return state; }, get revision() { return revision; }, get scrollTop() { return top; },
      setDoc(text: string) { state = EditorState.create({ doc: text }); revision++; },
      dispatch(spec: Parameters<EditorHandle['dispatch']>[0]) { state = state.update(spec).state; },
      scrollTo(target: { top: number }) { top = target.top; }, focus,
      lineTop(line: number) { return line * 18; },
      perf: {
        onPresent(cb: (sample: PresentSample) => void) { presentListeners.add(cb); return () => { presentListeners.delete(cb); }; },
        onInput(cb: (sample: InputSample) => void) { inputListeners.add(cb); return () => { inputListeners.delete(cb); }; },
      },
    } as unknown as EditorHandle;
    const hook = installPerfHook(handle, 'a\nb');
    expect(window.__emakiPerf).toBe(hook);
    const loading = hook.load();
    let resolved = false;
    void loading.then(() => { resolved = true; });
    for (const sample of [{ ...present, revision: 0 }, { ...present, visibleComplete: false }]) {
      for (const cb of presentListeners) cb(sample);
    }
    await Promise.resolve();
    expect(resolved).toBe(false);
    for (const cb of presentListeners) cb({ ...present, frameEnd: performance.now() });
    expect((await loading).firstPaintMs).toBeGreaterThanOrEqual(0);
    hook.startRecording();
    for (const cb of presentListeners) cb(present);
    for (const cb of inputListeners) cb(before);
    expect(hook.stopRecording()).toEqual({ presents: [present], inputs: [before] });
    expect(presentListeners.size + inputListeners.size).toBe(0);
    hook.scrollBy(20); hook.caretToLine(2, 1);
    expect(focus).toHaveBeenCalledOnce();
    expect(state.selection.main.head).toBe(3);
    expect(hook.info()).toEqual({ lines: 2, bytes: 3, revision: 1, scrollTop: 30, totalHeight: 36 });
  });
});

import { describe, expect, it } from 'vitest';
import type { AnimatedMark, PluginHost } from 'emaki/engine';
import { createBeatGlowPlugin } from '../../src/code/beat-glow';
import { EVAL_ERROR_MARK, PLAYING_MARK } from '../../src/code/decorations';

describe('beat glow plugin', () => {
  it('provides playing glow and eval marks in order and stops when empty', () => {
    let playing = [{ from: 0, to: 3 }];
    let flashes = [{ from: 4, to: 9, error: true }];
    let provider: (ms: number) => readonly AnimatedMark[] = () => [];
    const host = { animatedMarks: (fn: typeof provider) => { provider = fn; return () => {}; } } as PluginHost;
    const instance = createBeatGlowPlugin({ playing: () => playing, flashes: () => flashes }).attach(host);
    expect(provider(0)).toEqual([{ from: 0, to: 3, style: PLAYING_MARK }, { from: 4, to: 9, style: EVAL_ERROR_MARK }]);
    expect(PLAYING_MARK.glow).toEqual({ color: [0.7, 0.58, 0.26, 0.5], radius: 4 });
    expect(instance?.frame?.(0)).toBe(true);
    playing = []; flashes = [];
    expect(instance?.frame?.(0)).toBe(false);
  });
});

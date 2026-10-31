import type { EmakiPlugin, Range } from 'emaki/engine';
import { EVAL_ERROR_MARK, EVAL_MARK, PLAYING_MARK } from './decorations';

export function createBeatGlowPlugin(src: {
  playing(): readonly Range[];
  flashes(): readonly (Range & { error: boolean })[];
}): EmakiPlugin {
  return {
    name: 'beat-glow',
    attach(host) {
      host.animatedMarks(() => [
        ...src.playing().map(range => ({ ...range, style: PLAYING_MARK })),
        ...src.flashes().map(({ from, to, error }) => ({ from, to, style: error ? EVAL_ERROR_MARK : EVAL_MARK })),
      ]);
      return { frame: () => src.playing().length > 0 || src.flashes().length > 0 };
    },
  };
}

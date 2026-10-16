import { describe, expect, it } from 'vitest';
import { installPerfHook, removePerfHook } from '../../src/code/perf-hook';

describe('perf hook annotations', () => {
  it('returns only annotation kind and range', () => {
    const win = { document: { getElementsByTagName: () => [] } } as unknown as Window;
    const surface = { annotationRanges: () => [{ kind: 'diagnostic', from: 4, to: 7, className: 'x' }] };
    const api = installPerfHook({ win, surface: surface as never } as never);
    expect(api.annotations()).toEqual([{ kind: 'diagnostic', from: 4, to: 7 }]);
    expect(Object.keys(api.annotations()[0])).toEqual(['kind', 'from', 'to']);
    removePerfHook(win);
    expect((win as Window & { __vactrPerf?: unknown }).__vactrPerf).toBeUndefined();
  });
});

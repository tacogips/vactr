// @vitest-environment node
import { expect, it } from 'vitest';
import { LARGE_EVAL_RATIO_LIMIT, measureLargeEval } from './large-eval-shared';

it('keeps fresh-session large-document eval scaling bounded', async () => {
  const measurement = await measureLargeEval('large-eval node');
  expect(measurement.ratio).toBeLessThanOrEqual(LARGE_EVAL_RATIO_LIMIT);
}, 120_000);

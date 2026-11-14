// @vitest-environment node
import { expect, it } from 'vitest';
import { LARGE_EVAL_RATIO_LIMIT, measureLargeEval } from './large-eval-shared';

// CANVAS-EVIDENCE-RUNSTART budget: 4,800ms from TASK-304 medians 2401.2, 2396.3, 2390.5ms (m=2396.3ms; ceil(2*m/100)*100).
const LARGE_EVAL_BUDGET_MS = 4_800;

it('meets the serial large-document eval wall-clock budget', async () => {
  const measurement = await measureLargeEval('large-eval perf');
  expect(measurement.ratio).toBeLessThanOrEqual(LARGE_EVAL_RATIO_LIMIT);
  expect(measurement.largeMedian).toBeLessThanOrEqual(LARGE_EVAL_BUDGET_MS);
}, 120_000);

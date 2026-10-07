import { configDefaults, defineConfig } from 'vitest/config';
import solid from 'vite-plugin-solid';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } })
  .process.env;
const perf = env.VACTR_PERF === '1';

// jsdom by default (design 15.1.3); real-wasm test files declare
// `@vitest-environment node` themselves. Solid views (design 15.2) are
// compiled by vite-plugin-solid in tests as in the build.
// The serial absolute-time gate is isolated from default workers (design 15.3.8.11).
export default defineConfig({
  plugins: [solid()],
  resolve: { conditions: ['development', 'browser'] },
  test: {
    environment: 'jsdom',
    include: perf
      ? ['test/**/*.perf.test.ts']
      : ['test/**/*.test.ts', 'test/**/*.test.tsx'],
    exclude: perf
      ? configDefaults.exclude
      : [...configDefaults.exclude, 'test/**/*.perf.test.ts'],
  },
});

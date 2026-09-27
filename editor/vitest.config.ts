import { defineConfig } from 'vitest/config';
import solid from 'vite-plugin-solid';

// jsdom by default (design 15.1.3); real-wasm test files declare
// `@vitest-environment node` themselves. Solid views (design 15.2) are
// compiled by vite-plugin-solid in tests as in the build.
export default defineConfig({
  plugins: [solid()],
  resolve: { conditions: ['development', 'browser'] },
  test: {
    environment: 'jsdom',
    include: ['test/**/*.test.ts', 'test/**/*.test.tsx'],
  },
});

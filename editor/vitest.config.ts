import { defineConfig } from 'vitest/config';

// jsdom by default (design 15.1.3); real-wasm test files declare
// `@vitest-environment node` themselves.
export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['test/**/*.test.ts'],
  },
});

import { defineConfig } from 'vite';

export default defineConfig({
  root: 'examples',
  base: './',
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      input: [
        new URL('./examples/index.html', import.meta.url).pathname,
        new URL('./examples/plugin-showcase/index.html', import.meta.url).pathname,
        new URL('./examples/json-10k/index.html', import.meta.url).pathname,
        new URL('./examples/diff/index.html', import.meta.url).pathname,
      ],
    },
  },
});

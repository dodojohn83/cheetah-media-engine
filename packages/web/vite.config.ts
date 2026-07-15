import { defineConfig } from 'vite';
import { resolve } from 'node:path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.ts'),
      name: 'CheetahMedia',
      formats: ['es', 'iife'],
      fileName: (format) => (format === 'es' ? 'index.mjs' : 'cheetah-media.iife.js'),
    },
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: true,
    minify: false,
  },
});

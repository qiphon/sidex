import { defineConfig } from 'vite';
import { viteStaticCopy } from 'vite-plugin-static-copy';
import * as path from 'path';

export default defineConfig({
  clearScreen: false,
  assetsInclude: ['**/*.wasm'],
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
    fs: {
      allow: ['.', 'extensions', 'node_modules'],
    },
  },
  plugins: [
    viteStaticCopy({
      targets: [
        {
          src: 'extensions',
          dest: '.',
        },
        {
          src: 'extensions-meta.json',
          dest: '.',
        },
        {
          src: 'node_modules/vscode-oniguruma/release/onig.wasm',
          dest: '.',
        },
      ],
    }),
  ],
  envPrefix: ['VITE_', 'TAURI_'],
  resolve: {
    alias: {
      'vs': path.resolve(__dirname, 'src/vs'),
    },
  },
  build: {
    target: ['es2022', 'chrome100', 'safari15'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    chunkSizeWarningLimit: 15000,
    rollupOptions: {
      output: {
        manualChunks: {
          'monaco-editor': ['monaco-editor'],
        },
      },
    },
  },
  optimizeDeps: {
    include: ['vscode-textmate', 'vscode-oniguruma'],
    exclude: ['@tauri-apps/api', '@tauri-apps/plugin-dialog', '@tauri-apps/plugin-fs',
              '@tauri-apps/plugin-clipboard-manager', '@tauri-apps/plugin-shell',
              '@tauri-apps/plugin-notification', '@tauri-apps/plugin-opener'],
  },
  worker: {
    format: 'es',
  },
});

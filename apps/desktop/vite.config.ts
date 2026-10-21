import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// One id per build, shown to the running UI and published as /build-id.txt so every
// surface (window, browser) can tell when it is not running the build being served.
const buildId = Date.now().toString(36);
const stampBuild = (): Plugin => ({
  name: 'marquee-build-id',
  apply: 'build',
  generateBundle() {
    this.emitFile({ type: 'asset', fileName: 'build-id.txt', source: buildId });
  },
});

// Tauri expects a fixed port and no auto-opened browser; harmless in a plain browser too.
export default defineConfig(({ command }) => ({
  plugins: [svelte(), stampBuild()],
  define: command === 'build' ? { __BUILD_ID__: JSON.stringify(buildId) } : {},
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Browser dev against a running Marquee: same origin, same data.
    proxy: {
      '/api': { target: 'http://127.0.0.1:47820', changeOrigin: true },
      '/art': { target: 'http://127.0.0.1:47820', changeOrigin: true },
    },
  },
}));

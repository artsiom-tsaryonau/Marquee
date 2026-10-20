import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri expects a fixed port and no auto-opened browser; harmless in a plain browser too.
export default defineConfig({
  plugins: [svelte()],
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
});

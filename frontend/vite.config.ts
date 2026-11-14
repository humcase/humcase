import react from '@vitejs/plugin-react'
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { defineConfig, type Plugin } from 'vite'

/**
 * Serves Excalidraw's fonts from the app itself (`/excalidraw/fonts/...`): without them it would
 * fetch them from a CDN, which the app's content policy blocks (Humcase works offline). The
 * Chinese, Japanese and Korean font (Xiaolai, 13 MB) is left out: those scripts fall back to a
 * system font.
 */
function excalidrawFonts(): Plugin {
  const root = fileURLToPath(new URL('./node_modules/@excalidraw/excalidraw/dist/prod/fonts', import.meta.url));
  const files = (dir: string, base = ''): string[] => readdirSync(dir).flatMap(name => {
    if (name === 'Xiaolai') return [];
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path, `${base}${name}/`) : [`${base}${name}`];
  });
  return {
    name: 'excalidraw-fonts',
    configureServer(server) {
      server.middlewares.use('/excalidraw/fonts/', (request, response, next) => {
        const name = decodeURIComponent((request.url ?? '').split('?')[0].replace(/^\/+/, ''));
        if (!files(root).includes(name)) return next();
        response.setHeader('Content-Type', 'font/woff2');
        response.end(readFileSync(join(root, name)));
      });
    },
    generateBundle() {
      for (const name of files(root)) {
        this.emitFile({ type: 'asset', fileName: `excalidraw/fonts/${name}`, source: readFileSync(join(root, name)) });
      }
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), excalidrawFonts()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    // Vite's 500 kB warning is about download time on the web. The app reads its files from disk
    // (Tauri), and the heavy parts (Excalidraw, Mermaid) are already loaded only when used.
    // A chunk above 2 MB still warns: it would be worth splitting.
    chunkSizeWarningLimit: 2048,
    rollupOptions: {
      input: {
        main: './index.html',
        quick_capture: './quick_capture.html',
        assistant_mini: './assistant_mini.html'
      }
    }
  }
})

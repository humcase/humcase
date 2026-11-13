/**
 * Excalidraw is loaded only when a sketch is shown: its code stays out of the start of the app.
 * Its fonts come from the app itself (`/excalidraw/fonts/`, see `vite.config.ts`), never from a
 * CDN: Humcase works offline and its content policy blocks other origins.
 */
declare global {
  interface Window { EXCALIDRAW_ASSET_PATH?: string | string[] }
}

type ExcalidrawModule = typeof import('@excalidraw/excalidraw');
let loading: Promise<ExcalidrawModule> | null = null;

export function loadExcalidraw(): Promise<ExcalidrawModule> {
  if (!loading) {
    window.EXCALIDRAW_ASSET_PATH = '/excalidraw/';
    loading = Promise.all([import('@excalidraw/excalidraw'), import('@excalidraw/excalidraw/index.css')])
      .then(([module]) => module)
      .catch(error => { loading = null; throw error; });
  }
  return loading;
}

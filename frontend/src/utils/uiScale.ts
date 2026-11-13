import { getCurrentWebview } from '@tauri-apps/api/webview';

/**
 * Interface scale based on the screen's physical resolution, not the window width:
 * on a 2560 px screen displayed at 100 % by Windows, 14 px texts become too small, even
 * in a half-screen window. When Windows already applies scaling (≥ 125 %),
 * we add nothing.
 */
export function uiZoomFor(physicalWidth: number, osScale: number): number {
  if (osScale >= 1.2) return 1;
  if (physicalWidth < 2200) return 1;
  return Math.min(1.5, Math.round((physicalWidth / 2133) * 20) / 20);
}

// Native WebView zoom (like Ctrl+ in a browser): the width seen by the responsive
// rules is the zoomed width, so breakpoints stay consistent. The applied zoom
// multiplies `devicePixelRatio`: we remove it to recover Windows' scale.
let appliedZoom = 1;

export async function applyUiScale(miniMode: boolean): Promise<void> {
  try {
    const webview = getCurrentWebview();
    if (webview.label !== 'main') return;
    const ratio = window.devicePixelRatio || 1;
    const osScale = ratio / appliedZoom;
    // `screen.width` does not follow page zoom: only Windows' scale converts it.
    const physicalWidth = (window.screen?.width ?? 0) * osScale;
    const zoom = miniMode ? 1 : uiZoomFor(physicalWidth, osScale);
    if (zoom === appliedZoom) return;
    await webview.setZoom(zoom);
    appliedZoom = zoom;
  } catch {
    // Outside Tauri (tests) or permission missing: the interface stays at 100 %.
  }
}

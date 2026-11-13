import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PhysicalSize, getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import { applyUiScale } from '../utils/uiScale';
import { getMiniMode, getMiniSize, setMiniMode, setMiniSize } from '../utils/prefs';

// Minimum sizes consistent with the mode: the normal window does not go below 720×520 (three-column
// layout), mini mode allows its sticky-note size. `tauri.conf.json` sets the normal value
// at startup; it is adjusted on every toggle so that the constraint follows the mode.
const MIN_SIZE_NORMAL = { width: 720, height: 520 };
const MIN_SIZE_MINI = { width: 400, height: 500 };

/** Mini mode (pinned sticky note): state, restore at launch, remembered size, toggle, DPI. */
export function useMiniWindow(reportAction: (message: string) => void) {
  const { t } = useTranslation();
  const [isMiniMode, setIsMiniMode] = useState(() => getMiniMode());

  // Interface scale (native WebView zoom) on very high-resolution screens; mini mode keeps its density.
  useEffect(() => {
    void applyUiScale(isMiniMode);
  }, [isMiniMode]);

  // Mini mode: restored at launch (size included); it previously only survived for the current window.
  useEffect(() => {
    if (!getMiniMode()) return;
    const win = getCurrentWindow();
    const size = getMiniSize();
    void win.setAlwaysOnTop(true);
    void win.setMinSize(new LogicalSize(MIN_SIZE_MINI.width, MIN_SIZE_MINI.height));
    void win.setSize(new LogicalSize(size.width, size.height));
  }, []);

  // Mini mode size remembered as soon as it changes.
  // The scale factor is read **before** subscribing (otherwise it could still be 1 when the
  // first measurement arrived) and the `localStorage` write is debounced: resizing
  // emits one event per pixel, which left up to hundreds of writes per gesture.
  useEffect(() => {
    if (!isMiniMode) return;
    const win = getCurrentWindow();
    let unlisten: (() => void) | null = null;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let pendingSize: { width: number; height: number } | null = null;

    (async () => {
      let scaleFactor = 1;
      try {
        scaleFactor = (await win.scaleFactor()) || 1;
      } catch {
        scaleFactor = window.devicePixelRatio || 1;
      }
      if (disposed) return;
      const stop = await win.onResized(({ payload }) => {
        const width = Math.round(payload.width / scaleFactor);
        const height = Math.round(payload.height / scaleFactor);
        if (width <= 200 || height <= 200) return;
        pendingSize = { width, height };
        if (timer) clearTimeout(timer);
        timer = setTimeout(() => {
          if (pendingSize) setMiniSize(pendingSize.width, pendingSize.height);
        }, 300);
      });
      if (disposed) stop();
      else unlisten = stop;
    })();

    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
      unlisten?.();
    };
  }, [isMiniMode]);

  const toggleMiniMode = async () => {
    try {
      const win = getCurrentWindow();
      const newMode = !isMiniMode;
      setIsMiniMode(newMode);
      setMiniMode(newMode);
      if (newMode) {
        await win.unmaximize();
        await win.setAlwaysOnTop(true);
        await win.setMinSize(new LogicalSize(MIN_SIZE_MINI.width, MIN_SIZE_MINI.height));
        const size = getMiniSize();
        await win.setSize(new LogicalSize(size.width, size.height));
      } else {
        await win.setAlwaysOnTop(false);
        await win.setMinSize(new LogicalSize(MIN_SIZE_NORMAL.width, MIN_SIZE_NORMAL.height));
        await win.setSize(new LogicalSize(1000, 700));
      }
    } catch (e) {
      // The window may have been partially modified: we do not toggle back blindly, we say so.
      console.error("Error while resizing", e);
      reportAction(t('workspace.mini.resizeError'));
    }
  };

  // WebView2 may keep a render surface at the old scale after a DPI change
  // (window moved to a screen with a different scaling, Windows zoom changed): the content
  // then appears shifted, surrounded by unpainted areas. Reapplying the window size forces
  // the resynchronization of the WebView2 control.
  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | null = null;
    let disposed = false;
    const resync = async () => {
      try {
        if (await win.isMaximized() || await win.isMinimized()) return;
        const inner = await win.innerSize();
        const outer = await win.outerSize();
        await win.setSize(new PhysicalSize(inner.width, inner.height));
        await win.setSize(new PhysicalSize(outer.width, outer.height));
      } catch {
        /* best effort: a failure here must not break anything */
      }
    };
    void win.onScaleChanged(() => { void resync(); }).then(stop => {
      if (disposed) stop(); else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return { isMiniMode, toggleMiniMode };
}

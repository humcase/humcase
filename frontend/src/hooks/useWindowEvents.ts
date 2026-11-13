import { useEffect, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { emit, listen } from '@tauri-apps/api/event';
import { setQuickCapturePersistenceRefused } from '../utils/quickCaptureBarrier';

/**
 * List freshness: a note created or resolved elsewhere (MCP agent, Quick Capture) must
 * appear without user action. When the window returns to the foreground, the database fingerprint
 * is checked at once and the lists are reread only if it changed: a plain return to the window
 * costs no reread and no render. The draft in progress is not overwritten: `NoteEditor` only
 * adopts external values for fields not modified locally (data-loss safeguard).
 */
export function useRefreshOnFocus(checkNow: () => void) {
  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void win.onFocusChanged(({ payload: focused }) => {
      if (focused) checkNow();
    }).then(stop => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [checkNow]);
}

/** M2: the Quick Capture window reports whether its state is persisted. As long as it is not,
 * the restore must be refused (fail-closed): checking the draft is impossible. */
export function useQuickCapturePersistenceWatch() {
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void listen<{ refused?: boolean }>('humcase:qc-persistence', event => {
      setQuickCapturePersistenceRefused(Boolean(event.payload?.refused));
    })
      .then(stop => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        // Outside Tauri (tests, web execution): no listener available, the barrier is left as is.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}

/**
 * Quick Capture (Alt+Shift+N): the shortcut must be unregistered on unmount,
 * otherwise each mount stacks an additional handler.
 */
export function useQuickCaptureShortcut() {
  useEffect(() => {
    let unregister: (() => Promise<void>) | null = null;
    let disposed = false;
    (async () => {
      const [shortcut, { WebviewWindow }] = await Promise.all([
        import('@tauri-apps/plugin-global-shortcut'),
        import('@tauri-apps/api/webviewWindow'),
      ]);
      unregister = async () => {
        try {
          await shortcut.unregister('Alt+Shift+N');
        } catch (e) {
          console.warn('Failed to unregister the global shortcut', e);
        }
      };
      if (disposed) {
        await unregister();
        return;
      }
      await shortcut.register('Alt+Shift+N', async (e) => {
        if (e.state !== 'Released') return;
        const win = await WebviewWindow.getByLabel('quick_capture');
        if (!win) return;
        if (await win.isVisible()) {
          await win.hide();
        } else {
          await win.show();
          await win.setFocus();
        }
      });
    })().catch(console.error);
    return () => {
      disposed = true;
      void unregister?.();
    };
  }, []);
}

/**
 * Quick capture (0.9.83): the displayed note is announced to the capture window, which offers it
 * as a destination. Announced on every change and on request (capture opened afterwards).
 */
export function useOpenNoteAnnouncement(openNoteId: string | null, openNoteTitle: string) {
  const openNoteRef = useRef<{ id: string; title: string } | null>(null);
  useEffect(() => {
    const note = openNoteId ? { id: openNoteId, title: openNoteTitle } : null;
    openNoteRef.current = note;
    void emit('humcase:open-note', note).catch(() => {});
  }, [openNoteId, openNoteTitle]);
  useEffect(() => {
    let stopFn: (() => void) | null = null;
    let finished = false;
    void listen('humcase:open-note-query', () => { void emit('humcase:open-note', openNoteRef.current).catch(() => {}); })
      .then(stop => { if (finished) stop(); else stopFn = stop; })
      .catch(() => {});
    return () => { finished = true; stopFn?.(); };
  }, []);
}

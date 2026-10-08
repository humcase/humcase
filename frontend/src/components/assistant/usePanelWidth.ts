import { useCallback, useEffect, useState, type KeyboardEvent, type PointerEvent } from 'react';

const STORAGE_KEY = 'humcase-assistant-width';
const MIN = 320;
const STEP = 24;

/** The widest the panel may be: the page keeps at least 40 % of the window. */
const maxWidth = () => Math.max(MIN, Math.min(960, Math.round(window.innerWidth * 0.6)));
export const clampWidth = (width: number) => Math.min(maxWidth(), Math.max(MIN, Math.round(width)));

function stored(): number | null {
  try {
    const value = Number(localStorage.getItem(STORAGE_KEY));
    return Number.isFinite(value) && value > 0 ? value : null;
  } catch { return null; }
}

function remember(width: number | null) {
  try {
    if (width === null) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, String(width));
  } catch { /* kept for this session only */ }
}

/**
 * The width of the panel, chosen by dragging its left edge (or with the arrow keys on it) and
 * remembered on this computer. `null` keeps the default width of the layout; a double click
 * goes back to it.
 */
export function usePanelWidth() {
  const [width, setWidth] = useState<number | null>(() => { const value = stored(); return value === null ? null : clampWidth(value); });
  const [dragging, setDragging] = useState(false);

  // A smaller window keeps the panel within its bounds.
  useEffect(() => {
    if (width === null) return;
    const onResize = () => setWidth(current => (current === null ? null : clampWidth(current)));
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [width]);

  const onPointerDown = useCallback((event: PointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const handle = event.currentTarget;
    const panel = handle.parentElement;
    if (!panel) return;
    handle.setPointerCapture(event.pointerId);
    const right = panel.getBoundingClientRect().right;
    let last = panel.getBoundingClientRect().width;
    setDragging(true);
    const move = (e: globalThis.PointerEvent) => { last = clampWidth(right - e.clientX); setWidth(last); };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      setDragging(false);
      remember(last);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
  }, []);

  const onKeyDown = useCallback((event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const current = width ?? event.currentTarget.parentElement?.getBoundingClientRect().width ?? 400;
    // The handle is on the left edge: left widens the panel.
    const next = clampWidth(current + (event.key === 'ArrowLeft' ? STEP : -STEP));
    setWidth(next);
    remember(next);
  }, [width]);

  const reset = useCallback(() => { setWidth(null); remember(null); }, []);

  return { width, dragging, onPointerDown, onKeyDown, reset, min: MIN, max: maxWidth() };
}

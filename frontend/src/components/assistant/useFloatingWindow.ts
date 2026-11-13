import { useEffect, useRef, useState, type PointerEvent } from 'react';
import { getAssistantWindow, setAssistantWindow, type WindowRect } from '../../utils/prefs';

/** Space kept between the window and the edges it snaps to. */
export const MARGIN = 16;
/** The window never goes under the top bar. */
export const TOP_BAR = 56;
export const MIN_WIDTH = 340;
export const MIN_HEIGHT = 420;
const DEFAULT_WIDTH = 420;
const DEFAULT_HEIGHT = 640;
/** Closer than this to an edge, the window snaps to it. */
const SNAP_DISTANCE = 48;

/** The window within the viewport: never wider or taller than it, never under the top bar. */
export function clampRect(rect: WindowRect, viewWidth: number, viewHeight: number): WindowRect {
  const maxWidth = Math.max(0, viewWidth - 2 * MARGIN);
  const maxHeight = Math.max(0, viewHeight - TOP_BAR - 2 * MARGIN);
  const width = Math.min(maxWidth, Math.max(Math.min(MIN_WIDTH, maxWidth), rect.width));
  const height = Math.min(maxHeight, Math.max(Math.min(MIN_HEIGHT, maxHeight), rect.height));
  const x = Math.min(Math.max(0, viewWidth - width), Math.max(0, rect.x));
  const y = Math.min(Math.max(TOP_BAR, viewHeight - height), Math.max(TOP_BAR, rect.y));
  return { x, y, width, height };
}

/** Top right, under the top bar. */
export function defaultRect(viewWidth: number, viewHeight: number): WindowRect {
  const width = Math.min(DEFAULT_WIDTH, viewWidth - 2 * MARGIN);
  const height = Math.min(DEFAULT_HEIGHT, viewHeight - TOP_BAR - 2 * MARGIN);
  return clampRect({ x: viewWidth - width - MARGIN, y: TOP_BAR + MARGIN, width, height }, viewWidth, viewHeight);
}

/** The edge or corner the window snaps to when dropped at (x, y), or null far from every edge. */
export function snapTarget(rect: WindowRect, viewWidth: number, viewHeight: number): { x: number; y: number } | null {
  const near = (value: number, targets: number[]) => targets.find(target => Math.abs(value - target) < SNAP_DISTANCE) ?? null;
  const x = near(rect.x, [MARGIN, viewWidth - rect.width - MARGIN]);
  const y = near(rect.y, [TOP_BAR + MARGIN, viewHeight - rect.height - MARGIN]);
  return x === null && y === null ? null : { x: x ?? rect.x, y: y ?? rect.y };
}

/**
 * A window inside the app: moved by its header (snapping to the edges and corners, a ghost shows
 * where it will land), resized by its corner, its place and size remembered on this computer.
 */
export function useFloatingWindow() {
  const [rect, setRect] = useState<WindowRect>(() => {
    const saved = getAssistantWindow();
    return saved ? clampRect(saved, window.innerWidth, window.innerHeight) : defaultRect(window.innerWidth, window.innerHeight);
  });
  const [ghost, setGhost] = useState<WindowRect | null>(null);
  const [snapping, setSnapping] = useState(false);
  const rectRef = useRef(rect);
  useEffect(() => { rectRef.current = rect; }, [rect]);

  // A smaller app window keeps the assistant inside it.
  useEffect(() => {
    const onResize = () => setRect(current => clampRect(current, window.innerWidth, window.innerHeight));
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  const track = (event: PointerEvent<HTMLElement>, next: (e: globalThis.PointerEvent, start: WindowRect) => WindowRect, drop: (last: WindowRect) => WindowRect) => {
    event.preventDefault();
    const handle = event.currentTarget;
    handle.setPointerCapture?.(event.pointerId);
    const start = rectRef.current;
    let last = start;
    setSnapping(false);
    const move = (e: globalThis.PointerEvent) => { last = next(e, start); rectRef.current = last; setRect(last); };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      setGhost(null);
      const final = drop(last);
      rectRef.current = final;
      setRect(final);
      setAssistantWindow(final);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
  };

  const onDragStart = (event: PointerEvent<HTMLElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return;
    const offsetX = event.clientX - rectRef.current.x;
    const offsetY = event.clientY - rectRef.current.y;
    let snap: { x: number; y: number } | null = null;
    track(event, (e, start) => {
      const moved = clampRect({ ...start, x: e.clientX - offsetX, y: e.clientY - offsetY }, window.innerWidth, window.innerHeight);
      snap = snapTarget(moved, window.innerWidth, window.innerHeight);
      setGhost(snap ? { ...moved, ...snap } : null);
      return moved;
    }, last => {
      if (!snap) return last;
      setSnapping(true);
      return { ...last, ...snap };
    });
  };

  const onResizeStart = (event: PointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    const startX = event.clientX;
    const startY = event.clientY;
    // The top left corner stays where it is: the window grows towards the bottom right only.
    track(event, (e, start) => clampRect({
      ...start,
      width: Math.min(window.innerWidth - start.x, Math.max(MIN_WIDTH, start.width + e.clientX - startX)),
      height: Math.min(window.innerHeight - start.y, Math.max(MIN_HEIGHT, start.height + e.clientY - startY)),
    }, window.innerWidth, window.innerHeight), last => last);
  };

  return { rect, ghost, snapping, onDragStart, onResizeStart };
}

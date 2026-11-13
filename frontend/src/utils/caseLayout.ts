import type { Note } from '../types';

/** Card width on the board and spacing of the automatic layout, in board pixels. */
export const BOARD_CARD_WIDTH = 240;
export const SLOT_WIDTH = BOARD_CARD_WIDTH + 32;
export const SLOT_HEIGHT = 220;
const MARGIN = 24;
export type Point = { x: number; y: number };
/** Room a case takes on the board, in grid slots: a card is 1×1, a group is wider. */
export type Span = { w: number; h: number };
const ONE: Span = { w: 1, h: 1 };

/** Size in board pixels of an item spanning these slots. */
export const spanSize = (span: Span) => ({ width: span.w * SLOT_WIDTH - 32, height: span.h * SLOT_HEIGHT - 16 });

/** Grid columns that fit a board of this width (at least one). */
export function columnsFor(width: number): number {
  return Math.max(1, Math.floor((width - 2 * MARGIN + 32) / SLOT_WIDTH));
}

/**
 * Grid columns for a board of `count` cases: the visible width for a few cases, a wider grid for
 * many, so the whole board seen at once is a block and not a strip. Steps, so that one more case
 * does not reflow the board.
 */
export function gridColumns(count: number, visibleColumns: number): number {
  if (count <= 24) return visibleColumns;
  return Math.max(visibleColumns, count <= 100 ? 10 : 16);
}

const slotAt = (index: number, columns: number): Point =>
  ({ x: MARGIN + (index % columns) * SLOT_WIDTH, y: MARGIN + Math.floor(index / columns) * SLOT_HEIGHT });

type Rect = { x: number; y: number; width: number; height: number };
const rectOf = (point: Point, span: Span): Rect => ({ ...point, ...spanSize(span) });
const overlaps = (a: Rect, b: Rect) =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;

/**
 * First slot of the grid, from `from` on, where an item of this span overlaps nothing already
 * placed. Returns the slot and its index, so a layout can go on from there.
 */
function freeSlotFrom(taken: Rect[], columns: number, span: Span, from = 0): { slot: Point; index: number } {
  for (let index = from; ; index++) {
    // A wide item starts in a column where it fits, unless the board is narrower than it.
    if (columns >= span.w && (index % columns) + span.w > columns) continue;
    const slot = slotAt(index, columns);
    const rect = rectOf(slot, span);
    if (!taken.some(other => overlaps(other, rect))) return { slot, index };
  }
}
const freeSlot = (taken: Rect[], columns: number, span: Span) => freeSlotFrom(taken, columns, span).slot;

/**
 * Where each case sits: its stored position, or the next free slot of a grid for a case never
 * placed. Cases never placed are taken oldest first, so a new case never pushes the others.
 */
export function layoutCases(cases: Note[], stored: ReadonlyMap<string, Point>, columns = 4, spanOf: (note: Note) => Span = () => ONE): Map<string, Point> {
  const result = new Map<string, Point>();
  const taken: Rect[] = [];
  for (const note of cases) {
    const position = stored.get(note.id);
    if (!position) continue;
    result.set(note.id, position);
    taken.push(rectOf(position, spanOf(note)));
  }
  const unplaced = cases
    .filter(note => !stored.has(note.id))
    .sort((a, b) => a.created_at.localeCompare(b.created_at) || a.id.localeCompare(b.id));
  // Slots only fill up: the search for the next case of the same span starts where the last one
  // was found, which keeps a board of hundreds of cases quick to lay out.
  const cursors = new Map<string, number>();
  for (const note of unplaced) {
    const span = spanOf(note);
    const key = `${span.w}x${span.h}`;
    const { slot, index } = freeSlotFrom(taken, columns, span, cursors.get(key) ?? 0);
    cursors.set(key, index);
    result.set(note.id, slot);
    taken.push(rectOf(slot, span));
  }
  return result;
}

/** First free slot of the grid around the items already on the board (cards unless a span is given). */
export function nextFreeSlot(points: Iterable<Point | { point: Point; span: Span }>, columns = 4, span: Span = ONE): Point {
  const taken = [...points].map(item => 'point' in item ? rectOf(item.point, item.span) : rectOf(item, ONE));
  return freeSlot(taken, columns, span);
}

/** Rectangle around every item, to show the whole board at once; null when it is empty. */
export function boardBounds(items: Iterable<{ point: Point; span: Span }>): Rect | null {
  let bounds: { left: number; top: number; right: number; bottom: number } | null = null;
  for (const { point, span } of items) {
    const { width, height } = spanSize(span);
    bounds = bounds
      ? { left: Math.min(bounds.left, point.x), top: Math.min(bounds.top, point.y), right: Math.max(bounds.right, point.x + width), bottom: Math.max(bounds.bottom, point.y + height) }
      : { left: point.x, top: point.y, right: point.x + width, bottom: point.y + height };
  }
  return bounds && { x: bounds.left, y: bounds.top, width: bounds.right - bounds.left, height: bounds.bottom - bounds.top };
}

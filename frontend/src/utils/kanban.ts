import type { Note, NoteStatus } from '../types';
import { ORDERED_STATUSES, statusLabel } from './statuses';
import { trackEditorWrite } from './backupBarrier';

export interface KanbanColumnInfo {
  id: NoteStatus;
  /** Title in the current language, read on every access (never frozen at module load). */
  readonly title: string;
}

/**
 * Kanban columns, in display order. Titles come from the single source of statuses:
 * getter, because the language can change after the module is imported.
 */
export const KANBAN_COLUMNS: readonly KanbanColumnInfo[] = ORDERED_STATUSES.map(id => ({
  id,
  get title() { return statusLabel(id); },
}));

/**
 * Target status of a card drop (ticket B11).
 *
 * `overId` is either the id of a **column** (drop into an empty area, only possible if the
 * column is registered as droppable), or the id of a **card**. Without droppable on the
 * columns, the "column" branch was dead code and dropping a card into an empty column
 * did nothing.
 *
 * Returns `null` if the drop is neither on a column nor on a known card.
 */
export function resolveDropStatus(overId: string, notes: Note[]): NoteStatus | null {
  const column = KANBAN_COLUMNS.find(c => c.id === overId);
  if (column) return column.id;
  return notes.find(n => n.id === overId)?.status ?? null;
}

export interface ColumnRect {
  id: NoteStatus;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/**
 * Column that actually contains the center of the dragged card's final rectangle.
 *
 * We do not settle for the identifier returned by dnd-kit: with `closestCorners`, the computed
 * target can be a neighboring column (columns are much larger than cards, and
 * the board's auto-scroll shifts the geometry during the drag). The geometric test
 * answers the only question that matters: "under which column was the card dropped?".
 */
export function resolveColumnFromRect(
  rect: { left: number; top: number; width: number; height: number },
  columns: ColumnRect[]
): NoteStatus | null {
  return resolveColumnFromPoint(
    { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 },
    columns
  );
}

/**
 * Column that contains a screen point (cursor position on release).
 *
 * This is the source of truth for the drop: when the card is dropped in the empty area of a column,
 * dnd-kit returns `over = null` (no element hovered) and the drag was simply
 * ignored; depending on the position, it can also designate a neighboring column. The geometric test
 * always answers the question asked: "in which column was the card dropped?".
 */
export function resolveColumnFromPoint(
  point: { x: number; y: number },
  columns: ColumnRect[]
): NoteStatus | null {
  const hit = columns.find(
    c => point.x >= c.x1 && point.x <= c.x2 && point.y >= c.y1 && point.y <= c.y2
  );
  return hit ? hit.id : null;
}

export interface StatusQueue {
  /**
   * Status to display for a note: the latest in-flight intent, otherwise `receivedStatus`. Pure
   * read (usable during render). The intent is forgotten through `reconcile`.
   */
  targetStatus(noteId: string, receivedStatus: NoteStatus): NoteStatus;
  /**
   * Records the intent then queues the write **per note**. Returns `false` if the target is
   * already the current status (no write scheduled).
   */
  schedule(noteId: string, receivedStatus: NoteStatus, target: NoteStatus, write: (target: NoteStatus) => Promise<void>): boolean;
  /** Clears the intent once the received data has caught up with it (the real status takes over again). */
  reconcile(noteId: string, receivedStatus: NoteStatus): void;
  /** After a failure, forgets the failed intent if it has not been replaced by a more recent one. */
  abandon(noteId: string, failedTarget: NoteStatus): void;
}

/**
 * Serializes status changes of the **same** note.
 *
 * Two quick moves used to trigger two concurrent writes: the slower response
 * could overwrite the latest intent. Writes are chained per note (the second
 * waits for the first to finish) and the latest intent is remembered to ignore a stale status,
 * since props may not yet reflect the previous write. The last gesture therefore always
 * wins, whatever the order in which responses arrive.
 *
 * `targetStatus` is also used for rendering: until the write is confirmed, the card is displayed
 * in the target column rather than staying in the old one and then jumping.
 */
export function createStatusQueue(): StatusQueue {
  const intents = new Map<string, NoteStatus>();
  const files = new Map<string, Promise<void>>();

  function targetStatus(noteId: string, receivedStatus: NoteStatus): NoteStatus {
    return intents.get(noteId) ?? receivedStatus;
  }

  function schedule(noteId: string, receivedStatus: NoteStatus, target: NoteStatus, write: (target: NoteStatus) => Promise<void>): boolean {
    if (targetStatus(noteId, receivedStatus) === target) return false;
    intents.set(noteId, target);
    const previous = files.get(noteId) ?? Promise.resolve();
    // `catch` swallows the failure of a previous write: it must not prevent the next one.
    const next = previous.catch(() => {}).then(() => write(target));
    files.set(noteId, next);
    return true;
  }

  function reconcile(noteId: string, receivedStatus: NoteStatus): void {
    if (intents.get(noteId) === receivedStatus) intents.delete(noteId);
  }

  function abandon(noteId: string, failedTarget: NoteStatus): void {
    // A failure does not undo a more recent intent: the latest request remains the reference.
    if (intents.get(noteId) === failedTarget) intents.delete(noteId);
  }

  return { targetStatus, schedule, reconcile, abandon };
}

/**
 * Kanban columns with the in-flight intent applied: a note targeted by an unconfirmed
 * write already appears in the destination column (not in the old one).
 */
export function columnsWithIntents(notes: Note[], file: StatusQueue): { id: NoteStatus; items: Note[] }[] {
  // No title here: the result is memoized by the board, a copied title would stay there in
  // the old language. The title is read at render time (`statusLabel`).
  return KANBAN_COLUMNS.map(col => ({
    id: col.id,
    items: notes.filter(note => file.targetStatus(note.id, note.status) === col.id),
  }));
}

/**
 * Wraps a status write in the chain tracked by the editor (`trackEditorWrite`):
 * export/restore waits for these writes before freezing a snapshot. The Kanban ↔ Kanban
 * order, for its part, is ensured by `createStatusQueue`.
 */
export function writeTrackedStatus(write: () => Promise<void>): Promise<void> {
  return trackEditorWrite(write());
}

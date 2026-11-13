import type { Note } from '../types';

/** Time spent in the trash before automatic erasure (`TRASH_RETENTION_DAYS` on the Rust side). */
export const TRASH_RETENTION_DAYS = 30;
const DAY_MS = 24 * 60 * 60 * 1000;

/** Days remaining before automatic erasure, never negative. */
export function daysBeforeErasure(deletedAt: string, now = Date.now()): number {
  const remaining = Date.parse(deletedAt) + TRASH_RETENTION_DAYS * DAY_MS - now;
  return Math.max(0, Math.ceil(remaining / DAY_MS));
}

/**
 * Rows of the trash: a sub-case that went to the trash with its parent case (same instant) is
 * part of the parent's row and comes back with it; the others have their own row.
 */
export function trashRows(notes: readonly Note[]): { note: Note; subCases: number }[] {
  const byId = new Map(notes.map(note => [note.id, note]));
  const rodeAlong = (note: Note) => {
    const parent = note.parent_id ? byId.get(note.parent_id) : undefined;
    return !!parent && parent.deleted_at === note.deleted_at;
  };
  const rootOf = (note: Note): Note => {
    let current = note;
    while (rodeAlong(current)) current = byId.get(current.parent_id as string) as Note;
    return current;
  };
  const counts = new Map<string, number>();
  for (const note of notes) {
    if (rodeAlong(note)) { const root = rootOf(note).id; counts.set(root, (counts.get(root) ?? 0) + 1); }
  }
  return notes.filter(note => !rodeAlong(note)).map(note => ({ note, subCases: counts.get(note.id) ?? 0 }));
}

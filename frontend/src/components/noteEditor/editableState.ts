import type { Note, NoteType, Project, ProjectSpace, UpdateNoteInput } from '../../types';
import { readTags } from '../../utils/tags';

/** Space (Work/Private) of a project; `null` = unknown → no limit. */
export function projectSpace(projectId: string | null | undefined, projects?: Project[]): ProjectSpace | null {
  if (!projectId) return null;
  return projects?.find(project => project.id === projectId)?.space ?? null;
}

/** Space (Work/Private) of a note based on its project; `null` = unknown → no limit. */
export function spaceOfNote(note: Note | null, projects?: Project[]): ProjectSpace | null {
  if (!note) return null;
  return projectSpace(note.project_id, projects);
}

/** Editable fields of a note, normalized (`''` = no value, `tags` as an array). */
export interface EditableState {
  id: string | null;
  content: string;
  title: string;
  note_type: NoteType;
  priority: string;
  due_at: string;
  tags: string[];
}

export const EDITABLE_FIELDS = ['content', 'title', 'note_type', 'priority', 'due_at', 'tags'] as const;

export function stateFromNote(note: Note | null): EditableState {
  return {
    id: note?.id ?? null,
    content: note?.content ?? '',
    title: note?.title ?? '',
    note_type: (note?.note_type as NoteType) ?? 'note',
    priority: note?.priority ?? '',
    due_at: note?.due_at ?? '',
    tags: readTags(note?.tags),
  };
}

/** Array-tolerant equality (tags are compared by content, not by reference). */
export function equal(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) || Array.isArray(b)) return JSON.stringify(a) === JSON.stringify(b);
  return a === b;
}

/**
 * Builds the write payload **only from the fields that differ** from the last
 * known version in the database. This is what prevents overwriting, by rewriting a stale
 * local value, a field modified in the meantime by another source (MCP agent, other instance).
 */
export function buildPayload(base: EditableState, eff: EditableState): UpdateNoteInput {
  const payload: UpdateNoteInput = {};
  if (!equal(eff.content, base.content)) payload.content = eff.content;
  if (!equal(eff.title, base.title)) payload.title = eff.title;
  if (!equal(eff.note_type, base.note_type)) payload.note_type = eff.note_type;
  // `''` = removal (Rust convention): the value is sent as is.
  if (!equal(eff.priority, base.priority)) payload.priority = eff.priority;
  if (!equal(eff.due_at, base.due_at)) payload.due_at = eff.due_at;
  if (!equal(eff.tags, base.tags)) payload.tags = JSON.stringify(eff.tags);
  return payload;
}

/** Values of the last known state for only the fields of the failed draft. */
export function baseDuPayload(base: EditableState, payload: UpdateNoteInput): Partial<UpdateNoteInput> {
  const expectedValue: Partial<UpdateNoteInput> = {};
  if (payload.content !== undefined) expectedValue.content = base.content;
  if (payload.title !== undefined) expectedValue.title = base.title;
  if (payload.note_type !== undefined) expectedValue.note_type = base.note_type;
  if (payload.priority !== undefined) expectedValue.priority = base.priority;
  if (payload.due_at !== undefined) expectedValue.due_at = base.due_at;
  if (payload.tags !== undefined) expectedValue.tags = JSON.stringify(base.tags);
  return expectedValue;
}

/**
 * Only advances the "unmodified" reference for the fields **actually written**.
 *
 * Advancing every field of the snapshot (including those absent from a partial payload) made
 * it look like the database contained a value that was never sent to it: a later return
 * to the previous value was then no longer detected as a modification.
 */
export function advanceBaseline(
  base: EditableState,
  noteId: string,
  payload: UpdateNoteInput,
  eff: EditableState
): void {
  if (base.id !== noteId) return;
  if (payload.content !== undefined) base.content = eff.content;
  if (payload.title !== undefined) base.title = eff.title;
  if (payload.note_type !== undefined) base.note_type = eff.note_type;
  if (payload.priority !== undefined) base.priority = eff.priority;
  if (payload.due_at !== undefined) base.due_at = eff.due_at;
  if (payload.tags !== undefined) base.tags = eff.tags;
}

/** Length of the common prefix of two strings (dictation only accumulates text). */
export function commonPrefix(a: string, b: string): number {
  const n = Math.min(a.length, b.length);
  let i = 0;
  while (i < n && a[i] === b[i]) i += 1;
  return i;
}

/**
 * Part of a dictation that has not yet been persisted.
 *
 * A window close may have written a prefix of the raw text; the end of the dictation must
 * only add the rest, otherwise this prefix would be duplicated.
 */
export function unpersistedRemainder(text: string, alreadyPersisted: string): string {
  return text.slice(commonPrefix(text, alreadyPersisted)).trim();
}

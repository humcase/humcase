import type { Note, NoteStatus, NoteType } from '../types';
import { ORDERED_STATUSES } from './statuses';

/**
 * Shortcuts by nature, under "All cases" in the sidebar: every case of one nature across the
 * space, wherever it lives. A shortcut is a view: it moves nothing.
 * Decisions, tasks, ideas, questions, research and issues follow the case's nature (`note_type`);
 * documents and drawings follow what the case holds (`content_kind`).
 */
export type NatureShortcut = 'decisions' | 'tasks' | 'documents' | 'drawings' | 'ideas' | 'questions' | 'research' | 'issues';

export const NATURE_SHORTCUTS: NatureShortcut[] = ['decisions', 'tasks', 'documents', 'drawings', 'ideas', 'questions', 'research', 'issues'];
export const DEFAULT_SHORTCUTS: NatureShortcut[] = ['decisions', 'tasks', 'documents', 'drawings'];

/** The rule of the sidebar's counts: a live case that is not resolved, sub-cases included. */
export function countsAsUnresolved(note: Note): boolean {
  return !note.deleted_at && !note.archived_at && note.status !== 'resolved';
}

/** Unresolved cases of each project, sub-cases included. */
export function unresolvedByProject(notes: readonly Note[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const note of notes) if (countsAsUnresolved(note)) counts.set(note.project_id, (counts.get(note.project_id) ?? 0) + 1);
  return counts;
}

export function inShortcut(shortcut: NatureShortcut, note: Note): boolean {
  switch (shortcut) {
    case 'decisions': return note.note_type === 'decision';
    case 'tasks': return note.note_type === 'todo';
    case 'documents': return note.content_kind === 'link' || note.content_kind === 'file' || note.content_kind === 'image';
    case 'drawings': return note.content_kind === 'sketch';
    case 'ideas': return note.note_type === 'idea';
    case 'questions': return note.note_type === 'question';
    case 'research': return note.note_type === 'research';
    case 'issues': return note.note_type === 'bug';
  }
}

/**
 * The nature a shortcut creates with "New case"; null for documents and drawings, which hold
 * a link, a file, an image or a drawing and are added inside a case.
 */
export function shortcutNature(shortcut: NatureShortcut): NoteType | null {
  switch (shortcut) {
    case 'decisions': return 'decision';
    case 'tasks': return 'todo';
    case 'ideas': return 'idea';
    case 'questions': return 'question';
    case 'research': return 'research';
    case 'issues': return 'bug';
    case 'documents':
    case 'drawings': return null;
  }
}

/** Key of a document filter's label under `workspace.shortcuts.groups`. */
export type GroupLabel = 'links' | 'files' | 'images';

/**
 * A filter of a shortcut's view: a status, named like everywhere else (To do, In progress,
 * Resolved), or for documents what they hold.
 */
export type ShortcutGroup = { id: string; match: (note: Note) => boolean } & ({ status: NoteStatus; label?: never } | { label: GroupLabel; status?: never });

const BY_STATUS: ShortcutGroup[] = ORDERED_STATUSES.map(status => ({ id: status, status, match: (note: Note) => note.status === status }));

export function shortcutGroups(shortcut: NatureShortcut): ShortcutGroup[] {
  switch (shortcut) {
    case 'documents': return [
      { id: 'link', label: 'links', match: note => note.content_kind === 'link' },
      { id: 'file', label: 'files', match: note => note.content_kind === 'file' },
      { id: 'image', label: 'images', match: note => note.content_kind === 'image' },
    ];
    case 'drawings': return [];
    default: return BY_STATUS;
  }
}

/**
 * Unresolved cases of each shortcut, the rule of every count in the sidebar (a resolved, trashed
 * or archived case is not counted).
 */
export function shortcutCounts(notes: Note[]): Record<NatureShortcut, number> {
  const counts = Object.fromEntries(NATURE_SHORTCUTS.map(key => [key, 0])) as Record<NatureShortcut, number>;
  for (const note of notes) {
    if (!countsAsUnresolved(note)) continue;
    for (const key of NATURE_SHORTCUTS) if (inShortcut(key, note)) counts[key] += 1;
  }
  return counts;
}

// The shortcuts shown and whether the list is unfolded are remembered on this computer. Storage
// can be refused (disabled, private mode): the defaults then apply, nothing breaks.
const SHOWN_KEY = 'humcase-nature-shortcuts';
const OPEN_KEY = 'humcase-nature-shortcuts-open';

export function readShownShortcuts(): NatureShortcut[] {
  try {
    const raw = globalThis.localStorage?.getItem(SHOWN_KEY);
    if (!raw) return DEFAULT_SHORTCUTS;
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return DEFAULT_SHORTCUTS;
    return NATURE_SHORTCUTS.filter(key => parsed.includes(key));
  } catch { return DEFAULT_SHORTCUTS; }
}

export function saveShownShortcuts(shown: NatureShortcut[]): void {
  try { globalThis.localStorage?.setItem(SHOWN_KEY, JSON.stringify(shown)); } catch { /* kept for this session only */ }
}

export function readShortcutsOpen(): boolean {
  try { return globalThis.localStorage?.getItem(OPEN_KEY) !== 'false'; } catch { return true; }
}

export function saveShortcutsOpen(open: boolean): void {
  try { globalThis.localStorage?.setItem(OPEN_KEY, String(open)); } catch { /* kept for this session only */ }
}

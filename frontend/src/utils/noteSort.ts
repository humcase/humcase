import { currentLanguage } from '../i18n';
import type { Note } from '../types';

/** `relevance`: the order of the database search, best match first (only while searching). */
export type NoteSort = 'relevance' | 'attention' | 'recent' | 'priority' | 'title';

const statusOrder: Record<Note['status'], number> = { open: 0, in_progress: 1, resolved: 2 };

function priorityOrder(priority: Note['priority']): number {
  const match = /^P([0-3])$/.exec(priority ?? '');
  return match ? Number(match[1]) : 4;
}

function newestFirst(a: Note, b: Note): number {
  return b.updated_at.localeCompare(a.updated_at) || a.id.localeCompare(b.id);
}

/**
 * Presentation sort only: the get_notes contract (MCP/CLI) stays unchanged. `rank` is the position
 * of each case in the search answer; without it, `relevance` falls back to `attention`.
 */
export function sortNotes(notes: Note[], sort: NoteSort, rank?: ReadonlyMap<string, number> | null): Note[] {
  return [...notes].sort((a, b) => {
    if (sort === 'relevance' && rank) return (rank.get(a.id) ?? Infinity) - (rank.get(b.id) ?? Infinity) || newestFirst(a, b);
    if (sort === 'recent') return newestFirst(a, b);
    if (sort === 'title') return a.title.localeCompare(b.title, currentLanguage(), { sensitivity: 'base' }) || newestFirst(a, b);
    if (sort === 'priority') {
      return priorityOrder(a.priority) - priorityOrder(b.priority)
        || statusOrder[a.status] - statusOrder[b.status]
        || newestFirst(a, b);
    }
    return statusOrder[a.status] - statusOrder[b.status]
      || priorityOrder(a.priority) - priorityOrder(b.priority)
      || newestFirst(a, b);
  });
}

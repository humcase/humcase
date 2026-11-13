import { NOTE_TYPES, type Note, type NoteStatus, type NoteType } from '../types';
import type { Span } from './caseLayout';
import { readTags } from './tags';

/** Sub-cases listed inside a group of the size of a card, and inside a group two rows high. */
export const GROUP_ROWS_SMALL = 4;
export const GROUP_ROWS_TALL = 10;

/**
 * Room of a case on the board: a card, or two rows high for a group of more than four sub-cases,
 * which then lists its first ten. The board pins the cases around before a group changes size.
 */
export function caseSpan(subCaseCount: number): Span {
  return { w: 1, h: subCaseCount > GROUP_ROWS_SMALL ? 2 : 1 };
}

/** True when `targetId` is `id` itself or sits somewhere under it. */
export function isInside(targetId: string, id: string, byId: ReadonlyMap<string, Note>): boolean {
  const seen = new Set<string>();
  for (let cursor: string | null | undefined = targetId; cursor && !seen.has(cursor); cursor = byId.get(cursor)?.parent_id) {
    if (cursor === id) return true;
    seen.add(cursor);
  }
  return false;
}

/**
 * Whether these cases can go inside `parentId` (null: the top level): never into themselves or
 * their own sub-cases, never into another project, and not where they already are.
 */
export function canMoveInto(ids: readonly string[], parentId: string | null, byId: ReadonlyMap<string, Note>): boolean {
  if (ids.length === 0) return false;
  const parent = parentId ? byId.get(parentId) : null;
  if (parentId && !parent) return false;
  return ids.every(id => {
    const note = byId.get(id);
    if (!note) return false;
    if ((note.parent_id ?? null) === parentId) return false;
    if (!parent) return true;
    return parent.project_id === note.project_id && !isInside(parent.id, id, byId);
  });
}

/**
 * A case matches the search when its text holds every word of it, near forms included, and it
 * carries every `#tag` of the search, and when it passes the nature and status filters. These are
 * the cases the database search always returns (accents ignored, dictation and report included);
 * the database answers a moment later and may add cases holding only some of the words.
 */
export function caseMatches(note: Note, query: string, nature: NoteType | null, status: NoteStatus | null = null): boolean {
  if (nature && note.note_type !== nature) return false;
  if (status && note.status !== status) return false;
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const isTag = (word: string) => word.length > 1 && word.startsWith('#');
  const tags = words.filter(isTag).map(word => word.slice(1));
  if (tags.length) {
    const own = readTags(note.tags).map(tag => tag.toLocaleLowerCase());
    if (!tags.every(tag => own.includes(tag))) return false;
  }
  const textWords = words.filter(word => !isTag(word)).flatMap(searchWords);
  if (!textWords.length) return true;
  const text = new Set(searchWords([note.title, note.content, note.raw_content, note.resolution_comment].filter(Boolean).join('\n')));
  return textWords.every(word => {
    const root = nearFormRoot(word);
    return root === null ? text.has(word) : [...text].some(own => own.startsWith(root));
  });
}

/**
 * Root of a word's near forms, as the database search (`db/queries/search.rs`): from five
 * letters, a final s or x, then a final e, are dropped and the rest matches as a prefix
 * ("liens" finds "lien", "micro" finds "microphone"). `null` for a shorter word, matched whole.
 */
function nearFormRoot(word: string): string | null {
  const chars = [...word];
  if (chars.length < 5) return null;
  if (chars.at(-1) === 's' || chars.at(-1) === 'x') chars.pop();
  if (chars.at(-1) === 'e') chars.pop();
  return chars.join('');
}

/** The words of a text as the database's full-text index sees them (SQLite `unicode61`): letters and digits, lowercase, without accents. */
function searchWords(text: string): string[] {
  return text.normalize('NFD').replace(/\p{M}+/gu, '').toLocaleLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);
}

/** How many cases of each nature, in the order natures are offered. */
export function natureCounts(notes: readonly Note[]): [NoteType, number][] {
  const counts = new Map<NoteType, number>();
  for (const note of notes) counts.set(note.note_type, (counts.get(note.note_type) ?? 0) + 1);
  return NOTE_TYPES.filter(type => counts.has(type)).map(type => [type, counts.get(type) as number]);
}

/** How many live sub-cases, at any depth, these cases hold: they go to the trash with them. */
export function subCaseCount(ids: readonly string[], notes: readonly Note[]): number {
  const children = new Map<string, string[]>();
  for (const note of notes) {
    if (!note.parent_id || note.deleted_at) continue;
    children.set(note.parent_id, [...children.get(note.parent_id) ?? [], note.id]);
  }
  const seen = new Set<string>(ids);
  const queue = [...ids];
  let count = 0;
  while (queue.length) {
    for (const child of children.get(queue.pop() as string) ?? []) {
      if (seen.has(child)) continue;
      seen.add(child);
      count++;
      queue.push(child);
    }
  }
  return count;
}

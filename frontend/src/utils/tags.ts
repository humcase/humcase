/**
 * Reads the tags stored in the database (JSON), keeping only **strings**.
 *
 * A corrupted `tags` (number, object, invalid JSON) must never reach a component that
 * calls `toLowerCase()` on every entry: without a guard, `[42]` causes a `TypeError` that
 * unmounts the whole React tree (white screen). Single source shared by `NoteEditor` and `NoteList`:
 * the list must count tags **after decoding**, never the length of the raw JSON.
 */
export function readTags(tags: string | null | undefined): string[] {
  if (!tags) return [];
  try {
    const parsed = JSON.parse(tags) as unknown;
    return Array.isArray(parsed) ? parsed.filter((t): t is string => typeof t === 'string') : [];
  } catch {
    return [];
  }
}

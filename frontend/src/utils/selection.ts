/**
 * Resolution of the active note **independently of filters**.
 *
 * The displayed list may be filtered (status, search) while the command palette or
 * note creation select a note that is not in it: the editor must still open it.
 * So we search several lists, from the most complete to the most filtered.
 */
export function resolveActiveNote<T extends { id: string }>(
  activeNoteId: string | null,
  lists: ReadonlyArray<ReadonlyArray<T>>
): T | null {
  if (!activeNoteId) return null;
  for (const list of lists) {
    const found = list.find(n => n.id === activeNoteId);
    if (found) return found;
  }
  return null;
}

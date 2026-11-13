import type { Note } from '../types';

/** Direct live sub-cases of a case, oldest first (the order of `list_sub_cases`). */
export function directSubCases(caseId: string, notes: Note[]): Note[] {
  return notes
    .filter(candidate => candidate.parent_id === caseId && !candidate.deleted_at)
    .sort((a, b) => a.created_at.localeCompare(b.created_at) || a.id.localeCompare(b.id));
}

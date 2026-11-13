import type { Note, NoteStatus, NoteType } from '../types';
import { caseMatches } from './caseBoard';
import { casesToFollow } from './dueDates';
import { ORDERED_STATUSES } from './statuses';

export interface CaseViewInput {
  /** Every case of the database. */
  notes: readonly Note[];
  /** Projects of the chosen space. */
  visibleProjectIds: ReadonlySet<string>;
  archivedProjectIds: ReadonlySet<string>;
  /** Projects of the chosen activity; null without one. */
  activityProjectIds: ReadonlySet<string> | null;
  activeProjectId: string | null;
  activeNoteId: string | null;
  /** The search, trimmed. */
  query: string;
  /** Cases the database found for the search; null while it answers (a quick match stands in). */
  searchIds: ReadonlySet<string> | null;
  status: string;
  nature: NoteType | '';
  includeSubCases: boolean;
  today: boolean;
  /** The cases page shows the archived cases only. */
  showArchived: boolean;
  /** Clock for "Today". */
  now?: Date;
}

/**
 * Everything the cases page, Home, the shortcuts and the palette read from the cases, computed in
 * one place from the whole database and the page's filters. Pure: the application memoizes it, so a
 * click that changes no filter and no case recomputes nothing.
 *
 * Archiving deletes nothing: an archived case, or one filed in an archived project, leaves the general
 * views and reappears when searched for, when that project is opened or when archived cases are
 * displayed. The open case stays visible so as not to close the editor.
 */
export function deriveCaseView(input: CaseViewInput) {
  const { notes, visibleProjectIds, archivedProjectIds, activityProjectIds, activeProjectId, activeNoteId, query, searchIds, status, nature, includeSubCases, today, showArchived } = input;
  const searching = query !== '';
  const visible = (note: Note) => visibleProjectIds.has(note.project_id) && (
    note.id === activeNoteId || searching || (
      (!archivedProjectIds.has(note.project_id) || note.project_id === activeProjectId)
      && (showArchived ? Boolean(note.archived_at) : !note.archived_at)));
  const matchesSearch = (note: Note) => !searching || (searchIds ? searchIds.has(note.id) : caseMatches(note, query, null, null));

  // `allScoped`: every visible case of the space; `projectCases`: those of the active project (all
  // of them without one); `spaceCases`: those of the chosen activity, for Home, the shortcuts and Today.
  const allScoped = notes.filter(visible);
  const projectCases = activeProjectId ? allScoped.filter(note => note.project_id === activeProjectId) : allScoped;
  const spaceCases = activityProjectIds ? allScoped.filter(note => activityProjectIds.has(note.project_id)) : allScoped;
  const topLevelProjectCases = projectCases.filter(note => !note.parent_id);
  const homeCases = spaceCases.filter(note => !note.parent_id);
  // The page's list: the project, the status and the search, then what is visible.
  const pageNotes = notes.filter(note => (!activeProjectId || note.project_id === activeProjectId) && (!status || note.status === status) && matchesSearch(note));
  const scopedNotes = pageNotes.filter(visible);
  const archivedNotesCount = pageNotes.filter(note => note.archived_at && visibleProjectIds.has(note.project_id)).length;

  const subCaseCounts = new Map<string, number>();
  for (const note of allScoped) {
    if (note.parent_id && !note.deleted_at) subCaseCounts.set(note.parent_id, (subCaseCounts.get(note.parent_id) ?? 0) + 1);
  }

  // "Today": the cases to follow up across the space, the same rule as Home's "To follow up".
  const todayCases = casesToFollow(spaceCases, input.now);
  const todayIds = new Set(todayCases.map(note => note.id));
  // Sub-cases live inside their case: the views list top-level cases unless sub-cases are asked
  // for, or a search or "Today" looks for them.
  const levelShown = (note: Note) => today ? todayIds.has(note.id) : includeSubCases || !note.parent_id;
  const natureShown = (note: Note) => !nature || note.note_type === nature;
  const listed = (note: Note) => levelShown(note) || (searching && !today);
  const displayedNotes = scopedNotes.filter(note => listed(note) && natureShown(note));
  // Filter counts: a count is what a click shows, so they follow the search and the level shown;
  // each count leaves out its own filter (the status counts ignore the chosen status, and so on).
  const countBase = projectCases.filter(note => listed(note) && matchesSearch(note));
  const statusCounts = Object.fromEntries([['all', countBase.filter(natureShown).length], ...ORDERED_STATUSES.map(value => [value, countBase.filter(note => natureShown(note) && note.status === value).length])]) as Record<NoteStatus | 'all', number>;
  const natureCounts = new Map<NoteType, number>();
  for (const note of countBase) {
    if (!status || note.status === status) natureCounts.set(note.note_type, (natureCounts.get(note.note_type) ?? 0) + 1);
  }
  const subCaseTotal = projectCases.filter(note => note.parent_id && natureShown(note) && matchesSearch(note)).length;
  const caseById = new Map(allScoped.map(note => [note.id, note]));

  return {
    allScoped, projectCases, spaceCases, topLevelProjectCases, homeCases, scopedNotes, archivedNotesCount,
    subCaseCounts, todayCases, todayIds, displayedNotes, statusCounts, natureCounts, subCaseTotal, caseById,
  };
}

export type CaseView = ReturnType<typeof deriveCaseView>;

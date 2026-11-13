/** Display mode of the cases page: the board of cases, cards, list or Kanban. */
export type NotesViewMode = 'plan' | 'cards' | 'list' | 'kanban';

const KEY = 'humcase-view-mode';
const MODES: NotesViewMode[] = ['plan', 'cards', 'list', 'kanban'];

/** Remembered mode, the board by default (unreadable storage = the board). */
export function readViewMode(): NotesViewMode {
  try {
    const stored = localStorage.getItem(KEY);
    return MODES.includes(stored as NotesViewMode) ? stored as NotesViewMode : 'plan';
  } catch {
    return 'plan';
  }
}

export function saveViewMode(mode: NotesViewMode): void {
  try { localStorage.setItem(KEY, mode); } catch { /* session-only */ }
}

const SUMMARY_KEY = 'humcase-project-summary';

/** Whether a project's page opens on its summary (remembered; the cases by default). */
export function readProjectSummary(): boolean {
  try { return localStorage.getItem(SUMMARY_KEY) === '1'; } catch { return false; }
}

export function saveProjectSummary(summary: boolean): void {
  try { localStorage.setItem(SUMMARY_KEY, summary ? '1' : '0'); } catch { /* session-only */ }
}

import { useCallback } from 'react';
import { useNotesChangeWatch } from '../utils/useNotesChangeWatch';

interface UseWorkspaceChangeWatchOptions {
  /** Database fingerprint of notes, projects, activities and attachments (`notes_change_marker`). */
  readFingerprint: () => Promise<string>;
  loadNotes: () => Promise<void>;
  loadProjects: () => Promise<void>;
  loadActivities: () => Promise<void>;
}

/**
 * Rereads notes, projects and activities when the database fingerprint changes: an MCP agent or the CLI
 * (another process) wrote while the window keeps focus.
 *
 * These are the same rereads as on return to the foreground, hence the same safeguards: the editor
 * only adopts an external value for fields not modified locally (invariant 2), and the list
 * of projects is never an input of the open draft. A failure to reread activities is
 * transient (the next change retries) and does not block reading notes or projects.
 * Returns `checkNow`, for the return to the foreground.
 */
export function useWorkspaceChangeWatch({ readFingerprint, loadNotes, loadProjects, loadActivities }: UseWorkspaceChangeWatchOptions) {
  const onChange = useCallback(() => {
    void loadNotes();
    void loadProjects();
    void loadActivities().catch(cause => { console.error('Could not reread activities', cause); });
  }, [loadNotes, loadProjects, loadActivities]);
  return useNotesChangeWatch(readFingerprint, onChange);
}

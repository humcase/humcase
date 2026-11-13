import { useCallback, useEffect, useState } from 'react';
import type { CaseJournal } from '../../types';
import { api } from '../../services/tauriApi';

/**
 * Journal of a case (who did what, decisions, where they stand), reread when the case changes
 * revision or after a decision. A failed read is reported, never shown as an empty history.
 */
export function useCaseJournal(noteId: string, revision: number) {
  const [state, setState] = useState<{ noteId: string; journal: CaseJournal | null; failed: boolean }>({ noteId, journal: null, failed: false });
  const [reloads, setReloads] = useState(0);

  useEffect(() => {
    if (!noteId) return;
    let active = true;
    Promise.resolve(api.getCaseJournal?.(noteId))
      .then(journal => { if (active) setState({ noteId, journal: journal ?? null, failed: false }); })
      .catch(() => { if (active) setState({ noteId, journal: null, failed: true }); });
    return () => { active = false; };
  }, [noteId, revision, reloads]);

  const reload = useCallback(() => setReloads(count => count + 1), []);
  // The previous case's journal is never shown for the next one.
  const current = state.noteId === noteId;
  return { journal: current ? state.journal : null, failed: current && state.failed, reload };
}

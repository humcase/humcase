import { useState, type Dispatch, type MutableRefObject, type SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../services/tauriApi';
import { setLastProjectId } from '../utils/prefs';
import type { BackupPreparation } from '../utils/backupBarrier';
import type { Note, Project } from '../types';

interface UseInboxMoveOptions {
  /** Note open in the editor (or `null` outside the note page): bounds the validity of the choice. */
  moveScope: string | null;
  /** Writes what the editors hold; refuses while a write of this case is kept for recovery. */
  prepareMove: (noteId: string) => Promise<BackupPreparation>;
  spaceEpochRef: MutableRefObject<number>;
  setActiveProjectId: Dispatch<SetStateAction<string | null>>;
  loadNotes: () => Promise<void>;
  loadProjects: () => Promise<void>;
}

/** Filing a note from the Inbox into a project (backup prepared, then move). */
export function useInboxMove({ moveScope, prepareMove, spaceEpochRef, setActiveProjectId, loadNotes, loadProjects }: UseInboxMoveOptions) {
  const { t } = useTranslation();
  const [moveTargetId, setMoveTargetId] = useState('');
  // Optional case of the chosen project the note goes into, as a sub-case.
  const [moveParentId, setMoveParentId] = useState('');
  const [movePending, setMovePending] = useState(false);
  const [moveError, setMoveError] = useState<string | null>(null);
  // Filing from the Inbox: a project chosen but not applied must never be displayed on
  // another note, nor on the same note reopened (it would suggest the note is already filed).
  // Reset during render when the note changes or when leaving the note page.
  const [moveNoteId, setMoveNoteId] = useState<string | null>(null);
  if (moveNoteId !== moveScope) {
    setMoveNoteId(moveScope);
    setMoveTargetId('');
    setMoveParentId('');
    setMoveError(null);
  }

  const chooseTarget = (id: string) => { setMoveTargetId(id); setMoveParentId(''); setMoveError(null); };
  const chooseParent = (id: string) => { setMoveParentId(id); setMoveError(null); };

  const moveNote = async (activeNote: Note | undefined, activeProject: Project | undefined) => {
    if (!moveTargetId || !activeNote || !activeProject || movePending) return;
    const epoch = spaceEpochRef.current;
    setMovePending(true); setMoveError(null);
    try {
      const ready = await prepareMove(activeNote.id);
      if (!ready.ready) { setMoveError(ready.reason); return; }
      // One transaction: the project, then the case when one is chosen. Should the case refuse
      // the note, it stays in the Inbox and the filing can be tried again.
      await api.moveNoteToProject(activeNote.id, moveTargetId, moveParentId || null);
      if (spaceEpochRef.current !== epoch) return;
      setActiveProjectId(moveTargetId); setLastProjectId(moveTargetId); setMoveTargetId(''); setMoveParentId('');
      await loadNotes(); await loadProjects();
    } catch (cause) { setMoveError(t('workspace.inbox.moveError', { error: String(cause) })); }
    finally { setMovePending(false); }
  };

  return { moveTargetId, moveParentId, movePending, moveError, chooseTarget, chooseParent, moveNote };
}

import { useCallback, useEffect, useMemo, useState, type MutableRefObject } from 'react';
import { useTranslation } from 'react-i18next';
import type { Project, ProjectSpace, UpdateNoteInput } from '../../types';
import { api } from '../../services/tauriApi';
import {
  addPendingWrite,
  readPendingWrites,
  removePendingWrite,
  subscribePendingWrites,
  type PendingWrite,
} from '../../utils/pendingWrites';
import { projectSpace } from './editableState';
import type { ShowToast } from './useEditorToast';

/** Writes the raw dictation to a note by **appending** to the existing raw text (never overwriting it). */
export async function persistRaw(noteId: string, text: string): Promise<void> {
  await api.appendRawContent(noteId, text);
}

/**
 * The writes the editor could not make (draft or raw dictation), kept outside the component in
 * the escrow (`utils/pendingWrites.ts`) so they are offered again on the next display, and their
 * recovery. Replaces the former `rawPending` state, which disappeared with the component and was
 * erased by a later successful dictation.
 */
export function usePendingWrites({ noteSpace, currentNoteIdRef, showToast, onNoteUpdated }: {
  noteSpace: ProjectSpace | null;
  /** The case displayed, up to date on every render. */
  currentNoteIdRef: MutableRefObject<string | null>;
  showToast: ShowToast;
  onNoteUpdated: () => void;
}) {
  const { t } = useTranslation();
  const [pendingWrites, setPendingWrites] = useState<PendingWrite[]>(() => readPendingWrites());
  const [recoveryProblems, setRecoveryProblems] = useState<Record<string, string>>({});
  const [recoveryInProgress, setRecoveryInProgress] = useState(false);

  // The editor stays mounted while a drawing is open: the write a drawing could not save shows
  // here at once. The displayed case's own entries are refreshed by the editor itself, so one its
  // report form keeps after the list was shown is never ignored or replayed without being shown.
  useEffect(() => subscribePendingWrites(() => {
    const own = currentNoteIdRef.current;
    setPendingWrites(previous => {
      const shown = new Set(previous.map(entry => entry.id));
      return readPendingWrites().filter(entry => entry.noteId !== own || shown.has(entry.id));
    });
  }), [currentNoteIdRef]);

  // A draft without a remembered space (old version) stays visible everywhere: it is never
  // deleted nor made unrecoverable, unlike a draft from a known and different space.
  // Linked-task drafts are not replayed here: they are restored in their form
  // (NoteRelations). The panel must only show the editor's writes.
  const displayed = useMemo<PendingWrite[]>(() => {
    const editorWrites = pendingWrites.filter(e => e.kind !== 'task-draft');
    if (noteSpace == null) return editorWrites;
    return editorWrites.filter(e => e.space == null || e.space === noteSpace);
  }, [pendingWrites, noteSpace]);

  /** Shows the escrow as it is now. */
  const refresh = useCallback(() => setPendingWrites(readPendingWrites()), []);

  const recordDraftFailure = useCallback((noteId: string, payload: UpdateNoteInput, base: Partial<UpdateNoteInput>, space?: ProjectSpace | null, projectId?: string | null) => {
    addPendingWrite({ kind: 'update', noteId, payload, base, space: space ?? null, projectId: projectId ?? null });
    setPendingWrites(readPendingWrites());
  }, []);

  /** A confirmed write replaces the old failures on the same fields, never the others. */
  const removeCoveredFailures = useCallback((noteId: string, payload: UpdateNoteInput) => {
    const confirmedFields = Object.keys(payload).filter(field => payload[field as keyof UpdateNoteInput] !== undefined);
    let removed = false;
    for (const entry of readPendingWrites()) {
      if (entry.kind !== 'update' || entry.noteId !== noteId) continue;
      if (Object.keys(entry.payload).every(field => confirmedFields.includes(field))) {
        removePendingWrite(entry.id);
        removed = true;
      }
    }
    if (removed) setPendingWrites(readPendingWrites());
  }, []);

  /** The backend checks and replays the fields in a single SQLite transaction. */
  const retry = async () => {
    if (recoveryInProgress) return;
    setRecoveryInProgress(true);
    const problems: Record<string, string> = {};
    try {
      const displayedIds = new Set(displayed.map(entry => entry.id));
      // Same scope as "Ignore": a new input added between the render and the click
      // must not be applied to the database without having been shown to the user.
      for (const entry of readPendingWrites().filter(entry => displayedIds.has(entry.id))) {
        try {
          if (entry.kind === 'brut') {
            await persistRaw(entry.noteId, entry.texte);
          } else if (entry.kind === 'update') {
            const result = await api.retryNoteUpdate(entry.noteId, entry.payload, entry.base ?? {});
            if (result === 'not_found') {
              problems[entry.id] = t('editor.pending.notFound');
              continue;
            }
            if (result === 'conflict') {
              problems[entry.id] = entry.base
                ? t('editor.pending.conflict')
                : t('editor.pending.noReference');
              continue;
            }
          }
          removePendingWrite(entry.id);
        } catch (e) {
          console.error("Another failure while recovering a write", e);
          problems[entry.id] = t('editor.pending.failedAgain');
        }
      }
      const remainingEntries = readPendingWrites();
      setPendingWrites(remainingEntries);
      setRecoveryProblems(problems);
      if (remainingEntries.length === 0) showToast(t('editor.pending.recovered'));
      else showToast(t('editor.pending.remaining', { count: remainingEntries.length }), { duration: 6000 });
      onNoteUpdated();
    } finally {
      setRecoveryInProgress(false);
    }
  };

  const ignore = () => {
    // The report may have added a draft after the last render of this
    // list: never erase an entry that was not shown here.
    for (const entry of displayed) removePendingWrite(entry.id);
    setPendingWrites(readPendingWrites());
  };

  return { pendingWrites, displayed, refresh, recordDraftFailure, removeCoveredFailures, retry, ignore, recoveryInProgress, recoveryProblems };
}

/**
 * Re-anchors escrowed entries whose remembered space no longer matches the actual space of their
 * case (project moved): such an entry would be invisible in both spaces. Every entry is checked,
 * not only those of the displayed case: a project can change space while its case is never
 * reopened. The actual space is inferred from the `projectId` captured at failure, through
 * `projects` (loaded globally), without depending on the notes list (`allNotes` only holds the
 * active project in production).
 *
 * The displayed case is authoritative for **its own** entries: its current project takes
 * precedence over the entry's anchor (case filed in another project, deleted project, legacy
 * entry), and `projectId` is resynchronized even without a space change. An entry without
 * `space` is **never** pinned: it stays visible everywhere (escrow invariant). Nothing is ever
 * deleted: only anchors change.
 */
export function pendingWriteRepairs(
  entries: readonly PendingWrite[],
  noteId: string | undefined,
  noteProjectId: string | undefined,
  noteSpace: ProjectSpace | null,
  projects: Project[] | undefined,
): Array<{ id: string; space?: ProjectSpace; projectId?: string | null }> {
  const repairs: Array<{ id: string; space?: ProjectSpace; projectId?: string | null }> = [];
  for (const entry of entries) {
    if (entry.noteId === noteId) {
      // `|| null`: an empty `project_id` is the same as absent, consistent with the escrow normalization.
      const projectId = noteProjectId || null;
      const update: { id: string; space?: ProjectSpace; projectId?: string | null } = { id: entry.id };
      if ((entry.projectId ?? null) !== projectId) update.projectId = projectId;
      if (noteSpace != null && entry.space != null && entry.space !== noteSpace) update.space = noteSpace;
      if (update.space !== undefined || update.projectId !== undefined) repairs.push(update);
    } else {
      // Entry of another case: the actual space is inferred from its project captured at failure.
      const actualSpace = entry.projectId != null ? projectSpace(entry.projectId, projects) : null;
      if (actualSpace != null && entry.space != null && entry.space !== actualSpace) {
        repairs.push({ id: entry.id, space: actualSpace });
      }
    }
  }
  return repairs;
}

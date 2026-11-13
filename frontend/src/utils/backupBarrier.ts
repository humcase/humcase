import i18n from '../i18n';
import { readPendingWrites } from './pendingWrites';

export type BackupPreparation = { ready: true } | { ready: false; reason: string };

// Old NoteEditor instances may still write after being unmounted.
const writes = new Set<Promise<unknown>>();
let voiceProcessings = 0;

export function trackEditorWrite<T>(promise: Promise<T>): Promise<T> {
  writes.add(promise);
  void promise.then(
    () => { writes.delete(promise); },
    () => { writes.delete(promise); },
  );
  return promise;
}

export function beginVoiceProcessing(): () => void {
  voiceProcessings += 1;
  let finishedFlag = false;
  return () => {
    if (!finishedFlag) {
      voiceProcessings -= 1;
      finishedFlag = true;
    }
  };
}

export function voiceProcessing(): boolean {
  return voiceProcessings > 0;
}

/** Common point for export and a future local restore. */
export async function prepareSnapshot(
  prepareEditor: (() => Promise<BackupPreparation>) | null,
  /** Only the kept writes of these cases block (filing a case); all of them by default (a backup). */
  onlyNoteIds?: ReadonlySet<string>,
): Promise<BackupPreparation> {
  if (voiceProcessing()) {
    return { ready: false, reason: i18n.t('notes.barrier.dictationBusy') };
  }
  const editor = prepareEditor ? await prepareEditor() : { ready: true as const };
  if (!editor.ready) return editor;
  // An unmounted instance may have put a flush in its chain just before this request.
  while (writes.size > 0) await Promise.allSettled([...writes]);
  if (voiceProcessing()) {
    return { ready: false, reason: i18n.t('notes.barrier.dictationBusy') };
  }
  // Only writes that touch the database block a backup/restore. Linked-task
  // drafts (`task-draft`) are form inputs restored in
  // NoteRelations: they are never replayed into the database and must not create a deadlock.
  if (readPendingWrites().some(e => (e.kind === 'update' || e.kind === 'brut') && (!onlyNoteIds || onlyNoteIds.has(e.noteId)))) {
    return { ready: false, reason: i18n.t('notes.barrier.unsavedNotes') };
  }
  return { ready: true };
}

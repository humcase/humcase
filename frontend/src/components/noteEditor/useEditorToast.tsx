import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Note } from '../../types';
import { api } from '../../services/tauriApi';
import { Button } from '../ui/Button';
import { Toast } from '../ui/Toast';

export interface ToastOptions {
  /** A case just put in the trash: the message offers to take it back out. */
  undoNote?: Note;
  duration?: number;
  action?: { label: string; run: () => void };
}

export type ShowToast = (message: string, options?: ToastOptions) => void;

/** The editor's message line: one message at a time, with Undo when it has an action. */
export function useEditorToast(onNoteUpdated: () => void): { showToast: ShowToast; toast: ReactNode } {
  const { t } = useTranslation();
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  // Display only: restarts the remaining-time ring on every new message.
  const [toastShown, setToastShown] = useState({ id: 0, duration: 2500 });
  const [deletedNote, setDeletedNote] = useState<Note | null>(null);
  const [toastAction, setToastAction] = useState<{ label: string; run: () => void } | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (toastTimer.current) clearTimeout(toastTimer.current);
  }, []);

  // Stable: the editor's unmount cleanup reports a stopped dictation through it.
  const showToast: ShowToast = useCallback((message, options) => {
    setToastShown(shown => ({ id: shown.id + 1, duration: options?.duration ?? 2500 }));
    setToastMessage(message);
    setDeletedNote(options?.undoNote ?? null);
    setToastAction(options?.action ?? null);
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => {
      setToastMessage(null);
      setDeletedNote(null);
      toastTimer.current = null;
    }, options?.duration ?? 2500);
  }, []);

  const undoCancelledDelete = async () => {
    if (!deletedNote) return;
    // The note did not leave the database: it comes out of the trash unchanged (links, history,
    // raw dictation and images included).
    try {
      await api.restoreFromTrash(deletedNote.id);
    } catch (e) {
      console.error('Failed to restore the note', e);
      showToast(t('editor.messages.restoreFromTrashFailed'), { undoNote: deletedNote, duration: 8000 });
      return;
    }
    setDeletedNote(null);
    setToastMessage(null);
    onNoteUpdated();
  };

  const toast = toastMessage ? (
    <Toast key={toastShown.id} duration={deletedNote || toastAction ? toastShown.duration : undefined}
      action={deletedNote ? <Button variant="link" size="sm" onClick={undoCancelledDelete}>{t('common.cancel')}</Button>
        : toastAction ? <Button variant="link" size="sm" onClick={() => { const run = toastAction.run; setToastMessage(null); setToastAction(null); run(); }}>{toastAction.label}</Button> : undefined}>
      {toastMessage}
    </Toast>
  ) : null;

  return { showToast, toast };
}

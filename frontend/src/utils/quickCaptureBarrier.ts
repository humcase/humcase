import i18n from '../i18n';
import { NOTE_TYPES, type NoteType } from '../types';

export const QUICK_CAPTURE_STATE_KEY = 'humcase-quick-capture-state';

export type QuickCaptureState = {
  title: string;
  noteType: NoteType;
  projectId: string;
  submitting: boolean;
};

// In-memory fallback of the last written state: if local storage refuses the write (quota reached,
// storage disabled…), the draft stays alive in the current context instead of throwing in
// the input. Only persistence is degraded (reading after a restart impossible and the main
// window's restore guard potentially blind), and it is reported by
// `quickCapturePersistenceUnavailable` rather than discovered silently.
let memoryFallback: QuickCaptureState | null = null;
let persistenceRefused = false;

/** True as long as a write has been refused by local storage with no recovery since. */
export function quickCapturePersistenceUnavailable(): boolean {
  return persistenceRefused;
}

/** Cross-window injection: the main window learns, via the event
 *  `humcase:qc-persistence`, that the Quick Capture state is no longer persisted. The value goes back to
 *  `false` when a write succeeds again on the Quick Capture side. */
export function setQuickCapturePersistenceRefused(refused: boolean): void {
  persistenceRefused = refused;
}

/** The fallback is only authoritative if our own writes are refused: otherwise the
 *  persisted state (even empty) is the truth, notably after an external cleanup. */
function fallbackState(): QuickCaptureState | null {
  return persistenceRefused ? memoryFallback : null;
}

export function readQuickCaptureState(): QuickCaptureState | null {
  let raw: string | null = null;
  let readRefused = false;
  try {
    raw = localStorage.getItem(QUICK_CAPTURE_STATE_KEY);
  } catch {
    readRefused = true;
  }
  if (readRefused || raw === null) {
    const fallback = fallbackState();
    if (fallback) return fallback;
    if (readRefused) throw new Error('Quick capture state unreadable');
    return null;
  }
  try {
    const state: unknown = JSON.parse(raw);
    if (!state || typeof state !== 'object') throw new Error('Quick capture state unreadable');
    const value = state as Record<string, unknown>;
    if (typeof value.title !== 'string' || typeof value.projectId !== 'string' ||
        typeof value.submitting !== 'boolean' ||
        !(NOTE_TYPES as readonly string[]).includes(String(value.noteType))) {
      throw new Error('Quick capture state unreadable');
    }
    return value as QuickCaptureState;
  } catch (e) {
    const fallback = fallbackState();
    if (fallback) return fallback;
    throw e;
  }
}

export function writeQuickCaptureState(state: QuickCaptureState): void {
  memoryFallback = state;
  try {
    localStorage.setItem(QUICK_CAPTURE_STATE_KEY, JSON.stringify(state));
    persistenceRefused = false;
  } catch {
    persistenceRefused = true;
  }
}

/** Read from the main window just before replacing the user database. */
export function prepareQuickCaptureForRestore():
  | { ready: true }
  | { ready: false; reason: string } {
  // Fail-closed BEFORE any read: if the Quick Capture window reported that its state is not
  // persisted, the local read proves nothing (the draft lives in the other window, in
  // in-memory fallback) and the restore must be refused.
  if (persistenceRefused) {
    return { ready: false, reason: i18n.t('notes.barrier.quickCaptureNotPersisted') };
  }
  try {
    const state = readQuickCaptureState();
    if (state?.submitting) return { ready: false, reason: i18n.t('notes.barrier.quickCaptureSubmitting') };
    if (state?.title.length) return { ready: false, reason: i18n.t('notes.barrier.quickCaptureDraft') };
    return { ready: true };
  } catch {
    return { ready: false, reason: i18n.t('notes.barrier.quickCaptureUnreadable') };
  }
}

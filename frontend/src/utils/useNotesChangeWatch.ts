import { useCallback, useEffect, useRef } from 'react';

/** Interval for checking notes written by another process (MCP agent, CLI, quick capture). */
export const NOTES_WATCH_INTERVAL_MS = 4000;

/**
 * Rereads the notes when their fingerprint in the database changes, even if the window keeps focus.
 *
 * Before, the interface only reread notes when focus returned: a note resolved by an agent
 * stayed displayed as "In progress" as long as one was looking at the application (bug of 09/28/2026). The
 * check costs only a small query; the reread only happens if something changed.
 * Hidden window (taskbar, notification area): nothing is read. `checkNow` (returned) checks at once,
 * for the return to the foreground: a reread only if something changed meanwhile.
 */
export function useNotesChangeWatch(
  readFingerprint: () => Promise<string>,
  onChange: () => void,
  intervalMs = NOTES_WATCH_INTERVAL_MS,
) {
  const onChangeRef = useRef(onChange);
  useEffect(() => {
    onChangeRef.current = onChange;
  });
  const verifyRef = useRef<() => Promise<void>>(async () => {});

  useEffect(() => {
    let stopped = false;
    let inProgress = false;
    let last: string | null = null;
    const verify = async () => {
      if (stopped || inProgress || document.hidden) return;
      inProgress = true;
      try {
        const fingerprint = await readFingerprint();
        if (stopped) return;
        // First read: reference only, the notes were just loaded at startup.
        if (last !== null && fingerprint !== last) onChangeRef.current();
        last = fingerprint;
      } catch {
        // Transient failure: retry on the next cycle without rereading or alerting.
      } finally {
        inProgress = false;
      }
    };
    verifyRef.current = verify;
    void verify();
    const interval = setInterval(() => { void verify(); }, intervalMs);
    return () => {
      stopped = true;
      clearInterval(interval);
    };
  }, [readFingerprint, intervalMs]);

  return useCallback(() => { void verifyRef.current(); }, []);
}

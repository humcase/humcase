import { useCallback, useRef, useState } from 'react';

/**
 * Message for a one-off action (Handoff, Standup, creation, filing): visible and
 * non-blocking, cleared after 6 seconds. `reportAction` keeps a stable identity.
 */
export function useActionToast() {
  const [actionError, setActionError] = useState<string | null>(null);
  const [toneAction, setToneAction] = useState<'danger' | 'success'>('danger');
  const actionErrorTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reportAction = useCallback((message: string, tone: 'danger' | 'success' = 'danger') => {
    setToneAction(tone);
    setActionError(message);
    if (actionErrorTimer.current) clearTimeout(actionErrorTimer.current);
    actionErrorTimer.current = setTimeout(() => setActionError(null), 6000);
  }, []);
  return { actionError, toneAction, reportAction };
}

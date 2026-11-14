import { useCallback, useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import { saveBeforeExit } from '../utils/beforeExit';

/** Delay before the check: the startup reads (cases, search by meaning) go first. */
export const UPDATE_CHECK_DELAY_MS = 8000;

export type AppUpdate =
  | { phase: 'idle' }
  | { phase: 'available'; version: string }
  | { phase: 'downloading'; version: string; percent: number | null }
  | { phase: 'installing'; version: string }
  | { phase: 'failed'; version: string; error: string };

/**
 * A newer Humcase, found once per launch: the person installs it (download, save of what the
 * editors hold, then the installer closes and restarts Humcase) or puts it off until the next
 * launch. A check that fails says nothing: it is not something the person can act on.
 */
export function useAppUpdate(): { update: AppUpdate; install: () => void; dismiss: () => void } {
  const [update, setUpdate] = useState<AppUpdate>({ phase: 'idle' });

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(() => {
      api.checkForUpdate()
        .then(found => { if (!cancelled && found) setUpdate({ phase: 'available', version: found.version }); })
        .catch(e => { console.warn('Update check failed', e); });
    }, UPDATE_CHECK_DELAY_MS);
    return () => { cancelled = true; clearTimeout(timer); };
  }, []);

  const install = useCallback(() => {
    if (update.phase !== 'available' && update.phase !== 'failed') return;
    const { version } = update;
    setUpdate({ phase: 'downloading', version, percent: null });
    void (async () => {
      try {
        await api.downloadUpdate(percent => setUpdate({ phase: 'downloading', version, percent }));
        setUpdate({ phase: 'installing', version });
        await saveBeforeExit();
        await api.installUpdate();
      } catch (e) {
        setUpdate({ phase: 'failed', version, error: String(e) });
      }
    })();
  }, [update]);

  const dismiss = useCallback(() => setUpdate({ phase: 'idle' }), []);
  return { update, install, dismiss };
}

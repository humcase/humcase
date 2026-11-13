import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../services/tauriApi';
import { reuseUnchanged } from '../utils/reuse';
import { NOTES_RETRY_LIMIT, NOTES_RETRY_MS } from './useNotesLoading';
import type { Activity } from '../types';
import type { ProjectSpace } from '../utils/spaces';

interface UseActivitiesOptions {
  reportAction: (message: string, tone?: 'danger' | 'success') => void;
  setGlobalError: Dispatch<SetStateAction<string | null>>;
}

/** Activities (read at startup and on a database change, creation, rename, deletion) and the active filtering activity. */
export function useActivities({ reportAction, setGlobalError }: UseActivitiesOptions) {
  const { t } = useTranslation();
  const [activities, setActivities] = useState<Activity[]>([]);
  const [activeActivityId, setActiveActivityId] = useState<string | null>(null);

  const loadActivities = useCallback(async () => {
    const next = await api.getActivities();
    setActivities(previous => reuseUnchanged(previous, next));
    setActiveActivityId(current => current && next.some(activity => activity.id === current) ? current : null);
  }, []);
  // Reread after an action on activities: the action succeeded and the display is already up to date
  // locally, but a reread failure is reported (the list may diverge from the database).
  const refreshActivities = useCallback(() => {
    void loadActivities().catch(cause => {
      console.error('Could not reread activities', cause);
      reportAction(t('workspace.activities.refreshError'));
    });
  }, [loadActivities, reportAction, t]);
  const createActivity = useCallback(async (name: string, space: ProjectSpace) => { const created = await api.createActivity(name, space); setActivities(current => [...current, created]); refreshActivities(); }, [refreshActivities]);
  const renameActivity = useCallback(async (id: string, name: string) => { const renamed = await api.renameActivity(id, name); setActivities(current => current.map(activity => activity.id === id ? renamed : activity)); refreshActivities(); }, [refreshActivities]);
  const deleteActivity = useCallback(async (id: string) => { await api.deleteActivity(id); setActivities(current => current.filter(activity => activity.id !== id)); if (activeActivityId === id) setActiveActivityId(null); refreshActivities(); }, [activeActivityId, refreshActivities]);

  // Read at startup, retried a bounded number of times; afterwards the database fingerprint watch
  // rereads them when an agent or the command line changes them. A failure is reported without
  // blocking the application.
  const retryRef = useRef<{ timer: ReturnType<typeof setTimeout> | null; count: number }>({ timer: null, count: 0 });
  useEffect(() => {
    let cancelled = false;
    const retry = retryRef.current;
    const read = () => {
      loadActivities().then(() => { retry.count = 0; }).catch(error => {
        if (cancelled) return;
        setActiveActivityId(null);
        setGlobalError(String(error));
        if (retry.count < NOTES_RETRY_LIMIT) { retry.count += 1; retry.timer = setTimeout(read, NOTES_RETRY_MS); }
      });
    };
    read();
    return () => { cancelled = true; if (retry.timer) clearTimeout(retry.timer); };
  }, [loadActivities, setGlobalError]);

  return { activities, setActivities, activeActivityId, setActiveActivityId, loadActivities, refreshActivities, createActivity, renameActivity, deleteActivity };
}

import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { api } from '../services/tauriApi';
import { reuseUnchanged } from '../utils/reuse';
import { NOTES_RETRY_LIMIT, NOTES_RETRY_MS } from './useNotesLoading';
import type { Project } from '../types';

interface UseProjectsLoadingOptions {
  setGlobalError: Dispatch<SetStateAction<string | null>>;
}

/**
 * Reading of projects: at startup and on demand (`loadProjects`). Changes made elsewhere (an agent,
 * the command line) arrive through the database fingerprint watch, which calls `loadProjects`: no
 * periodic read, which re-rendered the whole application every 5 s for nothing.
 */
export function useProjectsLoading({ setGlobalError }: UseProjectsLoadingOptions) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [initialProjectsSettled, setInitialProjectsSettled] = useState(false);
  const retryRef = useRef<{ timer: ReturnType<typeof setTimeout> | null; count: number }>({ timer: null, count: 0 });

  const fetchProjects = useCallback(() => api.getProjects(), []);
  // The retry calls the current `loadProjects` through this reference.
  const reloadRef = useRef<() => Promise<void>>(async () => {});

  const loadProjects = useCallback(async () => {
    try {
      const next = await fetchProjects();
      // A reread that changed nothing keeps the same list: nothing re-renders.
      setProjects(previous => reuseUnchanged(previous, next));
      setInitialProjectsSettled(true);
      retryRef.current.count = 0;
      // A successful read clears the warning: a transient outage must not stay displayed.
      setGlobalError(null);
    } catch (e) {
      // A failure is **transient**: a non-blocking warning, and a bounded retry (at startup the
      // Rust state may not be ready yet). It never replaces the application with a fatal error.
      console.error("Failed to load projects", e);
      setGlobalError(String(e));
      setInitialProjectsSettled(true);
      const retry = retryRef.current;
      if (retry.count < NOTES_RETRY_LIMIT && !retry.timer) {
        retry.count += 1;
        retry.timer = setTimeout(() => { retry.timer = null; void reloadRef.current(); }, NOTES_RETRY_MS);
      }
    }
  }, [fetchProjects, setGlobalError]);

  useEffect(() => { reloadRef.current = loadProjects; });

  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => { if (!cancelled) void loadProjects(); });
    const retry = retryRef.current;
    return () => { cancelled = true; if (retry.timer) clearTimeout(retry.timer); };
  }, [loadProjects]);

  return { projects, setProjects, initialProjectsSettled, fetchProjects, loadProjects };
}

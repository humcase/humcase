import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { api } from '../services/tauriApi';
import { createLoadToken } from '../utils/loadToken';
import { reuseUnchanged } from '../utils/reuse';
import type { SpacePreferences } from '../utils/spaces';
import type { Note } from '../types';

/** Pause in typing before the search is asked of the database. */
export const SEARCH_DELAY_MS = 180;
/** A failed read is tried again after this delay, a bounded number of times (startup). */
export const NOTES_RETRY_MS = 2000;
export const NOTES_RETRY_LIMIT = 15;

interface UseNotesLoadingOptions {
  searchQuery: string;
  /** Also rereads notes when the space preferences change. */
  spacePreferences: SpacePreferences;
  setGlobalError: Dispatch<SetStateAction<string | null>>;
}

interface SearchHits { query: string; ids: ReadonlySet<string> }

const sameIds = (a: ReadonlySet<string>, b: ReadonlySet<string>) => a.size === b.size && [...a].every(id => b.has(id));
const readSearch = (query: string) => api.getNotes(undefined, undefined, query).then(items => new Set(items.map(note => note.id)));

/**
 * Reading of the notes: every case of the database, read once and narrowed by the page (project,
 * status, nature, level) without asking the database again. Only the full-text search goes to the
 * database, which returns the matching identifiers. A reread keeps the objects of the cases that did
 * not change (see `reuseUnchanged`), so the views keep their caches. A token invalidates any stale
 * response.
 */
export function useNotesLoading({ searchQuery, spacePreferences, setGlobalError }: UseNotesLoadingOptions) {
  const [initialNotesSettled, setInitialNotesSettled] = useState(false);
  // Every note, unfiltered: the page, "Today", the palette, the shortcuts and the tag suggestions
  // narrow it themselves.
  const [projectNotes, setProjectNotes] = useState<Note[]>([]);
  const [searchHits, setSearchHits] = useState<SearchHits | null>(null);
  // Token of the last notes read: invalidates stale responses (see `loadNotes`).
  // Kept in a `ref` (not a value): the callbacks that use it do not have to
  // declare it as a dependency, and the identity stays stable from one render to the next.
  const loadTokenRef = useRef(createLoadToken());
  // True if the last notes read failed (at startup, the Rust state may not be ready yet).
  const failedNotesRef = useRef(false);
  const retryRef = useRef<{ timer: ReturnType<typeof setTimeout> | null; count: number }>({ timer: null, count: 0 });
  const searchRef = useRef(searchQuery.trim());
  useEffect(() => { searchRef.current = searchQuery.trim(); });
  // The retry calls the current `loadNotes` through this reference.
  const reloadRef = useRef<() => Promise<void>>(async () => {});

  const loadNotes = useCallback(async () => {
    // Request token: a read started before a newer one must not replace its result.
    const token = loadTokenRef.current.fresh();
    const query = searchRef.current;
    try {
      const [all, ids] = await Promise.all([api.getNotes(), query ? readSearch(query) : Promise.resolve(null)]);
      if (!loadTokenRef.current.isCurrent(token)) return; // a more recent read took over
      setProjectNotes(previous => reuseUnchanged(previous, all));
      if (ids && query === searchRef.current) setSearchHits(previous => previous && previous.query === query && sameIds(previous.ids, ids) ? previous : { query, ids });
      setInitialNotesSettled(true);
      failedNotesRef.current = false;
      retryRef.current.count = 0;
      setGlobalError(null);
    } catch (e) {
      // Even the error banner follows the token: a stale failure must not stay displayed.
      if (!loadTokenRef.current.isCurrent(token)) return;
      console.error("Failed to load notes", e);
      failedNotesRef.current = true;
      setGlobalError(String(e));
      setInitialNotesSettled(true);
      // Bounded retry: at startup the Rust state may not be ready yet.
      const retry = retryRef.current;
      if (retry.count < NOTES_RETRY_LIMIT && !retry.timer) {
        retry.count += 1;
        retry.timer = setTimeout(() => { retry.timer = null; void reloadRef.current(); }, NOTES_RETRY_MS);
      }
    }
  }, [setGlobalError]);

  useEffect(() => { reloadRef.current = loadNotes; });

  // First read, and again when the space preferences change.
  useEffect(() => {
    void loadNotes();
  }, [loadNotes, spacePreferences]);

  useEffect(() => () => {
    const retry = retryRef.current;
    if (retry.timer) clearTimeout(retry.timer);
  }, []);

  // The search waits for a pause in typing, then asks the database for the matching cases.
  useEffect(() => {
    const query = searchQuery.trim();
    if (!query) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      readSearch(query)
        .then(ids => { if (!cancelled && searchRef.current === query) setSearchHits({ query, ids }); })
        .catch(e => { if (!cancelled) { console.error('Search failed', e); setGlobalError(String(e)); } });
    }, SEARCH_DELAY_MS);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [searchQuery, setGlobalError]);

  // Without a search, no restriction; while a new search is on its way, the last answer stays.
  const query = searchQuery.trim();
  const searchIds = query ? searchHits?.ids ?? null : null;
  const searchPending = Boolean(query) && searchHits?.query !== query;

  return { projectNotes, setProjectNotes, searchIds, searchPending, initialNotesSettled, loadTokenRef, failedNotesRef, loadNotes };
}

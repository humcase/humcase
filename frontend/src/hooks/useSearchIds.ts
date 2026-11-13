import { useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import { SEARCH_DELAY_MS } from './useNotesLoading';

/**
 * The cases the database search returns for `query` (`db/queries/search.rs`), asked after a pause
 * in typing. `null` without a search, until the first answer, or when the database could not
 * answer: the view then narrows the cases itself with `caseMatches`.
 */
export function useSearchIds(query: string): ReadonlySet<string> | null {
  const [answer, setAnswer] = useState<{ query: string; ids: ReadonlySet<string> } | null>(null);
  const trimmed = query.trim();
  useEffect(() => {
    if (!trimmed) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      api.getNotes(undefined, undefined, trimmed)
        .then(items => { if (!cancelled) setAnswer({ query: trimmed, ids: new Set(items.map(note => note.id)) }); })
        .catch(e => { if (!cancelled) console.warn('Search failed, narrowing locally', e); });
    }, SEARCH_DELAY_MS);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [trimmed]);
  return trimmed && answer?.query === trimmed ? answer.ids : null;
}

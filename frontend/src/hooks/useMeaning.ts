import { useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import type { MeaningHit, MeaningStatus } from '../types';
import { SEARCH_DELAY_MS } from './useNotesLoading';

/** How often the reading state is asked: often while cases are read, rarely otherwise. */
const READING_POLL_MS = 1500;
const IDLE_POLL_MS = 15000;

/**
 * Where the reading of the cases for search by meaning stands, asked again while it changes.
 * `null` until the first answer (or when the engine cannot answer).
 */
export function useMeaningStatus(active = true): [MeaningStatus | null, () => void] {
  const [status, setStatus] = useState<MeaningStatus | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ask = () => {
      api.meaningStatus()
        .then(next => {
          if (cancelled) return;
          setStatus(next);
          timer = setTimeout(ask, next.state === 'reading' ? READING_POLL_MS : IDLE_POLL_MS);
        })
        .catch(() => { if (!cancelled) timer = setTimeout(ask, IDLE_POLL_MS); });
    };
    ask();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [active, tick]);
  return [status, () => setTick(value => value + 1)];
}

/**
 * Cases closest in meaning to `query`, asked after a pause in typing. Empty without a search,
 * when search by meaning is off, or when it cannot answer: the word search stays.
 */
export function useMeaningHits(query: string): MeaningHit[] {
  const [answer, setAnswer] = useState<{ query: string; hits: MeaningHit[] } | null>(null);
  const trimmed = query.trim();
  useEffect(() => {
    if (!trimmed) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      api.searchByMeaning(trimmed, 30)
        .then(hits => { if (!cancelled) setAnswer({ query: trimmed, hits }); })
        .catch(e => { if (!cancelled) { console.warn('Search by meaning failed', e); setAnswer({ query: trimmed, hits: [] }); } });
    }, SEARCH_DELAY_MS);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [trimmed]);
  return trimmed && answer?.query === trimmed ? answer.hits : [];
}

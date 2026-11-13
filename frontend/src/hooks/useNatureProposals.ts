import { useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import type { NatureProposal, ProjectSpace } from '../types';

const REFRESH_MS = 60_000;

/**
 * Natures Jev read as another one, among the cases shown (`visibleIds`), asked again when the
 * cases change, when asked (after a review) and every minute (Jev reads in the background).
 * Nothing is sent from here: the review lists what was already read. Asked for `space` (every
 * space when null): another space's proposals never fill the list.
 */
export function useNatureProposals(visibleIds: ReadonlySet<string>, notes: unknown, space: ProjectSpace | null = null): [NatureProposal[], () => void] {
  const [proposals, setProposals] = useState<NatureProposal[]>([]);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let active = true;
    const ask = () => Promise.resolve(api.natureReview?.(200, space))
      .then(review => { if (active) setProposals(review?.proposals ?? []); })
      .catch(() => { if (active) setProposals([]); });
    void ask();
    const timer = setInterval(() => { void ask(); }, REFRESH_MS);
    return () => { active = false; clearInterval(timer); };
  }, [notes, tick, space]);
  return [proposals.filter(proposal => visibleIds.has(proposal.note_id)), () => setTick(value => value + 1)];
}

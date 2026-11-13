import { useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import type { Nudge, ProfileSettings, ProfileView, ProjectSpace } from '../types';
import { onHomeSettingsChanged } from '../utils/homeSettings';

/**
 * The cases waiting for a next step among the cases shown (`visibleIds`), read again when the
 * cases change. Humcase chooses them once a day, without AI.
 */
export function useNudges(visibleIds: ReadonlySet<string>, notes: unknown): { nudges: Nudge[]; reload: () => void } {
  const [nudges, setNudges] = useState<Nudge[]>([]);
  const [tick, setTick] = useState(0);
  useEffect(() => onHomeSettingsChanged(() => setTick(value => value + 1)), []);
  useEffect(() => {
    let active = true;
    Promise.resolve(api.nudges?.())
      .then(next => { if (active) setNudges(next ?? []); })
      .catch(() => { if (active) setNudges([]); });
    return () => { active = false; };
  }, [notes, tick]);
  return { nudges: nudges.filter(nudge => visibleIds.has(nudge.case.id)), reload: () => setTick(value => value + 1) };
}

/**
 * The profile of `space` and the lines proposed for it: looked for again on mount (counted lines,
 * and the decisions Jev reads when the person lets it), read again when the cases change or when
 * asked. Nothing is added from here.
 */
export function useWorkProfile(space: ProjectSpace, notes: unknown): { view: ProfileView | null; settings: ProfileSettings | null; failed: boolean; reload: () => void } {
  // Kept with its space: a switch never shows the other space's profile meanwhile.
  const [read, setRead] = useState<{ space: ProjectSpace; view: ProfileView } | null>(null);
  const setView = (next: ProfileView) => setRead({ space: next.profile.space, view: next });
  const [settings, setSettings] = useState<ProfileSettings | null>(null);
  const [failed, setFailed] = useState(false);
  const [tick, setTick] = useState(0);
  useEffect(() => onHomeSettingsChanged(() => setTick(value => value + 1)), []);
  useEffect(() => {
    let active = true;
    Promise.resolve(api.workProfile?.(space, true))
      .then(next => { if (active && next) { setView(next); setFailed(false); } })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [space]);
  useEffect(() => {
    let active = true;
    Promise.resolve(api.workProfile?.(space, false))
      .then(next => { if (active && next) { setView(next); setFailed(false); } })
      .catch(() => undefined);
    Promise.resolve(api.profileSettings?.())
      .then(next => { if (active && next) setSettings(next); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [space, notes, tick]);
  return { view: read?.space === space ? read.view : null, settings, failed, reload: () => setTick(value => value + 1) };
}

/**
 * How many lines wait for the profiles of `spaces` (both in the combined view), read again
 * after `after` changes (the profile read, which follows a look for new lines).
 */
export function useProfileProposalCount(spaces: readonly ProjectSpace[], after: unknown): number {
  const [count, setCount] = useState(0);
  const key = spaces.join(',');
  useEffect(() => {
    let active = true;
    const list = key ? key.split(',') as ProjectSpace[] : [];
    Promise.all(list.map(space => Promise.resolve(api.workProfile?.(space, false))
      .then(view => view?.pending.length ?? 0)
      .catch(() => 0)))
      .then(counts => { if (active) setCount(counts.reduce((sum, value) => sum + value, 0)); });
    return () => { active = false; };
  }, [key, after]);
  return count;
}

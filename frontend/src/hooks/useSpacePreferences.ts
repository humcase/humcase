import { useCallback, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from 'react';
import { loadSpacePreferences, normalizeSpacePreferences, saveSpacePreferences, type SpacePreferences, type SpaceScope, type SpaceUsage } from '../utils/spaces';

/**
 * Space preferences (Work/Private usage, combined view, active space) and epoch counter.
 * The epoch advances on every space change: an asynchronous action started before the
 * change (note creation, brief) must not act in the new space.
 */
export function useSpacePreferences() {
  const [spacePreferences, setSpacePreferences] = useState(loadSpacePreferences);
  const spaceEpochRef = useRef(0);
  return { spacePreferences, setSpacePreferences, spaceEpochRef };
}

interface UseSpaceActionsOptions {
  spacePreferences: SpacePreferences;
  setSpacePreferences: Dispatch<SetStateAction<SpacePreferences>>;
  spaceEpochRef: MutableRefObject<number>;
  /** Resets navigation and lists to zero (stable identity, provided by the caller). */
  resetWorkspace: () => void;
}

/** Space changes: normalization, persistence, then reset of the workspace. */
export function useSpaceActions({ spacePreferences, setSpacePreferences, spaceEpochRef, resetWorkspace }: UseSpaceActionsOptions) {
  const applySpacePreferences = useCallback((next: SpacePreferences) => {
    spaceEpochRef.current += 1;
    setSpacePreferences(next);
    saveSpacePreferences(next);
    resetWorkspace();
  }, [spaceEpochRef, setSpacePreferences, resetWorkspace]);
  const changeScope = useCallback((scope: SpaceScope) => {
    if (scope === spacePreferences.scope) return;
    applySpacePreferences(normalizeSpacePreferences({ ...spacePreferences, scope }));
  }, [spacePreferences, applySpacePreferences]);
  const changeSpaceUsage = useCallback((usage: SpaceUsage) => {
    applySpacePreferences(normalizeSpacePreferences({ ...spacePreferences, usage, scope: usage === 'both' ? 'work' : usage }));
  }, [spacePreferences, applySpacePreferences]);
  const changeCombinedView = useCallback((combined: boolean) => {
    applySpacePreferences(normalizeSpacePreferences({ ...spacePreferences, combined, scope: spacePreferences.scope === 'all' && !combined ? 'work' : spacePreferences.scope }));
  }, [spacePreferences, applySpacePreferences]);
  return { applySpacePreferences, changeScope, changeSpaceUsage, changeCombinedView };
}

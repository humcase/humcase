import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useSpaceActions, useSpacePreferences } from './useSpacePreferences';
import { loadSpacePreferences, normalizeSpacePreferences } from '../utils/spaces';

function mountHook(initial?: Parameters<typeof normalizeSpacePreferences>[0]) {
  if (initial) localStorage.setItem('humcase-spaces-v1', JSON.stringify(normalizeSpacePreferences(initial)));
  const resetWorkspace = vi.fn();
  const rendered = renderHook(() => {
    const state = useSpacePreferences();
    const actions = useSpaceActions({ ...state, resetWorkspace });
    return { ...state, ...actions };
  });
  return { ...rendered, resetWorkspace };
}

describe('space preferences', () => {
  beforeEach(() => localStorage.clear());

  it('changes usage: normalizes the active space, persists, advances the epoch and resets the workspace', () => {
    const { result, resetWorkspace } = mountHook();
    expect(result.current.spacePreferences).toEqual({ usage: 'both', combined: false, scope: 'work' });
    act(() => result.current.changeSpaceUsage('private'));
    expect(result.current.spacePreferences).toEqual({ usage: 'private', combined: false, scope: 'private' });
    expect(loadSpacePreferences().scope).toBe('private');
    expect(result.current.spaceEpochRef.current).toBe(1);
    expect(resetWorkspace).toHaveBeenCalledTimes(1);
    act(() => result.current.changeSpaceUsage('both'));
    expect(result.current.spacePreferences.scope).toBe('work');
  });

  it('ignores a change to the already active space', () => {
    const { result, resetWorkspace } = mountHook();
    act(() => result.current.changeScope('work'));
    expect(resetWorkspace).not.toHaveBeenCalled();
    expect(result.current.spaceEpochRef.current).toBe(0);
    act(() => result.current.changeScope('private'));
    expect(result.current.spacePreferences.scope).toBe('private');
    expect(resetWorkspace).toHaveBeenCalledTimes(1);
  });

  it('refuses an unavailable space: falls back to the usage default', () => {
    const { result } = mountHook({ usage: 'private' });
    act(() => result.current.changeScope('work'));
    expect(result.current.spacePreferences.scope).toBe('private');
  });

  it('disabling the combined view leaves "My spaces" for Work, otherwise keeps the space', () => {
    const { result } = mountHook({ usage: 'both', combined: true, scope: 'all' });
    expect(result.current.spacePreferences.scope).toBe('all');
    act(() => result.current.changeCombinedView(false));
    expect(result.current.spacePreferences).toEqual({ usage: 'both', combined: false, scope: 'work' });
    act(() => result.current.changeScope('private'));
    act(() => result.current.changeCombinedView(true));
    expect(result.current.spacePreferences).toEqual({ usage: 'both', combined: true, scope: 'private' });
  });
});

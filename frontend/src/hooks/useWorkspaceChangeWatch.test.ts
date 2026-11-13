import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useWorkspaceChangeWatch } from './useWorkspaceChangeWatch';
import { NOTES_WATCH_INTERVAL_MS } from '../utils/useNotesChangeWatch';

describe('useWorkspaceChangeWatch — projects and activities written by an agent', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  const mount = (fingerprints: { value: string }, activities?: () => Promise<void>) => {
    const loadNotes = vi.fn(async () => {});
    const loadProjects = vi.fn(async () => {});
    const loadActivities = vi.fn(activities ?? (async () => {}));
    renderHook(() => useWorkspaceChangeWatch({
      readFingerprint: async () => fingerprints.value, loadNotes, loadProjects, loadActivities,
    }));
    return { loadNotes, loadProjects, loadActivities };
  };

  it('rereads projects, activities and notes when the fingerprint changes, and only then', async () => {
    const fingerprint = { value: 'v1' };
    const { loadNotes, loadProjects, loadActivities } = mount(fingerprint);
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(NOTES_WATCH_INTERVAL_MS);
    expect(loadProjects).not.toHaveBeenCalled();
    expect(loadActivities).not.toHaveBeenCalled();
    expect(loadNotes).not.toHaveBeenCalled();

    fingerprint.value = 'v2'; // an agent created a project
    await vi.advanceTimersByTimeAsync(NOTES_WATCH_INTERVAL_MS);
    expect(loadProjects).toHaveBeenCalledTimes(1);
    expect(loadActivities).toHaveBeenCalledTimes(1);
    expect(loadNotes).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(NOTES_WATCH_INTERVAL_MS);
    expect(loadProjects).toHaveBeenCalledTimes(1);
  });

  it('a failure to reread activities blocks neither notes nor projects and is not an unhandled error', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      const fingerprint = { value: 'v1' };
      const { loadNotes, loadProjects, loadActivities } = mount(fingerprint, async () => { throw new Error('database busy'); });
      await vi.advanceTimersByTimeAsync(0);
      fingerprint.value = 'v2';
      await vi.advanceTimersByTimeAsync(NOTES_WATCH_INTERVAL_MS);
      expect(loadActivities).toHaveBeenCalledTimes(1);
      expect(loadProjects).toHaveBeenCalledTimes(1);
      expect(loadNotes).toHaveBeenCalledTimes(1);
      expect(error).toHaveBeenCalled();
    } finally {
      error.mockRestore();
    }
  });
});

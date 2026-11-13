import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useActionToast } from './useActionToast';
import { useBriefs } from './useBriefs';
import { api } from '../services/tauriApi';
import type { Note } from '../types';

vi.mock('../services/tauriApi', () => ({
  api: { getHandoff: vi.fn(), getDailyStandup: vi.fn() },
}));

const mockApi = vi.mocked(api);
const note = (title: string, status: Note['status']) => ({ id: title, title, status }) as Note;

describe('one-off action message', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('clears after 6 seconds and the last message restarts from a new delay', () => {
    const { result } = renderHook(() => useActionToast());
    act(() => result.current.reportAction('Premier'));
    expect(result.current.toneAction).toBe('danger');
    act(() => { vi.advanceTimersByTime(4000); });
    act(() => result.current.reportAction('Second', 'success'));
    act(() => { vi.advanceTimersByTime(4000); });
    expect(result.current.actionError).toBe('Second');
    expect(result.current.toneAction).toBe('success');
    act(() => { vi.advanceTimersByTime(2000); });
    expect(result.current.actionError).toBeNull();
  });
});

describe('briefs Handoff et Standup', () => {
  beforeEach(() => vi.clearAllMocks());

  function mount(epoch = { current: 0 }) {
    const reportAction = vi.fn();
    const rendered = renderHook(() => useBriefs({ activeProjectId: 'p1', visibleProjectIds: new Set(['p1']), spaceEpochRef: epoch, reportAction }));
    return { ...rendered, reportAction };
  }

  it('ignores a brief that came back after a space change', async () => {
    const epoch = { current: 0 };
    mockApi.getHandoff.mockImplementation(async () => { epoch.current += 1; return 'brief'; });
    const { result } = mount(epoch);
    await act(async () => { await result.current.openHandoff(); });
    expect(result.current.showHandoff).toBe(false);
    expect(result.current.briefPending).toBeNull();
  });

  it('assembles the daily update by status', async () => {
    mockApi.getDailyStandup.mockResolvedValue([note('A', 'resolved'), note('B', 'in_progress'), note('C', 'open')]);
    const { result } = mount();
    await act(async () => { await result.current.openStandup(); });
    expect(result.current.showStandup).toBe(true);
    expect(result.current.standupText).toContain('✅ **Fait (Hier) :**\n- A');
    expect(result.current.standupText).toContain('🔄 **En cours :**\n- B');
    expect(result.current.standupText).toContain('🚧 **À faire (Aujourd’hui) :**\n- C');
  });

  it('does not close the modal when the copy is refused', async () => {
    mockApi.getHandoff.mockResolvedValue('brief');
    const write = vi.fn().mockRejectedValue(new Error('refused'));
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: write }, configurable: true });
    const { result } = mount();
    await act(async () => { await result.current.openHandoff(); });
    expect(result.current.showHandoff).toBe(true);
    await act(async () => { await result.current.copyAndClose(); });
    expect(write).toHaveBeenCalledWith('brief');
    expect(result.current.showHandoff).toBe(true);
    expect(result.current.copyError).toMatch(/^Copie impossible/);
    write.mockResolvedValue(undefined);
    await act(async () => { await result.current.copyAndClose(); });
    expect(result.current.showHandoff).toBe(false);
    expect(result.current.copyError).toBeNull();
  });
});

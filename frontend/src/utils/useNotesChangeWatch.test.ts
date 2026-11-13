import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useNotesChangeWatch } from './useNotesChangeWatch';

describe('useNotesChangeWatch — notes written by an agent while the window is open', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => {
    vi.useRealTimers();
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => false });
  });

  it('rereads the notes when the fingerprint changes, without waiting for focus, and only then', async () => {
    const fingerprints = ['a', 'a', 'b', 'b'];
    const read = vi.fn(async () => fingerprints.shift() ?? 'b');
    const reread = vi.fn();
    renderHook(() => useNotesChangeWatch(read, reread, 1000));

    await vi.advanceTimersByTimeAsync(0);      // reference read
    await vi.advanceTimersByTimeAsync(1000);   // unchanged
    expect(reread).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);   // an agent wrote
    expect(reread).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1000);   // plus rien
    expect(reread).toHaveBeenCalledTimes(1);
  });

  it('reads nothing when the window is hidden and ignores a transient failure', async () => {
    let hidden = true;
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden });
    const read = vi.fn(async () => 'a');
    const reread = vi.fn();
    renderHook(() => useNotesChangeWatch(read, reread, 1000));
    await vi.advanceTimersByTimeAsync(3000);
    expect(read).not.toHaveBeenCalled();

    hidden = false;
    read.mockRejectedValueOnce(new Error('database busy'));
    await vi.advanceTimersByTimeAsync(1000);
    await vi.advanceTimersByTimeAsync(1000);
    expect(reread).not.toHaveBeenCalled();
  });

  it('stops on unmount', async () => {
    const read = vi.fn(async () => String(Math.random()));
    const reread = vi.fn();
    const { unmount } = renderHook(() => useNotesChangeWatch(read, reread, 1000));
    await vi.advanceTimersByTimeAsync(0);
    unmount();
    const calls = read.mock.calls.length;
    await vi.advanceTimersByTimeAsync(5000);
    expect(read.mock.calls.length).toBe(calls);
    expect(reread).not.toHaveBeenCalled();
  });
});

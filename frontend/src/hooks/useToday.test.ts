import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useToday } from './useToday';

describe('useToday', () => {
  afterEach(() => vi.useRealTimers());

  it('keeps the same day between renders and moves on at midnight', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 9, 2, 23, 59, 0));
    const { result, rerender } = renderHook(() => useToday());
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);

    act(() => { vi.advanceTimersByTime(61_000); });
    expect(result.current.getDate()).toBe(3);
  });
});

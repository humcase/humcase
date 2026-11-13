import { afterEach, describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useSubCasesChoice } from './useSubCasesChoice';

describe('useSubCasesChoice', () => {
  afterEach(() => localStorage.clear());

  it('starts unticked and keeps the choice across launches', () => {
    const first = renderHook(() => useSubCasesChoice());
    expect(first.result.current.shown).toBe(false);
    act(() => first.result.current.choose(true));
    expect(first.result.current.shown).toBe(true);
    first.unmount();
    expect(renderHook(() => useSubCasesChoice()).result.current.shown).toBe(true);
  });
});

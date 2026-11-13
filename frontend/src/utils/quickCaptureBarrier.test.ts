import { describe, it, expect, beforeEach } from 'vitest';
import {
  QUICK_CAPTURE_STATE_KEY,
  setQuickCapturePersistenceRefused,
  quickCapturePersistenceUnavailable,
  prepareQuickCaptureForRestore,
  writeQuickCaptureState,
} from './quickCaptureBarrier';

const HEALTHY_STATE = { title: '', noteType: 'note' as const, projectId: 'p1', submitting: false };

beforeEach(() => {
  localStorage.clear();
  setQuickCapturePersistenceRefused(false);
});

describe('Quick Capture barrier — cross-window fail-closed (M2)', () => {
  it('refuses the restore when persistence is reported refused, even if localStorage is readable', () => {
    // An otherwise healthy and readable state would normally be enough...
    localStorage.setItem(QUICK_CAPTURE_STATE_KEY, JSON.stringify(HEALTHY_STATE));
    expect(prepareQuickCaptureForRestore()).toEqual({ ready: true });

    // ... but the Quick Capture window reported that its state is not persisted: we refuse.
    setQuickCapturePersistenceRefused(true);
    const result = prepareQuickCaptureForRestore();
    expect(result.ready).toBe(false);
    if (!result.ready) {
      expect(result.reason).toMatch(/n’est pas persisté/);
    }
  });

  it("returns to normal behavior when persistence is ensured again", () => {
    setQuickCapturePersistenceRefused(true);
    expect(prepareQuickCaptureForRestore().ready).toBe(false);

    setQuickCapturePersistenceRefused(false);
    expect(prepareQuickCaptureForRestore().ready).toBe(true);
  });

  it('a successful write clears the refusal flag', () => {
    setQuickCapturePersistenceRefused(true);
    expect(quickCapturePersistenceUnavailable()).toBe(true);

    writeQuickCaptureState(HEALTHY_STATE);
    expect(quickCapturePersistenceUnavailable()).toBe(false);
    expect(prepareQuickCaptureForRestore().ready).toBe(true);
  });
});

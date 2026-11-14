import { describe, expect, it, vi } from 'vitest';
import { onBeforeExit, saveBeforeExit } from './beforeExit';

describe('save before Humcase quits for an update', () => {
  it('waits for every registered write, even when one fails', async () => {
    let written = false;
    const stopSlow = onBeforeExit(() => new Promise<void>(resolve => setTimeout(() => { written = true; resolve(); }, 20)));
    const stopFailing = onBeforeExit(() => { throw new Error('disk full'); });
    await saveBeforeExit();
    expect(written).toBe(true);
    stopSlow();
    stopFailing();
  });

  it('stops waiting after the delay instead of blocking the update', async () => {
    vi.useFakeTimers();
    const stop = onBeforeExit(() => new Promise(() => { /* never settles */ }));
    const done = saveBeforeExit(1000);
    await vi.advanceTimersByTimeAsync(1000);
    await expect(done).resolves.toBeUndefined();
    stop();
    vi.useRealTimers();
  });

  it('forgets a saver once its editor is gone', async () => {
    const saver = vi.fn();
    const stop = onBeforeExit(saver);
    stop();
    await saveBeforeExit();
    expect(saver).not.toHaveBeenCalled();
  });
});

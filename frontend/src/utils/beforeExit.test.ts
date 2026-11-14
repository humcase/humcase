import { describe, expect, it, vi } from 'vitest';
import { onBeforeExit, saveBeforeExit } from './beforeExit';

describe('save before Humcase quits for an update', () => {
  it('waits for every registered write and says they are all done', async () => {
    let written = false;
    const stop = onBeforeExit(() => new Promise<void>(resolve => setTimeout(() => { written = true; resolve(); }, 20)));
    expect(await saveBeforeExit()).toBe(true);
    expect(written).toBe(true);
    stop();
  });

  it('says not saved when a write fails, after waiting for the others', async () => {
    let written = false;
    const stopSlow = onBeforeExit(() => new Promise<void>(resolve => setTimeout(() => { written = true; resolve(); }, 20)));
    const stopFailing = onBeforeExit(() => { throw new Error('disk full'); });
    expect(await saveBeforeExit()).toBe(false);
    expect(written).toBe(true);
    stopSlow();
    stopFailing();
  });

  it('stops waiting after the delay and says not saved', async () => {
    vi.useFakeTimers();
    const stop = onBeforeExit(() => new Promise(() => { /* never settles */ }));
    const done = saveBeforeExit(1000);
    await vi.advanceTimersByTimeAsync(1000);
    await expect(done).resolves.toBe(false);
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

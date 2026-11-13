/**
 * Anti-race token for list reads.
 *
 * Each new read increments the token; a response is only applied if its token is
 * **still the current one**. A stale read (older project or filter, finished after a
 * more recent one) is therefore ignored instead of replacing the displayed list.
 *
 * Extracted from `App.tsx` to be testable in isolation (no React, no IPC).
 */
export interface LoadToken {
  /** Current value of the token (for comparison after an `await`). */
  current(): number;
  /** Starts a new read and returns its token. */
  fresh(): number;
  /** True if the token still matches the most recent read. */
  isCurrent(token: number): boolean;
}

export function createLoadToken(): LoadToken {
  let current = 0;
  return {
    current: () => current,
    fresh: () => ++current,
    isCurrent: (token: number) => token === current,
  };
}

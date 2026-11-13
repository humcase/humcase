import { describe, it, expect } from 'vitest';
import { createLoadToken } from './loadToken';

describe('Jeton de chargement anti-course', () => {
  it('applies the most recent response and ignores the stale response', () => {
    const token = createLoadToken();
    let displayedList = 'initial';

    // Read 1 (project A) starts.
    const j1 = token.fresh();
    // Read 2 (project B) starts before read 1 responds.
    const j2 = token.fresh();

    // The response of read 2 (current) is applied.
    if (token.isCurrent(j2)) displayedList = 'B';
    // The late response of read 1 (stale) is ignored.
    if (token.isCurrent(j1)) displayedList = 'A';

    expect(displayedList).toBe('B');
  });

  it('a single read stays current', () => {
    const token = createLoadToken();
    const j = token.fresh();
    expect(token.isCurrent(j)).toBe(true);
    expect(token.current()).toBe(j);
  });

  it("a read finished before another one starts stays current", () => {
    const token = createLoadToken();
    const j1 = token.fresh();
    expect(token.isCurrent(j1)).toBe(true);
    // A later read invalidates the previous one.
    const j2 = token.fresh();
    expect(token.isCurrent(j1)).toBe(false);
    expect(token.isCurrent(j2)).toBe(true);
  });
});

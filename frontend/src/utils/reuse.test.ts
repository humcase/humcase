import { describe, expect, it } from 'vitest';
import { reuseUnchanged, sameRecord } from './reuse';

const a = { id: 'a', title: 'A', revision: 1 };
const b = { id: 'b', title: 'B', revision: 1 };

describe('reuseUnchanged', () => {
  it('keeps the previous array when a reread changed nothing', () => {
    const previous = [a, b];
    expect(reuseUnchanged(previous, [{ ...a }, { ...b }])).toBe(previous);
  });

  it('keeps unchanged objects and takes the changed one', () => {
    const changed = { ...b, title: 'B2', revision: 2 };
    const result = reuseUnchanged([a, b], [{ ...a }, changed]);
    expect(result[0]).toBe(a);
    expect(result[1]).toBe(changed);
  });

  it('sees an added, a removed or a reordered case', () => {
    const previous = [a, b];
    expect(reuseUnchanged(previous, [{ ...a }])).toEqual([a]);
    expect(reuseUnchanged(previous, [{ ...b }, { ...a }])).not.toBe(previous);
    expect(reuseUnchanged(previous, [{ ...b }, { ...a }])[0]).toBe(b);
  });

  it('compares every field, the long ones included', () => {
    expect(sameRecord({ id: 'a', content: 'x'.repeat(10) }, { id: 'a', content: 'x'.repeat(10) })).toBe(true);
    expect(sameRecord({ id: 'a', content: 'x' }, { id: 'a', content: 'y' })).toBe(false);
    expect(sameRecord({ id: 'a' }, { id: 'a', extra: 1 } as { id: string })).toBe(false);
  });
});

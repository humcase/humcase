import { describe, expect, it } from 'vitest';
import { searchRank } from './searchRank';
import type { MeaningHit } from '../types';

const hit = (note_id: string, lead = 3): MeaningHit => ({ note_id, score: 0.8, lead, strong: lead >= 3, very_close: false });
const order = (rank: Map<string, number>) => [...rank.entries()].sort((a, b) => a[1] - b[1]).map(([id]) => id);

describe('search rank', () => {
  it('keeps the word order without meaning results', () => {
    expect(order(searchRank(new Set(['a', 'b', 'c']), []))).toEqual(['a', 'b', 'c']);
  });

  it('brings up a case the words found far down when meaning finds it first', () => {
    const words = new Set(['a', 'b', 'c', 'd', 'e', 'f', 'g', 'target']);
    expect(order(searchRank(words, [hit('target'), hit('zz')]))[0]).toBe('target');
  });

  it('keeps the first word result first when meaning agrees on it', () => {
    expect(order(searchRank(new Set(['a', 'b', 'c']), [hit('a'), hit('c')]))).toEqual(['a', 'c', 'b']);
  });

  it('never ranks a case the words did not find', () => {
    expect([...searchRank(new Set(['a']), [hit('elsewhere')]).keys()]).toEqual(['a']);
  });

  it('puts the cases kept as history after the others, in their order', () => {
    const words = new Set(['old', 'a', 'archived', 'b']);
    expect(order(searchRank(words, [hit('old')], new Set(['old', 'archived'])))).toEqual(['a', 'b', 'old', 'archived']);
  });
});

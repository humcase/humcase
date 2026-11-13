import type { MeaningHit } from '../types';

/** Weight of a rank: 1 / (RANK_OFFSET + position). Small enough that a top place counts. */
const RANK_OFFSET = 10;

/**
 * Order of the cases found by the words, "best match first", with the search by meaning as a
 * second opinion: each case scores 1 / (10 + its place among the word results), plus
 * 1 / (10 + its place among the meaning results) when meaning returned it (reciprocal rank
 * fusion). Only cases the words found are ranked; meaning alone never adds one here ("Close in
 * meaning" shows those). Without meaning results, the word order is kept as it is.
 *
 * Measured on the six-language bench (150 cases per language, 144 searches worded without the
 * case's words): the case sought is in the first five rows 94 times instead of 83 with the word
 * order and the "Close in meaning" section, and first 58 times instead of 36; searches typed with
 * the case's words keep it first (144 of 144).
 *
 * Cases kept as history (`history`: a decision marked as replaced, an archived case) come after
 * every other one, in the same order among themselves: what still holds is read first.
 */
export function searchRank(wordIds: ReadonlySet<string>, meaningHits: readonly MeaningHit[], history?: ReadonlySet<string>): Map<string, number> {
  const meaningPlace = new Map(meaningHits.map((hit, index) => [hit.note_id, index]));
  const scored = [...wordIds].map((id, index) => {
    const place = meaningPlace.get(id);
    return { id, index, score: 1 / (RANK_OFFSET + index + 1) + (place === undefined ? 0 : 1 / (RANK_OFFSET + place + 1)) };
  });
  const past = (id: string) => (history?.has(id) ? 1 : 0);
  scored.sort((a, b) => past(a.id) - past(b.id) || b.score - a.score || a.index - b.index);
  return new Map(scored.map((entry, rank) => [entry.id, rank]));
}

import type { MeaningHit, Note } from '../types';

/** Most cases shown under "Close in meaning". */
const SHOWN = 5;

/**
 * The rows "Close in meaning" shows: hits the words did not find, among the cases of the page;
 * when the words found cases, only the hits that stand out clearly.
 */
export function meaningRows(hits: MeaningHit[], cases: ReadonlyMap<string, Note>, wordIds: ReadonlySet<string> | null) {
  const wordsFound = wordIds !== null && wordIds.size > 0;
  return hits
    .filter(hit => !wordIds?.has(hit.note_id) && (hit.strong || !wordsFound))
    .map(hit => ({ hit, note: cases.get(hit.note_id) }))
    .filter((row): row is { hit: MeaningHit; note: Note } => row.note !== undefined)
    .slice(0, SHOWN);
}

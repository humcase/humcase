import { describe, expect, it } from 'vitest';
import type { Note } from '../types';
import { caseMatches } from './caseBoard';

const now = '2026-10-03T10:00:00.000Z';
const note = (over: Partial<Note>): Note => ({
  id: 'n', project_id: 'p', title: '', content: '', note_type: 'note', status: 'open', created_at: now, updated_at: now, revision: 1, ...over,
});

// Before the database answers, a view shows the cases the database search always returns: those
// holding every word, near forms included (`db/queries/search.rs`).
describe('caseMatches — the cases the database search always returns', () => {
  const trip = note({ title: 'Voyage à Séville', content: 'Trois hôtels retenus, 140 €.', raw_content: 'réserver le train', resolution_comment: 'Billets pris', tags: '["été"]' });

  it('needs every word, in any case and without accents', () => {
    expect(caseMatches(trip, 'seville VOYAGE', null)).toBe(true);
    expect(caseMatches(trip, 'hotels 140', null)).toBe(true);
    expect(caseMatches(trip, 'seville avion', null)).toBe(false);
  });

  it('finds the near forms of a word of five letters or more, not of a shorter one', () => {
    expect(caseMatches(trip, 'hôtel', null)).toBe(true);
    expect(caseMatches(trip, 'voyages', null)).toBe(true);
    expect(caseMatches(trip, 'retenue', null)).toBe(true);
    expect(caseMatches(trip, 'trois', null)).toBe(true);
    expect(caseMatches(trip, 'troi', null)).toBe(false);
    expect(caseMatches(trip, '14', null)).toBe(false);
  });

  it('reads the dictation and the report too, like the index', () => {
    expect(caseMatches(trip, 'train', null)).toBe(true);
    expect(caseMatches(trip, 'billets', null)).toBe(true);
  });

  it('keeps the tag, nature and status filters', () => {
    expect(caseMatches(trip, '#été seville', null)).toBe(true);
    expect(caseMatches(trip, '#hiver', null)).toBe(false);
    expect(caseMatches(trip, '', 'idea')).toBe(false);
    expect(caseMatches(trip, '', null, 'resolved')).toBe(false);
  });
});

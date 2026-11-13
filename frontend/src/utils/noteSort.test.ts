import { describe, expect, it } from 'vitest';
import type { Note } from '../types';
import { sortNotes } from './noteSort';

function note(id: string, status: Note['status'], priority: string | null, updated_at: string): Note {
  return { id, project_id: 'project', title: id, content: '', note_type: 'note', status, priority, created_at: updated_at, updated_at, revision: 0 };
}

describe('note sorting', () => {
  it('places To do before In progress and Resolved, even against a higher priority', () => {
    const input = [
      note('résolu', 'resolved', 'P0', '2026-09-25T12:00:00Z'),
      note('en-cours', 'in_progress', 'P0', '2026-09-25T11:00:00Z'),
      note('à-faire-P3', 'open', 'P3', '2026-09-25T10:00:00Z'),
      note('à-faire-P1', 'open', 'P1', '2026-09-25T09:00:00Z'),
    ];
    expect(sortNotes(input, 'attention').map(n => n.id)).toEqual(['à-faire-P1', 'à-faire-P3', 'en-cours', 'résolu']);
    expect(input[0].id).toBe('résolu');
  });

  it('also offers recency, priority and title sorts without modifying the notes', () => {
    const input = [
      note('Bêta', 'open', null, '2026-09-25T11:00:00Z'),
      note('Alpha', 'resolved', 'P2', '2026-09-25T10:00:00Z'),
      note('Zèbre', 'open', 'P0', '2026-09-25T12:00:00Z'),
    ];
    expect(sortNotes(input, 'recent').map(n => n.id)).toEqual(['Zèbre', 'Bêta', 'Alpha']);
    expect(sortNotes(input, 'priority').map(n => n.id)).toEqual(['Zèbre', 'Alpha', 'Bêta']);
    expect(sortNotes(input, 'title').map(n => n.id)).toEqual(['Alpha', 'Bêta', 'Zèbre']);
    expect(input.map(n => n.id)).toEqual(['Bêta', 'Alpha', 'Zèbre']);
  });

  it('orders a search best match first, and falls back to To do first without an answer', () => {
    const input = [
      note('a', 'open', 'P0', '2026-09-25T12:00:00Z'),
      note('b', 'resolved', null, '2026-09-25T11:00:00Z'),
      note('c', 'open', null, '2026-09-25T10:00:00Z'),
    ];
    const rank = new Map([['b', 0], ['c', 1], ['a', 2]]);
    expect(sortNotes(input, 'relevance', rank).map(n => n.id)).toEqual(['b', 'c', 'a']);
    expect(sortNotes(input, 'relevance', null).map(n => n.id)).toEqual(['a', 'c', 'b']);
  });
});

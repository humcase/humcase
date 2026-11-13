import { describe, expect, it } from 'vitest';
import type { Note } from '../types';
import { deriveCaseView, type CaseViewInput } from './caseView';
import { untilMidnight } from '../hooks/useCurrentDay';

const note = (id: string, extra: Partial<Note> = {}): Note => ({
  id, project_id: 'p1', title: id, content: '', note_type: 'note', status: 'open',
  created_at: '2026-10-01T10:00:00.000Z', updated_at: '2026-10-01T10:00:00.000Z', revision: 0, parent_id: null, content_kind: 'text', ...extra,
});

const notes = [
  note('a', { note_type: 'todo', status: 'in_progress' }),
  note('b', { status: 'resolved' }),
  note('a1', { parent_id: 'a', note_type: 'todo' }),
  note('c', { project_id: 'p2', title: 'Séville semaine' }),
  note('old', { archived_at: '2026-10-02T10:00:00.000Z' }),
];
const input = (extra: Partial<CaseViewInput> = {}): CaseViewInput => ({
  notes, visibleProjectIds: new Set(['p1', 'p2']), archivedProjectIds: new Set(), activityProjectIds: null,
  activeProjectId: null, activeNoteId: null, query: '', searchIds: null, status: '', nature: '',
  includeSubCases: false, today: false, showArchived: false, now: new Date('2026-10-03T12:00:00'), ...extra,
});
const ids = (list: Note[]) => list.map(item => item.id);

describe('deriveCaseView', () => {
  it('narrows the page by project and status from the whole database', () => {
    expect(ids(deriveCaseView(input({ activeProjectId: 'p1' })).displayedNotes)).toEqual(['a', 'b']);
    expect(ids(deriveCaseView(input({ status: 'resolved' })).displayedNotes)).toEqual(['b']);
  });

  it('lists sub-cases on demand, and counts them for the box', () => {
    const view = deriveCaseView(input({ includeSubCases: true }));
    expect(ids(view.displayedNotes)).toContain('a1');
    expect(deriveCaseView(input()).subCaseTotal).toBe(1);
    expect(deriveCaseView(input()).subCaseCounts.get('a')).toBe(1);
  });

  it('takes the database answer to a search, and matches the words itself while it answers', () => {
    expect(ids(deriveCaseView(input({ query: 'semaine Séville', searchIds: new Set(['c']) })).displayedNotes)).toEqual(['c']);
    // No answer yet: a quick match of the words, every one required.
    expect(ids(deriveCaseView(input({ query: 'séville', searchIds: null })).displayedNotes)).toEqual(['c']);
    expect(ids(deriveCaseView(input({ query: 'semaine séville', searchIds: null })).displayedNotes)).toEqual(['c']);
  });

  it('keeps archived cases out, unless they are asked for or searched', () => {
    expect(ids(deriveCaseView(input()).allScoped)).not.toContain('old');
    expect(ids(deriveCaseView(input({ showArchived: true })).scopedNotes)).toEqual(['old']);
    expect(deriveCaseView(input()).archivedNotesCount).toBe(1);
  });

  it('counts statuses and natures of the view', () => {
    const view = deriveCaseView(input({ activeProjectId: 'p1' }));
    expect(view.statusCounts.all).toBe(2);
    expect(view.statusCounts.resolved).toBe(1);
    // Like the statuses, natures count the level shown: the sub-case a1 only with the box ticked.
    expect(view.natureCounts.get('todo')).toBe(1);
    expect(deriveCaseView(input({ activeProjectId: 'p1', includeSubCases: true })).natureCounts.get('todo')).toBe(2);
  });

  it('counts only what the search finds, as a click on the filter would show', () => {
    const view = deriveCaseView(input({ query: 'semaine Séville', searchIds: new Set(['c']) }));
    expect(ids(view.displayedNotes)).toEqual(['c']);
    expect(view.statusCounts.all).toBe(1);
    expect(view.statusCounts.open).toBe(1);
    expect(view.statusCounts.resolved).toBe(0);
    expect([...view.natureCounts]).toEqual([['note', 1]]);
    expect(view.subCaseTotal).toBe(0);
  });
});

describe('untilMidnight', () => {
  it('counts the time left in the day', () => {
    expect(untilMidnight(new Date('2026-10-03T23:00:00'))).toBe(3600_000);
  });
});

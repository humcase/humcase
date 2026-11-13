import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { Brief, CaseEvent, Note } from '../../types';
import { WorkspaceHome } from './WorkspaceHome';

vi.mock('../../services/tauriApi', () => ({
  api: {
    getBrief: vi.fn(),
    listAgentRevisionsSince: vi.fn().mockResolvedValue([]),
    listAgentEventsSince: vi.fn().mockResolvedValue([]),
  },
}));

import { api } from '../../services/tauriApi';

const now = new Date().toISOString();
const minutesAgo = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();
const note = (id: string, title: string, over: Partial<Note> = {}): Note => ({
  id, project_id: 'p1', title, content: '', note_type: 'todo', status: 'resolved', resolution_comment: `Report of ${title}`,
  created_at: now, updated_at: now, revision: 3, ...over,
});
const event = (noteId: string, at: string): CaseEvent => ({
  id: `e-${noteId}`, note_id: noteId, kind: 'edited', actor_kind: 'agent', actor_name: 'Codex', detail: {}, fields: ['content'], created_at: at, last_at: at,
});
const project = { id: 'p1', name: 'Project', created_at: now } as never;

describe('WorkspaceHome and the brief', () => {
  it('never offers to pick up a resolved or archived case, and leads from "To follow up" to every case', async () => {
    vi.mocked(api.getBrief).mockResolvedValue(null as never);
    const onSeeAll = vi.fn();
    const notes = [
      note('done', 'Finished yesterday', { updated_at: minutesAgo(1) }),
      note('shelved', 'Shelved', { status: 'open', archived_at: minutesAgo(2), updated_at: minutesAgo(2) }),
      note('next', 'Still to do', { status: 'open', updated_at: minutesAgo(10) }),
      ...[1, 2, 3, 4, 5].map(n => note(`p${n}`, `Urgent ${n}`, { status: 'open', priority: 'P0', updated_at: minutesAgo(20 + n) })),
    ];
    render(<WorkspaceHome projects={[project]} notes={notes} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} onSeeAllFollowup={onSeeAll} />);
    expect(await screen.findByRole('article', { name: 'Still to do' })).toBeTruthy();
    expect(screen.queryByRole('article', { name: 'Finished yesterday' })).toBeNull();
    expect(screen.queryByRole('article', { name: 'Shelved' })).toBeNull();
    screen.getByRole('button', { name: 'Tout voir (5)' }).click();
    expect(onSeeAll).toHaveBeenCalled();
  });

  it('shows the day first, then the brief, and never repeats below a case the brief shows', async () => {
    const brief: Brief = {
      since: now, since_is_fallback: false, changes: [event('changed', minutesAgo(3))],
      to_review: [{ note_id: 'review', rework_count: 0 }],
      reworks_pending: [{ note_id: 'rework', rework_count: 1, last_review: { id: 'r', note_id: 'rework', decision: 'rework_requested', revision: 2, comment: 'Too generic', rework_number: 1, created_at: now } }],
    };
    vi.mocked(api.getBrief).mockResolvedValue(brief);
    // The brief's cases are the most recent: without the rule, one of them would be the case to pick up.
    const notes = [
      note('review', 'Declared result', { updated_at: minutesAgo(1) }),
      note('rework', 'Sent back', { status: 'open', updated_at: minutesAgo(2) }),
      note('changed', 'Written by an agent', { status: 'in_progress', updated_at: minutesAgo(3) }),
      note('plain', 'Plain case', { status: 'open', updated_at: minutesAgo(30) }),
      note('older', 'Older case', { updated_at: minutesAgo(60) }),
    ];
    render(<WorkspaceHome projects={[project]} notes={notes} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);

    const day = screen.getByRole('heading', { name: 'Votre journée' });
    const since = await screen.findByRole('heading', { name: 'Depuis votre dernière visite' });
    const pickUp = screen.getByRole('article', { name: 'Plain case' });
    expect(day.compareDocumentPosition(since) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(since.compareDocumentPosition(pickUp) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    // Each case of the brief is opened from the brief only, not again from the resume, the
    // cases to follow or the recent cases.
    for (const title of ['Declared result', 'Sent back', 'Written by an agent']) {
      expect(screen.getAllByRole('button', { name: `Ouvrir « ${title} »` })).toHaveLength(1);
      expect(screen.queryByRole('button', { name: `Ouvrir ${title}` })).toBeNull();
      expect(screen.queryByRole('button', { name: `Poursuivre ${title}` })).toBeNull();
    }
    expect(screen.getByRole('button', { name: 'Ouvrir Older case' })).toBeTruthy();
    expect(screen.queryByText(/Comptes rendus/)).toBeNull();
  });

  it('counts in the day the agent writes on a sub-case', async () => {
    vi.mocked(api.getBrief).mockResolvedValue({ since: now, since_is_fallback: false, changes: [], to_review: [], reworks_pending: [] });
    vi.mocked(api.listAgentEventsSince).mockResolvedValue([event('sub', minutesAgo(5))]);
    const top = note('top', 'Top case', { updated_at: minutesAgo(600), created_at: minutesAgo(700) });
    const sub = note('sub', 'Sub-case', { parent_id: 'top', updated_at: minutesAgo(5), created_at: minutesAgo(700) });
    render(<WorkspaceHome projects={[project]} notes={[top]} allNotes={[top, sub]} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    expect(await screen.findByText(/1 écriture d’agent/)).toBeTruthy();
  });
});

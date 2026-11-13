import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { NatureReview, Note } from '../../types';
import { WorkspaceHome } from './WorkspaceHome';

vi.mock('../../services/tauriApi', () => ({
  api: {
    getBrief: vi.fn().mockResolvedValue(null),
    listAgentRevisionsSince: vi.fn().mockResolvedValue([]),
    listAgentEventsSince: vi.fn().mockResolvedValue([]),
    natureReview: vi.fn(),
    answerNature: vi.fn().mockResolvedValue(true),
  },
}));

import { api } from '../../services/tauriApi';

const now = new Date().toISOString();
const note = (id: string, title: string): Note => ({
  id, project_id: 'p1', title, content: '', note_type: 'note', status: 'open', created_at: now, updated_at: now, revision: 1,
} as Note);
const project = { id: 'p1', name: 'Project', created_at: now } as never;
const review = (ids: string[]): NatureReview => ({
  proposals: ids.map(id => ({ note_id: id, title: `Case ${id}`, project_id: 'p1', revision: 1, nature: 'note', proposed: 'decision', reasons: [{ kind: 'reading', confidence: 0.95 }], confidence: 0.95 })),
  total: ids.length, enabled: true, readable: 3, read: 3,
});

describe('WorkspaceHome and the natures to check', () => {
  it('counts the proposals of the cases shown, and asks again once the review closes', async () => {
    // 'elsewhere' belongs to another space: not counted here.
    vi.mocked(api.natureReview).mockResolvedValueOnce(review(['a', 'elsewhere'])).mockResolvedValue(review([]));
    const notes = [note('a', 'Case a'), note('b', 'Case b')];
    render(<WorkspaceHome projects={[project]} notes={notes} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    fireEvent.click(await screen.findByRole('button', { name: /Vérifier/ }));
    expect(screen.getByText('01 / 01')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Garder Pensée' }));
    await waitFor(() => expect(api.answerNature).toHaveBeenCalledWith('a', 'note', 'decision', false));
    fireEvent.click((await screen.findAllByRole('button', { name: 'Fermer' })).at(-1)!);
    await waitFor(() => expect(screen.queryByRole('button', { name: /Vérifier/ })).toBeNull());
  });
});

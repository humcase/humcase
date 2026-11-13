import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import type { Note, NoteRevision } from '../types';
import NoteHistory from './NoteHistory';

vi.mock('../services/tauriApi', () => ({
  api: {
    getNoteHistory: vi.fn(),
  },
}));

import { api } from '../services/tauriApi';
const mockApi = vi.mocked(api);

const note: Note = {
  id: 'n1',
  project_id: 'p1',
  title: 'Titre',
  content: 'Contenu',
  note_type: 'note',
  status: 'open',
  created_at: '2026-09-24T00:00:00Z',
  updated_at: '2026-09-24T00:00:00Z',
  revision: 2,
};

/** Revision with a JSON snapshot of previous values. */
const revision = (over: Partial<NoteRevision> = {}): NoteRevision => ({
  id: 'rev-1',
  note_id: 'n1',
  revision: 1,
  source: 'agent',
  fields: '{}',
  created_at: '2026-09-25T08:00:00Z',
  ...over,
});

beforeEach(() => {
  vi.clearAllMocks();
  mockApi.getNoteHistory.mockResolvedValue([]);
});

describe('NoteHistory — agent history', () => {
  it('shows revisions from most recent to oldest with derived labels', async () => {
    // Deliberately provided in reverse order: the display must stay "most recent first".
    mockApi.getNoteHistory.mockResolvedValue([
      revision({ id: 'rev-1', revision: 1, fields: JSON.stringify({ title: 'Titre précédent', status: 'open' }), created_at: '2026-09-25T08:00:00Z' }),
      revision({ id: 'rev-2', revision: 2, fields: JSON.stringify({ priority: 'P2' }), created_at: '2026-09-26T09:00:00Z' }),
    ]);

    render(<NoteHistory note={note} />);

    const lines = await screen.findAllByRole('listitem');
    expect(lines).toHaveLength(2);
    // Most recent first: revision 2 (priority) comes before revision 1 (title, status).
    expect(lines[0].textContent).toContain('priorité');
    expect(lines[1].textContent).toContain('titre');
    expect(lines[1].textContent).toContain('statut');
    // Each line attributes the modification to the agent.
    expect(lines[0].textContent).toContain('modifié par l’agent');
    expect(lines[1].textContent).toContain('modifié par l’agent');
  });

  it('attributes by source: a non-agent source is not presented as an agent edit', async () => {
    // A snapshot whose source is not 'agent' (future source type) must not be
    // falsely attributed to the agent: the raw source is displayed as is.
    mockApi.getNoteHistory.mockResolvedValue([
      revision({ id: 'rev-agent', revision: 1, source: 'agent', fields: JSON.stringify({ title: 'x' }) }),
      revision({ id: 'rev-import', revision: 2, source: 'restauration', fields: JSON.stringify({ title: 'y' }) }),
    ]);

    render(<NoteHistory note={note} />);

    const lines = await screen.findAllByRole('listitem');
    expect(lines[0].textContent).toContain('restauration');
    expect(lines[0].textContent).not.toContain('modifié par l’agent');
    expect(lines[1].textContent).toContain('modifié par l’agent');
  });

  it('reloads the history when the note is modified by an agent while it is open', async () => {
    // First read: no revision (the note was just opened).
    mockApi.getNoteHistory.mockResolvedValueOnce([]);
    const view = render(<NoteHistory note={note} />);
    await waitFor(() => expect(mockApi.getNoteHistory).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole('listitem')).toBeNull();

    // An agent modifies the note while it is open: `revision` increases, the same note
    // (same id) is re-rendered by the editor: the history must be reloaded and displayed.
    mockApi.getNoteHistory.mockResolvedValueOnce([
      revision({ id: 'rev-1', revision: 1, fields: JSON.stringify({ title: 'Ancien titre' }) }),
    ]);
    view.rerender(<NoteHistory note={{ ...note, revision: note.revision + 1 }} />);

    await waitFor(() => expect(mockApi.getNoteHistory).toHaveBeenCalledTimes(2));
    const lines = await screen.findAllByRole('listitem');
    expect(lines).toHaveLength(1);
    expect(lines[0].textContent).toContain('titre');
    expect(lines[0].textContent).toContain('modifié par l’agent');
  });

  it('shows an unknown field key raw without crashing', async () => {
    mockApi.getNoteHistory.mockResolvedValue([
      revision({ fields: JSON.stringify({ priority: 'P2', champ_futur: 'valeur' }) }),
    ]);

    render(<NoteHistory note={note} />);

    expect(await screen.findByText(/Champs :/)).toBeTruthy();
    expect(screen.getByText(/priorité/)).toBeTruthy();
    expect(screen.getByText(/champ_futur/)).toBeTruthy();
  });

  it('shows no section when the history is empty', async () => {
    mockApi.getNoteHistory.mockResolvedValue([]);
    render(<NoteHistory note={note} />);

    await waitFor(() => expect(mockApi.getNoteHistory).toHaveBeenCalledWith('n1'));
    expect(screen.queryByRole('region', { name: /Historique de l’agent/ })).toBeNull();
  });

  it('stays non-blocking and announces the failure instead of passing for an empty history (review 0.9.69)', async () => {
    mockApi.getNoteHistory.mockRejectedValue(new Error('database locked'));
    render(<NoteHistory note={note} />);

    await waitFor(() => expect(mockApi.getNoteHistory).toHaveBeenCalledWith('n1'));
    expect((await screen.findByRole('alert')).textContent).toContain('n’a pas pu être lu');
    expect(screen.queryAllByRole('listitem')).toHaveLength(0);
  });

  it('still shows nothing for a note never modified by an agent', async () => {
    render(<NoteHistory note={note} />);
    await waitFor(() => expect(mockApi.getNoteHistory).toHaveBeenCalledWith('n1'));
    expect(screen.queryByRole('region', { name: /Historique de l’agent/ })).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

describe('NoteHistory — versions de structuration (0.9.78)', () => {
  it('shows "version replaced" and restores the previous content', async () => {
    mockApi.getNoteHistory.mockResolvedValue([
      revision({ id: 'rev-s', revision: 3, source: 'structure', fields: JSON.stringify({ content: 'Version brute' }), created_at: '2026-09-28T08:00:00Z' }),
    ]);
    const onRestore = vi.fn();
    render(<NoteHistory note={note} onRestore={onRestore} />);
    const line = await screen.findByRole('listitem');
    expect(line.textContent).toContain('version remplacée');
    screen.getByRole('button', { name: 'Restaurer cette version' }).click();
    expect(onRestore).toHaveBeenCalledWith('Version brute');
  });
});

describe('NoteHistory — inside the case history', () => {
  it('does not repeat an agent edit the journal lists, and keeps every version that can be restored', async () => {
    mockApi.getNoteHistory.mockResolvedValue([
      revision({ id: 'in-journal', revision: 4, fields: JSON.stringify({ title: 'Avant' }), created_at: '2026-10-02T10:00:01Z' }),
      revision({ id: 'older', revision: 2, fields: JSON.stringify({ priority: 'P2' }), created_at: '2026-09-20T10:00:00Z' }),
      revision({ id: 'version', revision: 3, source: 'structure', fields: JSON.stringify({ content: 'Brut' }), created_at: '2026-10-02T10:00:00Z' }),
    ]);
    const journalEvents = [{ id: 'e1', note_id: 'n1', kind: 'edited', actor_kind: 'agent', actor_name: 'Codex', detail: {}, fields: ['title', 'content'], created_at: '2026-10-02T10:00:00Z', last_at: '2026-10-02T10:00:00Z' }] as never;
    render(<NoteHistory note={note} embedded journalEvents={journalEvents} onRestore={vi.fn()} />);
    const lines = await screen.findAllByRole('listitem');
    expect(lines.map(line => line.textContent)).toEqual([
      expect.stringContaining('version remplacée'),
      expect.stringContaining('priorité'),
    ]);
    expect(screen.queryByRole('heading')).toBeNull();
  });
});

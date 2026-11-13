import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { Note, Project } from '../../types';

vi.mock('../../services/tauriApi', () => ({
  api: {
    getTrashedNotes: vi.fn(),
    restoreFromTrash: vi.fn(),
    purgeNote: vi.fn(),
    emptyTrash: vi.fn(),
  },
}));

import { api } from '../../services/tauriApi';
import { TrashView } from './TrashView';
import { trashRows } from '../../utils/trash';
import { daysBeforeErasure } from '../../utils/trash';
import i18n from '../../i18n';

const mockApi = vi.mocked(api);
const project: Project = { id: 'p', name: 'Campagne', color: '#6353a5', space: 'work', created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 0 };
const note = (id: string, title: string): Note => ({
  id, project_id: 'p', title, content: '', note_type: 'note', status: 'open', resolution_comment: null,
  created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 3,
  deleted_at: '2026-09-28T10:00:00.000Z',
});

describe('Corbeille (0.9.82)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockApi.restoreFromTrash.mockResolvedValue(undefined);
    mockApi.purgeNote.mockResolvedValue(undefined);
    mockApi.emptyTrash.mockResolvedValue(2);
  });

  it('counts the remaining days without ever going below zero', () => {
    const deleted = '2026-09-28T10:00:00.000Z';
    expect(daysBeforeErasure(deleted, Date.parse('2026-09-28T10:00:00.000Z'))).toBe(30);
    expect(daysBeforeErasure(deleted, Date.parse('2026-10-27T11:00:00.000Z'))).toBe(1);
    expect(daysBeforeErasure(deleted, Date.parse('2026-11-30T00:00:00.000Z'))).toBe(0);
  });

  it('counts the rows, not the sub-cases trashed with their case', async () => {
    mockApi.getTrashedNotes.mockResolvedValue([note('a', 'Rénovation'), { ...note('a1', 'Devis'), parent_id: 'a' }]);
    const { container } = render(<TrashView projects={[project]} onChanged={vi.fn()} onMessage={vi.fn()} />);
    expect(await screen.findByText('Rénovation')).toBeTruthy();
    expect(container.querySelector('.workspace-page__count')?.textContent).toBe('1');
  });

  it('restores a note and rereads the application notes', async () => {
    mockApi.getTrashedNotes.mockResolvedValueOnce([note('a', 'Idée jetée')]).mockResolvedValueOnce([]);
    const onChanged = vi.fn();
    const onMessage = vi.fn();
    render(<TrashView projects={[project]} onChanged={onChanged} onMessage={onMessage} />);
    expect(await screen.findByText('Idée jetée')).toBeTruthy();
    expect(screen.getByText(/Campagne · Supprimée le/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Restaurer/ }));
    await waitFor(() => expect(mockApi.restoreFromTrash).toHaveBeenCalledWith('a'));
    expect(onChanged).toHaveBeenCalled();
    expect(onMessage).toHaveBeenCalledWith(expect.stringContaining('de retour dans Campagne'), 'success');
    expect(await screen.findByText('La corbeille est vide')).toBeTruthy();
  });

  it('only erases a note after confirmation, and only the targeted one', async () => {
    mockApi.getTrashedNotes.mockResolvedValue([note('a', 'Première'), note('b', 'Seconde')]);
    render(<TrashView projects={[project]} onChanged={vi.fn()} onMessage={vi.fn()} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Effacer définitivement « Seconde »' }));
    expect(mockApi.purgeNote).not.toHaveBeenCalled();
    const dialogue = screen.getByRole('dialog');
    expect(within(dialogue).getByText('Effacer définitivement « Seconde » ?')).toBeTruthy();
    fireEvent.click(within(dialogue).getByRole('button', { name: 'Annuler' }));
    expect(mockApi.purgeNote).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Effacer définitivement « Seconde »' }));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Effacer définitivement' }));
    await waitFor(() => expect(mockApi.purgeNote).toHaveBeenCalledWith('b'));
    expect(mockApi.purgeNote).toHaveBeenCalledTimes(1);
  });

  it('empties the trash after confirmation', async () => {
    const shown = [note('a', 'Première'), note('b', 'Seconde')];
    mockApi.getTrashedNotes.mockResolvedValueOnce(shown).mockResolvedValueOnce(shown).mockResolvedValueOnce([]);
    const onMessage = vi.fn();
    render(<TrashView projects={[project]} onChanged={vi.fn()} onMessage={onMessage} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Vider la corbeille' }));
    expect(mockApi.emptyTrash).not.toHaveBeenCalled();
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Vider la corbeille' }));
    await waitFor(() => expect(mockApi.emptyTrash).toHaveBeenCalledTimes(1));
    // Only the cases shown are erased, whatever reaches the trash in between.
    expect(mockApi.emptyTrash).toHaveBeenCalledWith(['a', 'b']);
    expect(onMessage).toHaveBeenCalledWith('2 cases effacées définitivement.', 'success');
    expect(await screen.findByText('La corbeille est vide')).toBeTruthy();
  });

  it('never empties a case trashed after the confirmation was shown', async () => {
    mockApi.getTrashedNotes
      .mockResolvedValueOnce([note('a', 'Première')])
      .mockResolvedValueOnce([note('a', 'Première'), note('late', 'Arrivée entre-temps')]);
    render(<TrashView projects={[project]} onChanged={vi.fn()} onMessage={vi.fn()} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Vider la corbeille' }));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Vider la corbeille' }));
    expect(await within(screen.getByRole('dialog')).findByRole('alert')).toBeTruthy();
    expect(mockApi.emptyTrash).not.toHaveBeenCalled();
    expect(await screen.findByText('Arrivée entre-temps')).toBeTruthy();
  });

  it('displays in English when the interface language is English (i18n)', async () => {
    mockApi.getTrashedNotes.mockResolvedValue([note('a', 'Plan'), note('b', 'Budget')]);
    await i18n.changeLanguage('en');
    try {
      const onMessage = vi.fn();
      render(<TrashView projects={[project]} onChanged={vi.fn()} onMessage={onMessage} />);
      expect(screen.getByRole('heading', { level: 1, name: 'Trash' })).toBeTruthy();
      fireEvent.click(await screen.findByRole('button', { name: 'Empty trash' }));
      const dialogue = screen.getByRole('dialog');
      expect(within(dialogue).getByText(/The 2 cases in the trash/)).toBeTruthy();
      fireEvent.click(within(dialogue).getByRole('button', { name: 'Empty trash' }));
      await waitFor(() => expect(onMessage).toHaveBeenCalledWith('2 cases permanently erased.', 'success'));
    } finally {
      await i18n.changeLanguage('fr');
    }
  });
});

describe('trashRows', () => {
  const at = (id: string, parent_id: string | null, deleted_at: string) => ({ id, parent_id, deleted_at }) as Note;
  it('keeps the sub-cases trashed with their case in its row, and the others apart', () => {
    const rows = trashRows([
      at('root', null, '2026-10-01T10:00:00.000Z'),
      at('child', 'root', '2026-10-01T10:00:00.000Z'),
      at('grandchild', 'child', '2026-10-01T10:00:00.000Z'),
      at('earlier', 'root', '2026-10-01T09:00:00.000Z'),
      at('orphan', 'live-parent', '2026-10-01T10:00:00.000Z'),
    ]);
    expect(rows.map(row => [row.note.id, row.subCases])).toEqual([['root', 2], ['earlier', 0], ['orphan', 0]]);
  });
});

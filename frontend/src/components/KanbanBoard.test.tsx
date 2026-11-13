import { describe, it, expect, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import i18n from '../i18n';
import KanbanBoard from './KanbanBoard';
import { api } from '../services/tauriApi';
import { trackEditorWrite } from '../utils/backupBarrier';
import type { Note } from '../types';

vi.mock('../services/tauriApi', () => ({ api: { updateNote: vi.fn(), retryNoteUpdate: vi.fn() } }));
vi.mock('../utils/backupBarrier', () => ({ trackEditorWrite: vi.fn((promise: Promise<unknown>) => promise) }));

const makeNote = (over: Partial<Note> = {}): Note => ({
  id: 'n1',
  project_id: 'p1',
  title: 'Note',
  content: '',
  note_type: 'note',
  status: 'open',
  resolution_comment: null,
  raw_content: null,
  priority: null,
  git_branch: null,
  tags: null,
  created_at: '2026-09-22T10:00:00.000Z',
  updated_at: '2026-09-22T10:00:00.000Z',
  due_at: null,
  revision: 0,
  ...over,
});

function renderHelper(notes: Note[], options: { activeNoteId?: string | null; onSelectNote?: (id: string) => void; onNotesUpdate?: () => void } = {}) {
  return render(
    <KanbanBoard
      notes={notes}
      activeNoteId={options.activeNoteId ?? null}
      onSelectNote={options.onSelectNote ?? (() => {})}
      onNotesUpdate={options.onNotesUpdate ?? (() => {})}
    />
  );
}

describe('KanbanBoard — column titles', () => {
  it('switches column titles to English when the language changes', async () => {
    try {
      await act(() => i18n.changeLanguage('fr'));
      renderHelper([]);
      // The title is followed by the card counter (0 here) in the same header.
      const titles = () => screen.getAllByRole('heading', { level: 3 }).map(h => h.textContent);
      expect(titles()).toEqual(['À faire0', 'En cours0', 'Résolu0']);
      await act(() => i18n.changeLanguage('en'));
      expect(titles()).toEqual(['To do0', 'In progress0', 'Done0']);
    } finally {
      await act(() => i18n.changeLanguage('fr'));
    }
  });
});

describe('KanbanBoard — statut visible et ouverture clavier', () => {
  it('shows the status label on each card (independently of the column)', () => {
    const { container } = renderHelper([
      makeNote({ id: 'a', title: 'Alpha', status: 'open' }),
      makeNote({ id: 'b', title: 'Beta', status: 'in_progress' }),
      makeNote({ id: 'c', title: 'Gamma', status: 'resolved' }),
    ]);
    const badgeList = [...container.querySelectorAll('.ui-status')].map(b => b.textContent);
    expect(badgeList).toHaveLength(3);
    expect(badgeList).toContain('À faire');
    expect(badgeList).toContain('En cours');
    expect(badgeList).toContain('Résolu');
  });

  it("does not use another word for a resolved status", () => {
    renderHelper([makeNote({ id: 'c', title: 'Gamma', status: 'resolved' })]);
    expect(screen.queryByText('Terminé')).toBeNull();
    expect(screen.queryByText('Terminée')).toBeNull();
  });

  it('opens the note through a button distinct from the drag, reachable by keyboard', () => {
    const onSelectNote = vi.fn();
    renderHelper([makeNote({ id: 'a', title: 'Alpha' })], { onSelectNote });
    const button = screen.getByRole('button', { name: 'Ouvrir la case Alpha' });
    expect(button.tagName).toBe('BUTTON');
    button.focus();
    expect(document.activeElement).toBe(button);
    fireEvent.keyDown(button, { key: 'Enter' });
    expect(onSelectNote).toHaveBeenCalledExactlyOnceWith('a');
  });

  it('keeps the selection marker of the active card', () => {
    const { container } = renderHelper([makeNote({ id: 'a', title: 'Alpha' })], { activeNoteId: 'a' });
    expect(container.querySelector('.workspace-kanban__card[data-active]')).toBeTruthy();
  });

  it('shows the card in the target column during the write and wraps the call in the editor chain', async () => {
    let release!: () => void;
    vi.mocked(api.updateNote).mockReturnValue(new Promise<void>(resolve => { release = resolve; }));
    const onNotesUpdate = vi.fn();
    const { container } = renderHelper([makeNote({ id: 'a', title: 'Alpha', status: 'resolved' })], { onNotesUpdate });
    const card = container.querySelector('.workspace-kanban__card') as HTMLElement;

    // Mouse drag to the first column ("To do"): all columns have a zero
    // rectangle in jsdom, so the drop at (0,0) targets the first one.
    fireEvent.pointerDown(card, { clientX: 10, clientY: 10, pointerId: 1, button: 0, isPrimary: true });
    fireEvent.pointerMove(document, { clientX: 0, clientY: 0, pointerId: 1, isPrimary: true });
    fireEvent.pointerUp(document, { clientX: 0, clientY: 0, pointerId: 1, isPrimary: true });

    await waitFor(() => expect(api.updateNote).toHaveBeenCalledWith('a', { status: 'open' }));
    expect(trackEditorWrite).toHaveBeenCalled();

    // Optimistic: the card is already rendered in the target column until the write is confirmed.
    expect(container.querySelector('[data-column="open"]')?.textContent).toContain('Alpha');
    expect(container.querySelector('[data-column="resolved"]')?.textContent).not.toContain('Alpha');

    release();
    await waitFor(() => expect(onNotesUpdate).toHaveBeenCalled());
  });
});

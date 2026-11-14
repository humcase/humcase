import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { Note } from '../types';
import NoteRelations from './NoteRelations';
import { readPendingWrites, clearPendingWrites } from '../utils/pendingWrites';
import { saveBeforeExit } from '../utils/beforeExit';

vi.mock('../services/tauriApi', () => ({ api: {
  getNoteRelations: vi.fn(),
  createLinkedTask: vi.fn(),
  linkNoteTask: vi.fn(),
  unlinkNoteTask: vi.fn(),
} }));

// Simulated Tauri window: closing does not unmount React, it must be possible to trigger it.
const winBus = vi.hoisted(() => {
  const handlers: Array<() => void> = [];
  return {
    handlers,
    onCloseRequested: async (cb: () => void) => {
      handlers.push(cb);
      return () => {
        const i = handlers.indexOf(cb);
        if (i >= 0) handlers.splice(i, 1);
      };
    },
    reset: () => { handlers.length = 0; },
  };
});

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ onCloseRequested: winBus.onCloseRequested }),
}));

import { api } from '../services/tauriApi';
const mockApi = vi.mocked(api);

const note = (id: string, over: Partial<Note> = {}): Note => ({
  id,
  project_id: 'p1',
  title: id,
  content: '',
  note_type: 'note',
  status: 'open',
  created_at: '2026-09-24T00:00:00Z',
  updated_at: '2026-09-24T00:00:00Z',
  revision: 0,
  ...over,
});

const source = note('source', { title: 'Idée initiale' });
const task = note('task', { title: 'Préparer une version', note_type: 'todo', status: 'resolved' });

beforeEach(() => {
  vi.clearAllMocks();
  clearPendingWrites();
  winBus.reset();
  mockApi.getNoteRelations.mockResolvedValue({ sources: [], tasks: [] });
  mockApi.createLinkedTask.mockResolvedValue('new-task');
  mockApi.linkNoteTask.mockResolvedValue(undefined);
  mockApi.unlinkNoteTask.mockResolvedValue(undefined);
});

describe('NoteRelations', () => {
  it('shows the declared counter and allows going back to the source note', async () => {
    mockApi.getNoteRelations.mockResolvedValue({ sources: [source], tasks: [task] });
    const open = vi.fn();
    render(<NoteRelations note={task} allNotes={[source, task]} onOpenNote={open} onChanged={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /Tâches liées/ }).textContent).toContain('1 source'));
    expect(screen.getByRole('button', { name: /Tâches liées/ }).textContent).toContain('1 résolue');
    expect(screen.getByText('Résolu')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Idée initiale' }));
    await waitFor(() => expect(open).toHaveBeenCalledWith(source));
  });

  it('creates a linked task only once, with title and content, then can cancel another draft', async () => {
    let finish: (id: string) => void = () => {};
    mockApi.createLinkedTask.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    const changed = vi.fn();
    render(<NoteRelations note={source} allNotes={[source]} onOpenNote={() => {}} onChanged={changed} />);
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByText('Aucune tâche liée.')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Créer une tâche' }));
    fireEvent.change(screen.getByLabelText('Titre de la tâche'), { target: { value: '  Tester le parcours  ' } });
    fireEvent.change(screen.getByLabelText('Contenu facultatif'), { target: { value: '  Critères  ' } });
    const submit = screen.getByRole('button', { name: 'Créer et lier' });
    fireEvent.click(submit);
    fireEvent.click(submit);
    expect(mockApi.createLinkedTask).toHaveBeenCalledTimes(1);
    expect(mockApi.createLinkedTask).toHaveBeenCalledWith('source', 'Tester le parcours', 'Critères');
    await act(async () => { finish('new-task'); });
    await waitFor(() => expect(changed).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('button', { name: 'Créer une tâche' }));
    fireEvent.change(screen.getByLabelText('Titre de la tâche'), { target: { value: 'Annulée' } });
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    expect(screen.queryByDisplayValue('Annulée')).toBeNull();
    expect(mockApi.createLinkedTask).toHaveBeenCalledTimes(1);
  });

  it('only offers tasks of the same project not already linked, and unlinks without deleting', async () => {
    const otherProject = note('other', { project_id: 'p2', note_type: 'todo' });
    const plain = note('plain');
    const available = note('available', { title: 'Tâche disponible', note_type: 'todo' });
    mockApi.getNoteRelations.mockResolvedValue({ sources: [], tasks: [task] });
    const changed = vi.fn();
    render(<NoteRelations note={source} allNotes={[source, task, available, otherProject, plain]} onOpenNote={() => {}} onChanged={changed} />);
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByText('Préparer une version')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Rattacher une tâche' }));
    const select = screen.getByLabelText('Tâche existante du même projet') as HTMLSelectElement;
    expect([...select.options].map(option => option.value)).toEqual(['', 'available']);
    fireEvent.change(select, { target: { value: 'available' } });
    fireEvent.click(screen.getByRole('button', { name: 'Rattacher' }));
    await waitFor(() => expect(mockApi.linkNoteTask).toHaveBeenCalledWith('source', 'available'));
    fireEvent.click(screen.getByRole('button', { name: 'Délier la tâche Préparer une version' }));
    await waitFor(() => expect(mockApi.unlinkNoteTask).toHaveBeenCalledWith('source', 'task'));
    expect(changed).toHaveBeenCalledTimes(2);
    expect(mockApi.deleteNote).toBeUndefined();
  });

  it('keeps the links on screen and reports a failure without announcing a success', async () => {
    mockApi.getNoteRelations.mockResolvedValue({ sources: [], tasks: [task] });
    mockApi.unlinkNoteTask.mockRejectedValue(new Error('disk locked'));
    const changed = vi.fn();
    render(<NoteRelations note={source} allNotes={[source, task]} onOpenNote={() => {}} onChanged={changed} />);
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByText('Préparer une version')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Délier la tâche Préparer une version' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('disk locked'));
    expect(screen.getByText('Préparer une version')).toBeTruthy();
    expect(changed).not.toHaveBeenCalled();
  });

  it('ignores a late read after a note change', async () => {
    let finish: (value: { sources: Note[]; tasks: Note[] }) => void = () => {};
    mockApi.getNoteRelations.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    const { rerender } = render(<NoteRelations key="source" note={source} allNotes={[source, task]} onOpenNote={() => {}} onChanged={() => {}} />);
    await waitFor(() => expect(mockApi.getNoteRelations).toHaveBeenCalledWith('source'));
    rerender(<NoteRelations key="other" note={note('other')} allNotes={[source, task]} onOpenNote={() => {}} onChanged={() => {}} />);
    await act(async () => { finish({ sources: [], tasks: [task] }); });
    await waitFor(() => expect(screen.getByRole('button', { name: /Tâches liées/ }).textContent).toContain('0'));
    expect(screen.queryByText('Préparer une version')).toBeNull();
  });

  it('resumes the linked-task draft after a note change, then consumes it on creation', async () => {
    const { unmount } = render(<NoteRelations key="source" note={source} allNotes={[source]} onOpenNote={() => {}} onChanged={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByText('Aucune tâche liée.')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Créer une tâche' }));
    fireEvent.change(screen.getByLabelText('Titre de la tâche'), { target: { value: 'Brouillon en cours' } });
    fireEvent.change(screen.getByLabelText('Contenu facultatif'), { target: { value: 'Détails à ne pas perdre' } });
    // The component is recreated on note change: unmounting escrows the input.
    unmount();
    expect(readPendingWrites()).toEqual([
      expect.objectContaining({ kind: 'task-draft', noteId: 'source', titre: 'Brouillon en cours', contenu: 'Détails à ne pas perdre' }),
    ]);

    // Back on the source note: the form is prefilled with the draft.
    render(<NoteRelations key="source" note={source} allNotes={[source]} onOpenNote={() => {}} onChanged={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByDisplayValue('Brouillon en cours')).toBeTruthy());
    expect(screen.getByDisplayValue('Détails à ne pas perdre')).toBeTruthy();
    expect(screen.getByText(/Brouillon de tâche récupéré/)).toBeTruthy();

    // "Create and link" consumes the draft.
    mockApi.createLinkedTask.mockResolvedValue('new-task');
    fireEvent.click(screen.getByRole('button', { name: 'Créer et lier' }));
    await waitFor(() => expect(mockApi.createLinkedTask).toHaveBeenCalledWith('source', 'Brouillon en cours', 'Détails à ne pas perdre'));
    await waitFor(() => expect(readPendingWrites()).toHaveLength(0));
  });

  it('explicitly abandoning the task draft removes it from the escrow', async () => {
    render(<NoteRelations key="source" note={source} allNotes={[source]} onOpenNote={() => {}} onChanged={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByText('Aucune tâche liée.')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Créer une tâche' }));
    fireEvent.change(screen.getByLabelText('Titre de la tâche'), { target: { value: 'À abandonner' } });
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    expect(readPendingWrites()).toHaveLength(0);
  });

  it('escrows the draft on window close, without React unmounting', async () => {
    render(<NoteRelations key="source" note={source} allNotes={[source]} onOpenNote={() => {}} onChanged={() => {}} />);
    await waitFor(() => expect(winBus.handlers).toHaveLength(1));
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByText('Aucune tâche liée.')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Créer une tâche' }));
    fireEvent.change(screen.getByLabelText('Titre de la tâche'), { target: { value: 'À la fermeture' } });
    fireEvent.change(screen.getByLabelText('Contenu facultatif'), { target: { value: 'Ne pas perdre' } });

    // The window is hidden (no unmount): the close request must escrow the input.
    await act(async () => { winBus.handlers.forEach(handler => handler()); });
    expect(readPendingWrites()).toEqual([
      expect.objectContaining({ kind: 'task-draft', noteId: 'source', titre: 'À la fermeture', contenu: 'Ne pas perdre' }),
    ]);
  });

  it('escrows the draft before Humcase quits for an update, without close nor unmount', async () => {
    render(<NoteRelations key="source" note={source} allNotes={[source]} onOpenNote={() => {}} onChanged={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    await waitFor(() => expect(screen.getByText('Aucune tâche liée.')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Créer une tâche' }));
    fireEvent.change(screen.getByLabelText('Titre de la tâche'), { target: { value: 'Avant la mise à jour' } });

    await act(async () => { await saveBeforeExit(); });
    expect(readPendingWrites()).toEqual([
      expect.objectContaining({ kind: 'task-draft', noteId: 'source', titre: 'Avant la mise à jour' }),
    ]);
  });
});

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NoteReport, NoteStatusControl } from './NoteFollowUp';
import { api } from '../services/tauriApi';
import type { Note } from '../types';
import { readPendingWrites, clearPendingWrites } from '../utils/pendingWrites';

vi.mock('../services/tauriApi', () => ({ api: { updateNote: vi.fn(), retryNoteUpdate: vi.fn() } }));
const note: Note = { id: 'n', project_id: 'p', title: 'Une idée', content: 'Source intacte', note_type: 'idea', status: 'open', created_at: '', updated_at: '', revision: 0, resolution_comment: '## Résultat\n\n**Vérifié** avec une limite.' };
beforeEach(() => { vi.resetAllMocks(); clearPendingWrites(); });

describe('Suivi de note', () => {
  it('preserves a human report draft across close and remount, then writes conditionally', async () => {
    vi.mocked(api.retryNoteUpdate).mockResolvedValue('applied');
    const empty = { ...note, resolution_comment: null };
    const view = render(<NoteReport note={empty} />);
    fireEvent.click(screen.getByText('Ajouter un compte rendu'));
    fireEvent.change(screen.getByLabelText('Résultat et limites'), { target: { value: 'Résultat humain' } });
    fireEvent.click(screen.getByRole('button', { name: 'Fermer' }));
    expect(readPendingWrites()).toHaveLength(1);
    view.unmount();
    const refresh = vi.fn();
    render(<NoteReport note={empty} onUpdated={refresh} />);
    fireEvent.click(screen.getByText('Reprendre le brouillon du compte rendu'));
    expect((screen.getByLabelText('Résultat et limites') as HTMLTextAreaElement).value).toBe('Résultat humain');
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    expect(api.retryNoteUpdate).toHaveBeenCalledWith('n', { resolution_comment: 'Résultat humain' }, { resolution_comment: null });
    expect(readPendingWrites()).toHaveLength(0);
  });

  it('Escape closes the report without losing the draft', () => {
    render(<NoteReport note={{ ...note, resolution_comment: null }} />);
    fireEvent.click(screen.getByText('Ajouter un compte rendu'));
    fireEvent.change(screen.getByLabelText('Résultat et limites'), { target: { value: 'Brouillon à garder' } });
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Votre compte rendu' })).toBeNull();
    expect(readPendingWrites()).toHaveLength(1);
  });

  it('refuses to close the report (Escape or button) while it is being saved', async () => {
    let finish!: (value: 'applied') => void;
    vi.mocked(api.retryNoteUpdate).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
    render(<NoteReport note={{ ...note, resolution_comment: null }} />);
    fireEvent.click(screen.getByText('Ajouter un compte rendu'));
    fireEvent.change(screen.getByLabelText('Résultat et limites'), { target: { value: 'En cours d’écriture' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await waitFor(() => expect(api.retryNoteUpdate).toHaveBeenCalledOnce());
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    expect(screen.getByRole('dialog', { name: 'Votre compte rendu' })).toBeTruthy();
    finish('applied');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Votre compte rendu' })).toBeNull());
  });

  it('retains the human draft when a report changed externally', async () => {
    vi.mocked(api.retryNoteUpdate).mockResolvedValue('conflict');
    render(<NoteReport note={{ ...note, resolution_comment: null }} />);
    fireEvent.click(screen.getByText('Ajouter un compte rendu'));
    fireEvent.change(screen.getByLabelText('Résultat et limites'), { target: { value: 'Mon résultat' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await screen.findByRole('alert');
    expect(readPendingWrites()).toHaveLength(1);
    expect((screen.getByLabelText('Résultat et limites') as HTMLTextAreaElement).value).toBe('Mon résultat');
    fireEvent.click(screen.getByRole('button', { name: 'Abandonner le brouillon' }));
    expect(readPendingWrites()).toHaveLength(0);
  });
  it('writes only the requested status and refreshes after success', async () => {
    let finish!: () => void;
    vi.mocked(api.updateNote).mockImplementation(() => new Promise<void>(resolve => { finish = resolve; }));
    const refresh = vi.fn();
    render(<NoteStatusControl note={note} onUpdated={refresh} />);
    fireEvent.keyDown(screen.getByRole('button', { name: 'Statut de la case : À faire' }), { key: 'Enter' });
    fireEvent.click(screen.getByRole('menuitemradio', { name: 'En cours' }));
    expect(api.updateNote).toHaveBeenCalledWith('n', { status: 'in_progress' });
    expect((screen.getByRole('button', { name: 'Statut de la case : À faire' }) as HTMLButtonElement).disabled).toBe(true);
    expect(refresh).not.toHaveBeenCalled();
    finish();
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
  });

  it('keeps the persisted status on failure and allows retry', async () => {
    vi.mocked(api.updateNote).mockRejectedValueOnce(new Error('offline')).mockResolvedValue(undefined);
    render(<NoteStatusControl note={note} onUpdated={vi.fn()} />);
    fireEvent.keyDown(screen.getByRole('button', { name: 'Statut de la case : À faire' }), { key: 'Enter' });
    fireEvent.click(screen.getByRole('menuitemradio', { name: 'Résolu' }));
    await screen.findByRole('alert');
    expect(screen.getByRole('button', { name: 'Statut de la case : À faire' })).toBeTruthy();
    fireEvent.keyDown(screen.getByRole('button', { name: 'Statut de la case : À faire' }), { key: 'Enter' });
    fireEvent.click(screen.getByRole('menuitemradio', { name: 'Résolu' }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
    expect(api.updateNote).toHaveBeenCalledTimes(2);
  });

  it('uses the single vocabulary of statuses (never "Terminé")', () => {
    render(<NoteStatusControl note={{ ...note, status: 'resolved' }} onUpdated={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Statut de la case : Résolu' })).toBeTruthy();
    expect(screen.queryByText('Terminé')).toBeNull();
    expect(screen.queryByText('Terminée')).toBeNull();
    fireEvent.keyDown(screen.getByRole('button', { name: 'Statut de la case : Résolu' }), { key: 'Enter' });
    expect(screen.getByRole('menuitemradio', { name: 'Résolu' })).toBeTruthy();
    expect(screen.queryByRole('menuitemradio', { name: 'Terminé' })).toBeNull();
  });

  it('renders an existing MCP report as Markdown without implying the reopened note is done', () => {
    const { container, rerender } = render(<NoteReport note={note} />);
    expect(screen.getByText('Compte rendu conservé · case à poursuivre')).not.toBeNull();
    expect(container.querySelector('h2')?.textContent).toBe('Résultat');
    expect(container.querySelector('strong')?.textContent).toBe('Compte rendu');
    expect(container.querySelector('details')?.open).toBe(false);
    rerender(<NoteReport note={{ ...note, resolution_comment: 'Nouveau compte rendu externe' }} />);
    expect(screen.getByText('Nouveau compte rendu externe')).not.toBeNull();
    expect(screen.queryByText('Résultat')).toBeNull();
  });
});

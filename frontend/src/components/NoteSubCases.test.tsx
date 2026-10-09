import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { Note } from '../types';
import NoteSubCases, { CasePath } from './NoteSubCases';
import { directSubCases } from '../utils/subCases';

vi.mock('../services/tauriApi', () => ({ api: {
  getCaseContext: vi.fn(),
  createSubCase: vi.fn(),
  moveCase: vi.fn(),
  getNote: vi.fn(),
} }));

import { api } from '../services/tauriApi';
const mockApi = vi.mocked(api);

const note = (id: string, title: string, parent_id: string | null = null, created_at = '2026-10-01T10:00:00.000Z'): Note => ({
  id, project_id: 'p1', title, content: '', note_type: 'note', status: 'open',
  created_at, updated_at: created_at, revision: 0, parent_id, content_kind: 'text',
});

const voyage = note('voyage', 'Voyage');
const hotels = note('hotels', 'Hôtels', 'voyage', '2026-10-01T10:01:00.000Z');
const plan = note('plan', 'Plan', 'voyage', '2026-10-01T10:02:00.000Z');
const hotelX = note('hotel-x', 'Hôtel X', 'hotels');
const trashed = { ...note('old', 'Ancienne', 'voyage'), deleted_at: '2026-10-01T11:00:00.000Z' };
const allNotes = [voyage, plan, hotels, hotelX, trashed];

beforeEach(() => {
  vi.clearAllMocks();
  mockApi.getCaseContext.mockResolvedValue({ path: [], provenance: [] });
});

describe('NoteSubCases', () => {
  it('lists the direct live sub-cases in creation order, with their own count', async () => {
    expect(directSubCases('voyage', allNotes).map(n => n.title)).toEqual(['Hôtels', 'Plan']);
    render(<NoteSubCases note={voyage} allNotes={allNotes} onOpenNote={vi.fn()} onChanged={vi.fn()} onBeforeWrite={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Hôtels' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Plan' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Hôtel X' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Ancienne' })).toBeNull();
    expect(screen.getByText('1 sous-case')).toBeTruthy();
    await waitFor(() => expect(mockApi.getCaseContext).toHaveBeenCalledWith('voyage'));
  });

  it('opens a sub-case', async () => {
    const onOpenNote = vi.fn();
    render(<NoteSubCases note={voyage} allNotes={allNotes} onOpenNote={onOpenNote} onChanged={vi.fn()} onBeforeWrite={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Hôtels' }));
    await waitFor(() => expect(onOpenNote).toHaveBeenCalledWith(hotels));
  });

  it('creates a sub-case, rereads the notes and opens it', async () => {
    const created = note('new', 'Restaurants', 'voyage');
    mockApi.createSubCase.mockResolvedValue('new');
    mockApi.getNote.mockResolvedValue(created);
    const onOpenNote = vi.fn();
    const onChanged = vi.fn();
    render(<NoteSubCases note={voyage} allNotes={allNotes} onOpenNote={onOpenNote} onChanged={onChanged} onBeforeWrite={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle sous-case' }));
    fireEvent.change(screen.getByLabelText('Titre de la sous-case'), { target: { value: '  Restaurants ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Créer et ouvrir' }));
    await waitFor(() => expect(onOpenNote).toHaveBeenCalledWith(created));
    expect(mockApi.createSubCase).toHaveBeenCalledWith('voyage', 'Restaurants');
    expect(onChanged).toHaveBeenCalled();
  });

  it('keeps the input and shows the error when creation fails', async () => {
    mockApi.createSubCase.mockRejectedValue('parent case in the trash');
    const onOpenNote = vi.fn();
    render(<NoteSubCases note={voyage} allNotes={allNotes} onOpenNote={onOpenNote} onChanged={vi.fn()} onBeforeWrite={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle sous-case' }));
    fireEvent.change(screen.getByLabelText('Titre de la sous-case'), { target: { value: 'Restaurants' } });
    fireEvent.click(screen.getByRole('button', { name: 'Créer et ouvrir' }));
    await screen.findByText('parent case in the trash');
    expect((screen.getByLabelText('Titre de la sous-case') as HTMLInputElement).value).toBe('Restaurants');
    expect(onOpenNote).not.toHaveBeenCalled();
  });

  it('saves the editor input before moving a sub-case up into its grandparent', async () => {
    mockApi.getCaseContext.mockResolvedValue({ path: [{ id: 'voyage', title: 'Voyage' }, { id: 'hotels', title: 'Hôtels' }], provenance: [] });
    const order: string[] = [];
    const onBeforeWrite = vi.fn(async () => { order.push('flush'); });
    mockApi.moveCase.mockImplementation(async () => { order.push('move'); });
    render(<NoteSubCases note={hotelX} allNotes={allNotes} onOpenNote={vi.fn()} onChanged={vi.fn()} onBeforeWrite={onBeforeWrite} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Remonter dans Voyage' }));
    await waitFor(() => expect(mockApi.moveCase).toHaveBeenCalledWith('hotel-x', 'voyage'));
    expect(order).toEqual(['flush', 'move']);
  });

  it('moves a direct sub-case back to top level', async () => {
    mockApi.getCaseContext.mockResolvedValue({ path: [{ id: 'voyage', title: 'Voyage' }], provenance: [] });
    render(<NoteSubCases note={hotels} allNotes={allNotes} onOpenNote={vi.fn()} onChanged={vi.fn()} onBeforeWrite={vi.fn(async () => {})} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Sortir au premier niveau' }));
    await waitFor(() => expect(mockApi.moveCase).toHaveBeenCalledWith('hotels', null));
  });

  it('offers no move for a top-level case', async () => {
    render(<NoteSubCases note={voyage} allNotes={allNotes} onOpenNote={vi.fn()} onChanged={vi.fn()} onBeforeWrite={vi.fn()} />);
    await waitFor(() => expect(mockApi.getCaseContext).toHaveBeenCalled());
    expect(screen.queryByRole('button', { name: /Remonter|Sortir/ })).toBeNull();
  });

  it('shows the provenance with its agent, link and exact excerpt', async () => {
    mockApi.getCaseContext.mockResolvedValue({ path: [], provenance: [
      { id: 'pr1', note_id: 'voyage', kind: 'research', label: 'Comparatif', url: 'https://example.com/hotels', excerpt: 'Trois hôtels retenus.', agent: 'claude', created_at: '2026-10-01T10:00:00.000Z' },
    ] });
    render(<NoteSubCases note={voyage} allNotes={allNotes} onOpenNote={vi.fn()} onChanged={vi.fn()} onBeforeWrite={vi.fn()} />);
    await screen.findByText('Provenance');
    expect(screen.getByText('research')).toBeTruthy();
    expect(screen.getByText(/Comparatif/)).toBeTruthy();
    expect(screen.getByText('https://example.com/hotels')).toBeTruthy();
    expect(screen.getByText('Trois hôtels retenus.')).toBeTruthy();
    expect(screen.getByText(/Ajoutée par claude/)).toBeTruthy();
  });

  it('names the provenances Humcase writes in the language of the interface', async () => {
    mockApi.getCaseContext.mockResolvedValue({ path: [], provenance: [
      { id: 'pr1', note_id: 'voyage', kind: 'ask', label: 'Humcase · Cerebras', url: null, excerpt: 'Où en est le voyage ?', agent: null, created_at: '2026-10-08T10:00:00.000Z' },
      { id: 'pr2', note_id: 'voyage', kind: 'source', label: 'Hôtels', url: null, excerpt: 'Trois hôtels retenus.', agent: null, created_at: '2026-10-08T10:00:00.000Z' },
    ] });
    render(<NoteSubCases note={voyage} allNotes={allNotes} onOpenNote={vi.fn()} onChanged={vi.fn()} onBeforeWrite={vi.fn()} />);
    expect(await screen.findByText('Réponse de Humcase')).toBeTruthy();
    expect(screen.getByText('Source')).toBeTruthy();
    expect(screen.queryByText('ask')).toBeNull();
    expect(screen.queryByText(/Cerebras/)).toBeNull();
  });
});

describe('CasePath', () => {
  it('shows the ancestors of a sub-case and opens one', async () => {
    mockApi.getCaseContext.mockResolvedValue({ path: [{ id: 'voyage', title: 'Voyage' }, { id: 'hotels', title: 'Hôtels' }], provenance: [] });
    const onOpenNote = vi.fn();
    render(<CasePath note={hotelX} allNotes={allNotes} onOpenNote={onOpenNote} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Voyage' }));
    await waitFor(() => expect(onOpenNote).toHaveBeenCalledWith(voyage));
    expect(screen.getByRole('navigation', { name: 'Chemin de la case' })).toBeTruthy();
  });

  it('reads an ancestor missing from the list (archived)', async () => {
    const archived = { ...note('archived', 'Archivée'), archived_at: '2026-10-01T09:00:00.000Z' };
    mockApi.getCaseContext.mockResolvedValue({ path: [{ id: 'archived', title: 'Archivée' }], provenance: [] });
    mockApi.getNote.mockResolvedValue(archived);
    const onOpenNote = vi.fn();
    render(<CasePath note={note('child', 'Enfant', 'archived')} allNotes={[]} onOpenNote={onOpenNote} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Archivée' }));
    await waitFor(() => expect(onOpenNote).toHaveBeenCalledWith(archived));
  });

  it('shows nothing for a top-level case', () => {
    const { container } = render(<CasePath note={voyage} allNotes={allNotes} onOpenNote={vi.fn()} />);
    expect(container.textContent).toBe('');
  });
});

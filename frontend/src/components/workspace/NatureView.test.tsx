import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { MeaningHit, Note, Project } from '../../types';
import { NatureView } from './NatureView';
import WorkspaceSidebar from '../WorkspaceSidebar';
import { inShortcut, readShownShortcuts, shortcutCounts, DEFAULT_SHORTCUTS } from '../../utils/natureShortcuts';

// The database search answers only in the test that sets it; otherwise the view narrows locally.
const getNotes = vi.hoisted(() => vi.fn((..._args: unknown[]) => new Promise<Note[]>(() => {})));
const searchByMeaning = vi.hoisted(() => vi.fn(async (..._args: unknown[]) => [] as MeaningHit[]));
vi.mock('../../services/tauriApi', () => ({ api: { getNotes, searchByMeaning } }));
vi.mock('../sketch/SketchThumbnail', () => ({ default: ({ label }: { label: string }) => <img alt={label} /> }));

const now = '2026-10-03T10:00:00.000Z';
const project: Project = { id: 'p', name: 'Distribution', color: '#378ADD', space: 'work', created_at: now, updated_at: now, revision: 0 };
const note = (id: string, title: string, over: Partial<Note> = {}): Note => ({
  id, project_id: 'p', title, content: '', note_type: 'note', status: 'open', created_at: now, updated_at: now, revision: 1, ...over,
});
const landing = note('landing', 'Landing open source', { note_type: 'idea' });
const notes = [
  landing,
  note('d1', 'Licence AGPL', { note_type: 'decision', status: 'resolved', updated_at: '2026-10-01T10:00:00.000Z' }),
  note('d2', 'Nom de domaine', { note_type: 'decision', parent_id: 'landing' }),
  note('d3', 'Décision archivée', { note_type: 'decision', archived_at: now }),
  note('t1', 'Captures', { note_type: 'todo', parent_id: 'landing' }),
  note('k1', 'Plan de la landing', { content_kind: 'sketch', parent_id: 'landing' }),
  note('l1', 'Dépôt GitHub', { content_kind: 'link', parent_id: 'landing' }),
];

describe('Shortcuts by nature', () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => localStorage.clear());

  it('gathers every decision of the space, sub-cases included, and says where each lives', () => {
    const open = vi.fn();
    render(<NatureView shortcut="decisions" notes={notes} projects={[project]} onNote={open} />);
    expect(screen.getByRole('heading', { name: /Décisions/ })).toBeTruthy();
    const list = screen.getByRole('list');
    expect(within(list).getByText('Licence AGPL')).toBeTruthy();
    expect(within(list).getByText('Nom de domaine')).toBeTruthy();
    // An archived case is not gathered; the sub-case carries its path.
    expect(screen.queryByText('Décision archivée')).toBeNull();
    expect(within(list).getByText('Distribution › Landing open source')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir « Nom de domaine »' }));
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ id: 'd2' }));
  });

  it('searches its cases and creates one of its nature, except for documents and drawings', () => {
    const onCreate = vi.fn();
    const { rerender } = render(<NatureView shortcut="decisions" notes={notes} projects={[project]} onNote={vi.fn()} onCreate={onCreate} />);
    fireEvent.change(screen.getByRole('searchbox', { name: 'Rechercher dans Décisions' }), { target: { value: 'domaine' } });
    expect(screen.queryByText('Licence AGPL')).toBeNull();
    expect(screen.getByText('Nom de domaine')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle case' }));
    expect(onCreate).toHaveBeenCalled();
    rerender(<NatureView shortcut="drawings" notes={notes} projects={[project]} onNote={vi.fn()} onCreate={onCreate} />);
    expect(screen.queryByRole('button', { name: 'Nouvelle case' })).toBeNull();
  });

  it('filters by state and starts again from All on another shortcut', () => {
    const { rerender } = render(<NatureView shortcut="decisions" notes={notes} projects={[project]} onNote={vi.fn()} />);
    fireEvent.click(screen.getByRole('radio', { name: 'À faire' }));
    expect(screen.queryByText('Licence AGPL')).toBeNull();
    expect(screen.getByText('Nom de domaine')).toBeTruthy();
    rerender(<NatureView shortcut="tasks" notes={notes} projects={[project]} onNote={vi.fn()} />);
    const all = screen.getByRole('radio', { name: 'Toutes' });
    expect(all.getAttribute('aria-checked')).toBe('true');
    expect(all.querySelector('.ui-segmented__count')?.textContent).toBe('1');
  });

  it('shows drawings as pictures and documents by what they hold', () => {
    const { rerender } = render(<NatureView shortcut="drawings" notes={notes} projects={[project]} onNote={vi.fn()} />);
    expect(screen.getByAltText('Dessin « Plan de la landing »')).toBeTruthy();
    rerender(<NatureView shortcut="documents" notes={notes} projects={[project]} onNote={vi.fn()} />);
    expect(screen.getByText('Dépôt GitHub')).toBeTruthy();
    expect(screen.queryByText('Plan de la landing')).toBeNull();
  });

  it('counts unresolved live cases only, like every count of the sidebar, and falls back to the defaults when storage is unreadable', () => {
    // Two live decisions, one of them resolved: the sidebar counts what is left to handle.
    expect(shortcutCounts(notes).decisions).toBe(1);
    expect(inShortcut('drawings', notes[5])).toBe(true);
    localStorage.setItem('humcase-nature-shortcuts', '{broken');
    expect(readShownShortcuts()).toEqual(DEFAULT_SHORTCUTS);
  });

  it('unfolds under All cases in the sidebar and lets the person choose them', () => {
    const noop = vi.fn();
    const onShortcut = vi.fn();
    render(<WorkspaceSidebar
      projects={[project]} inboxProjects={[]} activities={[]} activeActivityId={null} onActivity={noop} onProjectSettings={noop}
      archivedProjects={[]} page="home" activeProjectId={null} todayActive={false} todayCount={0} mobileOpen={false} onCloseMobile={noop}
      onHome={noop} onAllNotes={noop} onToday={noop} onProject={noop} onInbox={noop} onNewProject={noop} onSearch={noop}
      onSettings={noop} onTrash={noop} spacePreferences={{ usage: 'both', combined: true, scope: 'work' }} onScope={noop}
      unassignedCount={0} onAssignProject={noop} shortcutCounts={shortcutCounts(notes)} onShortcut={onShortcut} />);
    const shortcuts = screen.getByRole('navigation', { name: 'Raccourcis par nature' });
    fireEvent.click(within(shortcuts).getByRole('button', { name: /Décisions/ }));
    expect(onShortcut).toHaveBeenCalledWith('decisions');
    expect(within(shortcuts).queryByRole('button', { name: /Questions/ })).toBeNull();
    fireEvent.click(within(shortcuts).getByRole('button', { name: 'Modifier la liste' }));
    fireEvent.click(within(shortcuts).getByRole('checkbox', { name: 'Questions' }));
    expect(within(shortcuts).getByRole('button', { name: /Questions/ })).toBeTruthy();
    expect(JSON.parse(localStorage.getItem('humcase-nature-shortcuts') ?? '[]')).toContain('questions');
    // Folding hides them, and is remembered.
    fireEvent.click(screen.getByRole('button', { name: 'Masquer les raccourcis' }));
    expect(screen.queryByRole('navigation', { name: 'Raccourcis par nature' })).toBeNull();
    expect(localStorage.getItem('humcase-nature-shortcuts-open')).toBe('false');
  });

  it('shows the database answer once it arrives, partial matches included', async () => {
    getNotes.mockResolvedValueOnce([notes[1]]);
    render(<NatureView shortcut="decisions" notes={notes} projects={[project]} onNote={vi.fn()} />);
    fireEvent.change(screen.getByRole('searchbox', { name: 'Rechercher dans Décisions' }), { target: { value: 'licences gratuites' } });
    // Locally, no decision holds both words.
    expect(screen.queryByText('Licence AGPL')).toBeNull();
    await waitFor(() => expect(screen.getByText('Licence AGPL')).toBeTruthy());
    expect(getNotes).toHaveBeenCalledWith(undefined, undefined, 'licences gratuites');
    expect(screen.queryByText('Nom de domaine')).toBeNull();
  });

  it('puts first a case the words found lower when it is the closest in meaning', async () => {
    getNotes.mockResolvedValueOnce([notes[1], notes[2]]);
    searchByMeaning.mockResolvedValueOnce([{ note_id: 'd2', score: 0.82, lead: 3.4, strong: true, very_close: false }]);
    render(<NatureView shortcut="decisions" notes={notes} projects={[project]} onNote={vi.fn()} />);
    fireEvent.change(screen.getByRole('searchbox', { name: 'Rechercher dans Décisions' }), { target: { value: 'adresse du site' } });
    await waitFor(() => expect(searchByMeaning).toHaveBeenCalledWith('adresse du site', 30));
    await waitFor(() => {
      const rows = screen.getAllByText(/Licence AGPL|Nom de domaine/).map(row => row.textContent);
      expect(rows[0]).toBe('Nom de domaine');
    });
  });
});

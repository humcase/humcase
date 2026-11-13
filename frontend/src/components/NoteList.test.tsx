import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import NoteList from './NoteList';
import type { Note } from '../types';

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

interface RenderOptions {
  onSelectNote?: (id: string) => void;
  onCreateNote?: () => void;
  onSearchQueryChange?: (query: string) => void;
  onStatusFilterChange?: (status: string) => void;
  activeNoteId?: string | null;
  searchQuery?: string;
  statusFilter?: string;
  hideFilters?: boolean;
  emptyMessage?: string;
}

function renderHelper(notes: Note[], options: RenderOptions = {}) {
  return render(
    <NoteList
      notes={notes}
      activeNoteId={options.activeNoteId ?? null}
      onSelectNote={options.onSelectNote ?? (() => {})}
      onCreateNote={options.onCreateNote ?? (() => {})}
      searchQuery={options.searchQuery ?? ''}
      onSearchQueryChange={options.onSearchQueryChange ?? (() => {})}
      statusFilter={options.statusFilter}
      onStatusFilterChange={options.onStatusFilterChange}
      hideFilters={options.hideFilters}
      emptyMessage={options.emptyMessage}
    />
  );
}

describe('NoteList — tag display', () => {
  it("shows a single-character tag (counted after decoding, not by the JSON length)", () => {
    renderHelper([makeNote({ tags: '["a"]' })]);
    // `["a"]` is 5 characters long: the old `n.tags.length > 5` test hid it.
    expect(screen.getByText('#a')).toBeTruthy();
  });

  it('shows all decodable tags', () => {
    renderHelper([makeNote({ tags: '["bug","P1"]' })]);
    expect(screen.getByText('#bug')).toBeTruthy();
    expect(screen.getByText('#P1')).toBeTruthy();
  });

  it("shows nothing for empty or corrupted tags", () => {
    renderHelper([makeNote({ tags: '[]' }), makeNote({ id: 'n2', title: 'Autre', tags: '[42]' })]);
    expect(screen.queryByText(/^#/)).toBeNull();
  });

  it('renders the priority discreetly on a resolved note (history)', () => {
    renderHelper([
      makeNote({ priority: 'P0', status: 'open', title: 'Ouverte' }),
      makeNote({ id: 'n2', priority: 'P0', status: 'resolved', title: 'Résolue' }),
    ]);
    const badgeList = screen.getAllByText('P0').map(label => label.closest('.ui-priority'));
    expect(badgeList).toHaveLength(2);
    expect(badgeList[0]?.getAttribute('data-tone')).toBe('danger');
    expect(badgeList[1]?.getAttribute('data-tone')).toBe('neutral');
    expect(badgeList[1]?.hasAttribute('data-resolved')).toBe(true);
  });
});

describe('NoteList — keyboard access and distinct actions', () => {
  it('associates an explicit label with the search field', () => {
    renderHelper([]);
    expect(screen.getByRole('textbox', { name: 'Rechercher une case' })).toBeTruthy();
  });

  it('separates the selection button from the tag button and keeps their actions', () => {
    const onSelectNote = vi.fn();
    const onSearchQueryChange = vi.fn();
    renderHelper([makeNote({ title: 'Ma note', tags: '["bug"]' })], { onSelectNote, onSearchQueryChange });

    const openAction = screen.getByRole('button', { name: 'Ouvrir la case Ma note' });
    const tag = screen.getByRole('button', { name: 'Rechercher les cases avec le tag bug' });
    expect(openAction.tagName).toBe('BUTTON');
    expect(tag.tagName).toBe('BUTTON');
    expect(openAction.contains(tag)).toBe(false);

    openAction.focus();
    expect(document.activeElement).toBe(openAction);
    tag.focus();
    expect(document.activeElement).toBe(tag);

    fireEvent.click(tag);
    expect(onSearchQueryChange).toHaveBeenCalledExactlyOnceWith('#bug');
    expect(onSelectNote).not.toHaveBeenCalled();

    fireEvent.click(openAction);
    expect(onSelectNote).toHaveBeenCalledExactlyOnceWith('n1');
  });

  it('indicates the active note on its selection button', () => {
    renderHelper([makeNote({ title: 'Active' })], { activeNoteId: 'n1' });
    expect(screen.getByRole('button', { name: 'Ouvrir la case Active' }).getAttribute('aria-current')).toBe('true');
  });

  it('hides search and status in the Today view without hiding the notes', () => {
    renderHelper([makeNote({ title: 'Urgente' })], {
      searchQuery: 'Autre filtre',
      statusFilter: 'resolved',
      onStatusFilterChange: vi.fn(),
      hideFilters: true,
    });
    expect(screen.queryByRole('textbox', { name: 'Rechercher une case' })).toBeNull();
    expect(screen.queryByRole('radio', { name: 'Résolu' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Ouvrir la case Urgente' })).toBeTruthy();
  });

  it('does not describe a hidden search as the cause of an empty list', () => {
    renderHelper([], {
      searchQuery: 'Filtre mémorisé',
      hideFilters: true,
      emptyMessage: "Aucune urgence aujourd’hui",
    });
    expect(screen.getByText("Aucune urgence aujourd’hui")).toBeTruthy();
  });

  it('captures from the List even without a selected project (creation in the Inbox delegated)', () => {
    const onCreateNote = vi.fn();
    renderHelper([], { onCreateNote });
    const button = screen.getByRole('button', { name: /Nouvelle case/i }) as HTMLButtonElement;
    // Without a project, the button stays active: the local guard must not come back.
    expect(button.disabled).toBe(false);
    fireEvent.click(button);
    expect(onCreateNote).toHaveBeenCalledTimes(1);
  });
});

describe('NoteList — status at the start of the row', () => {
  it('shows the status badge with the single label of the shared source', () => {
    const { container } = renderHelper([
      makeNote({ id: 'n1', title: 'Ouverte', status: 'open' }),
      makeNote({ id: 'n2', title: 'Terminee', status: 'resolved' }),
    ]);
    const badgeList = [...container.querySelectorAll('.ui-status')].map(b => b.textContent);
    expect(badgeList).toContain('À faire');
    expect(badgeList).toContain('Résolu');
    expect(screen.queryByText('Terminé')).toBeNull();
    expect(screen.queryByText('Terminée')).toBeNull();
  });

  it('renders the badge before the priority and the type in the first row', () => {
    const { container } = renderHelper([makeNote({ title: 'Ma note', priority: 'P1', note_type: 'bug' })]);
    const badge = container.querySelector('.workspace-list-row .ui-status');
    const group = badge?.parentElement;
    const line = group?.parentElement;
    expect(group?.firstElementChild).toBe(badge);
    expect(line?.firstElementChild).toBe(group);
  });

  it('does not make the badge a button (the "Resolved" filter keeps its unique role)', () => {
    const { container } = renderHelper([makeNote({ title: 'Terminee', status: 'resolved' })], {
      statusFilter: '',
      onStatusFilterChange: vi.fn(),
    });
    const badge = container.querySelector('.ui-status');
    expect(badge?.tagName).toBe('SPAN');
    expect(screen.getAllByRole('radio', { name: 'Résolu' })).toHaveLength(1);
  });
});

describe('NoteList — empty states', () => {
  it('invites to create a first note when the scope is empty, without filter or search', () => {
    const onCreateNote = vi.fn();
    renderHelper([], { onCreateNote, onStatusFilterChange: () => {} });
    expect(screen.getByText('Encore aucune case')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Créer une case/ }));
    expect(onCreateNote).toHaveBeenCalledOnce();
  });

  it('does not offer to create a note when a filter or a search hides the results', () => {
    renderHelper([], { statusFilter: 'open', onStatusFilterChange: () => {} });
    expect(screen.getByText('Aucune case trouvée')).toBeTruthy();
    expect(screen.queryByText('Encore aucune case')).toBeNull();
    renderHelper([], { searchQuery: 'x' });
    expect(screen.getByText('Aucun résultat')).toBeTruthy();
  });

  it('offers to clear search and filter when they hide everything (review 0.9.69)', () => {
    const onSearchQueryChange = vi.fn();
    const onStatusFilterChange = vi.fn();
    renderHelper([], { searchQuery: 'x', statusFilter: 'open', onSearchQueryChange, onStatusFilterChange });
    fireEvent.click(screen.getByRole('button', { name: 'Effacer les filtres' }));
    expect(onSearchQueryChange).toHaveBeenCalledWith('');
    expect(onStatusFilterChange).toHaveBeenCalledWith('');
  });
});

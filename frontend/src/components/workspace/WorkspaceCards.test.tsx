import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { NoteCard } from './WorkspaceCards';
import { WorkspaceHome } from './WorkspaceHome';
import type { Note, Project } from '../../types';

const project: Project = { id: 'p', name: 'Projet témoin', color: '#6353a5', space: 'work', created_at: '2026-09-26T10:00:00Z', updated_at: '2026-09-26T10:00:00Z', revision: 0 };
const note: Note = { id: 'n', project_id: 'p', title: 'Une pensée à suivre', content: 'Le contenu original reste accessible.', note_type: 'idea', status: 'in_progress', priority: 'P1', due_at: '2026-09-28', tags: '["design",42,"sources","archive"]', git_branch: 'feature/contexte', resolution_comment: '## Résultat\nUne **décision** conservée. <script>test</script>', created_at: '2026-09-25T10:00:00Z', updated_at: '2026-09-26T10:00:00Z', revision: 1 };

describe('Real cards — information and access preserved', () => {
  beforeEach(() => { vi.useFakeTimers({ now: new Date('2026-09-27T12:00:00Z'), toFake: ['Date'] }); });
  afterEach(() => { vi.useRealTimers(); });
  it('keeps status, type, priority, due date, project, tags and branch with direct access to the full note', () => {
    const open = vi.fn();
    const { container } = render(<NoteCard note={note} project={project} onOpen={open} />);
    for (const text of ['En cours', 'Idée', 'P1', 'Projet témoin', '#design', '#sources', '+1', 'feature/contexte', 'Compte rendu']) expect(screen.getByText(text)).toBeTruthy();
    expect(container.querySelector('.ui-due')).toBeTruthy();
    expect(container.querySelector('time')?.dateTime).toBe('2026-09-26T10:00:00.000Z');
    expect(container.querySelector('script')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir Une pensée à suivre' }));
    expect(open).toHaveBeenCalledWith(note);
  });

  it('fabricates neither a report nor tags if the data is absent or malformed', () => {
    render(<NoteCard note={{ ...note, tags: '{corrompu', resolution_comment: null }} onOpen={vi.fn()} />);
    expect(screen.queryByText('Compte rendu')).toBeNull();
    expect(screen.queryByText('#design')).toBeNull();
  });

  it('keeps previews inert, replaces images with their alt and keeps the metadata', () => {
    const { container } = render(<NoteCard note={{
      ...note,
      note_type: 'bug',
      content: 'Texte avant. <img src="https://example.test/raw.png" onerror="alert(1)"> [documentation](https://example.test) ![schéma local](https://example.test/image.png)',
    }} project={project} onOpen={vi.fn()} />);

    expect(container.querySelector('img, a, script')).toBeNull();
    expect(screen.getByText('Image : schéma local')).toBeTruthy();
    expect(screen.getByText('documentation').tagName).toBe('SPAN');
    expect(screen.getByText('En cours')).toBeTruthy();
    expect(screen.getByText('Incident')).toBeTruthy();
    expect(screen.getByText('P1')).toBeTruthy();
    expect(screen.getByText('Projet témoin')).toBeTruthy();
  });

  it('shows a local thumbnail then a fallback if the file is missing, without losing the note', () => {
    const open = vi.fn();
    render(<NoteCard note={{ ...note, content: '![Plan](http://asset.localhost/plan.png)' }} onOpen={open} />);
    const image = screen.getByRole('img', { name: 'Plan' });
    expect(image.getAttribute('src')).toBe('http://asset.localhost/plan.png');
    fireEvent.error(image);
    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText('Image indisponible · Plan')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir Une pensée à suivre' }));
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ id: note.id }));
  });

  it('only shows a case present in the received notes and opens it', () => {
    const open = vi.fn();
    const { rerender } = render(<WorkspaceHome projects={[project]} notes={[note]} onNote={open} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Continuer Une pensée à suivre' }));
    expect(open).toHaveBeenCalledWith(note);
    rerender(<WorkspaceHome projects={[project]} notes={[]} onNote={open} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    expect(screen.queryByText('Une pensée à suivre')).toBeNull();
    expect(screen.getByText('Votre espace est prêt pour une première case.')).toBeTruthy();
  });
});

describe('Resume home — factual labels', () => {
  beforeEach(() => { vi.useFakeTimers({ now: new Date('2026-09-27T12:00:00Z'), toFake: ['Date'] }); });
  afterEach(() => { vi.useRealTimers(); });
  const home = (notes: Note[], onNote = vi.fn()) => render(<WorkspaceHome projects={[project]} notes={notes} onNote={onNote} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
  const plain = (id: string, updated_at: string, extra: Partial<Note> = {}): Note => ({ ...note, id, title: `Note ${id}`, updated_at, status: 'open', priority: null, due_at: null, resolution_comment: null, tags: '[]', git_branch: null, ...extra });

  it('reads a report on its case card, without a separate reports list', () => {
    home([plain('a', '2026-09-26T10:00:00Z'), plain('b', '2026-09-25T10:00:00Z', { status: 'resolved', resolution_comment: 'Fait et vérifié.' })]);
    expect(screen.getByRole('button', { name: 'Ouvrir Note b' })).toBeTruthy();
    expect(screen.getByText('Compte rendu')).toBeTruthy();
    expect(screen.queryByText(/Comptes rendus|compte rendu depuis hier/)).toBeNull();
  });

  it('leaves the projects to the sidebar, and offers the first one when there is none', () => {
    home([plain('a', '2026-09-26T10:00:00Z')]);
    expect(screen.queryByText('Vos projets')).toBeNull();
    render(<WorkspaceHome projects={[]} notes={[]} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    expect(screen.getByText('Un projet pour commencer')).toBeTruthy();
  });

  it('presents the last modified note without repeating it in the recent cases', () => {
    const open = vi.fn();
    home([plain('ancienne', '2026-09-20T10:00:00Z'), plain('recente', '2026-09-26T10:00:00Z')], open);
    expect(screen.getByRole('article', { name: 'Note recente' })).toBeTruthy();
    // A single access: the resume title, no duplicate card in the recent cases.
    expect(screen.getAllByRole('button', { name: 'Ouvrir Note recente' })).toHaveLength(1);
    expect(screen.getByRole('button', { name: 'Ouvrir Note ancienne' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Continuer Note recente' }));
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ id: 'recente' }));
  });

  it('does not count a high priority as a due date', () => {
    home([plain('p1', '2026-09-26T10:00:00Z', { priority: 'P1' }), plain('autre', '2026-09-27T10:00:00Z')]);
    // Exact text of the summary: no due date mention added for a simple priority.
    expect(screen.getByText('1 case à poursuivre')).toBeTruthy();
  });
});

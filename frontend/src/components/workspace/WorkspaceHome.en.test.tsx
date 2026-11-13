import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { WorkspaceHome } from './WorkspaceHome';
import WorkspaceSidebar from '../WorkspaceSidebar';
import i18n from '../../i18n';
import type { Note, Project } from '../../types';

const project: Project = { id: 'p', name: 'Campaign', color: '#6353a5', space: 'work', created_at: '2026-09-26T10:00:00Z', updated_at: '2026-09-26T10:00:00Z', revision: 0 };
const note: Note = {
  id: 'n', project_id: 'p', title: 'A thought to follow', content: 'Body of the note.', note_type: 'idea', status: 'in_progress',
  priority: null, due_at: null, tags: '[]', git_branch: null, resolution_comment: null,
  created_at: '2026-09-25T10:00:00Z', updated_at: '2026-09-26T10:00:00Z', revision: 1,
};

describe('Home and sidebar in English', () => {
  beforeEach(async () => {
    vi.useFakeTimers({ now: new Date('2026-09-27T12:00:00Z'), toFake: ['Date'] });
    await i18n.changeLanguage('en');
  });
  afterEach(async () => {
    vi.useRealTimers();
    await i18n.changeLanguage('fr');
  });

  it('shows the home in native English, with no leftover French', () => {
    render(<WorkspaceHome projects={[project]} notes={[note]} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    expect(screen.getByRole('heading', { name: 'Pick up where you left off' })).toBeTruthy();
    expect(screen.getByText('Your day')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'New case' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Open A thought to follow' })).toBeTruthy();
    expect(screen.getByText('Recent cases')).toBeTruthy();
    expect(screen.getByText('Last edited')).toBeTruthy();
    expect(screen.queryByText('Reprendre le fil')).toBeNull();
  });

  it('agrees the plural of the English counts', () => {
    // The case to pick up is counted too: it is in progress.
    const { unmount } = render(<WorkspaceHome projects={[project]} notes={[note]} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    expect(screen.getByText('1 case to follow up')).toBeTruthy();
    unmount();
    render(<WorkspaceHome projects={[project]} notes={[note, { ...note, id: 'm', title: 'Another' }]} onNote={vi.fn()} onCreateProject={vi.fn()} onCreateNote={vi.fn()} />);
    expect(screen.getByText('2 cases to follow up')).toBeTruthy();
  });

  it('shows the sidebar in English with the glossary terms', () => {
    const noop = vi.fn();
    render(<WorkspaceSidebar
      projects={[project]} inboxProjects={[]} activities={[]} activeActivityId={null} onActivity={noop} onProjectSettings={noop}
      archivedProjects={[]} page="home" activeProjectId={null} todayActive={false} todayCount={0} mobileOpen={false} onCloseMobile={noop}
      onHome={noop} onAllNotes={noop} onToday={noop} onProject={noop} onInbox={noop} onNewProject={noop} onSearch={noop}
      onSettings={noop} onTrash={noop} spacePreferences={{ usage: 'both', combined: true, scope: 'all' }} onScope={noop}
      unassignedCount={0} onAssignProject={noop} />);
    expect(screen.getByRole('button', { name: 'Search' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Home' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Today' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Trash' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Settings' })).toBeTruthy();
    expect(screen.getByText('Inboxes')).toBeTruthy();
    expect(screen.getByText('Projects', { selector: '[data-sidebar="group-label"]' })).toBeTruthy();
    expect(screen.getByLabelText('Switch space: All spaces')).toBeTruthy();
    expect(screen.queryByText('Accueil')).toBeNull();
  });
});

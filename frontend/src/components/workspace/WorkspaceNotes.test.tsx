import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { WorkspaceNotes } from './WorkspaceNotes';

describe('Empty state — first launch', () => {
  const props = { projectName: 'Toutes les cases', onNote: vi.fn(), onCreateNote: vi.fn() };

  it('keeps the default project title and accepts a title outside a project', () => {
    const { unmount } = render(<WorkspaceNotes {...props} notes={[]} />);
    expect(screen.getByRole('heading', { name: 'Ce projet attend sa première case' })).toBeTruthy();
    unmount();
    render(<WorkspaceNotes {...props} notes={[]} firstNoteTitle="Encore aucune case" />);
    expect(screen.getByRole('heading', { name: 'Encore aucune case' })).toBeTruthy();
  });

  it('a filter without results stays "No case found", whatever the first-launch title', () => {
    render(<WorkspaceNotes {...props} notes={[]} filtered onClearFilters={vi.fn()} firstNoteTitle="Encore aucune case" />);
    expect(screen.getByRole('heading', { name: 'Aucune case trouvée' })).toBeTruthy();
  });
});

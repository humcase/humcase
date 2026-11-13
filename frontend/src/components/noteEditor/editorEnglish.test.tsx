import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '../../i18n';
import { DeleteConfirmation } from './DeleteConfirmation';
import { NoteActions } from './NoteActions';
import { PendingWritesPanel } from './PendingWritesPanel';
import TagInput from '../TagInput';

/** Renders the "editor" area in English, then restores French (the rest of the tests expect it). */
async function inEnglish(rendered: () => void) {
  await i18n.changeLanguage('en');
  try {
    rendered();
  } finally {
    await i18n.changeLanguage('fr');
  }
}

describe('Note editor in English', () => {
  it('shows the delete confirmation and the actions in English', async () => {
    await inEnglish(() => {
      render(<DeleteConfirmation isConfirming onAskDelete={vi.fn()} onConfirm={vi.fn()} onCancel={vi.fn()} />);
      expect(screen.getByRole('group', { name: 'Confirm moving to the trash' })).toBeTruthy();
      expect(screen.getByText('Move this case to the trash?')).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Move to the trash' })).toBeTruthy();
      render(<NoteActions archived onCopy={vi.fn()} onToggleArchive={vi.fn()} isConfirmingDelete={false} onAskDelete={vi.fn()} onConfirmDelete={vi.fn()} onCancelDelete={vi.fn()} />);
      expect(screen.getByRole('button', { name: /Unarchive/ })).toBeTruthy();
      expect(screen.getByRole('button', { name: /Copy/ })).toBeTruthy();
    });
  });

  it('agrees the count of pending writes and names the drafts', async () => {
    const writes = [{ id: 'a', kind: 'brut' as const, noteId: 'abcdefghij', texte: 'dictée', space: null, projectId: null, createdAt: 1 }];
    await inEnglish(() => {
      render(<PendingWritesPanel writes={writes as never} persistentWrites recoveryInProgress={false} recoveryProblems={{}} onRetry={vi.fn()} onIgnore={vi.fn()} />);
      expect(screen.getByText('1 unsaved write — text kept:')).toBeTruthy();
      expect(screen.getByText(/Raw dictation · case abcdefgh/)).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Ignore these writes' })).toBeTruthy();
    });
  });

  it('translates the tags field', async () => {
    await inEnglish(() => {
      render(<TagInput tags={['api']} onChange={vi.fn()} availableTags={[]} />);
      expect(screen.getByRole('button', { name: 'Remove tag api' })).toBeTruthy();
      expect(screen.getByLabelText('Add a tag')).toBeTruthy();
    });
  });
});

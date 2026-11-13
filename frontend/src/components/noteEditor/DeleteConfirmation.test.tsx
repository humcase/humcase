import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import '../../i18n';
import { DeleteConfirmation } from './DeleteConfirmation';
import { subCaseCount } from '../../utils/caseBoard';
import type { Note } from '../../types';

const note = (id: string, parent_id: string | null, deleted_at?: string): Note => ({
  id, project_id: 'p1', title: id, content: '', note_type: 'note', status: 'open',
  created_at: '2026-10-01T10:00:00.000Z', updated_at: '2026-10-01T10:00:00.000Z', revision: 0, parent_id, content_kind: 'text',
  ...(deleted_at ? { deleted_at } : {}),
});

describe('deleting a case that holds sub-cases', () => {
  it('says the sub-cases go to the trash with it', () => {
    render(<DeleteConfirmation isConfirming onAskDelete={vi.fn()} onConfirm={vi.fn()} onCancel={vi.fn()} subCases={3} />);
    expect(screen.getByText('Mettre cette case et ses 3 sous-cases à la corbeille ?')).toBeTruthy();
  });

  it('counts live sub-cases at any depth, not the ones already in the trash', () => {
    const notes = [note('root', null), note('a', 'root'), note('b', 'a'), note('gone', 'root', '2026-10-01T11:00:00.000Z'), note('other', null)];
    expect(subCaseCount(['root'], notes)).toBe(2);
    expect(subCaseCount(['other'], notes)).toBe(0);
  });
});

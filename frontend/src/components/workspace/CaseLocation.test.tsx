import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { Note, Project } from '../../types';
import { CaseLocation } from './CaseLocation';
import i18n from '../../i18n';

const inbox = { id: 'inbox', name: '__humcase_inbox_work_x', color: null, space: 'work', is_inbox: true } as unknown as Project;
const note = { id: 'n', project_id: 'inbox', title: 'Idée rapide', parent_id: null } as unknown as Note;

describe('Where a case is', () => {
  beforeEach(async () => { await i18n.changeLanguage('fr'); });

  it('names an Inbox by its space, never by its technical name', () => {
    render(<CaseLocation note={note} byId={new Map([[note.id, note]])} projectById={new Map([[inbox.id, inbox]])} />);
    expect(screen.getByText('Réception · Pro')).toBeTruthy();
    expect(screen.queryByText(/__humcase_inbox/)).toBeNull();
  });
});

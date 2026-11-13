import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { CaseRef, Note, TidyItem } from '../../types';

const api = vi.hoisted(() => ({
  tidyList: vi.fn(),
  tidySettings: vi.fn(async () => ({ archive_weeks: 4, supersede: true, group: true })),
  answerTidy: vi.fn(async () => ({ archived: 1, superseded: 1, grouped: 0, to_fix: 0, refused: 1 })),
  undoTidy: vi.fn(async () => undefined),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

import { TidyCard, TidyView } from './Tidy';
import i18n from '../../i18n';

const ref = (id: string, title: string, nature: CaseRef['nature'] = 'decision'): CaseRef => ({ id, title, nature, project_name: 'Humcase', date: '2026-09-28T10:00:00Z', excerpt: `${title}, en bref` });
const note = (id: string, title: string): Note => ({
  id, project_id: 'p', title, content: '', note_type: 'decision', status: 'open', resolution_comment: null,
  created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 1,
});
const extra = { passage: null, other_passage: null, duplicate: false, proposed_by: null, created_at: '2026-10-01T10:00:00Z' };
const pair = (id: string, older: CaseRef, newer: CaseRef, doubt: boolean): TidyItem => ({ ...extra, id, kind: 'supersede', case: older, other: newer, doubt, checked: true });
const archive = (id: string, target: CaseRef): TidyItem => ({ ...extra, id, kind: 'archive', case: target, other: null, doubt: false, checked: false });

const items = [
  pair('p1', ref('old1', 'Héberger sur Supabase'), ref('new1', 'Abandonner Supabase'), false),
  pair('p2', ref('old2', 'Version payante'), ref('new2', 'Tout gratuit avec votre clé'), true),
  archive('a1', ref('done1', 'Bouton Exporter', 'bug')),
  archive('a2', ref('done2', 'Raccourci Ctrl+F', 'todo')),
];
const notes = ['old1', 'new1', 'old2', 'new2', 'done1', 'done2'].map(id => note(id, id));

describe('To tidy', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('fr');
    api.tidyList.mockResolvedValue(items);
  });

  it('says on Home what is proposed, and that nothing is deleted', () => {
    const onStart = vi.fn();
    render(<TidyCard items={items} weeks={4} onStart={onStart} />);
    expect(screen.getByText(/2 décisions peut-être remplacées par une plus récente, 2 cases résolues depuis plus de 4 semaines\. Rien n’est supprimé\./)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Revoir/ }));
    expect(onStart).toHaveBeenCalled();
  });

  it('names an Inbox by its space, never by its technical name', async () => {
    api.tidyList.mockResolvedValue([archive('a3', { ...ref('done3', 'Idée rapide', 'idea'), project_name: '__humcase_inbox_work_x', inbox_space: 'work' })]);
    render(<TidyView notes={[note('done3', 'Idée rapide')]} onNote={vi.fn()} onChanged={vi.fn()} />);
    expect((await screen.findAllByText(/Réception · Pro/)).length).toBeGreaterThan(0);
    expect(screen.queryByText(/__humcase_inbox/)).toBeNull();
  });

  it('shows no card when nothing is proposed', () => {
    const { container } = render(<TidyCard items={[]} weeks={4} onStart={vi.fn()} />);
    expect(container.innerHTML).toBe('');
  });

  it('applies the answers: a sure pair accepted, a doubtful one and an unchecked case left waiting', async () => {
    api.answerTidy.mockResolvedValueOnce({ archived: 1, superseded: 1, grouped: 0, to_fix: 0, refused: 0 });
    render(<TidyView notes={notes} onNote={vi.fn()} onChanged={vi.fn()} />);
    expect(await screen.findByText('Héberger sur Supabase')).toBeTruthy();
    expect(screen.getByText(/À vérifier/)).toBeTruthy();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Archiver « Raccourci Ctrl+F »' }));
    fireEvent.click(screen.getByRole('button', { name: 'Marquer 1 remplacée · Archiver 1 case' }));
    // Unchecking means "not now", never a refusal for good: the case is not answered and stays proposed.
    await waitFor(() => expect(api.answerTidy).toHaveBeenCalledWith([
      { id: 'p1', accept: true },
      { id: 'a1', accept: true },
    ]));
    expect(await screen.findByText('1 décision marquée remplacée · 1 case archivée')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(api.undoTidy).toHaveBeenCalledWith(['p1', 'a1']));
  });

  it('sends the doubtful pair once the person answers it', async () => {
    render(<TidyView notes={notes} onNote={vi.fn()} onChanged={vi.fn()} />);
    await screen.findByText('Version payante');
    fireEvent.click(screen.getAllByRole('radio', { name: 'Non, les deux restent' })[1]);
    fireEvent.click(screen.getByRole('button', { name: 'Marquer 1 remplacée · Archiver 2 cases' }));
    await waitFor(() => expect(api.answerTidy).toHaveBeenCalledWith(expect.arrayContaining([{ id: 'p2', accept: false }])));
  });

  it('refuses everything at once', async () => {
    render(<TidyView notes={notes} onNote={vi.fn()} onChanged={vi.fn()} />);
    await screen.findByText('Version payante');
    fireEvent.click(screen.getByRole('button', { name: 'Tout refuser' }));
    await waitFor(() => expect(api.answerTidy).toHaveBeenCalledWith([
      { id: 'p1', accept: false }, { id: 'p2', accept: false }, { id: 'a1', accept: false }, { id: 'a2', accept: false },
    ]));
  });

  it('says so when there is nothing to tidy, in English too', async () => {
    api.tidyList.mockResolvedValue([]);
    await i18n.changeLanguage('en');
    render(<TidyView notes={notes} onNote={vi.fn()} onChanged={vi.fn()} />);
    expect(await screen.findByText('Nothing to tidy for now.')).toBeTruthy();
    await i18n.changeLanguage('fr');
  });
});

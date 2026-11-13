import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { CaseRef, Note, ProjectSummary, TidyItem } from '../../types';

const api = vi.hoisted(() => ({
  tidyList: vi.fn(),
  tidySettings: vi.fn(async () => ({ archive_weeks: 4, supersede: true, group: true })),
  answerTidy: vi.fn(async () => ({ archived: 0, superseded: 0, grouped: 1, to_fix: 1, refused: 0 })),
  undoTidy: vi.fn(async () => undefined),
  projectSummary: vi.fn(),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

import { TidyCard, TidyView } from './Tidy';
import { ProjectSummaryView } from './ProjectSummary';
import i18n from '../../i18n';

const ref = (id: string, title: string, nature: CaseRef['nature'] = 'todo'): CaseRef => ({ id, title, nature, project_name: 'Humcase', date: '2026-09-28T10:00:00Z', excerpt: `${title}, en bref` });
const note = (id: string): Note => ({
  id, project_id: 'p', title: id, content: '', note_type: 'todo', status: 'open', resolution_comment: null,
  created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 1,
});
const base = { passage: null, other_passage: null, duplicate: false, doubt: false, checked: true, proposed_by: null, created_at: '2026-10-01T10:00:00Z' };
const conflict: TidyItem = {
  ...base, id: 'c1', kind: 'conflict', case: ref('task', 'Afficher le coût en dollars'), other: ref('rule', 'Jamais de prix affiché', 'decision'),
  passage: 'Montrer le coût de chaque appel en dollars.', other_passage: 'On n’affiche aucun prix.',
};
const group: TidyItem = { ...base, id: 'g1', kind: 'group', case: ref('ryokan', 'Réserver un ryokan'), other: ref('trip', 'Voyage au Japon') };
const duplicate: TidyItem = { ...base, id: 'g2', kind: 'group', case: ref('dup2', 'Export : clic sans effet', 'bug'), other: ref('dup1', 'Le bouton Exporter ne fait rien', 'bug'), duplicate: true, doubt: true, checked: false };
const notes = ['task', 'rule', 'ryokan', 'trip', 'dup1', 'dup2'].map(note);

describe('To tidy: filing together and checking against decisions', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('fr');
    api.tidyList.mockResolvedValue([conflict, group, duplicate]);
  });

  it('counts them on Home', () => {
    render(<TidyCard items={[conflict, group, duplicate]} weeks={4} onStart={vi.fn()} />);
    expect(screen.getByText(/1 case qui va peut-être contre une décision, 2 paires de cases à regrouper\. Rien n’est supprimé\./)).toBeTruthy();
  });

  it('cites both passages, waits for the person on a conflict, files the pairs, and opens the case to correct', async () => {
    const onNote = vi.fn();
    render(<TidyView notes={notes} onNote={onNote} onChanged={vi.fn()} />);
    expect(await screen.findByText('« Montrer le coût de chaque appel en dollars. »')).toBeTruthy();
    expect(screen.getByText('« On n’affiche aucun prix. »')).toBeTruthy();
    expect(screen.getByText(/Doublon probable/)).toBeTruthy();
    // A conflict, and a duplicate found by words alone, have no answer until the person gives one.
    expect(screen.getByRole('button', { name: 'Regrouper 1 case' })).toBeTruthy();
    fireEvent.click(screen.getByRole('radio', { name: 'Je corrige la case' }));
    // Swapped: the trip goes into the ryokan.
    fireEvent.click(screen.getAllByRole('button', { name: 'Inverser : ranger l’autre dans celle-ci' })[0]);
    fireEvent.click(screen.getByRole('button', { name: '1 à corriger · Regrouper 1 case' }));
    await waitFor(() => expect(api.answerTidy).toHaveBeenCalledWith([
      { id: 'c1', accept: true },
      { id: 'g1', accept: true, swap: true },
    ]));
    await waitFor(() => expect(onNote).toHaveBeenCalledWith(expect.objectContaining({ id: 'task' })));
  });

  it('refuses all, the conflict included', async () => {
    render(<TidyView notes={notes} onNote={vi.fn()} onChanged={vi.fn()} />);
    await screen.findByText('Voyage au Japon');
    fireEvent.click(screen.getByRole('button', { name: 'Tout refuser' }));
    await waitFor(() => expect(api.answerTidy).toHaveBeenCalledWith([
      { id: 'c1', accept: false }, { id: 'g1', accept: false }, { id: 'g2', accept: false },
    ]));
  });
});

describe('A project summary', () => {
  const summary: ProjectSummary = {
    project_id: 'p', project_name: 'Humcase',
    decisions: [{ ...ref('rule', 'Jamais de prix affiché', 'decision'), excerpt: 'On n’affiche aucun prix.' }],
    replaced_count: 2,
    in_progress: [ref('lot', 'Lot 5')],
    open_questions: [ref('q', 'Un profil par espace ?', 'question')],
    to_check: [conflict],
    recently_resolved: [],
  };
  beforeEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('fr');
  });

  it('shows what holds, what moves, what stays open, and leads to what to check', async () => {
    api.projectSummary.mockResolvedValue(summary);
    const onTidy = vi.fn();
    const onNote = vi.fn();
    render(<ProjectSummaryView projectId="p" notes={[note('rule')]} onNote={onNote} onTidy={onTidy} />);
    expect(await screen.findByText('On n’affiche aucun prix.')).toBeTruthy();
    expect(screen.getByText('2 remplacées, en historique')).toBeTruthy();
    expect(screen.getByText('Lot 5')).toBeTruthy();
    expect(screen.getByText('Un profil par espace ?')).toBeTruthy();
    expect(screen.getByText('Rien de résolu ces deux semaines.')).toBeTruthy();
    expect(screen.getByText('1 case va peut-être contre une décision en vigueur.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Revoir dans À ranger' }));
    expect(onTidy).toHaveBeenCalled();
    fireEvent.click(screen.getByText('Jamais de prix affiché'));
    expect(onNote).toHaveBeenCalledWith(expect.objectContaining({ id: 'rule' }));
  });

  it('says so when the project has nothing to sum up, in English too', async () => {
    api.projectSummary.mockResolvedValue({ ...summary, decisions: [], replaced_count: 0, in_progress: [], open_questions: [], to_check: [] });
    await i18n.changeLanguage('en');
    render(<ProjectSummaryView projectId="p" notes={[]} onNote={vi.fn()} onTidy={vi.fn()} />);
    expect(await screen.findByText('Nothing to sum up yet.')).toBeTruthy();
    await i18n.changeLanguage('fr');
  });
});

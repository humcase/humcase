import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { CaseRef, Note, Supersession, TidyItem } from '../../types';

const api = vi.hoisted(() => ({
  caseSupersession: vi.fn(),
  tidyCheckDecision: vi.fn(),
  answerTidy: vi.fn(async () => ({ archived: 0, superseded: 1, refused: 0 })),
  undoTidy: vi.fn(async () => undefined),
  keepDecisionInForce: vi.fn(async () => undefined),
  caseConflicts: vi.fn(async (): Promise<TidyItem[]> => []),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

import { DecisionHistory } from './DecisionHistory';
import i18n from '../../i18n';

const ref = (id: string, title: string): CaseRef => ({ id, title, nature: 'decision', project_name: 'Humcase', date: '2026-09-28T10:00:00Z', excerpt: '' });
const note = (id: string, title: string): Note => ({
  id, project_id: 'p', title, content: '', note_type: 'decision', status: 'open', resolution_comment: null,
  created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 1,
});
const none: Supersession = { superseded_by: null, superseded_at: null, replaces: [], asking: [] };
const asking: Supersession = { ...none, asking: [{ id: 'p1', kind: 'supersede', case: ref('old', 'Héberger sur Supabase'), other: ref('new', 'Abandonner Supabase'), passage: null, other_passage: null, duplicate: false, doubt: false, checked: true, proposed_by: null, created_at: '2026-10-01T10:00:00Z' }] };

describe('A decision in the history', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('fr');
  });

  it('shows nothing for a decision without history', async () => {
    api.caseSupersession.mockResolvedValue(none);
    const { container } = render(<DecisionHistory noteId="new" isDecision revision={1} allNotes={[]} onChanged={vi.fn()} />);
    await waitFor(() => expect(api.caseSupersession).toHaveBeenCalledWith('new'));
    expect(container.innerHTML).toBe('');
  });

  it('asks whether it replaces the older one, marks it on yes, and takes it back', async () => {
    api.caseSupersession.mockResolvedValueOnce(asking).mockResolvedValue({ ...none, replaces: [ref('old', 'Héberger sur Supabase')] });
    const onChanged = vi.fn();
    render(<DecisionHistory noteId="new" isDecision revision={1} allNotes={[note('old', 'Héberger sur Supabase')]} onOpen={vi.fn()} onChanged={onChanged} />);
    expect(await screen.findByText(/Elle remplace celle du/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Oui, elle la remplace' }));
    await waitFor(() => expect(api.answerTidy).toHaveBeenCalledWith([{ id: 'p1', accept: true }]));
    expect(onChanged).toHaveBeenCalled();
    expect(await screen.findByRole('status')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(api.undoTidy).toHaveBeenCalledWith(['p1']));
  });

  it('marks a replaced decision as history, with the way back', async () => {
    api.caseSupersession.mockResolvedValueOnce({ ...none, superseded_by: ref('new', 'Abandonner Supabase'), superseded_at: '2026-09-30T10:00:00Z' }).mockResolvedValue(none);
    const onOpen = vi.fn();
    render(<DecisionHistory noteId="old" isDecision revision={1} allNotes={[note('new', 'Abandonner Supabase')]} onOpen={onOpen} onChanged={vi.fn()} />);
    expect(await screen.findByText(/Historique : remplacée le/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Voir la décision en vigueur' }));
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: 'new' }));
    fireEvent.click(screen.getByRole('button', { name: 'Elle reste en vigueur' }));
    await waitFor(() => expect(api.keepDecisionInForce).toHaveBeenCalledWith('old'));
  });

  it('looks for what it replaces only after the decision is written while open', async () => {
    vi.useFakeTimers();
    try {
      api.caseSupersession.mockResolvedValue(none);
      api.tidyCheckDecision.mockResolvedValue(asking);
      const { rerender } = render(<DecisionHistory noteId="new" isDecision revision={1} allNotes={[]} onChanged={vi.fn()} />);
      await act(async () => { await vi.advanceTimersByTimeAsync(9000); });
      expect(api.tidyCheckDecision).not.toHaveBeenCalled();
      rerender(<DecisionHistory noteId="new" isDecision revision={2} allNotes={[]} onChanged={vi.fn()} />);
      await act(async () => { await vi.advanceTimersByTimeAsync(9000); });
      expect(api.tidyCheckDecision).toHaveBeenCalledWith('new');
    } finally {
      vi.useRealTimers();
    }
  });
  it('warns on any case that may go against a decision, cites both, and takes a refusal', async () => {
    api.caseSupersession.mockResolvedValue(none);
    const item: TidyItem = { id: 'c1', kind: 'conflict', case: { ...ref('task', 'Afficher le coût en dollars'), nature: 'todo' }, other: ref('rule', 'Jamais de prix affiché'),
      passage: 'Montrer le coût en dollars.', other_passage: 'On n’affiche aucun prix.', duplicate: false, doubt: false, checked: true, proposed_by: null, created_at: '2026-10-01T10:00:00Z' };
    api.caseConflicts.mockResolvedValueOnce([item]).mockResolvedValue([]);
    const onOpen = vi.fn();
    const onChanged = vi.fn();
    render(<DecisionHistory noteId="task" isDecision={false} revision={1} allNotes={[note('rule', 'Jamais de prix affiché')]} onOpen={onOpen} onChanged={onChanged} />);
    expect(await screen.findByText('Va peut-être contre la décision « Jamais de prix affiché »')).toBeTruthy();
    expect(screen.getByText('Ici : « Montrer le coût en dollars. » La décision : « On n’affiche aucun prix. »')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Voir la décision' }));
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: 'rule' }));
    fireEvent.click(screen.getByRole('button', { name: 'Ce n’est pas le cas' }));
    await waitFor(() => expect(api.answerTidy).toHaveBeenCalledWith([{ id: 'c1', accept: false }]));
    await waitFor(() => expect(screen.queryByText(/Va peut-être contre/)).toBeNull());
    expect(onChanged).toHaveBeenCalled();
  });
});

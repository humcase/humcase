import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NatureReviewCard, NatureReviewDialog } from './NatureReview';
import type { NatureProposal, Note } from '../../types';

const api = vi.hoisted(() => ({ answerNature: vi.fn() }));
vi.mock('../../services/tauriApi', () => ({ api }));

const proposal = (id: string, title: string, nature: NatureProposal['nature'], proposed: NatureProposal['proposed'], reasons: NatureProposal['reasons'] = [{ kind: 'reading', confidence: 0.97 }]): NatureProposal =>
  ({ note_id: id, title, project_id: 'p', revision: 1, nature, proposed, reasons, confidence: 0.97 });
const note = (id: string, title: string) => ({ id, title } as Note);

const proposals = [
  proposal('a', 'Décision — stack de l’offre en ligne', 'note', 'decision', [{ kind: 'reading', confidence: 0.99 }, { kind: 'wording', evidence: { kind: 'words', words: 'Décision' } }]),
  proposal('b', 'Corriger la boucle useEffect', 'todo', 'bug'),
  proposal('c', 'Postgres ou SQLite', 'note', 'question'),
];
const notes = [note('a', 'Décision — stack de l’offre en ligne'), note('b', 'Corriger la boucle useEffect'), note('c', 'Postgres ou SQLite')];

describe('Natures to check', () => {
  beforeEach(() => { api.answerNature.mockReset(); });

  it('shows the Home card only when there is something to check', () => {
    const { rerender } = render(<NatureReviewCard count={0} onStart={() => {}} />);
    expect(screen.queryByRole('button', { name: /Vérifier/ })).toBeNull();
    const start = vi.fn();
    rerender(<NatureReviewCard count={6} onStart={start} />);
    expect(screen.getByRole('heading', { name: /06 cases ont peut-être une autre nature/ })).toBeTruthy();
    expect(screen.getByText('Relues par Jev. Rien ne change sans vous.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Vérifier/ }));
    expect(start).toHaveBeenCalled();
  });

  it('changes with Enter, keeps with G, says why, and ends with the counts', async () => {
    api.answerNature.mockResolvedValue(true);
    const onClose = vi.fn();
    render(<NatureReviewDialog proposals={proposals} notes={notes} onNote={() => {}} onClose={onClose} />);
    expect(screen.getByText('01 / 03')).toBeTruthy();
    expect(screen.getByText(/Lue par Jev : Décision/)).toBeTruthy();
    expect(screen.getByText(/« Décision »/)).toBeTruthy();
    const dialog = screen.getByRole('dialog');

    fireEvent.keyDown(dialog.firstElementChild!, { key: 'Enter' });
    await waitFor(() => expect(api.answerNature).toHaveBeenLastCalledWith('a', 'note', 'decision', true));
    expect(await screen.findByText('02 / 03')).toBeTruthy();

    fireEvent.keyDown(dialog.firstElementChild!, { key: 'g' });
    await waitFor(() => expect(api.answerNature).toHaveBeenLastCalledWith('b', 'todo', 'bug', false));

    fireEvent.click(await screen.findByRole('button', { name: 'Garder Pensée' }));
    await waitFor(() => expect(api.answerNature).toHaveBeenLastCalledWith('c', 'note', 'question', false));

    expect(await screen.findByText('Terminé')).toBeTruthy();
    expect(screen.getByText('Changées').nextSibling?.textContent).toBe('01');
    expect(screen.getByText('Gardées').nextSibling?.textContent).toBe('02');
    fireEvent.click(screen.getAllByRole('button', { name: 'Fermer' }).at(-1)!);
    expect(onClose).toHaveBeenCalledWith(true);
  });

  it('applies nothing to a case changed meanwhile, and says so', async () => {
    api.answerNature.mockResolvedValueOnce(false);
    const onClose = vi.fn();
    render(<NatureReviewDialog proposals={[proposals[1]]} notes={notes} onNote={() => {}} onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'Changer en Incident' }));
    expect(await screen.findByText(/a changé entre-temps : rien n’a été modifié/)).toBeTruthy();
    expect(screen.getByText('Changées').nextSibling?.textContent).toBe('00');
    fireEvent.click(screen.getAllByRole('button', { name: 'Fermer' }).at(-1)!);
    expect(onClose).toHaveBeenCalledWith(false);
  });

  it('leaves Enter to a focused button: Keep stays Keep, never a change', () => {
    render(<NatureReviewDialog proposals={proposals} notes={notes} onNote={() => {}} onClose={() => {}} />);
    fireEvent.keyDown(screen.getByRole('button', { name: 'Garder Pensée' }), { key: 'Enter' });
    fireEvent.keyDown(screen.getAllByRole('button', { name: 'Fermer' })[0], { key: 'o' });
    expect(api.answerNature).not.toHaveBeenCalled();
  });

  it('opens the case with O, without answering', () => {
    const onNote = vi.fn();
    const onClose = vi.fn();
    render(<NatureReviewDialog proposals={proposals} notes={notes} onNote={onNote} onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole('dialog').firstElementChild!, { key: 'o' });
    expect(onNote).toHaveBeenCalledWith(notes[0]);
    expect(onClose).toHaveBeenCalledWith(false);
    expect(api.answerNature).not.toHaveBeenCalled();
  });
});

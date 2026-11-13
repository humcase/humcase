import { describe, expect, it, vi, beforeEach } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import type { Note, Proposal } from '../../types';
import { GerminatePanel } from './GerminatePanel';
import { useGermination, answerOf } from './useGermination';
import { PassageMarks, findPassage, passageKey } from './passageMarks';

const api = vi.hoisted(() => ({
  listProposals: vi.fn(),
  germinateCase: vi.fn(),
  answerProposals: vi.fn(),
  undoProposals: vi.fn(async () => undefined),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

const note = { id: 'case', project_id: 'p', title: 'Offre en ligne', content: 'texte', note_type: 'idea', status: 'open', revision: 3 } as Note;

const proposal = (id: string, extra: Partial<Proposal> = {}): Proposal => ({
  id, note_id: 'case', nature: 'todo', title: `Étape ${id}`, passage: `passage ${id}`, reason: null,
  proposed_by: null, near: null, doubt: false, created_at: '2026-10-04T10:00:00Z', ...extra,
});

const waiting = [
  proposal('a', { nature: 'decision', title: 'Choisir l’hébergement' }),
  proposal('b', { nature: 'research', near: { id: 'study', title: 'Étude comparative cloud', nature: 'research', place: 'movable', parent_title: null } }),
  proposal('c', { doubt: true, title: 'Rédiger une FAQ' }),
  proposal('d', { proposed_by: 'Claude Code', nature: 'research' }),
];

function Harness({ flush = vi.fn(async () => {}), updated = vi.fn() }: { flush?: () => Promise<void>; updated?: () => void }) {
  const germination = useGermination({ note, flushDraft: flush, onNoteUpdated: updated });
  return (
    <>
      <button type="button" onClick={() => { void germination.germinate(); }}>Faire germer</button>
      <GerminatePanel germination={germination} />
      <output data-testid="marks">{germination.marks.map(m => `${m.nature}:${m.passage}:${m.lit ? 'lit' : ''}`).join('|')}</output>
    </>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('Faire germer', () => {
  it('shows waiting proposals as a band, then each one with its passage, a doubt unchecked and a near case', async () => {
    api.listProposals.mockResolvedValue(waiting);
    render(<Harness />);
    expect(await screen.findByText('4 propositions en attente')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Reprendre' }));

    expect(screen.getByText('« passage a »')).toBeTruthy();
    const keep = (title: string) => screen.getByRole('checkbox', { name: `Garder la proposition : ${title}` }) as HTMLInputElement;
    expect(keep('Choisir l’hébergement').checked).toBe(true);
    expect(keep('Rédiger une FAQ').checked).toBe(false);
    expect(screen.getByText('À vérifier')).toBeTruthy();
    expect(screen.getByText('Proposée par Claude Code')).toBeTruthy();
    expect(screen.getByText('Étude comparative cloud')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Ranger ici' }).getAttribute('aria-pressed')).toBe('true');
    // Kept proposals mark their passage with their nature; the doubted one does not.
    expect(screen.getByTestId('marks').textContent).toBe('decision:passage a:|research:passage b:|research:passage d:');
    expect(screen.getByRole('button', { name: 'Créer 2 sous-cases · ranger 1 case' })).toBeTruthy();
  });

  it('sends what the person chose, then takes it back with Undo', async () => {
    api.listProposals.mockResolvedValueOnce(waiting).mockResolvedValue([]);
    api.answerProposals.mockResolvedValue({ created: ['x', 'y'], placed: [], skipped: 0, refused: 2 });
    const updated = vi.fn();
    render(<Harness updated={updated} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Reprendre' }));

    fireEvent.change(screen.getAllByLabelText('Titre de la sous-case')[0], { target: { value: 'Choisir l’hébergeur' } });
    expect(screen.getByText('modifiée')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Créer quand même' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Garder la proposition : Étape d' }));
    fireEvent.click(screen.getByRole('button', { name: 'Créer 2 sous-cases' }));

    await waitFor(() => expect(api.answerProposals).toHaveBeenCalledWith('case', [
      { id: 'a', choice: 'create', title: 'Choisir l’hébergeur' },
      { id: 'b', choice: 'create' },
      { id: 'c', choice: 'refuse' },
      { id: 'd', choice: 'refuse' },
    ]));
    expect(await screen.findByText('2 sous-cases créées · 2 refusées')).toBeTruthy();
    expect(updated).toHaveBeenCalled();

    api.listProposals.mockResolvedValue(waiting);
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(api.undoProposals).toHaveBeenCalledWith('case', ['a', 'b', 'c', 'd']));
    expect(await screen.findByRole('button', { name: 'Tout refuser' })).toBeTruthy();
  });

  it('saves the text, asks the AI chosen in Settings, and says what was set aside', async () => {
    api.listProposals.mockResolvedValueOnce([]).mockResolvedValue([waiting[0]]);
    api.germinateCase.mockResolvedValue({ proposals: [waiting[0]], dropped: 2, checked: true });
    const flush = vi.fn(async () => {});
    render(<Harness flush={flush} />);
    await waitFor(() => expect(api.listProposals).toHaveBeenCalled());
    fireEvent.click(screen.getByRole('button', { name: 'Faire germer' }));
    await waitFor(() => expect(api.germinateCase).toHaveBeenCalled());
    expect(flush).toHaveBeenCalled();
    expect(api.germinateCase.mock.calls[0][0]).toBe('case');
    expect(await screen.findByText(/2 propositions écartées/)).toBeTruthy();
    expect(screen.getByText(/Contrôlées par Jev/)).toBeTruthy();
    // Refuse all, then the button records the refusals.
    fireEvent.click(screen.getByRole('button', { name: 'Tout refuser' }));
    expect(screen.getByRole('button', { name: 'Enregistrer les refus' })).toBeTruthy();
  });

  it('shows an AI failure without losing the proposals already waiting', async () => {
    api.listProposals.mockResolvedValue([]);
    api.germinateCase.mockRejectedValue('[ai_key_missing] Clé manquante');
    render(<Harness />);
    await waitFor(() => expect(api.listProposals).toHaveBeenCalled());
    fireEvent.click(screen.getByRole('button', { name: 'Faire germer' }));
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain('Clé manquante');
    expect(screen.getByRole('alert').textContent).not.toContain('ai_key_missing');
  });

  it('answers a near case by placing it, skipping it or creating anyway', () => {
    const near = proposal('n', { near: { id: 'x', title: 'X', nature: 'todo', place: 'elsewhere', parent_title: 'Y' } });
    const base = { keep: true, nature: 'todo' as const, title: 'Étape n' };
    expect(answerOf(near, { ...base, nearChoice: 'skip' })).toEqual({ id: 'n', choice: 'skip' });
    expect(answerOf(near, { ...base, nearChoice: 'create', nature: 'question' })).toEqual({ id: 'n', choice: 'create', nature: 'question' });
    expect(answerOf(near, { ...base, keep: false, nearChoice: 'create' })).toEqual({ id: 'n', choice: 'refuse' });
  });
});

describe('Passages marked in the text', () => {
  it('finds a passage as the Rust side checks it and marks it without touching the document', () => {
    const editor = new Editor({
      extensions: [StarterKit, PassageMarks],
      content: '<p>Je voudrais Humcase en ligne.</p><p>Il faut choisir l’hébergement : <strong>Supabase</strong> ou Cloudflare, je n’ai pas tranché.</p>',
    });
    const range = findPassage(editor.state.doc, "« il faut choisir l'hébergement : Supabase ou Cloudflare »");
    expect(range).not.toBeNull();
    expect(editor.state.doc.textBetween(range!.from, range!.to)).toBe('Il faut choisir l’hébergement : Supabase ou Cloudflare');
    expect(findPassage(editor.state.doc, 'Il faut choisir un hébergeur')).toBeNull();

    const html = editor.getHTML();
    act(() => { editor.view.dispatch(editor.state.tr.setMeta(passageKey, [{ passage: 'Je voudrais Humcase en ligne', nature: 'idea', lit: true }])); });
    expect(editor.view.dom.querySelector('.passage-mark--lit[data-type="idea"]')?.textContent).toBe('Je voudrais Humcase en ligne');
    expect(editor.getHTML()).toBe(html);
    editor.destroy();
  });
});

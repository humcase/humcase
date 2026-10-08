import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { Note } from '../../types';
import type { ReflectEngine, ReflectReply, ReflectSummary, ReflectTurn } from '../../types/reflect';
import { reflectPreviousTurns } from '../../types/reflect';
import { ReflectThread } from './ReflectThread';
import { reflectSummaryMarkdown } from './reflectSummary';
import i18n from '../../i18n';

vi.mock('./useComposerDictation', () => ({
  useComposerDictation: () => ({ listening: false, starting: false, interim: '', error: null, start: vi.fn(), stop: vi.fn(async () => '') }),
}));

const gift = { id: 'g', title: 'Des cartes cadeaux pour les fêtes' } as Note;
const shop = { id: 's', title: 'Refonte de la boutique' } as Note;

const opening: ReflectReply = {
  restated: 'Vous voulez des cartes cadeaux de 20 à 100 €.',
  links: [],
  question: 'Pourquoi pour les fêtes ?',
};
const second: ReflectReply = {
  restated: 'Le but : faire venir de nouveaux clients.',
  links: [{ kind: 'conflicts', source: { note_id: 's', title: shop.title, passages: ['Maquettes le 14, développement en novembre'] } }],
  lead: 'Une version simple d’abord.',
  question: 'Une version simple suffirait-elle ?',
  offer_summary: true,
};
const summary: ReflectSummary = {
  title: 'Réflexion : cartes cadeaux',
  direction: 'Trois montants fixes avant les fêtes.',
  kept: ['Faire venir de nouveaux clients.'],
  rejected: ['Retarder la mise en ligne.'],
  evolution: [],
  questions: ['Quelle solution de paiement ?'],
  actions: [{ title: 'Créer les trois cartes', create: true }, { title: 'Montant libre', create: false }],
  waiting_on: [{ note_id: 's', title: shop.title }],
};

function thread(engine: ReflectEngine, subject: { id: string; title: string } | null = gift) {
  const props = {
    engine, notes: [gift, shop], subject, candidate: null,
    onStart: vi.fn(), onOpenNote: vi.fn(), onKeep: vi.fn(async () => 'kept'), onClose: vi.fn(),
  };
  render(<ReflectThread {...props} />);
  return props;
}

describe('ReflectThread', () => {
  beforeEach(async () => { await i18n.changeLanguage('fr'); });

  it('opens on the subject with a question, then answers what the person says with its links', async () => {
    const reply = vi.fn<ReflectEngine['reply']>().mockResolvedValueOnce(opening).mockResolvedValueOnce(second);
    const props = thread({ reply, summarize: vi.fn() });
    expect(await screen.findByText('Pourquoi pour les fêtes ?')).toBeTruthy();
    expect(reply).toHaveBeenCalledWith(gift, []);

    const box = screen.getByRole('textbox');
    fireEvent.change(box, { target: { value: 'Pour vendre plus.' } });
    fireEvent.keyDown(box, { key: 'Enter' });
    expect(await screen.findByText('Une version simple suffirait-elle ?')).toBeTruthy();
    const turns = reply.mock.calls[1][1] as ReflectTurn[];
    expect(turns.map(turn => turn.said ?? turn.reply?.question)).toEqual(['Pourquoi pour les fêtes ?', 'Pour vendre plus.']);
    expect(screen.getByText('À concilier')).toBeTruthy();
    expect(screen.getByText('Une version simple d’abord.')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: shop.title }));
    expect(props.onOpenNote).toHaveBeenCalledWith(shop);
  });

  it('keeps the summary as the person corrected it, nothing before the click', async () => {
    const reply = vi.fn<ReflectEngine['reply']>().mockResolvedValueOnce(opening).mockResolvedValueOnce(second);
    const props = thread({ reply, summarize: vi.fn(async () => summary) });
    await screen.findByText('Pourquoi pour les fêtes ?');
    const box = screen.getByRole('textbox');
    fireEvent.change(box, { target: { value: 'Pour vendre plus.' } });
    fireEvent.keyDown(box, { key: 'Enter' });
    fireEvent.click(await screen.findByRole('button', { name: 'Proposer la synthèse' }));

    const direction = await screen.findByDisplayValue('Trois montants fixes avant les fêtes.');
    expect(props.onKeep).not.toHaveBeenCalled();
    fireEvent.change(direction, { target: { value: 'Deux montants fixes.' } });
    fireEvent.click(screen.getAllByRole('button', { name: 'Retirer' })[0]);
    fireEvent.click(screen.getByRole('button', { name: 'Garder, avec 1 sous-case' }));

    await waitFor(() => expect(props.onKeep).toHaveBeenCalledTimes(1));
    const kept = props.onKeep.mock.calls[0] as unknown as [unknown, ReflectSummary];
    expect(kept[1].direction).toBe('Deux montants fixes.');
    expect(kept[1].kept).toEqual([]);
    expect(await screen.findByRole('button', { name: 'Synthèse gardée : l’ouvrir' })).toBeTruthy();
  });
});

describe('reflection text', () => {
  it('sends the previous turns as plain text with the cases they linked', () => {
    const turns: ReflectTurn[] = [{ id: 1, reply: opening }, { id: 2, said: 'Pour vendre plus.' }, { id: 3, reply: second }];
    expect(reflectPreviousTurns(turns)).toEqual([
      { reply: 'Vous voulez des cartes cadeaux de 20 à 100 €. Pourquoi pour les fêtes ?', source_ids: [] },
      { said: 'Pour vendre plus.', source_ids: [] },
      { reply: 'Le but : faire venir de nouveaux clients. Une version simple d’abord. Une version simple suffirait-elle ?', source_ids: ['s'] },
    ]);
  });

  it('keeps the whole conversation below the summary, empty sections left out', () => {
    const text = reflectSummaryMarkdown(summary, [{ id: 1, reply: opening }, { id: 2, said: 'Pour vendre plus.' }], key => key);
    expect(text).toContain('### section.direction\nTrois montants fixes avant les fêtes.');
    expect(text).toContain('### section.rejected\n- Retarder la mise en ligne.');
    expect(text).not.toContain('section.evolution');
    expect(text).toContain('**you** : Pour vendre plus.');
    expect(text).toContain('**partner** : Vous voulez des cartes cadeaux de 20 à 100 €. Pourquoi pour les fêtes ?');
  });
});

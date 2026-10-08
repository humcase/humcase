import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { Note } from '../../types';
import type { AssistantAnswer } from '../../types/assistant';
import { previousTurn } from '../../types/assistant';
import { AssistantPanel } from './AssistantPanel';
import { keptAnswerMarkdown } from './keptAnswer';
import i18n from '../../i18n';

const checkout = { id: 'a', title: 'Paiement sur une seule page' } as Note;
const providers = { id: 'b', title: 'Comparer les solutions de paiement' } as Note;

const answer: AssistantAnswer = {
  kind: 'answer',
  parts: [{ text: 'Le paiement se fera sur une seule page.', cite: 1 }, { text: ' Trois solutions conviennent.', cite: 2 }],
  sources: [
    { note_id: 'a', title: checkout.title, passages: ['Adresse, livraison et paiement sur une seule page.'] },
    { note_id: 'b', title: providers.title, passages: ['Les trois permettent un paiement sur une page.', 'Proposition : Stripe.'] },
  ],
  action: { kind: 'create_sub_case', title: 'Choisir le prestataire', parent_id: 'b', parent_title: providers.title },
  sent: { service: 'Cerebras', excerpts: 2 },
};

function panel(engine: (q: string, previous: unknown[]) => Promise<AssistantAnswer>, extra: Partial<Parameters<typeof AssistantPanel>[0]> = {}) {
  const props = {
    engine,
    notes: [checkout, providers],
    scopeLabel: 'Espace pro',
    onOpenNote: vi.fn(),
    onKeep: vi.fn(async () => 'kept'),
    onCreateSubCase: vi.fn(async () => 'sub'),
    onConnectAi: vi.fn(),
    onClose: vi.fn(),
    ...extra,
  };
  render(<AssistantPanel {...props} />);
  return props;
}

async function ask(question: string) {
  const box = screen.getByRole('textbox');
  fireEvent.change(box, { target: { value: question } });
  fireEvent.keyDown(box, { key: 'Enter' });
}

describe('Ask Humcase', () => {
  beforeEach(async () => { await i18n.changeLanguage('fr'); });

  it('shows each source with its passage and opens the case of a citation', async () => {
    const props = panel(async () => answer);
    await ask('Qu’avait-on décidé pour le paiement ?');
    await screen.findByText(/Le paiement se fera/);
    expect(screen.getByText('Adresse, livraison et paiement sur une seule page.')).toBeTruthy();
    expect(screen.getByText('Envoyé à Cerebras : des extraits de 2 cases.')).toBeTruthy();
    fireEvent.click(screen.getAllByRole('button', { name: 'Ouvrir la case Paiement sur une seule page' })[0]);
    expect(props.onOpenNote).toHaveBeenCalledWith(checkout);
  });

  it('sends the previous exchanges with a follow-up, as text and cited cases', async () => {
    const engine = vi.fn(async () => answer);
    panel(engine);
    await ask('Qu’avait-on décidé pour le paiement ?');
    await screen.findByText(/Le paiement se fera/);
    await ask('Et pourquoi pas PayPal ?');
    await waitFor(() => expect(engine).toHaveBeenCalledTimes(2));
    const previous = (engine.mock.calls[1] as unknown as [string, Parameters<typeof previousTurn>[0][]])[1];
    expect(previous.map(previousTurn)).toEqual([{
      question: 'Qu’avait-on décidé pour le paiement ?',
      answer: 'Le paiement se fera sur une seule page. Trois solutions conviennent.',
      source_ids: ['a', 'b'],
    }]);
  });

  it('creates the proposed sub-case only on click', async () => {
    const props = panel(async () => answer);
    await ask('paiement');
    await screen.findByText(/Le paiement se fera/);
    expect(props.onCreateSubCase).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Créer' }));
    await waitFor(() => expect(props.onCreateSubCase).toHaveBeenCalledWith('b', 'Choisir le prestataire'));
  });

  it('without an AI, lists the closest cases and offers to connect one', async () => {
    const props = panel(async () => ({ kind: 'no_ai', parts: [], sources: answer.sources }));
    await ask('paiement');
    await screen.findByText('Cases les plus proches');
    expect(screen.queryByText('Sources')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Brancher une IA' }));
    expect(props.onConnectAi).toHaveBeenCalled();
  });

  it('keeps an answer as a case with its sources in the language of the interface', () => {
    const text = keptAnswerMarkdown(answer, 'Sources', (title, passage) => `${title} : « ${passage} »`);
    expect(text).toBe([
      'Le paiement se fera sur une seule page. [1] Trois solutions conviennent. [2]',
      '### Sources\n1. Paiement sur une seule page : « Adresse, livraison et paiement sur une seule page. »\n2. Comparer les solutions de paiement : « Les trois permettent un paiement sur une page. … Proposition : Stripe. »',
    ].join('\n\n'));
  });
});

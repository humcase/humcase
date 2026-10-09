import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useState } from 'react';
import type { AssistantReply } from '../../types/assistant';
import { previousTurnsV2 } from '../../types/assistant';
import { AssistantWindow, type AssistantKickoff, type CaseRef } from './AssistantWindow';
import { keptConversationMarkdown } from './keptConversation';
import { clampRect, defaultRect, snapTarget, TOP_BAR, MARGIN } from './useFloatingWindow';
import i18n from '../../i18n';

const api = vi.hoisted(() => ({
  assistantReply: vi.fn(),
  assistantStatus: vi.fn(),
  createSubCase: vi.fn(async () => 'new-task'),
  deleteNote: vi.fn(async () => undefined),
  setNoteArchived: vi.fn(async () => undefined),
  moveNoteToProject: vi.fn(async () => undefined),
  answerTidy: vi.fn(async () => ({ archived: 0, superseded: 1, grouped: 0, to_fix: 0, refused: 0, skipped: 0 })),
  undoTidy: vi.fn(async () => undefined),
  keepAnswer: vi.fn(async () => 'kept-case'),
}));
vi.mock('../../services/tauriApi', () => ({ api }));
vi.mock('./useComposerDictation', () => ({
  useComposerDictation: () => ({ listening: false, starting: false, interim: '', error: null, start: vi.fn(), stop: vi.fn(async () => '') }),
}));

const reply = (extra: Partial<AssistantReply> = {}): AssistantReply => ({
  kind: 'reply',
  parts: [{ text: 'Le paiement se fera sur une seule page.', cite: 1 }, { text: ' Rangez-la depuis la case.', cite: 2 }],
  question: 'Voulez-vous garder Stripe ?',
  sources: [
    { note_id: 'checkout', title: 'Paiement sur une seule page', passages: ['Adresse et paiement sur une page.'], help: false },
    { note_id: null, title: 'Ranger une case', passages: ['Ranger dans un projet.'], help: true },
  ],
  actions: [],
  model_label: 'Claude Haiku 5.5',
  dropped: 0,
  ...extra,
});

function Harness({ initialOpen = true, kickoff = null, openCase = null, props = {} }: { initialOpen?: boolean; kickoff?: AssistantKickoff | null; openCase?: CaseRef | null; props?: Record<string, unknown> }) {
  const [open, setOpen] = useState(initialOpen);
  return (
    <div className="workspace-app">
      <button type="button" onClick={() => setOpen(value => !value)}>toggle</button>
      <AssistantWindow open={open} onOpenChange={setOpen} space="work" scopeLabel="Pro" openCase={openCase} kickoff={kickoff}
        keepSpace="work" onOpenCase={vi.fn()} onChanged={vi.fn()} onConnectAi={vi.fn()} {...props} />
    </div>
  );
}

async function say(text: string) {
  const box = screen.getByRole('textbox');
  fireEvent.change(box, { target: { value: text } });
  fireEvent.keyDown(box, { key: 'Enter' });
}

describe('Assistant window', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('fr');
    vi.clearAllMocks();
    try { localStorage.clear(); } catch { /* no storage */ }
    api.assistantStatus.mockResolvedValue({ model_label: 'Claude Haiku 5.5', cases: 142, to_decide: 3, to_tidy: 2 });
  });

  it('shows the small screen from the status, the AI ready and four suggestions', async () => {
    render(<Harness />);
    expect(await screen.findByText('142 cases')).toBeTruthy();
    expect(screen.getByText('À trancher').nextSibling?.textContent).toBe('3');
    expect(screen.getByText('À ranger').nextSibling?.textContent).toBe('2');
    expect(screen.getByText('Claude Haiku 5.5 · prêt')).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Que voulez-vous faire\u00a0?' })).toBeTruthy();
    expect(screen.getByText('Rien ne change sans votre clic.')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Résume ma semaine/ })).toBeTruthy();
    expect(screen.getAllByRole('button').filter(button => button.className === 'assistant-window__suggestion')).toHaveLength(4);
    expect(api.assistantStatus).toHaveBeenCalledWith('work', expect.any(String));
  });

  it('sends what the person says and shows the reply: citations, sources, the question', async () => {
    api.assistantReply.mockResolvedValue(reply());
    const onOpenCase = vi.fn();
    render(<Harness openCase={{ id: 'open', title: 'Mise en public' }} props={{ onOpenCase }} />);
    await say('Où en est le paiement ?');

    expect(await screen.findByText('Voulez-vous garder Stripe ?')).toBeTruthy();
    expect(screen.getByText('Où en est le paiement ?').className).toBe('assistant-window__said');
    expect(api.assistantReply).toHaveBeenCalledWith('Où en est le paiement ?', [], 'work', 'open', expect.any(String), undefined);
    // A case cited opens; a help section is cited but not clickable.
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir la case Paiement sur une seule page' }));
    expect(onOpenCase).toHaveBeenCalledWith('checkout');
    expect(screen.getByText('Sources · 2')).toBeTruthy();
    expect(screen.getByText('Aide de Humcase · Ranger une case').tagName).toBe('SPAN');
    expect(screen.getByText('Adresse et paiement sur une page.')).toBeTruthy();

    // The next message carries the exchange before it, help sources left out.
    api.assistantReply.mockResolvedValue(reply({ question: null }));
    await say('Et la livraison ?');
    await waitFor(() => expect(api.assistantReply).toHaveBeenCalledTimes(2));
    expect(api.assistantReply.mock.calls[1][1]).toEqual([{
      said: 'Où en est le paiement ?',
      reply: 'Le paiement se fera sur une seule page. Rangez-la depuis la case. Voulez-vous garder Stripe ?',
      source_ids: ['checkout'],
    }]);
  });

  it('the context chip removed, no case is sent', async () => {
    api.assistantReply.mockResolvedValue(reply());
    render(<Harness openCase={{ id: 'open', title: 'Mise en public' }} />);
    expect(screen.getByText('Mise en public')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Retirer du contexte' }));
    expect(screen.queryByText('Mise en public')).toBeNull();
    await say('Bonjour');
    await waitFor(() => expect(api.assistantReply).toHaveBeenCalled());
    expect(api.assistantReply.mock.calls[0][3]).toBeUndefined();
  });

  it('creates the proposed sub-case on click, and Undo puts it in the trash', async () => {
    api.assistantReply.mockResolvedValue(reply({ actions: [{ kind: 'create_sub_case', case_id: 'release', case_title: 'Mise en public', title: 'Ouvrir le dépôt' }] }));
    const onChanged = vi.fn();
    render(<Harness props={{ onChanged }} />);
    await say('Que reste-t-il ?');
    expect(await screen.findByText('Créer la tâche « Ouvrir le dépôt » dans « Mise en public ».')).toBeTruthy();
    expect(api.createSubCase).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Créer la sous-case' }));
    expect(await screen.findByText('Sous-case « Ouvrir le dépôt » créée dans « Mise en public ».')).toBeTruthy();
    expect(api.createSubCase).toHaveBeenCalledWith('release', 'Ouvrir le dépôt', 'todo');
    expect(onChanged).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(api.deleteNote).toHaveBeenCalledWith('new-task'));
    // Undone: the proposal is back, to answer again.
    expect(await screen.findByRole('button', { name: 'Créer la sous-case' })).toBeTruthy();
    expect(onChanged).toHaveBeenCalledTimes(2);
  });

  it('answers a "To tidy" item with its two passages, and Undo takes the answer back', async () => {
    api.assistantReply.mockResolvedValue(reply({
      actions: [{
        kind: 'tidy', tidy_id: 't1', tidy_kind: 'supersede', case_id: 'old', case_title: 'Distribution Windows', other_id: 'new', other_title: 'Publication des versions',
        passage: 'Le téléchargement passe par le site.', other_passage: 'L’installeur vient de GitHub Releases.',
      }],
    }));
    render(<Harness />);
    await say('Y a-t-il des contradictions ?');
    expect(await screen.findByText('Le téléchargement passe par le site.')).toBeTruthy();
    expect(screen.getByText('L’installeur vient de GitHub Releases.').closest('[data-newer]')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Marquer l’ancienne comme remplacée' }));
    expect(await screen.findByText('« Distribution Windows » marquée remplacée par « Publication des versions ».')).toBeTruthy();
    expect(api.answerTidy).toHaveBeenCalledWith([{ id: 't1', accept: true }]);

    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(api.undoTidy).toHaveBeenCalledWith(['t1']));
  });

  it('archives and files a case only on click, each undone with the opposite call', async () => {
    api.assistantReply.mockResolvedValue(reply({
      actions: [
        { kind: 'archive', case_id: 'a', case_title: 'Ancienne piste' },
        { kind: 'move', case_id: 'm', case_title: 'Réflexion audit', project_id: 'humcase', project_name: 'Humcase', from_project_id: 'inbox' },
      ],
    }));
    render(<Harness />);
    await say('À ranger ?');
    fireEvent.click(await screen.findByRole('button', { name: 'Archiver' }));
    await waitFor(() => expect(api.setNoteArchived).toHaveBeenCalledWith('a', true));
    fireEvent.click(screen.getByRole('button', { name: 'Ranger dans Humcase' }));
    await waitFor(() => expect(api.moveNoteToProject).toHaveBeenCalledWith('m', 'humcase'));
    const done = await screen.findByText('« Réflexion audit » rangée dans Humcase.');
    fireEvent.click(within(done.parentElement!).getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(api.moveNoteToProject).toHaveBeenCalledWith('m', 'inbox'));
  });

  it('a kickoff from a case sends an empty message with the case and shows no bubble', async () => {
    api.assistantReply.mockResolvedValue(reply({ parts: [{ text: 'Vous voulez ouvrir le dépôt.', cite: null }], sources: [], question: 'Qu’est-ce qui bloque ?' }));
    const { container } = render(<Harness kickoff={{ key: 1, case: { id: 'release', title: 'Mise en public' } }} openCase={{ id: 'release', title: 'Mise en public' }} />);
    expect(await screen.findByText('Qu’est-ce qui bloque ?')).toBeTruthy();
    expect(api.assistantReply).toHaveBeenCalledWith('', [], 'work', 'release', expect.any(String), undefined);
    expect(container.querySelector('.assistant-window__said')).toBeNull();
  });

  it('says when no AI is connected and offers to connect one', async () => {
    api.assistantStatus.mockResolvedValue({ model_label: null, cases: 4, to_decide: 0, to_tidy: 0 });
    api.assistantReply.mockResolvedValue(reply({ kind: 'no_ai', parts: [], sources: [], question: null, model_label: null }));
    const onConnectAi = vi.fn();
    render(<Harness props={{ onConnectAi }} />);
    expect(await screen.findByText('Aucune IA branchée')).toBeTruthy();
    await say('Bonjour');
    fireEvent.click(await screen.findByRole('button', { name: 'Brancher une IA' }));
    expect(onConnectAi).toHaveBeenCalled();
  });

  it('shows an engine error inline', async () => {
    api.assistantReply.mockRejectedValue(new Error('délai dépassé'));
    render(<Harness />);
    await say('Bonjour');
    expect((await screen.findByRole('alert')).textContent).toBe('Pas de réponse\u00a0: délai dépassé');
  });

  it('minimises to the key, lights it when a reply arrives meanwhile, and reopens from it', async () => {
    let answer: (value: AssistantReply) => void = () => {};
    api.assistantReply.mockReturnValue(new Promise<AssistantReply>(resolve => { answer = resolve; }));
    render(<Harness />);
    await say('Résume ma semaine');
    fireEvent.click(screen.getByRole('button', { name: 'Réduire' }));
    expect(screen.queryByRole('region', { name: 'Assistant' })).toBeNull();
    const key = screen.getByRole('button', { name: 'Assistant (Ctrl+J)' });
    expect(screen.queryByLabelText('Nouvelle réponse')).toBeNull();

    answer(reply());
    expect(await screen.findByLabelText('Nouvelle réponse')).toBeTruthy();
    fireEvent.click(key);
    // The conversation was kept while minimised.
    expect(await screen.findByText('Voulez-vous garder Stripe ?')).toBeTruthy();
    expect(screen.getByText('Résume ma semaine').className).toBe('assistant-window__said');
    expect(screen.queryByLabelText('Nouvelle réponse')).toBeNull();

    // Close does the same as minimise.
    fireEvent.click(screen.getByRole('button', { name: 'Fermer' }));
    expect(screen.getByRole('button', { name: 'Assistant (Ctrl+J)' })).toBeTruthy();
  });
});

describe('Assistant window: place and kept conversation', () => {
  it('opens top right under the top bar, stays in the viewport and snaps to the edges', () => {
    const start = defaultRect(1440, 900);
    expect(start).toEqual({ x: 1440 - 420 - MARGIN, y: TOP_BAR + MARGIN, width: 420, height: 640 });
    expect(clampRect({ x: -50, y: 0, width: 200, height: 2000 }, 1440, 900)).toEqual({ x: 0, y: TOP_BAR, width: 340, height: 900 - TOP_BAR - 2 * MARGIN });
    expect(snapTarget({ x: 30, y: 300, width: 420, height: 500 }, 1440, 900)).toEqual({ x: MARGIN, y: 300 });
    expect(snapTarget({ x: 400, y: 300, width: 420, height: 500 }, 1440, 900)).toBeNull();
  });

  it('keeps the conversation as text with its sources, numbered once', () => {
    const entries = [{ id: 1, said: 'Où en est le paiement ?', reply: reply() }];
    const markdown = keptConversationMarkdown(entries, { you: 'Vous', assistant: 'Assistant', sources: 'Sources', source: (title, passage) => `${title} : « ${passage} »` });
    expect(markdown).toBe([
      '**Vous:** Où en est le paiement ?',
      '**Assistant:** Le paiement se fera sur une seule page. [1] Rangez-la depuis la case. Voulez-vous garder Stripe ?',
      '### Sources\n1. Paiement sur une seule page : « Adresse et paiement sur une page. »',
    ].join('\n\n'));
    expect(previousTurnsV2(entries)[0].source_ids).toEqual(['checkout']);
  });
});

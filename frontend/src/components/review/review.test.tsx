import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import i18n from '../../i18n';
import type { Brief, CaseEvent, CaseJournal, CaseReview, Note, Project } from '../../types';
import { HomeBrief } from '../workspace/HomeBrief';
import { CaseReviewBand } from './CaseReviewBand';
import { CaseActivity } from './CaseActivity';
import { describeEvent } from './journalText';

vi.mock('../../services/tauriApi', () => ({
  api: { reviewCase: vi.fn(), markBriefSeen: vi.fn() },
}));

import { api } from '../../services/tauriApi';
const mockApi = vi.mocked(api);

const note = (over: Partial<Note> = {}): Note => ({
  id: 'n1', project_id: 'p1', title: 'Code signing options', content: '', note_type: 'research', status: 'resolved',
  resolution_comment: 'SignPath vs OV certificate', created_at: '2026-10-01T08:00:00Z', updated_at: '2026-10-02T09:12:00Z', revision: 7,
  ...over,
});

const event = (over: Partial<CaseEvent> = {}): CaseEvent => ({
  id: 'e1', note_id: 'n1', kind: 'status', actor_kind: 'agent', actor_name: 'Claude Code', detail: { from: 'in_progress', to: 'resolved' },
  fields: [], created_at: '2026-10-02T09:12:00Z', last_at: '2026-10-02T09:12:00Z', ...over,
});

const review = (over: Partial<CaseReview> = {}): CaseReview => ({
  id: 'r1', note_id: 'n1', decision: 'rework_requested', revision: 4, comment: 'Missing the yearly cost', rework_number: 1, created_at: '2026-09-30T19:05:00Z', ...over,
});

const brief = (over: Partial<Brief> = {}): Brief => ({
  since: '2026-10-01T16:42:00Z', since_is_fallback: false, to_review: [], changes: [], reworks_pending: [], ...over,
});

beforeEach(() => {
  vi.clearAllMocks();
  void i18n.changeLanguage('fr');
});

describe('describeEvent', () => {
  it('names the agent or addresses the person', () => {
    expect(describeEvent(event(), i18n.t)).toBe('Claude Code a passé le statut de En cours à Résolu');
    expect(describeEvent(event({ kind: 'edited', actor_kind: 'human', actor_name: null, fields: ['content', 'title'], detail: {} }), i18n.t))
      .toBe('Vous avez modifié : contenu, titre');
    expect(describeEvent(event({ kind: 'created', actor_kind: 'cli', actor_name: null, detail: {} }), i18n.t)).toBe('La ligne de commande a créé la case');
  });
});

describe('HomeBrief', () => {
  it('lists results to review, agent writes and reworks of visible cases only', () => {
    const visible = [note(), note({ id: 'n2', title: 'Landing page', status: 'open' }), note({ id: 'n3', title: 'Working profile', status: 'open' }),
      note({ id: 'n4', title: 'Page mockup', parent_id: 'n2' })];
    const projects = [{ id: 'p1', name: 'Distribution', color: '#3b82f6' } as Project];
    render(<HomeBrief notes={visible} projects={projects} failed={false} onNote={vi.fn()} onReviewed={vi.fn()} onAcceptedAll={vi.fn()} brief={brief({
      to_review: [{ note_id: 'n1', rework_count: 1, declared: event() }, { note_id: 'n4', rework_count: 0, declared: event({ note_id: 'n4' }) }],
      changes: [
        event({ id: 'e4', note_id: 'sub1', kind: 'created', detail: { parent_id: 'n2' } }),
        event({ id: 'e5', note_id: 'sub2', kind: 'created', detail: { parent_id: 'n2' } }),
        event({ id: 'e2', note_id: 'n2', kind: 'created', detail: {} }),
        event({ id: 'e3', note_id: 'hidden', kind: 'created', detail: {} }),
      ],
      reworks_pending: [{ note_id: 'n3', rework_count: 1, last_review: review({ note_id: 'n3', comment: 'Too generic' }) }],
    })} />);
    expect(screen.getByRole('heading', { name: 'Depuis votre dernière visite' })).toBeTruthy();
    // Each case says where it is: its project, and the path of its parent cases.
    expect(screen.getAllByText('Distribution').length).toBeGreaterThan(0);
    // A sub-case an agent declared done is to review too, with its path.
    expect(screen.getByRole('button', { name: 'Ouvrir « Page mockup »' })).toBeTruthy();
    expect(screen.getByText('Distribution › Landing page')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Ouvrir « Code signing options »' })).toBeTruthy();
    expect(screen.getByText('Reprise n°1 rendue')).toBeTruthy();
    expect(screen.getByText('Claude Code a créé la case')).toBeTruthy();
    expect(screen.getByText('« Too generic »')).toBeTruthy();
    // Sub-cases an agent created are counted under their visible parent, not listed alone.
    expect(screen.getByText(/Claude Code a créé 2 sous-cases/)).toBeTruthy();
    // A case outside the visible space is not listed.
    expect(screen.getAllByText(/a créé la case/)).toHaveLength(1);
  });

  it('says so when nothing came from agents, and states the period', () => {
    render(<HomeBrief notes={[note()]} failed={false} onNote={vi.fn()} onReviewed={vi.fn()} onAcceptedAll={vi.fn()} brief={brief({ since_is_fallback: true })} />);
    expect(screen.getByText('Rien de nouveau de la part des agents.')).toBeTruthy();
    expect(screen.getByText(/Dernières 24 heures/)).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Tout accepter' })).toBeNull();
  });

  it('accepts every listed result after a confirmation, and says which changed since', async () => {
    const onAcceptedAll = vi.fn();
    mockApi.reviewCase.mockResolvedValueOnce(review({ decision: 'accepted' })).mockRejectedValueOnce('changed');
    mockApi.markBriefSeen.mockResolvedValue('2026-10-03T15:00:00Z');
    render(<HomeBrief notes={[note(), note({ id: 'n2', title: 'Landing page', revision: 3 })]} failed={false} onNote={vi.fn()} onReviewed={vi.fn()} onAcceptedAll={onAcceptedAll}
      brief={brief({ to_review: [{ note_id: 'n1', rework_count: 0, declared: event() }, { note_id: 'n2', rework_count: 0, declared: event({ note_id: 'n2' }) }] })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Tout accepter' }));
    // Nothing is decided before the confirmation.
    expect(mockApi.reviewCase).not.toHaveBeenCalled();
    expect(screen.getByRole('heading', { name: 'Accepter les 2 résultats à examiner ?' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Accepter les 2' }));
    await waitFor(() => expect(onAcceptedAll).toHaveBeenCalled());
    expect(mockApi.reviewCase).toHaveBeenNthCalledWith(1, 'n1', 'accepted', 7);
    expect(mockApi.reviewCase).toHaveBeenNthCalledWith(2, 'n2', 'accepted', 3);
    expect(mockApi.markBriefSeen).toHaveBeenCalled();
    expect(screen.getByText(/Un résultat a changé depuis l’affichage/)).toBeTruthy();
  });

  it('reports a failed read instead of an empty brief', () => {
    render(<HomeBrief notes={[note()]} failed brief={null} onNote={vi.fn()} onReviewed={vi.fn()} onAcceptedAll={vi.fn()} />);
    expect(screen.getByText('Le brief n’a pas pu être lu.')).toBeTruthy();
  });
});

describe('CaseReviewBand', () => {
  const journal = (over: Partial<CaseJournal['review']> = {}): CaseJournal => ({
    events: [event()], reviews: [],
    review: { note_id: 'n1', state: 'to_review', rework_count: 0, declared: event(), last_review: null, ...over },
  });

  it('accepts the reviewed revision', async () => {
    const onReviewed = vi.fn();
    mockApi.reviewCase.mockResolvedValue(review({ decision: 'accepted', comment: null, rework_number: null, revision: 7 }));
    render(<CaseReviewBand note={note()} journal={journal()} onReviewed={onReviewed} />);
    expect(screen.getByText('Résultat déclaré par Claude Code')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Accepter' }));
    await waitFor(() => expect(onReviewed).toHaveBeenCalled());
    expect(mockApi.reviewCase).toHaveBeenCalledWith('n1', 'accepted', 7, undefined);
  });

  it('refuses a rework without a reason, then sends it with the reason', async () => {
    const onReviewed = vi.fn();
    mockApi.reviewCase.mockResolvedValue(review({ revision: 7 }));
    render(<CaseReviewBand note={note()} journal={journal()} onReviewed={onReviewed} />);
    fireEvent.click(screen.getByRole('button', { name: 'Demander une reprise' }));
    fireEvent.click(screen.getByRole('button', { name: 'Envoyer la reprise' }));
    expect(screen.getByText('Indiquez à l’agent ce qui manque.')).toBeTruthy();
    expect(mockApi.reviewCase).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Ce qui manque'), { target: { value: '  Add the yearly cost ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Envoyer la reprise' }));
    await waitFor(() => expect(onReviewed).toHaveBeenCalled());
    expect(mockApi.reviewCase).toHaveBeenCalledWith('n1', 'rework_requested', 7, 'Add the yearly cost');
  });

  it('shows the backend refusal, never a silent failure', async () => {
    mockApi.reviewCase.mockRejectedValue('La case a changé depuis que vous l’avez examinée : relisez-la avant de décider.');
    render(<CaseReviewBand note={note()} journal={journal()} onReviewed={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Accepter' }));
    expect(await screen.findByText(/La case a changé/)).toBeTruthy();
  });

  it('shows a requested rework with its reason, and nothing when there is nothing to decide', () => {
    const { rerender } = render(<CaseReviewBand note={note({ status: 'open' })} journal={journal({ state: 'rework_requested', rework_count: 2, last_review: review({ rework_number: 2 }) })} onReviewed={vi.fn()} />);
    expect(screen.getByText('Reprise n°2 demandée · en attente de l’agent')).toBeTruthy();
    expect(screen.getByText('« Missing the yearly cost »')).toBeTruthy();
    rerender(<CaseReviewBand note={note()} journal={journal({ state: 'none' })} onReviewed={vi.fn()} />);
    expect(screen.queryByRole('button', { name: 'Accepter' })).toBeNull();
  });
});

describe('CaseActivity', () => {
  it('merges writes and decisions, and says when earlier changes were not recorded', () => {
    render(<CaseActivity failed={false} journal={{
      events: [event(), event({ id: 'e0', kind: 'edited', actor_kind: 'human', actor_name: null, fields: ['content'], detail: {}, last_at: '2026-10-01T10:00:00Z' })],
      reviews: [review({ created_at: '2026-10-02T10:00:00Z' })],
      review: { note_id: 'n1', state: 'rework_requested', rework_count: 1 },
    }} />);
    const items = screen.getAllByRole('listitem').map(item => item.textContent);
    expect(items[0]).toContain('Vous avez demandé la reprise n°1 : « Missing the yearly cost »');
    expect(items[1]).toContain('Claude Code a passé le statut');
    expect(items[2]).toContain('Vous avez modifié : contenu');
    expect(screen.getByText(/les modifications plus anciennes ne sont pas attribuées/)).toBeTruthy();
  });
});

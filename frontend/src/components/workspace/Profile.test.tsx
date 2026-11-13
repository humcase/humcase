import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { CaseRef, Note, Nudge, ProfileItem, ProfileView } from '../../types';

const api = vi.hoisted(() => ({
  workProfile: vi.fn(),
  profileSettings: vi.fn(async () => ({ propose: true, nudges: true })),
  setProfileSettings: vi.fn(async (settings: unknown) => settings),
  answerProfile: vi.fn(async () => ({ added: 1, refused: 1, ids: ['p1', 'p2'], profiles: ['profile'] })),
  undoProfile: vi.fn(async () => undefined),
  answerNudge: vi.fn(async () => undefined),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

import { NudgesCard, ProfileCard, ProfilePage } from './Profile';
import { ProfileSection } from '../settings/ProfileSection';
import { readProfile } from '../../utils/profileText';
import i18n from '../../i18n';

const ref = (id: string, title: string, nature: CaseRef['nature'] = 'decision'): CaseRef => ({ id, title, nature, project_name: 'Humcase', date: '2026-09-30T10:00:00Z', excerpt: '' });
const note = (id: string): Note => ({
  id, project_id: 'p', title: id, content: '', note_type: 'decision', status: 'open', resolution_comment: null,
  created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 1,
});
const item = (id: string, over: Partial<ProfileItem>): ProfileItem => ({
  id, space: 'work', section: 'rules', source: 'rule', text: '', case: null, detail: '', proposed_by: null, chance: 0.9, created_at: '2026-10-04T10:00:00Z', ...over,
});
const rule = item('p1', { text: 'Jamais de prix affiché dans l’application', case: ref('price', 'Jamais de prix affiché dans l’application') });
const refused = item('p2', { section: 'refused', source: 'refused', text: 'Les recherches proposées par Faire germer : souvent refusées', detail: '7 refusées sur 9', chance: null });
const view = (content: string, pending: ProfileItem[]): ProfileView => ({ profile: { space: 'work', note_id: content ? 'profile' : null, content, revision: 1 }, pending });

describe('Work profile and nudges', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('fr');
  });

  it('reads the profile: headings, lines with their source, free text', () => {
    expect(readProfile('Intro.\n\n## Règles\n\n- Une PR par lot (décision « Une PR par lot »)\n- Sans source\n')).toEqual([
      { kind: 'text', text: 'Intro.' },
      { kind: 'heading', text: 'Règles' },
      { kind: 'line', text: 'Une PR par lot', source: 'décision « Une PR par lot »' },
      { kind: 'line', text: 'Sans source', source: null },
    ]);
  });

  it('shows the profile, proposes each line with its origin, applies the answers and undoes them', async () => {
    api.workProfile.mockResolvedValue(view('Intro.\n\n## Règles\n\n- Une PR par lot (décision « Une PR par lot »)\n', [rule, refused]));
    const onEdit = vi.fn();
    const onNote = vi.fn();
    render(<ProfilePage space="work" spaces={['work']} onSpace={vi.fn()} notes={[note('price')]} onNote={onNote} onEdit={onEdit} />);
    expect(await screen.findByText('Une PR par lot')).toBeTruthy();
    expect(screen.getByText('— décision « Une PR par lot »')).toBeTruthy();
    expect(screen.getByText('Jev : une règle de travail, pas un choix ponctuel')).toBeTruthy();
    expect(screen.getByText('Vos réponses dans Faire germer : 7 refusées sur 9')).toBeTruthy();
    expect(screen.getByText('Compté sur votre ordinateur, sans IA')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'De la décision « Jamais de prix affiché dans l’application » (30 sept.)' }));
    expect(onNote).toHaveBeenCalledWith(expect.objectContaining({ id: 'price' }));
    // Every line is proposed; the person says no to one.
    expect(screen.getByRole('button', { name: 'Ajouter 2 lignes' })).toBeTruthy();
    fireEvent.click(screen.getAllByRole('radio', { name: 'Non' })[1]);
    fireEvent.click(screen.getByRole('button', { name: 'Ajouter 1 ligne' }));
    await waitFor(() => expect(api.answerProfile).toHaveBeenCalledWith([{ id: 'p1', accept: true }, { id: 'p2', accept: false }]));
    expect(await screen.findByText('1 ligne ajoutée au profil · 1 refusée')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(api.undoProfile).toHaveBeenCalledWith(['p1', 'p2']));
    fireEvent.click(screen.getByRole('button', { name: 'Modifier' }));
    expect(onEdit).toHaveBeenCalledWith('work');
  });

  it('says when the profile is empty and refuses everything at once', async () => {
    api.workProfile.mockResolvedValue(view('', [rule]));
    render(<ProfilePage space="work" spaces={['work', 'private']} onSpace={vi.fn()} notes={[]} onNote={vi.fn()} onEdit={vi.fn()} />);
    expect(await screen.findByText('Votre profil est encore vide')).toBeTruthy();
    expect(screen.getByRole('radiogroup', { name: 'Espace du profil' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Tout refuser' }));
    await waitFor(() => expect(api.answerProfile).toHaveBeenCalledWith([{ id: 'p1', accept: false }]));
  });

  it('counts the proposed lines on Home, and hides the card without any', () => {
    const onOpen = vi.fn();
    const { rerender } = render(<ProfileCard count={3} onOpen={onOpen} />);
    expect(screen.getByText('3 ajouts proposés')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Revoir/ }));
    expect(onOpen).toHaveBeenCalled();
    rerender(<ProfileCard count={0} onOpen={onOpen} />);
    expect(screen.queryByText(/ajouts? proposés?/)).toBeNull();
  });

  it('answers a nudge: never, later, or sprout the case', async () => {
    const nudges: Nudge[] = [
      { id: 'n1', space: 'work', case: ref('host', 'Cloudflare ou Supabase ?'), idle_days: 6, can_sprout: true },
      { id: 'n2', space: 'work', case: ref('tip', 'Infobulle d’aperçu', 'idea'), idle_days: 9, can_sprout: true },
      { id: 'n3', space: 'work', case: ref('q', 'Un profil par espace ?', 'question'), idle_days: 5, can_sprout: true },
      { id: 'n4', space: 'work', case: ref('short', 'Point relais ?', 'question'), idle_days: 7, can_sprout: false },
    ];
    const onGerminate = vi.fn();
    const onOpen = vi.fn();
    const onAnswered = vi.fn();
    render(<NudgesCard nudges={nudges} onOpen={onOpen} onGerminate={onGerminate} onAnswered={onAnswered} />);
    expect(screen.getByText('Décision à prendre')).toBeTruthy();
    expect(screen.getByText('Humcase · inchangée depuis 9 jours, sans sous-case.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Ne plus proposer « Cloudflare ou Supabase ? »' }));
    await waitFor(() => expect(api.answerNudge).toHaveBeenCalledWith('n1', 'refused'));
    fireEvent.click(screen.getByRole('button', { name: 'Proposer « Infobulle d’aperçu » plus tard' }));
    await waitFor(() => expect(api.answerNudge).toHaveBeenCalledWith('n2', 'skipped'));
    expect(onGerminate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Faire germer « Un profil par espace ? »' }));
    await waitFor(() => expect(onGerminate).toHaveBeenCalledWith('q'));
    expect(api.answerNudge).toHaveBeenCalledWith('n3', 'accepted');
    // Too short for Sprout: it is only opened.
    expect(screen.queryByRole('button', { name: 'Faire germer « Point relais ? »' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir « Point relais ? »' }));
    await waitFor(() => expect(onOpen).toHaveBeenCalledWith('short'));
    expect(onGerminate).toHaveBeenCalledTimes(1);
    expect(onAnswered).toHaveBeenCalledTimes(4);
  });

  it('turns proposals and nudges off in Settings and opens a profile', async () => {
    const onOpenProfile = vi.fn();
    render(<ProfileSection spaces={['work', 'private']} onOpenProfile={onOpenProfile} />);
    const nudges = await screen.findByRole('checkbox', { name: 'Montrer sur l’accueil ce qui attend une suite' });
    await waitFor(() => expect((nudges as HTMLInputElement).checked).toBe(true));
    fireEvent.click(nudges);
    await waitFor(() => expect(api.setProfileSettings).toHaveBeenCalledWith({ propose: true, nudges: false }));
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir le profil · Privé' }));
    expect(onOpenProfile).toHaveBeenCalledWith('private');
  });

  it('speaks English', async () => {
    await i18n.changeLanguage('en');
    render(<ProfileCard count={1} onOpen={vi.fn()} />);
    expect(screen.getByText('How you work')).toBeTruthy();
    expect(screen.getByText('1 addition proposed')).toBeTruthy();
  });
});

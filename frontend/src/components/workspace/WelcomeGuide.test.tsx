import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import type { Note, Project } from '../../types';
import { api } from '../../services/tauriApi';
import { WelcomeGuide } from './WelcomeGuide';

vi.mock('../../services/tauriApi', () => ({ api: { createProject: vi.fn(), createNote: vi.fn(), createSubCase: vi.fn(), setProjectArchived: vi.fn() } }));

const note = (fields: Partial<Note>): Note => ({ id: 'n1', project_id: 'p1', title: 'Une case', content: '', note_type: 'note', status: 'open', created_at: '2026-10-07T10:00:00Z', updated_at: '2026-10-07T10:00:00Z', ...fields } as Note);
const project = (fields: Partial<Project>): Project => ({ id: 'p1', name: 'Projet', archived_at: null, ...fields } as Project);
const guide = (props: Partial<Parameters<typeof WelcomeGuide>[0]> = {}) => render(
  <WelcomeGuide notes={[]} projects={[]} agents={[]} space="work" onCreateNote={vi.fn()} onConnectAgent={vi.fn()} onDataChanged={vi.fn()} {...props} />);

beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); });

describe('Home first steps', () => {
  it('offers both steps on an empty profile', () => {
    const onCreateNote = vi.fn(); const onConnectAgent = vi.fn();
    guide({ onCreateNote, onConnectAgent });
    expect(screen.getByText('0 sur 2 pour commencer')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Nouvelle case/ }));
    fireEvent.click(screen.getByRole('button', { name: /Brancher un agent/ }));
    expect(onCreateNote).toHaveBeenCalledOnce();
    expect(onConnectAgent).toHaveBeenCalledOnce();
  });

  it('ticks each step from real data: a case written, an agent that opened a session', () => {
    guide({ notes: [note({})], projects: [project({})], agents: [{ name: 'Claude Code', at_unix_ms: 1 }] });
    expect(screen.getByText('2 sur 2 pour commencer')).toBeTruthy();
    expect(screen.getByText('Case créée')).toBeTruthy();
    expect(screen.getByText('Claude Code est branché')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Brancher un agent/ })).toBeNull();
  });

  it('creates the filled example, whose cases do not count as the first case', async () => {
    vi.mocked(api.createProject).mockResolvedValue('ex');
    vi.mocked(api.createNote).mockResolvedValueOnce('main').mockResolvedValue('other');
    vi.mocked(api.createSubCase).mockResolvedValue('sub');
    const onDataChanged = vi.fn();
    const { rerender } = guide({ onDataChanged });
    fireEvent.click(screen.getByRole('button', { name: 'Voir avec un exemple rempli' }));
    await waitFor(() => expect(onDataChanged).toHaveBeenCalledOnce());
    expect(api.createProject).toHaveBeenCalledWith('Exemple · Boutique en ligne', undefined, 'work');
    expect(api.createNote).toHaveBeenCalledTimes(3);
    expect(vi.mocked(api.createSubCase).mock.calls.map(call => call[0])).toEqual(['main', 'main', 'main']);

    rerender(<WelcomeGuide notes={[note({ project_id: 'ex' })]} projects={[project({ id: 'ex', name: 'Exemple · Boutique en ligne' })]} agents={[]} space="work"
      onCreateNote={vi.fn()} onConnectAgent={vi.fn()} onDataChanged={onDataChanged} />);
    expect(screen.getByText('0 sur 2 pour commencer')).toBeTruthy();
    expect(screen.getByRole('status').textContent).toContain('6 cases');
    fireEvent.click(screen.getByRole('button', { name: 'Retirer l’exemple' }));
    await waitFor(() => expect(api.setProjectArchived).toHaveBeenCalledWith('ex', true));
  });

  it('says why the example could not be created', async () => {
    vi.mocked(api.createProject).mockRejectedValue('disk full');
    guide();
    fireEvent.click(screen.getByRole('button', { name: 'Voir avec un exemple rempli' }));
    expect((await screen.findByRole('alert')).textContent).toContain('disk full');
  });
});

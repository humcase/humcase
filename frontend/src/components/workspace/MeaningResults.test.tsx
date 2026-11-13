import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import type { MeaningHit, MeaningStatus, Note } from '../../types';
import { MeaningResults } from './MeaningResults';

const now = '2026-10-04T10:00:00.000Z';
const note = (id: string, title: string, over: Partial<Note> = {}): Note => ({
  id, project_id: 'p', title, content: '', note_type: 'note', status: 'open', created_at: now, updated_at: now, revision: 1, ...over,
});
const save = note('save', 'Sauvegarde : les liens se perdent', { note_type: 'bug' });
const words = note('words', 'Liens perdus');

const loose = note('loose', 'Archives et liens');
const cases = new Map([save, words, loose].map(n => [n.id, n]));
const hit = (note_id: string, lead: number): MeaningHit => ({ note_id, score: 0.8, lead, strong: lead >= 3, very_close: lead >= 4.5 });
const hits: MeaningHit[] = [hit('words', 6), hit('save', 5), hit('other', 4), hit('loose', 2.8)];
const ready: MeaningStatus = { state: 'ready', done: 2, total: 2, error: null };

describe('Close in meaning', () => {
  it('shows the cases of the page the words did not find, closest first, and opens one', () => {
    const open = vi.fn();
    render(<MeaningResults hits={hits} status={ready} cases={cases} wordIds={new Set(['words'])} onOpen={open} />);
    const list = screen.getByRole('list', { name: 'Proches par le sens' });
    // Found by the words: not repeated; outside the page (another space): left out.
    expect(within(list).queryByText('Liens perdus')).toBeNull();
    expect(within(list).queryByText('Autre espace')).toBeNull();
    expect(within(list).getByText('Sauvegarde : les liens se perdent')).toBeTruthy();
    expect(within(list).getByText('très proche')).toBeTruthy();
    fireEvent.click(within(list).getByRole('button', { name: 'Sauvegarde : les liens se perdent' }));
    expect(open).toHaveBeenCalledWith(save);
    // Next to the cases the words found, a case that barely stands out is not added...
    expect(within(list).queryByText('Archives et liens')).toBeNull();
  });

  it('helps with the closest cases when the words found nothing', () => {
    render(<MeaningResults hits={hits} status={ready} cases={cases} wordIds={new Set()} onOpen={() => {}} />);
    const list = screen.getByRole('list', { name: 'Proches par le sens' });
    expect(within(list).getByText('Archives et liens')).toBeTruthy();
    expect(within(list).getByText('proche')).toBeTruthy();
  });

  it('says how far the reading is, and shows nothing when meaning has nothing to add', () => {
    const { rerender, container } = render(<MeaningResults hits={[]} status={{ state: 'reading', done: 212, total: 451, error: null }}
      cases={cases} wordIds={null} onOpen={() => {}} />);
    expect(screen.getByRole('status').textContent).toBe('Lecture des cases · 212 / 451');
    rerender(<MeaningResults hits={[]} status={ready} cases={cases} wordIds={null} onOpen={() => {}} />);
    expect(container.textContent).toBe('');
    rerender(<MeaningResults hits={[]} status={{ state: 'off', done: 0, total: 0, error: null }} cases={cases} wordIds={null} onOpen={() => {}} />);
    expect(container.textContent).toBe('');
  });
});

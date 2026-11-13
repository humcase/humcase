import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import type { Note } from '../types';

vi.mock('../services/tauriApi', () => ({
  api: { getNoteRelations: vi.fn() },
}));

import { api } from '../services/tauriApi';
import i18n from '../i18n';
import NoteRelations from '../components/NoteRelations';
import { statusLabel } from './statuses';
import { shortDay, fullDay, dueLabel } from './dueDates';
import { formatDateTime, formatRelative } from './datetime';
import { formatClock, uses12HourClock } from './dayTimeline';
import { TYPE_LABELS } from './notePresentation';
import { spaceLabel } from './spaces';

const note = (id: string, status: Note['status'] = 'open'): Note => ({
  id, project_id: 'p', title: `Note ${id}`, content: '', note_type: 'todo', status, resolution_comment: null,
  created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 1,
});

async function inEnglish(action: () => void | Promise<void>) {
  await i18n.changeLanguage('en');
  try {
    await action();
  } finally {
    await i18n.changeLanguage('fr');
  }
}

describe('Note texts in English', () => {
  it('status and type labels', async () => {
    expect(statusLabel('in_progress')).toBe('En cours');
    await inEnglish(() => {
      expect(statusLabel('open')).toBe('To do');
      expect(statusLabel('in_progress')).toBe('In progress');
      expect(statusLabel('resolved')).toBe('Done');
      expect(TYPE_LABELS.bug).toBe('Issue');
      expect(spaceLabel('unassigned')).toBe('Unsorted');
    });
    expect(statusLabel('resolved')).toBe('Résolu');
    expect(TYPE_LABELS.bug).toBe('Incident');
  });

  it('due date and relative time follow the current language', async () => {
    const now = new Date(2026, 8, 29, 12, 0, 0);
    expect(dueLabel('2026-09-27', now)).toBe('En retard');
    expect(shortDay('2026-09-27')).toBe('27/09');
    expect(fullDay('2026-09-27')).toBe('27/09/2026');
    await inEnglish(() => {
      expect(dueLabel('2026-09-27', now)).toBe('Overdue');
      expect(dueLabel('2026-09-29', now)).toBe('Today');
      expect(shortDay('2026-09-27')).toBe('09/27');
      expect(fullDay('2026-09-27')).toBe('09/27/2026');
      const il = new Date(now.getTime() - 5 * 60_000).toISOString();
      expect(formatRelative(il, now)).toBe('5 min ago');
      expect(formatRelative(now.toISOString(), now)).toBe('just now');
    });
    expect(formatRelative(new Date(now.getTime() - 3 * 3_600_000).toISOString(), now)).toBe('il y a 3 h');
  });

  it('in English on a French Windows, dates read day/month and 24 h, like the date fields of Windows', async () => {
    const languages = vi.spyOn(navigator, 'languages', 'get').mockReturnValue(['fr-FR', 'fr', 'en-US', 'en']);
    try {
      await inEnglish(() => {
        expect(shortDay('2026-10-07')).toBe('07/10');
        expect(fullDay('2026-10-07')).toBe('07/10/2026');
        expect(formatClock(new Date(2026, 9, 5, 16, 48))).toBe('16:48');
        expect(uses12HourClock()).toBe(false);
        expect(formatDateTime('2026-10-05T14:20:33Z')).not.toMatch(/AM|PM/);
      });
    } finally {
      languages.mockRestore();
    }
  });

  it('on a US computer in English, the 12-hour clock and month/day are kept', async () => {
    const languages = vi.spyOn(navigator, 'languages', 'get').mockReturnValue(['en-US', 'en']);
    try {
      await inEnglish(() => {
        expect(shortDay('2026-10-07')).toBe('10/07');
        expect(uses12HourClock()).toBe(true);
        expect(formatClock(new Date(2026, 9, 5, 16, 48))).toMatch(/PM/);
      });
    } finally {
      languages.mockRestore();
    }
  });

  it('plural of the number of sources', async () => {
    vi.mocked(api.getNoteRelations).mockResolvedValue({ sources: [note('a'), note('b')], tasks: [note('t', 'resolved')] });
    await inEnglish(async () => {
      render(<NoteRelations note={note('n')} allNotes={[]} onOpenNote={() => {}} onChanged={() => {}} />);
      await waitFor(() => expect(screen.getByText(/· 2 sources/)).toBeTruthy());
      expect(screen.getByText(/1 done/)).toBeTruthy();
      expect(screen.getByText('Linked tasks', { exact: false })).toBeTruthy();
    });
  });
});

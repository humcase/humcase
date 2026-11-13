import { describe, it, expect, vi } from 'vitest';
import { KANBAN_COLUMNS, columnsWithIntents, createStatusQueue, writeTrackedStatus } from './kanban';
import i18n from '../i18n';
import { trackEditorWrite } from './backupBarrier';
import type { Note, NoteStatus } from '../types';

vi.mock('./backupBarrier', () => ({ trackEditorWrite: vi.fn((promise: Promise<unknown>) => promise) }));

const note = (id: string, status: NoteStatus = 'open'): Note => ({
  id,
  project_id: 'p1',
  title: id,
  content: '',
  note_type: 'note',
  status,
  created_at: '2026-09-24T00:00:00Z',
  updated_at: '2026-09-24T00:00:00Z',
  revision: 0,
});

describe('createStatusQueue — reliable Kanban moves', () => {
  it("serializes writes of the same note: the last intent wins, even with reversed responses", async () => {
    const file = createStatusQueue();
    const written: NoteStatus[] = [];
    let releasesFirst!: () => void;
    const write = (target: NoteStatus) => {
      written.push(target);
      // The first write is slow: without a queue, the second response would replace it.
      if (target === 'in_progress') return new Promise<void>(resolve => { releasesFirst = resolve; });
      return Promise.resolve();
    };

    expect(file.schedule('n1', 'open', 'in_progress', write)).toBe(true);
    // Props are not refreshed yet: the memoized intent remains the reference.
    expect(file.targetStatus('n1', 'open')).toBe('in_progress');
    expect(file.schedule('n1', 'open', 'resolved', write)).toBe(true);

    // The second write has not started until the first one is finished.
    await vi.waitFor(() => expect(written).toEqual(['in_progress']));

    releasesFirst();
    await vi.waitFor(() => expect(written).toEqual(['in_progress', 'resolved']));
  });

  it("resumes a move that returned to the original status despite stale props", async () => {
    const file = createStatusQueue();
    const written: NoteStatus[] = [];
    const write = async (target: NoteStatus) => { written.push(target); };

    file.schedule('n1', 'open', 'in_progress', write);
    // Second gesture: back to "open" while props still say "open".
    expect(file.schedule('n1', 'open', 'open', write)).toBe(true);
    await vi.waitFor(() => expect(written).toEqual(['in_progress', 'open']));
  });

  it("forgets the intent when the received data has caught up with it", () => {
    const file = createStatusQueue();
    file.schedule('n1', 'open', 'resolved', () => Promise.resolve());
    expect(file.targetStatus('n1', 'open')).toBe('resolved');
    file.reconcile('n1', 'resolved');
    // Once caught up, props become the reference again (no phantom intent).
    expect(file.targetStatus('n1', 'open')).toBe('open');
    expect(file.targetStatus('n1', 'resolved')).toBe('resolved');
  });

  it('after a failure, does not undo a more recent intent', () => {
    const file = createStatusQueue();
    file.schedule('n1', 'open', 'in_progress', () => Promise.resolve());
    file.schedule('n1', 'in_progress', 'resolved', () => Promise.resolve());
    file.abandon('n1', 'in_progress');
    expect(file.targetStatus('n1', 'open')).toBe('resolved');
  });

  it("after a failure alone, forgets the intent (props become the reference again)", () => {
    const file = createStatusQueue();
    file.schedule('n1', 'open', 'in_progress', () => Promise.resolve());
    // The write failed on the caller side: the matching intent is abandoned.
    file.abandon('n1', 'in_progress');
    expect(file.targetStatus('n1', 'open')).toBe('open');
  });
});

describe('KANBAN_COLUMNS — titles in the current language', () => {
  it('follows a language change fr → en (title read at display time, not at module load)', async () => {
    try {
      await i18n.changeLanguage('fr');
      expect(KANBAN_COLUMNS.map(c => c.title)).toEqual(['À faire', 'En cours', 'Résolu']);
      await i18n.changeLanguage('en');
      expect(KANBAN_COLUMNS.map(c => c.title)).toEqual(['To do', 'In progress', 'Done']);
    } finally {
      await i18n.changeLanguage('fr');
    }
  });
});

describe('columnsWithIntents — the card follows the target column during the write', () => {
  const column = (columns: ReturnType<typeof columnsWithIntents>, id: NoteStatus) =>
    columns.find(c => c.id === id)?.items.map(n => n.id) ?? [];

  it('places the card in the target column until the write is confirmed', () => {
    const file = createStatusQueue();
    const card = note('n1', 'open');
    file.schedule('n1', 'open', 'resolved', () => new Promise(() => {}));
    const columns = columnsWithIntents([card], file);
    expect(column(columns, 'resolved')).toEqual(['n1']);
    expect(column(columns, 'open')).toEqual([]);
  });

  it('returns to the actual column after confirmation or abandon', () => {
    const file = createStatusQueue();
    const card = note('n1', 'open');
    file.schedule('n1', 'open', 'resolved', () => new Promise(() => {}));
    // Confirmation: props switch to "resolved", the intent is forgotten.
    file.reconcile('n1', 'resolved');
    expect(column(columnsWithIntents([{ ...card, status: 'resolved' }], file), 'resolved')).toEqual(['n1']);

    // New gesture then failure: the intent is abandoned, the card returns to its actual column.
    file.schedule('n1', 'resolved', 'in_progress', () => new Promise(() => {}));
    file.abandon('n1', 'in_progress');
    const columns = columnsWithIntents([{ ...card, status: 'resolved' }], file);
    expect(column(columns, 'in_progress')).toEqual([]);
    expect(column(columns, 'resolved')).toEqual(['n1']);
  });
});

describe('writeTrackedStatus — drag write recorded in the editor chain', () => {
  it('wraps the write with trackEditorWrite', async () => {
    const write = Promise.resolve();
    const drop = vi.fn(() => write);
    await writeTrackedStatus(drop);
    expect(drop).toHaveBeenCalledTimes(1);
    expect(trackEditorWrite).toHaveBeenCalledWith(write);
  });
});

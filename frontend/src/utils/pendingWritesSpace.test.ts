import { describe, it, expect, beforeEach } from 'vitest';

const STORAGE_KEY = 'humcase-pending-writes';

describe('pendingWrites — robustness of the escrowed space', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('normalizes an unknown space or an empty projectId instead of purging the entry', async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([
      { id: 'e1', kind: 'update', noteId: 'n1', payload: { content: 'gardé' }, at: '2026-01-01T00:00:00.000Z', space: 'galaxy', projectId: '' },
    ]));

    // Dynamic import: this module is not statically imported here, so it reads local
    // storage on its first read (regression F5: purge of an entry with an unknown space).
    const { readPendingWrites, addPendingWrite } = await import('./pendingWrites');

    const read = readPendingWrites();
    expect(read).toHaveLength(1);
    // An unknown value is normalized to "no space" (visible everywhere), never rejected.
    expect(read[0].space ?? null).toBeNull();
    // An empty projectId is the same as absent: never an orphan anchor.
    expect(read[0].projectId ?? null).toBeNull();

    // It survives a rewrite of the list (adding another write).
    addPendingWrite({ kind: 'brut', noteId: 'n2', texte: 'x', space: 'private' });
    const after = readPendingWrites();
    expect(after).toHaveLength(2);
    expect(after.some(e => e.id === 'e1')).toBe(true);
  });
});

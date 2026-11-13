import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  addPendingWrite,
  pendingWritesPersistent,
  readPendingWrites,
  clearPendingWrites,
  removePendingWrite,
  subscribePendingWrites,
} from './pendingWrites';

describe('pendingWrites — recoverable escrow', () => {
  beforeEach(() => {
    clearPendingWrites();
    localStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
  });

  it('tells its readers about every change, an entry added by a drawing included', () => {
    const seen: number[] = [];
    const stop = subscribePendingWrites(() => seen.push(readPendingWrites().length));
    const entry = addPendingWrite({ kind: 'update', noteId: 's1', payload: { content: '{}' } });
    removePendingWrite(entry.id);
    stop();
    addPendingWrite({ kind: 'update', noteId: 's1', payload: { content: '{}' } });
    expect(seen).toEqual([1, 0]);
  });

  it('adds then rereads a write (local persistence)', () => {
    addPendingWrite({ kind: 'brut', noteId: 'n1', texte: 'bonjour' });
    expect(pendingWritesPersistent()).toBe(true);
    expect(readPendingWrites()).toHaveLength(1);
  });

  it("keeps the write in memory if local storage refuses (quota)", () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('QuotaExceededError');
    });

    expect(() =>
      addPendingWrite({ kind: 'brut', noteId: 'n1', texte: 'a sauver' })
    ).not.toThrow();

    // Retention is no longer presented as persistent (local storage refused)…
    expect(pendingWritesPersistent()).toBe(false);
    // …but the text remains available for recovery within the session.
    expect(readPendingWrites()).toHaveLength(1);
    expect(readPendingWrites()[0]).toMatchObject({ kind: 'brut', texte: 'a sauver' });

    // A later successful write becomes persistent again and does not erase the previous one.
    setItem.mockRestore();
    addPendingWrite({ kind: 'brut', noteId: 'n2', texte: 'deux' });
    expect(pendingWritesPersistent()).toBe(true);
    expect(readPendingWrites()).toHaveLength(2);
  });

  it("does not stack a strict duplicate again and empties on demand", () => {
    addPendingWrite({ kind: 'brut', noteId: 'n1', texte: 'bonjour' });
    addPendingWrite({ kind: 'brut', noteId: 'n1', texte: 'bonjour' });
    expect(readPendingWrites()).toHaveLength(1);

    clearPendingWrites();
    expect(readPendingWrites()).toHaveLength(0);
  });
});

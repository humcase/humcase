import { describe, it, expect } from 'vitest';
import { resolveActiveNote } from './selection';

type NoteMin = { id: string; title: string };

const opened: NoteMin = { id: 'n1', title: 'Ouverte' };
const resolvedNote: NoteMin = { id: 'n2', title: 'Résolue' };

describe('resoudreNoteActive', () => {
  it('resolves a note missing from the filtered list (selection via palette under a filter)', () => {
    // `projectNotes` (complete) contains the open note; `notes` ("Resolved" filter) only contains
    // the resolved note. The active note must still be found.
    const complete = [opened, resolvedNote];
    const filtered = [resolvedNote];
    expect(resolveActiveNote('n1', [complete, filtered])).toEqual(opened);
  });

  it('falls back to the filtered list if the complete one does not contain it', () => {
    expect(resolveActiveNote('n2', [[], [resolvedNote]])).toEqual(resolvedNote);
  });

  it("returns null without a selection or if the note exists nowhere", () => {
    expect(resolveActiveNote(null, [[opened]])).toBeNull();
    expect(resolveActiveNote('absent', [[opened], [resolvedNote]])).toBeNull();
  });
});

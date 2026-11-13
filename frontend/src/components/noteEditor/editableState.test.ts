import { describe, expect, it } from 'vitest';
import type { Note, Project } from '../../types';
import {
  advanceBaseline,
  baseDuPayload,
  buildPayload,
  equal,
  spaceOfNote,
  projectSpace,
  stateFromNote,
  commonPrefix,
  unpersistedRemainder,
  type EditableState,
} from './editableState';

function state(override: Partial<EditableState> = {}): EditableState {
  return {
    id: 'n1',
    content: 'contenu',
    title: 'titre',
    note_type: 'note',
    priority: '',
    due_at: '',
    tags: ['a'],
    ...override,
  };
}

describe('construirePayload', () => {
  it("only sends the fields modified locally", () => {
    const base = state();
    expect(buildPayload(base, state())).toEqual({});
    expect(buildPayload(base, state({ title: 'autre' }))).toEqual({ title: 'autre' });
    expect(buildPayload(base, state({ content: 'x', priority: 'P1' }))).toEqual({ content: 'x', priority: 'P1' });
  });

  it("sends '' to remove a priority or a due date", () => {
    const base = state({ priority: 'P1', due_at: '2026-01-01' });
    expect(buildPayload(base, state({ priority: '', due_at: '' }))).toEqual({ priority: '', due_at: '' });
  });

  it('compares tags by content and serializes them as JSON', () => {
    const base = state({ tags: ['a', 'b'] });
    expect(buildPayload(base, state({ tags: ['a', 'b'] }))).toEqual({});
    expect(buildPayload(base, state({ tags: ['a', 'c'] }))).toEqual({ tags: '["a","c"]' });
  });
});

describe('egales', () => {
  it('treats arrays by content', () => {
    expect(equal(['a'], ['a'])).toBe(true);
    expect(equal(['a'], ['b'])).toBe(false);
    expect(equal('a', 'a')).toBe(true);
  });
});

describe('baseDuPayload', () => {
  it('only takes from the database the fields present in the payload', () => {
    const base = state({ title: 'ancien', priority: 'P2', tags: ['x'] });
    expect(baseDuPayload(base, { title: 'nouveau', tags: '["y"]' })).toEqual({ title: 'ancien', tags: '["x"]' });
    expect(baseDuPayload(base, {})).toEqual({});
  });
});

describe('avancerBaseline', () => {
  it('only advances the fields actually written', () => {
    const base = state();
    advanceBaseline(base, 'n1', { title: 'nouveau' }, state({ title: 'nouveau', content: 'jamais envoyé' }));
    expect(base.title).toBe('nouveau');
    expect(base.content).toBe('contenu');
  });

  it("does not touch the reference of another note", () => {
    const base = state();
    advanceBaseline(base, 'autre', { title: 'nouveau' }, state({ title: 'nouveau' }));
    expect(base.title).toBe('titre');
  });

  it('advances tags and the removed priority', () => {
    const base = state({ priority: 'P1' });
    advanceBaseline(base, 'n1', { priority: '', tags: '["z"]' }, state({ priority: '', tags: ['z'] }));
    expect(base.priority).toBe('');
    expect(base.tags).toEqual(['z']);
  });
});

describe('prefixeCommun et resteNonPersiste', () => {
  it('measures the common prefix', () => {
    expect(commonPrefix('bonjour', 'bonsoir')).toBe(3);
    expect(commonPrefix('abc', 'abc')).toBe(3);
    expect(commonPrefix('', 'abc')).toBe(0);
  });

  it('only returns the end of the dictation not yet persisted', () => {
    expect(unpersistedRemainder('bonjour tout le monde', 'bonjour')).toBe('tout le monde');
    expect(unpersistedRemainder('bonjour', 'bonjour')).toBe('');
    expect(unpersistedRemainder('bonjour', '')).toBe('bonjour');
    // The already-persisted text diverges: only the truly common prefix is removed.
    expect(unpersistedRemainder('bonjour tout', 'bonsoir')).toBe('jour tout');
  });
});

describe('etatDepuisNote', () => {
  it('normalizes a missing note', () => {
    expect(stateFromNote(null)).toEqual({
      id: null, content: '', title: '', note_type: 'note', priority: '', due_at: '', tags: [],
    });
  });

  it('reads the editable fields and the tags', () => {
    const note = { id: 'n9', content: 'c', title: 't', note_type: 'bug', priority: 'P0', due_at: null, tags: '["x"]' } as unknown as Note;
    expect(stateFromNote(note)).toEqual({
      id: 'n9', content: 'c', title: 't', note_type: 'bug', priority: 'P0', due_at: '', tags: ['x'],
    });
  });
});

describe('espace', () => {
  const projects = [{ id: 'p1', space: 'pro' }] as unknown as Project[];

  it("derives the space from the project, null if unknown", () => {
    expect(projectSpace('p1', projects)).toBe('pro');
    expect(projectSpace('p2', projects)).toBeNull();
    expect(projectSpace(null, projects)).toBeNull();
    expect(spaceOfNote({ project_id: 'p1' } as unknown as Note, projects)).toBe('pro');
    expect(spaceOfNote(null, projects)).toBeNull();
  });
});

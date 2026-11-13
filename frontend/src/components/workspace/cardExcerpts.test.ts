import { describe, expect, it } from 'vitest';
import { renderExcerpt } from './cardExcerpts';
import { notePreview, PREVIEW_TEXT_LIMIT } from '../../utils/notePresentation';
import type { Note } from '../../types';

describe('card excerpts', () => {
  it('renders a text once and reuses it', () => {
    expect(renderExcerpt('**Bold** and a list:\n\n- one')).toBe(renderExcerpt('**Bold** and a list:\n\n- one'));
  });

  it('keeps a preview to what a view can show, once per version of the case', () => {
    const note = { id: 'n', title: 'T', content: 'word '.repeat(400), content_kind: 'text' } as Note;
    const preview = notePreview(note);
    expect(preview.length).toBeLessThanOrEqual(PREVIEW_TEXT_LIMIT + 1);
    expect(preview.endsWith('…')).toBe(true);
    expect(notePreview(note)).toBe(preview);
  });

  it('keeps the underscores of identifiers and removes emphasis markers', () => {
    const note = { id: 'u', title: 'Clé', content: 'Renommer `mon_identifiant_api` et *vite*, _pas_ __demain__ ~~hier~~.', content_kind: 'text' } as Note;
    expect(notePreview(note)).toBe('Renommer mon_identifiant_api et vite, pas demain hier.');
  });
});

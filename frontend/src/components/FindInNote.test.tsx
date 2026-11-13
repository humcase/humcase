import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `http://asset.localhost/${encodeURIComponent(path)}`,
  invoke: vi.fn(),
}));
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from 'tiptap-markdown';
import { ImagePortable } from './PortableImage';
import { SearchInNote, searchState, findOccurrences } from './FindInNote';
import { FindBar } from './FindBar';

function makeEditor(markdown: string) {
  const element = document.createElement('div');
  document.body.appendChild(element);
  return new Editor({ element, extensions: [StarterKit, Markdown, ImagePortable, SearchInNote], content: markdown });
}
const markdown = (editor: Editor) => (editor.storage as unknown as { markdown: { getMarkdown: () => string } }).markdown.getMarkdown();
const text = (editor: Editor, o: { from: number; to: number }) => editor.state.doc.textBetween(o.from, o.to);

describe('Find in note — occurrences', () => {
  it('ignores case and accents and gives exact positions, even after an image', () => {
    const editor = makeEditor('Été **ete** ÉTÉ\n\n![x](humcase-image://a.png)\n\nAprès l’image : été.');
    const occurrences = findOccurrences(editor.state.doc, 'ete');
    expect(occurrences.map(o => text(editor, o))).toEqual(['Été', 'ete', 'ÉTÉ', 'été']);
    expect(findOccurrences(editor.state.doc, '   ')).toEqual([]);
    expect(findOccurrences(editor.state.doc, 'absent')).toEqual([]);
    editor.destroy();
  });

  it('does not cross two paragraphs', () => {
    const editor = makeEditor('fin de ligne\n\nsuite');
    expect(findOccurrences(editor.state.doc, 'lignesuite')).toEqual([]);
    editor.destroy();
  });
});

describe('Find in note — bar (Ctrl+F)', () => {
  let rects: ReturnType<typeof vi.spyOn>;
  // jsdom does not compute layout: the editor is considered displayed.
  beforeEach(() => {
    rects = vi.spyOn(Element.prototype, 'getClientRects').mockReturnValue([{}] as unknown as DOMRectList);
    // Selecting an occurrence makes ProseMirror scroll, which measures a Range (absent from jsdom).
    Range.prototype.getClientRects ??= () => [] as unknown as DOMRectList;
    Range.prototype.getBoundingClientRect ??= () => ({ top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
  });
  afterEach(() => { rects.mockRestore(); });

  it('Ctrl+F opens the search, Enter steps through, Escape selects without writing anything', async () => {
    const source = 'Un chat, puis un autre chat.\n\nEncore un chat.';
    const editor = makeEditor(source);
    const before = markdown(editor);
    render(<FindBar editor={editor} />);

    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    const field = await screen.findByLabelText('Trouver dans la case');
    fireEvent.change(field, { target: { value: 'chat' } });
    expect(screen.getByText('1 / 3')).toBeTruthy();
    expect(editor.view.dom.querySelectorAll('.find-match')).toHaveLength(3);

    fireEvent.keyDown(field, { key: 'Enter' });
    expect(screen.getByText('2 / 3')).toBeTruthy();
    fireEvent.keyDown(field, { key: 'Enter', shiftKey: true });
    fireEvent.keyDown(field, { key: 'Enter', shiftKey: true });
    expect(screen.getByText('3 / 3')).toBeTruthy();

    fireEvent.change(field, { target: { value: 'chien' } });
    expect(screen.getByText('Aucun résultat')).toBeTruthy();
    fireEvent.change(field, { target: { value: 'autre' } });

    await act(async () => { fireEvent.keyDown(field, { key: 'Escape' }); });
    expect(screen.queryByLabelText('Trouver dans la case')).toBeNull();
    const { from, to } = editor.state.selection;
    expect(editor.state.doc.textBetween(from, to)).toBe('autre');
    expect(editor.view.dom.querySelectorAll('.find-match')).toHaveLength(0);
    expect(searchState(editor.state)?.query).toBe('');
    expect(markdown(editor)).toBe(before);
    editor.destroy();
  });

  it('takes the selected text and follows the text modifications', () => {
    const editor = makeEditor('alpha beta alpha');
    editor.commands.setTextSelection({ from: 1, to: 6 });
    render(<FindBar editor={editor} />);
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    expect((screen.getByLabelText('Trouver dans la case') as HTMLInputElement).value).toBe('alpha');
    expect(screen.getByText('1 / 2')).toBeTruthy();
    act(() => { editor.commands.insertContentAt(editor.state.doc.content.size - 1, ' alpha'); });
    expect(screen.getByText('1 / 3')).toBeTruthy();
    editor.destroy();
  });

  it('does not open when the editor is hidden', () => {
    rects.mockReturnValue([] as unknown as DOMRectList);
    const editor = makeEditor('texte');
    render(<FindBar editor={editor} />);
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    expect(screen.queryByLabelText('Trouver dans la case')).toBeNull();
    editor.destroy();
  });
});

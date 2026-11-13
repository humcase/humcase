import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, act } from '@testing-library/react';

// ProseMirror does not run in jsdom: we capture the options passed to `useEditor` to drive
// the paste handler directly, and we observe the image insertion command.
const setImageMock = vi.hoisted(() => vi.fn());
let optionsCapture: { editorProps?: { handlePaste: (view: unknown, event: unknown) => boolean; handleClick: (view: unknown, pos: number, event: unknown) => boolean } } | null = null;

const fakeEditor = {
  isDestroyed: false,
  chain: () => ({ focus: () => ({ setImage: () => ({ run: setImageMock }) }) }),
  storage: { markdown: { getMarkdown: () => '' } },
  commands: { setContent: vi.fn() },
};

vi.mock('@tiptap/react', () => ({
  useEditor: (options: typeof optionsCapture) => {
    optionsCapture = options;
    return fakeEditor;
  },
  EditorContent: () => null,
}));

vi.mock('./EditorToolbar', () => ({ EditorToolbar: () => null }));
vi.mock('./FindBar', () => ({ FindBar: () => null }));
vi.mock('../services/tauriApi', () => ({ api: { openLink: vi.fn(() => Promise.resolve()) } }));
import { api } from '../services/tauriApi';

import TiptapEditor from './TiptapEditor';

function pasteEvent(file: File) {
  return { clipboardData: { items: [{ type: 'image/png', getAsFile: () => file }] } };
}

describe("TiptapEditor — image paste tied to the note", () => {
  beforeEach(() => {
    setImageMock.mockClear();
    optionsCapture = null;
  });

  it("inserts the image if the note has not changed", async () => {
    const file = new File([new Uint8Array([1])], 'x.png', { type: 'image/png' });
    const onPasteImage = vi.fn().mockResolvedValue('asset://img');
    render(<TiptapEditor value="" noteId="n1" onChange={() => {}} onPasteImage={onPasteImage} />);

    const handled = optionsCapture!.editorProps!.handlePaste(null, pasteEvent(file));
    expect(handled).toBe(true);

    await act(async () => {});
    expect(setImageMock).toHaveBeenCalledTimes(1);
  });

  it("does not insert the image if the note changed during the save", async () => {
    const file = new File([new Uint8Array([1])], 'x.png', { type: 'image/png' });
    let resolve: (v: string) => void = () => {};
    const onPasteImage = vi.fn().mockReturnValue(new Promise<string>(r => { resolve = r; }));
    const { rerender } = render(
      <TiptapEditor value="" noteId="n1" onChange={() => {}} onPasteImage={onPasteImage} />
    );

    optionsCapture!.editorProps!.handlePaste(null, pasteEvent(file));

    // The user changes note before the image save is finished.
    rerender(<TiptapEditor value="" noteId="n2" onChange={() => {}} onPasteImage={onPasteImage} />);

    await act(async () => { resolve('asset://img'); });
    expect(setImageMock).not.toHaveBeenCalled();
  });
});

describe('TiptapEditor — links', () => {
  it('opens a web link in the browser on Ctrl + click only, never another address', () => {
    render(<TiptapEditor value="" onChange={() => {}} />);
    const handleClick = optionsCapture!.editorProps!.handleClick;
    const link = (href: string) => { const a = document.createElement('a'); a.setAttribute('href', href); return a; };
    expect(handleClick(null, 0, { ctrlKey: false, metaKey: false, target: link('https://example.com') })).toBe(false);
    expect(handleClick(null, 0, { ctrlKey: true, metaKey: false, target: link('file:///C:/Windows/notepad.exe') })).toBe(false);
    expect(api.openLink).not.toHaveBeenCalled();
    expect(handleClick(null, 0, { ctrlKey: true, metaKey: false, target: link('https://example.com/a') })).toBe(true);
    expect(api.openLink).toHaveBeenCalledWith('https://example.com/a');
  });
});

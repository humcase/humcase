import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `http://asset.localhost/${encodeURIComponent(path)}`,
  invoke: vi.fn(),
}));
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from 'tiptap-markdown';
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight';
import Image from '@tiptap/extension-image';
import Link from '@tiptap/extension-link';
import { TableKit } from '@tiptap/extension-table';
import { TaskItem, TaskList } from '@tiptap/extension-list';
import { common, createLowlight } from 'lowlight';
import { ImagePortable } from './PortableImage';
import { setImagesFolder } from '../utils/imageRefs';

const lowlight = createLowlight(common);

/**
 * Markdown round trip with the **real** Tiptap (not mocked), in jsdom.
 *
 * It is the only test that exercises the real parse/serialize: it protects the structures that
 * component tests do not see (Tiptap is replaced there by a textarea).
 */
function makeEditor(markdown: string) {
  return new Editor({
    element: document.createElement('div'),
    extensions: [
      StarterKit.configure({ codeBlock: false, link: false }),
      Markdown,
      CodeBlockLowlight.configure({ lowlight }),
      Link,
      Image,
      TaskList,
      TaskItem.configure({ nested: true }),
      TableKit,
    ],
    content: markdown,
  });
}

type Noeud = { type: string; attrs?: Record<string, unknown>; content?: Noeud[] };

/** `getMarkdown` is provided by `tiptap-markdown` but absent from the base `Storage` type. */
function getMarkdown(editor: Editor): string {
  return (editor.storage as unknown as { markdown: { getMarkdown: () => string } }).markdown.getMarkdown();
}

describe('Tiptap — real Markdown round trip', () => {
  it('keeps a dictated checklist (title + bold) without breaking it', () => {
    const markdown =
      '## Prochaine tâche à effectuer\n- [ ] Vérifier que **toute la documentation** est **clean** et **propre**.';
    const editor = makeEditor(markdown);

    const doc = editor.getJSON() as Noeud;
    const list = doc.content?.find(n => n.type === 'taskList');
    expect(list, 'the checklist must be a taskList node').toBeTruthy();

    const item = list!.content?.[0];
    expect(item?.type).toBe('taskItem');
    expect(item?.attrs?.checked).toBe(false);
    // The text belongs to the item (no empty box followed by a separate paragraph).
    expect(JSON.stringify(item?.content)).toContain('Vérifier que');
    expect(JSON.stringify(item?.content)).toContain('toute la documentation');

    // The serialization stays a correct checklist item.
    expect(getMarkdown(editor)).toContain('- [ ] Vérifier que');
  });

  it('keeps a table and a checked checklist', () => {
    const editor = makeEditor('| A | B |\n| - | - |\n| 1 | 2 |\n\n- [x] Fait');
    const output = getMarkdown(editor);
    expect(output).toContain('| A | B |');
    expect(output).toContain('| 1 | 2 |');
    expect(output).toContain('- [x] Fait');
  });

  it('keeps a Markdown link (text + URL) without losing the URL', () => {
    const markdown = 'Voir [la doc](https://exemple.com) pour plus de détails.';
    const editor = makeEditor(markdown);

    // The `link` mark must exist in the schema, otherwise the URL is lost for good.
    const doc = editor.getJSON() as Noeud;
    expect(JSON.stringify(doc)).toContain('"link"');

    const output = getMarkdown(editor);
    expect(output).toContain('[la doc](https://exemple.com)');
  });

  it('keeps a Markdown image (alt text + URL)', () => {
    const editor = makeEditor('![un chat](https://exemple.com/chat.png)');
    const output = getMarkdown(editor);
    expect(output).toContain('![un chat](https://exemple.com/chat.png)');
  });

  it('keeps a fenced code block', () => {
    const editor = makeEditor('```js\nconst x = 1;\n```');
    const output = getMarkdown(editor);
    expect(output).toContain('```');
    expect(output).toContain('const x = 1;');
  });

  it("a new editor does not have the undo history of the previous note", () => {
    // `NoteEditor` strategy: `key={noteId}` recreates the editor on note change. We check
    // here that two distinct instances do have isolated histories: a Ctrl+Z in B cannot
    // undo a modification made in A.
    const noteA = makeEditor('Note A');
    noteA.commands.insertContent(' modifiée');
    expect(noteA.can().undo()).toBe(true);

    const noteB = makeEditor('Note B');
    expect(noteB.can().undo()).toBe(false);

    noteA.destroy();
    noteB.destroy();
  });

  it("onUpdate passes the Markdown to the callback (wiring of real input)", () => {
    // The component tests title derivation with a **mocked** Tiptap. Here we check that the real
    // `onUpdate` of Tiptap does call `onChange` with the current Markdown: the link that ties
    // typing to `handleContentChange` (title derivation on manual input).
    const recus: string[] = [];
    const editor = new Editor({
      element: document.createElement('div'),
      extensions: [
        StarterKit.configure({ codeBlock: false, link: false }),
        Markdown,
        CodeBlockLowlight.configure({ lowlight }),
        Link,
        Image,
        TaskList,
        TaskItem.configure({ nested: true }),
        TableKit,
      ],
      content: '',
      onUpdate: ({ editor }) => recus.push(getMarkdown(editor)),
    });

    editor.commands.insertContent('Bonjour');

    expect(recus.length).toBeGreaterThan(0);
    expect(recus.at(-1)).toContain('Bonjour');
    editor.destroy();
  });
});

describe('Tiptap — images portables (0.9.81)', () => {
  const portableEditor = (markdown: string) => new Editor({
    element: document.createElement('div'),
    extensions: [StarterKit.configure({ codeBlock: false, link: false }), Markdown, Link, ImagePortable],
    content: markdown,
  });

  it('keeps the portable reference in the Markdown and only translates it for display', () => {
    setImagesFolder('/home/x/.local/share/com.humcase.desktop/images');
    try {
      const markdown = 'Avant\n\n![capture](humcase-image://abc.webp)\n\nAprès';
      const editor = portableEditor(markdown);
      expect(getMarkdown(editor)).toBe(markdown);
      const img = editor.view.dom.querySelector('img');
      expect(img?.getAttribute('src')).not.toContain('humcase-image://');
      expect(img?.getAttribute('src')).toContain('abc.webp');
    } finally {
      setImagesFolder(null);
    }
  });

  it('converts an old local URL of an internal image back on the next save', () => {
    const oldOne = 'http://asset.localhost/C%3A%5CUsers%5Cx%5CAppData%5CRoaming%5Ccom.humcase.desktop%5Cimages%5Cabc.png';
    const editor = portableEditor(`![a](${oldOne})`);
    expect(getMarkdown(editor)).toBe('![a](humcase-image://abc.png)');
  });
});

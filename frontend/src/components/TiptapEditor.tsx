import { EditorToolbar } from './EditorToolbar';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import { Markdown } from 'tiptap-markdown';
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight';
import { ImagePortable } from './PortableImage';
import Link from '@tiptap/extension-link';
import { TableKit } from '@tiptap/extension-table';
import { TaskItem, TaskList } from '@tiptap/extension-list';
import { common, createLowlight } from 'lowlight';
import { useEffect, useRef, type CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import { SearchInNote } from './FindInNote';
import { PassageMarks, passageKey, findPassage, type PassageMark } from './noteEditor/passageMarks';
import { FindBar } from './FindBar';
import { useEditorFontScale } from '../utils/prefs';
import { api } from '../services/tauriApi';
import { webAddress } from '../utils/caseContent';

const lowlight = createLowlight(common);

interface TiptapEditorProps {
  value: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
  /** Displayed note: an asynchronous operation (image paste) must not spill onto another. */
  noteId?: string;
  /** Saves the image and returns the URL (`asset://…`) to insert, or null on failure. */
  onPasteImage?: (file: File) => Promise<string | null>;
  /** Passages proposals cite, marked in the text (Faire germer). */
  passages?: PassageMark[];
  /** A passage to bring into view; `at` changes on every request. */
  reveal?: { passage: string; at: number } | null;
  /** Put the cursor in the text when it appears (a case just created by the person). */
  autoFocus?: boolean;
  /** Each new value puts the cursor at the start of the text (Enter in the title). */
  focusSignal?: number;
}

export default function TiptapEditor({ value, onChange, onBlur, noteId, onPasteImage, passages, reveal, autoFocus = false, focusSignal = 0 }: TiptapEditorProps) {
  const { t } = useTranslation();
  const fontScale = useEditorFontScale();
  // Note actually displayed when the asynchronous insertion finishes: the same Tiptap
  // instance is reused from one note to another, a paste started on A must never land in B.
  const noteIdRef = useRef<string | undefined>(noteId);
  useEffect(() => {
    noteIdRef.current = noteId;
  }, [noteId]);
  const editor = useEditor({
    autofocus: autoFocus ? 'end' : false,
    extensions: [
      StarterKit.configure({
        codeBlock: false, // We use CodeBlockLowlight instead
        link: false, // Link is loaded explicitly below (direct dependency, tested)
      }),
      Markdown,
      Placeholder.configure({
        placeholder: t('editor.tiptap.placeholder'),
        emptyEditorClass: 'is-editor-empty',
      }),
      CodeBlockLowlight.configure({
        lowlight,
      }),
      // The schema must know these nodes: without them, any table, checklist, image or link present
      // in the Markdown is removed or flattened on the next save (silent loss).
      // Link is loaded here explicitly (and not via StarterKit) so that the dependency is
      // direct and covered by a round-trip test that fails if it is removed.
      // A link followed in place would replace the app in its window: Ctrl + click opens it in the
      // browser (below), a plain click places the cursor to edit it.
      Link.configure({ openOnClick: false, HTMLAttributes: { title: t('editor.tiptap.linkHint') } }),
      ImagePortable,
      TaskList,
      TaskItem.configure({ nested: true }),
      TableKit,
      SearchInNote,
      PassageMarks,
    ],
    content: value,
    editorProps: {
      attributes: {
        class: 'prose prose-invert prose-sm sm:prose-base focus:outline-none max-w-none min-h-[300px]',
      },
      handleClick(_view, _pos, event) {
        if (!event.ctrlKey && !event.metaKey) return false;
        const link = (event.target as HTMLElement | null)?.closest?.('a[href]');
        const url = link ? webAddress(link.getAttribute('href') ?? '') : null;
        if (!url) return false;
        void api.openLink(url).catch(() => {});
        return true;
      },
      handlePaste(_view, event) {
        const items = event.clipboardData?.items;
        if (!items || !onPasteImage) return false;
        
        for (const item of items) {
          if (item.type.indexOf('image') === 0) {
            const file = item.getAsFile();
            if (file) {
              // Note displayed at the time of the paste: if the user changes note before
              // the image is saved, nothing is inserted (otherwise A's image lands in B).
              const target = noteIdRef.current;
              onPasteImage(file).then(src => {
                if (src && editor && !editor.isDestroyed && noteIdRef.current === target) {
                  // Insert a real Image node (not a Markdown string): the document stays
                  // schema-compliant and the image survives the Markdown round trip.
                  editor.chain().focus().setImage({ src }).run();
                }
              });
              return true; // We handled it
            }
          }
        }
        return false;
      }
    },
    onUpdate: ({ editor }) => {
      const markdown = (editor.storage as any).markdown.getMarkdown();
      onChange(markdown);
    },
    onBlur: () => {
      if (onBlur) onBlur();
    }
  });

  useEffect(() => {
    if (focusSignal > 0) editor?.commands.focus('start');
  }, [focusSignal, editor]);

  // Sync external changes (e.g., when switching notes or AI appends text)
  useEffect(() => {
    if (editor && value !== (editor.storage as any).markdown.getMarkdown()) {
      editor.commands.setContent(value, { emitUpdate: false });
    }
  }, [value, editor]);

  useEffect(() => {
    if (!editor || editor.isDestroyed || !editor.view || !editor.state) return;
    editor.view.dispatch(editor.state.tr.setMeta(passageKey, passages ?? []));
  }, [passages, editor]);

  useEffect(() => {
    if (!editor || editor.isDestroyed || !editor.view || !reveal) return;
    const range = findPassage(editor.state.doc, reveal.passage);
    if (!range) return;
    const node = editor.view.domAtPos(range.from).node;
    const element = node instanceof Element ? node : node.parentElement;
    element?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [reveal, editor]);

  if (!editor) {
    return null;
  }

  return (
    <div
      className="w-full h-full overflow-y-auto tiptap-wrapper"
      style={{ '--editor-font-scale': fontScale } as CSSProperties}
    >
      <EditorToolbar key={noteId} editor={editor} />
      <FindBar key={`find-${noteId}`} editor={editor} />
      <EditorContent editor={editor} />
    </div>
  );
}

import { Extension } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Plugin, PluginKey, type EditorState } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

/**
 * "Find in note" (Ctrl+F in the editor). Highlights the occurrences without writing anything to
 * the document: plain decorations, never saved in the Markdown.
 */
export interface Occurrence { from: number; to: number }
interface SearchState { query: string; currentValue: number; occurrences: Occurrence[]; decorations: DecorationSet }

export const searchKey = new PluginKey<SearchState>('rechercheDansNote');

/** Case- and accent-insensitive comparison ("ete" finds "Été"). */
function normalize(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
}

/**
 * Occurrences of `query` in the document text, block by block. An inline node without text
 * (image, line break) counts for one character, as in ProseMirror: positions stay
 * exact. An occurrence does not cross two paragraphs.
 */
export function findOccurrences(doc: ProseMirrorNode, query: string): Occurrence[] {
  const target = normalize(query);
  if (!target.trim()) return [];
  const occurrences: Occurrence[] = [];
  doc.descendants((node, pos) => {
    if (!node.isTextblock) return true;
    const text = node.textBetween(0, node.content.size, undefined, '￼');
    // Normalization removes combining marks: we compare character by character to
    // keep the match with the original positions.
    const normalized = [...text].map(normalize);
    const lengths = [...text].map(c => c.length);
    for (let i = 0; i < normalized.length; i++) {
      let j = i;
      let lu = '';
      while (j < normalized.length && lu.length < target.length) lu += normalized[j++];
      if (lu === target) {
        const start = lengths.slice(0, i).reduce((a, b) => a + b, 0);
        const fin = lengths.slice(0, j).reduce((a, b) => a + b, 0);
        occurrences.push({ from: pos + 1 + start, to: pos + 1 + fin });
      }
    }
    return false;
  });
  return occurrences;
}

function build(doc: ProseMirrorNode, query: string, currentValue: number): SearchState {
  const occurrences = findOccurrences(doc, query);
  const index = occurrences.length === 0 ? 0 : Math.min(Math.max(currentValue, 0), occurrences.length - 1);
  const decorations = DecorationSet.create(doc, occurrences.map((o, i) =>
    Decoration.inline(o.from, o.to, { class: i === index ? 'find-match find-match--current' : 'find-match' })));
  return { query, currentValue: index, occurrences, decorations };
}

type Meta = { query?: string; currentValue?: number };

export function searchState(state: EditorState): SearchState | undefined {
  return searchKey.getState(state);
}

export const SearchInNote = Extension.create({
  name: 'rechercheDansNote',
  addProseMirrorPlugins() {
    return [new Plugin<SearchState>({
      key: searchKey,
      state: {
        init: (_, state) => build(state.doc, '', 0),
        apply(tr, previous, _previous, fresh) {
          const meta = tr.getMeta(searchKey) as Meta | undefined;
          if (meta) return build(fresh.doc, meta.query ?? previous.query, meta.currentValue ?? previous.currentValue);
          // The text changes during the search: the occurrences are recomputed.
          if (tr.docChanged && previous.query) return build(fresh.doc, previous.query, previous.currentValue);
          return previous;
        },
      },
      props: { decorations: state => searchKey.getState(state)?.decorations },
    })];
  },
});

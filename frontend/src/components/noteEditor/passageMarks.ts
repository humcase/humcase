import { Extension } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { NoteType } from '../../types';

/**
 * The passages a proposal cites, marked in the text with the proposal's nature (Faire germer).
 * Plain decorations: nothing is written in the document nor in its Markdown. The passage is
 * found as the Rust side checks it (`germinate::passage_in`): within one block, quotes, dashes
 * and spaces unified, case ignored.
 */
export interface PassageMark {
  passage: string;
  nature: NoteType;
  /** Shown strongly (the proposal under the pointer, or asked for). */
  lit: boolean;
}

export const passageKey = new PluginKey<DecorationSet>('passageMarks');

const QUOTES: Record<string, string> = {
  '\u2018': "'", '\u2019': "'", '\u02BC': "'", '`': "'", '\u00B4': "'",
  '\u201C': '"', '\u201D': '"', '\u00AB': '"', '\u00BB': '"', '\u201E': '"',
  '\u2013': '-', '\u2014': '-', '\u2212': '-',
};
const SKIPPED = new Set(['*', '_', '#', '>', '[', ']', '~']);

/** Normalized text and, for each of its characters, the offset (UTF-16) it comes from. */
function normalizeWithOffsets(text: string): { text: string; from: number[]; to: number[] } {
  let out = '';
  const from: number[] = [];
  const to: number[] = [];
  let offset = 0;
  for (const raw of text) {
    const start = offset;
    offset += raw.length;
    if (SKIPPED.has(raw)) continue;
    let c = QUOTES[raw] ?? raw;
    if (/\s/u.test(c)) {
      if (out === '' || out.endsWith(' ')) continue;
      c = ' ';
    }
    for (const lower of c.toLowerCase()) {
      out += lower;
      from.push(start);
      to.push(offset);
    }
  }
  return { text: out, from, to };
}

/** The passage as compared: normalized, without the punctuation and quotes around it. */
export function passageText(passage: string): string {
  return normalizeWithOffsets(passage).text.trim().replace(/^[\s.,;:!?"'()\-…]+|[\s.,;:!?"'()\-…]+$/gu, '');
}

/** Where `passage` is in the document, block by block (first occurrence). */
export function findPassage(doc: ProseMirrorNode, passage: string): { from: number; to: number } | null {
  const key = passageText(passage);
  if (!key) return null;
  let found: { from: number; to: number } | null = null;
  doc.descendants((node, pos) => {
    if (found) return false;
    if (!node.isTextblock) return true;
    const text = node.textBetween(0, node.content.size, undefined, '\uFFFC');
    const normalized = normalizeWithOffsets(text);
    const at = normalized.text.indexOf(key);
    if (at >= 0) {
      found = { from: pos + 1 + normalized.from[at], to: pos + 1 + normalized.to[at + key.length - 1] };
    }
    return false;
  });
  return found;
}

function build(doc: ProseMirrorNode, marks: PassageMark[]): DecorationSet {
  const decorations: Decoration[] = [];
  for (const mark of marks) {
    const range = findPassage(doc, mark.passage);
    if (!range) continue;
    decorations.push(Decoration.inline(range.from, range.to, {
      class: mark.lit ? 'passage-mark passage-mark--lit' : 'passage-mark',
      'data-type': mark.nature,
    }));
  }
  return DecorationSet.create(doc, decorations);
}

export const PassageMarks = Extension.create({
  name: 'passageMarks',
  addStorage() {
    return { marks: [] as PassageMark[] };
  },
  addProseMirrorPlugins() {
    const storage = this.storage as { marks: PassageMark[] };
    return [new Plugin<DecorationSet>({
      key: passageKey,
      state: {
        init: (_, state) => build(state.doc, storage.marks),
        apply(tr, previous, _old, fresh) {
          const marks = tr.getMeta(passageKey) as PassageMark[] | undefined;
          if (marks) {
            storage.marks = marks;
            return build(fresh.doc, marks);
          }
          return tr.docChanged ? build(fresh.doc, storage.marks) : previous;
        },
      },
      props: { decorations: state => passageKey.getState(state) },
    })];
  },
});

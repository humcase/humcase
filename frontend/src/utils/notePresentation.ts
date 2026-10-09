import { WarningCircle as AlertCircle, ChatCircleText, Gavel, Lightbulb, ListChecks as ListTodo, MagnifyingGlass, Question } from '@phosphor-icons/react';
import i18n from '../i18n';
import type { Note, NoteType } from '../types';
import { stripInlineMarkdown } from './inlineMarkdown';

/** Labels of note types: getters, the current language is read on every access. */
export const TYPE_LABELS: Record<NoteType, string> = {
  get note() { return i18n.t('notes.type.note'); },
  get todo() { return i18n.t('notes.type.todo'); },
  get bug() { return i18n.t('notes.type.bug'); },
  get idea() { return i18n.t('notes.type.idea'); },
  get question() { return i18n.t('notes.type.question'); },
  get research() { return i18n.t('notes.type.research'); },
  get decision() { return i18n.t('notes.type.decision'); },
};

export const TYPE_ICONS = {
  note: ChatCircleText,
  todo: ListTodo,
  bug: AlertCircle,
  idea: Lightbulb,
  question: Question,
  research: MagnifyingGlass,
  decision: Gavel,
} as const;

/** Longest preview kept: the views show two to four lines of it. */
export const PREVIEW_TEXT_LIMIT = 400;
const previews = new WeakMap<Note, string>();

/**
 * Text only: Markdown never becomes HTML inside a preview card. Computed once per version of the
 * case (a reread keeps the same object when nothing changed) and cut to what a view can show.
 */
export function notePreview(note: Note): string {
  const cached = previews.get(note);
  if (cached !== undefined) return cached;
  const preview = computePreview(note);
  const bounded = preview.length > PREVIEW_TEXT_LIMIT ? `${preview.slice(0, PREVIEW_TEXT_LIMIT).trimEnd()}…` : preview;
  previews.set(note, bounded);
  return bounded;
}

function computePreview(note: Note): string {
  // A sketch holds a drawing, shown as a picture: its scene is not text to read.
  if (note.content_kind === 'sketch') return '';
  const text = note.content
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/<[^>]+>/g, ' ')
    .replace(/^[ \t]*(```|~~~).*$/gm, '')
    // Tables: the separator line disappears, cells are read as an enumeration.
    .replace(/^[ \t]*\|?(?:[ \t]*:?-{3,}:?[ \t]*\|)+(?:[ \t]*:?-{3,}:?[ \t]*)?$/gm, '')
    .replace(/^[ \t]*\|(.*)\|[ \t]*$/gm, (_, row: string) => row.split('|').map(cell => cell.trim()).filter(Boolean).join(', '))
    .replace(/^[ \t]*([-*+]\s+)?\[[ xX]\]\s*/gm, '')
    .replace(/^[ \t]*(?:#{1,6}\s+|>\s*|[-*+]\s+|\d+\.\s+)/gm, '')
    .split(/\r?\n/)
    .map(line => stripInlineMarkdown(line).trim())
    .filter(Boolean);

  if (text[0]?.replace(/[.:;!?]+$/, '').toLocaleLowerCase() === note.title.trim().replace(/[.:;!?]+$/, '').toLocaleLowerCase()) {
    text.shift();
  }
  // A line without final punctuation (title, list item) is separated from the next one.
  return text.map((line, index) => index < text.length - 1 && !/[.!?:;,…»)]$/.test(line) ? `${line} ·` : line)
    .join(' ').replace(/\s+/g, ' ').trim();
}

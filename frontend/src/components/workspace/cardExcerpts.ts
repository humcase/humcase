import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { ReactElement } from 'react';
import { PreviewImage, PreviewLink } from './CardPreviewParts';
import { transformerUrlMarkdown } from '../../utils/imageRefs';
import type { Note } from '../../types';

/* Markdown excerpts of the cards: rendered once per text and kept, prepared ahead in idle time. */

// A card shows four lines of its excerpt: rendering more only filled the page with hidden text.
const PREVIEW_MAX_CHARS = 600;

function markdownExcerpt(content: string): string {
  if (content.length <= PREVIEW_MAX_CHARS) return content;
  const excerpt = content.slice(0, PREVIEW_MAX_CHARS);
  const lastLineBreak = excerpt.lastIndexOf('\n');
  const bounded = lastLineBreak > PREVIEW_MAX_CHARS - 200 ? excerpt.slice(0, lastLineBreak) : excerpt;
  const fencePattern = /^ {0,3}(`{3,}|~{3,})/gm;
  let openFence: string | undefined;
  for (const match of bounded.matchAll(fencePattern)) {
    const fence = match[1];
    if (!openFence) openFence = fence;
    else if (fence[0] === openFence[0] && fence.length >= openFence.length) openFence = undefined;
  }
  return openFence ? `${bounded}\n${openFence}` : bounded;
}

const previewMarkdownComponents = { a: PreviewLink, img: PreviewImage };

const EXCERPT_CACHE_LIMIT = 1000;
const excerpts = new Map<string, ReactElement>();
/**
 * A card's Markdown excerpt, rendered once per text and kept: the cards page re-renders on every
 * filter or selection and mounts every card when it opens, and parsing every card again froze it for
 * up to a second. `ReactMarkdown` is synchronous and pure, so its element can be reused.
 */
export function renderExcerpt(text: string): ReactElement {
  const cached = excerpts.get(text);
  if (cached) return cached;
  const element = ReactMarkdown({ skipHtml: true, remarkPlugins: [remarkGfm], components: previewMarkdownComponents, urlTransform: transformerUrlMarkdown, children: markdownExcerpt(text) });
  excerpts.set(text, element);
  if (excerpts.size > EXCERPT_CACHE_LIMIT) excerpts.delete(excerpts.keys().next().value as string);
  return element;
}

type IdleDeadline = { timeRemaining: () => number };
type IdleWindow = Window & { requestIdleCallback?: (callback: (deadline: IdleDeadline) => void) => number; cancelIdleCallback?: (handle: number) => void };

/**
 * Prepares the excerpts of these cases in the idle moments of the window, a few at a time, so the
 * cards page opens without parsing them. Returns a function that stops the preparation.
 */
export function warmCardExcerpts(notes: readonly Note[]): () => void {
  const idle = window as IdleWindow;
  if (!idle.requestIdleCallback) return () => {};
  const texts = notes.flatMap(note => [note.content_kind === 'sketch' ? '' : note.content, note.resolution_comment ?? '']).filter(text => text.trim() && !excerpts.has(text));
  let index = 0;
  let handle = 0;
  const step = (deadline: IdleDeadline) => {
    while (index < texts.length && deadline.timeRemaining() > 4) renderExcerpt(texts[index++]);
    if (index < texts.length) handle = idle.requestIdleCallback!(step);
  };
  handle = idle.requestIdleCallback(step);
  return () => idle.cancelIdleCallback?.(handle);
}

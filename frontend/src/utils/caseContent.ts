import type { Note } from '../types';

/**
 * Contents of the case kinds beyond text. A sketch keeps an Excalidraw scene (the `.excalidraw`
 * file format, checked on the Rust side); an image and a link stay Markdown, so every reader
 * (lists, document, agents) understands them without knowing the kind.
 */

/** Scene of an empty sketch: same text as `EMPTY_SKETCH` on the Rust side. */
export const EMPTY_SKETCH = '{"type":"excalidraw","version":2,"source":"humcase","elements":[],"appState":{},"files":{}}';

/** Largest scene a sketch keeps (5 MB), as the Rust side checks it. */
export const MAX_SKETCH_BYTES = 5 * 1024 * 1024;

export type SketchScene = {
  elements: readonly Record<string, unknown>[];
  appState: Record<string, unknown>;
  files: Record<string, unknown>;
};

/** The scene a sketch holds, or null when its content is not a drawing. */
export function sketchScene(content: string): SketchScene | null {
  try {
    const scene = JSON.parse(content) as Record<string, unknown>;
    if (!scene || scene.type !== 'excalidraw' || !Array.isArray(scene.elements)) return null;
    const object = (value: unknown) => (value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {});
    return { elements: scene.elements as Record<string, unknown>[], appState: object(scene.appState), files: object(scene.files) };
  } catch {
    return null;
  }
}

/** The address when `text` is a web address (http or https, with a host), null otherwise. */
export function webAddress(text: string): string | null {
  const value = text.trim();
  if (!value || /\s/.test(value)) return null;
  try {
    const url = new URL(value);
    return (url.protocol === 'http:' || url.protocol === 'https:') && url.hostname ? value : null;
  } catch {
    return null;
  }
}

/** Content of a new link case: its address, as a Markdown autolink on the first line. */
export function linkContent(url: string): string {
  return `<${url}>`;
}

/**
 * Address of a link case: the first web address of its content, written as an autolink, a
 * Markdown link or a bare address (an agent may write any of them).
 */
export function linkOf(note: Pick<Note, 'content'>): string | null {
  for (const match of note.content.matchAll(/<([^>\s]+)>|\]\(([^)\s]+)\)|(https?:\/\/[^\s<>)\]]+)/g)) {
    const url = webAddress(match[1] ?? match[2] ?? match[3] ?? '');
    if (url) return url;
  }
  return null;
}

/** Host shown for a link: `example.com` for `https://www.example.com/page`. */
export function linkHost(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

/** Content of a new image case: the image, then room for words under it. */
export function imageContent(title: string, reference: string): string {
  const alt = title.replace(/[[\]\\]/g, '');
  return `![${alt}](${reference})\n`;
}

/** Language of the Excalidraw interface for the language of Humcase. */
export function excalidrawLanguage(language: string): string {
  const base = language.toLowerCase().split('-')[0];
  const codes: Record<string, string> = { fr: 'fr-FR', es: 'es-ES', de: 'de-DE', it: 'it-IT', pt: 'pt-BR' };
  return codes[base] ?? 'en';
}

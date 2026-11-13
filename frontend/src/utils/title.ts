import i18n from '../i18n';

/**
 * Recognized default titles, all languages and old versions combined: the title is
 * stored in the database, a case created in English or before the switch to "case" must stay
 * recognized. List shared with the engine (`DEFAULT_TITLES`, `db/queries/notes.rs`).
 */
export const DEFAULT_TITLES = ['Nouvelle note', 'Nouvelle case', 'New note', 'New case', 'Nuevo caso', 'Neuer Fall', 'Novo caso', 'Nuovo caso'] as const;

/** Title given to a case created without a title, in the interface language. */
export function defaultTitle(): string {
  return i18n.t('notes.defaultTitle');
}

/** True if the title was never customized (empty or still a default title). */
export function isDefaultTitle(title: string | null | undefined): boolean {
  const t = (title ?? '').trim();
  return t.length === 0 || (DEFAULT_TITLES as readonly string[]).includes(t);
}

const TITLE_LENGTH = 60;

function truncate(text: string): string {
  if (text.length <= TITLE_LENGTH) return text;
  return `${text.slice(0, TITLE_LENGTH - 1).trimEnd()}…`;
}

/**
 * Derives a title from a cleaned text, **without invention or AI call**:
 * 1. the first Markdown title (`# …`) if there is one;
 * 2. otherwise the first meaningful line (bullets, quotes and numbers removed);
 * 3. `null` if the text contains nothing usable.
 */
export function deriveTitle(markdown: string): string | null {
  const lines = markdown
    .split('\n')
    .map(l => l.trim())
    .filter(l => l.length > 0);
  if (lines.length === 0) return null;

  for (const line of lines) {
    const title = /^#{1,6}\s+(.+?)\s*#*\s*$/.exec(line);
    if (title && title[1].trim()) {
      return truncate(title[1].replace(/[*_`]/g, '').trim());
    }
  }

  const first = lines[0]
    .replace(/^(?:[-*+]\s+|>\s+|\d+\.\s+)/, '')
    .replace(/[*_`]/g, '')
    .trim();
  return first ? truncate(first) : null;
}

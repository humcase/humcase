/**
 * Inline Markdown removed from a line of text (bold, italics, strikethrough, code), for the plain
 * text of previews and derived titles. An underscore inside a word is text, not emphasis
 * (`api_key`, `mon_fichier_v2`), as in CommonMark: only `_x_` and `__x__` that open and close at
 * word boundaries are markup. Asterisks, tildes and backticks are never kept.
 */
export function stripInlineMarkdown(text: string): string {
  return text
    .replace(/(^|[^\p{L}\p{N}_])__(?=\S)(.+?)(?<=\S)__(?![\p{L}\p{N}_])/gu, '$1$2')
    .replace(/(^|[^\p{L}\p{N}_])_(?=\S)(.+?)(?<=\S)_(?![\p{L}\p{N}_])/gu, '$1$2')
    .replace(/[*`~]/g, '');
}

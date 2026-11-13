// Reading the stylesheets and sources from tests that check rules across files.
// `?raw` on a .css returns an empty string under Vitest: files are read directly.
// Non-literal specifier: Node types stay out of the application's compilation.
const fsModule = 'node:fs';
type Fs = {
  readFileSync: (path: string, encoding: 'utf8') => string;
  readdirSync: (path: string, options: { recursive: true }) => string[];
};
const { readFileSync, readdirSync } = (await import(/* @vite-ignore */ fsModule)) as Fs;

/** Every file under `src/`, with `/` separators. */
export const sourceFiles = (): string[] => readdirSync('src', { recursive: true }).map(file => file.replace(/\\/g, '/'));

/** Text of a file under `src/`. */
export const readSource = (file: string): string => readFileSync(`src/${file}`, 'utf8');

/** Flat CSS rules (@media are unfolded): selector + declarations. */
export function cssRules(css: string): { selector: string; declarations: string }[] {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const output: { selector: string; declarations: string }[] = [];
  let start = 0;
  let depth = 0;
  let head = '';
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '{') {
      head = text.slice(start, i).trim();
      start = i + 1;
      if (!head.startsWith('@')) depth++;
    } else if (text[i] === '}') {
      if (depth > 0) { output.push({ selector: head, declarations: text.slice(start, i) }); depth--; }
      start = i + 1;
    }
  }
  return output;
}

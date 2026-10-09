import { describe, it, expect } from 'vitest';
import { resources } from '../i18n';
import { deriveTitle, isDefaultTitle, DEFAULT_TITLES, defaultTitle } from './title';
// Non-literal specifier: Node types stay out of the application's compilation.
const fsModule = 'node:fs';
const { readFileSync } = (await import(/* @vite-ignore */ fsModule)) as { readFileSync: (path: string, encoding: 'utf8') => string };

describe('isDefaultTitle', () => {
  it('recognizes the default title, empty or whitespace', () => {
    for (const title of DEFAULT_TITLES) expect(isDefaultTitle(title)).toBe(true);
    expect(isDefaultTitle(defaultTitle())).toBe(true);
    expect(isDefaultTitle('')).toBe(true);
    expect(isDefaultTitle('   ')).toBe(true);
    expect(isDefaultTitle(null)).toBe(true);
  });

  it('does not touch a custom title', () => {
    expect(isDefaultTitle('Corriger le crash')).toBe(false);
    expect(isDefaultTitle('Nouvelle case de réunion')).toBe(false);
  });
});

describe('default title of each language', () => {
  it('is recognized as a default title (otherwise it would never be replaced)', () => {
    for (const { translation } of Object.values(resources)) {
      expect(DEFAULT_TITLES).toContain(translation.notes.defaultTitle);
    }
  });

  // A dictation appended in the background titles the case on the engine side: both sides must
  // recognize the same default titles, or one of them would keep "New case" for good.
  it("is the same list as the engine's", () => {
    const rust = readFileSync('../src-tauri/src/db/queries/notes.rs', 'utf8');
    const list = /pub const DEFAULT_TITLES: \[&str; \d+\] = \[([^\]]*)\]/.exec(rust)?.[1] ?? '';
    expect([...list.matchAll(/"([^"]*)"/g)].map(m => m[1])).toEqual([...DEFAULT_TITLES]);
  });
});

describe('deriveTitle', () => {
  it('prefers the first Markdown title', () => {
    expect(deriveTitle('# Corriger le crash\n\nDétails…')).toBe('Corriger le crash');
    expect(deriveTitle('## Étapes\n1. …')).toBe('Étapes');
  });

  it('falls back to the first meaningful line', () => {
    expect(deriveTitle("Si cette note marche, c'est que ça fonctionne.")).toBe(
      "Si cette note marche, c'est que ça fonctionne."
    );
    expect(deriveTitle('- Première tâche\n- Seconde')).toBe('Première tâche');
    expect(deriveTitle('> Citation utile')).toBe('Citation utile');
    expect(deriveTitle('1. Étape une\n2. Étape deux')).toBe('Étape une');
  });

  it('removes inline Markdown markers and truncates', () => {
    expect(deriveTitle('**Important** : à retenir')).toBe('Important : à retenir');
    const long = 'a'.repeat(120);
    const title = deriveTitle(long);
    expect(title).not.toBeNull();
    expect(title!.length).toBeLessThanOrEqual(60);
    expect(title!.endsWith('…')).toBe(true);
  });

  it('keeps the underscores of a word, removes only emphasis', () => {
    expect(deriveTitle('vérifier mon_identifiant_api')).toBe('vérifier mon_identifiant_api');
    expect(deriveTitle('# _urgent_ : relancer __Didier__')).toBe('urgent : relancer Didier');
  });

  it('returns null on an empty text', () => {
    expect(deriveTitle('')).toBeNull();
    expect(deriveTitle('   \n\n  ')).toBeNull();
  });
});

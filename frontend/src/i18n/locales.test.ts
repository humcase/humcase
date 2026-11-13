import { describe, expect, it } from 'vitest';
import { LANGUAGES } from './language';
import enInterface from './locales/en';
import frInterface from './locales/fr';
import esInterface from './locales/es';
import deInterface from './locales/de';
import ptBRInterface from './locales/pt-BR';
import itInterface from './locales/it';
import enEngine from './locales/en/engine.json';
import frEngine from './locales/fr/engine.json';
import esEngine from './locales/es/engine.json';
import deEngine from './locales/de/engine.json';
import ptBREngine from './locales/pt-BR/engine.json';
import itEngine from './locales/it/engine.json';

type Tree = { [key: string]: string | Tree };

// The engine's texts (read by Rust) follow the same rules as the interface's.
const files: Record<string, Tree> = {
  en: { ...enInterface, engine: enEngine },
  fr: { ...frInterface, engine: frEngine },
  es: { ...esInterface, engine: esEngine },
  de: { ...deInterface, engine: deEngine },
  'pt-BR': { ...ptBRInterface, engine: ptBREngine },
  it: { ...itInterface, engine: itEngine },
};
const PLURAL_SUFFIXES = /_(zero|one|two|few|many|other)$/;

function flatten(tree: Tree, prefix = ''): Map<string, string> {
  const texts = new Map<string, string>();
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') texts.set(path, value);
    else for (const [c, v] of flatten(value, path)) texts.set(c, v);
  }
  return texts;
}

/** Key without plural suffix: each language has its own forms (English: one/other). */
const base = (key: string) => key.replace(PLURAL_SUFFIXES, '');
const variables = (text: string) => new Set([...text.matchAll(/\{\{\s*(\w+)\s*\}\}/g)].map(m => m[1]).filter(v => v !== 'count'));
/** Tags such as <strong> or <0> kept by `Trans`. */
const tags = (text: string) => [...text.matchAll(/<\/?(\w+)\s*\/?>/g)].map(m => m[0]).sort();

function byBase(texts: Map<string, string>): Map<string, Set<string>> {
  const result = new Map<string, Set<string>>();
  for (const [key, text] of texts) {
    const vars = result.get(base(key)) ?? new Set<string>();
    for (const v of variables(text)) vars.add(v);
    result.set(base(key), vars);
  }
  return result;
}

const reference = byBase(flatten(files.fr));
const english = flatten(files.en);

describe('Text files', () => {
  it('every language of the menu has its files', () => {
    expect(Object.keys(files).sort()).toEqual(LANGUAGES.map(l => l.code).sort());
  });

  for (const [language, tree] of Object.entries(files)) {
    describe(language, () => {
      const texts = flatten(tree);
      const bases = byBase(texts);

      it('has exactly the same keys as French', () => {
        expect([...bases.keys()].sort()).toEqual([...reference.keys()].sort());
      });

      it('keeps the same variables ({{title}}, {{error}}…) and tags', () => {
        for (const [key, vars] of reference) expect([key, [...(bases.get(key) ?? [])].sort()]).toEqual([key, [...vars].sort()]);
        for (const [key, text] of texts) {
          const source = english.get(key) ?? english.get(`${base(key)}_other`);
          if (source !== undefined) expect([key, tags(text)]).toEqual([key, tags(source)]);
        }
      });

      it('has every plural form the language uses, so no count falls back to English', () => {
        // i18next asks Intl which form a count takes (French and Spanish: `many` for 1,000,000).
        const categories = new Intl.PluralRules(language).resolvedOptions().pluralCategories;
        const plurals = new Set([...texts.keys()].filter(key => PLURAL_SUFFIXES.test(key)).map(base));
        for (const key of plurals) {
          for (const category of categories) expect([key, category, texts.has(`${key}_${category}`)]).toEqual([key, category, true]);
        }
      });

      it('has no empty text', () => {
        for (const [key, text] of texts) expect([key, text.trim().length > 0]).toEqual([key, true]);
      });

      if (language !== 'en') {
        it('is translated, not left in English', () => {
          // Short words can match English (« Email », « Canvas »); whole files cannot.
          const same = [...texts].filter(([key, text]) => text.length > 12 && english.get(key) === text);
          expect(same.length / texts.size).toBeLessThan(0.05);
        });
      }
    });
  }
});

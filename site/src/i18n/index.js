import * as fr from './fr.js';
import * as es from './es.js';
import * as de from './de.js';
import * as ptBR from './pt-BR.js';
import * as it from './it.js';

/**
 * The page's languages. `beta`: translated from English without a native review yet, like the app's
 * (docs/TRANSLATION.md). `shots`: the language of the app screenshots, English until a language has
 * its own example data. `locale`: Open Graph's.
 */
export const LANGS = {
  en: { name: 'English', path: '/', locale: 'en_GB', shots: 'en' },
  fr: { name: 'Français', path: '/fr/', locale: 'fr_FR', shots: 'fr' },
  es: { name: 'Español', path: '/es/', locale: 'es_ES', shots: 'en', beta: true },
  de: { name: 'Deutsch', path: '/de/', locale: 'de_DE', shots: 'en', beta: true },
  'pt-BR': { name: 'Português (Brasil)', path: '/pt-br/', locale: 'pt_BR', shots: 'en', beta: true },
  it: { name: 'Italiano', path: '/it/', locale: 'it_IT', shots: 'en', beta: true },
};

/** Each language's texts (English is written in the pages and is the key). */
const TABLES = { fr, es, de, 'pt-BR': ptBR, it };

/**
 * The translator of plain strings, for one language. Self-contained on purpose: the page also
 * serializes it (`String(makeTR)`) so its scripts translate what they build with the same rules.
 * A string missing from the table is kept as is; dates such as "5 Oct 09:02" are translated by the
 * table's `DATES` rules, and a string made of parts joined by " · " is translated part by part.
 */
export function makeTR(table, dates, lang) {
  const fill = (pattern, d, mo, t) => pattern.replace('{d}', d).replace('{m}', dates.months[mo]).replace('{t}', t || '');
  const date = s => !dates ? s : s
    .replace(/^since (\d+) (Sep|Oct)$/, (m, d, mo) => fill(dates.since, d, mo))
    .replace(/^done (\d+) (Sep|Oct)$/, (m, d, mo) => fill(dates.done, d, mo))
    .replace(/^(\d+) (Sep|Oct)( \d\d:\d\d)?$/, (m, d, mo, t) => fill(dates.day, d, mo, t));
  const one = s => (s in table ? table[s] : date(s));
  return s => {
    if (lang === 'en' || s == null) return s;
    if (s in table) return table[s];
    return s.includes(' · ') ? s.split(' · ').map(one).join(' · ') : one(s);
  };
}

/** Everything a page needs to speak `lang`: plain text, text with inline markup, image descriptions. */
export function useLang(lang) {
  const { T = {}, EL = {}, ALT = {}, DATES = null } = TABLES[lang] ?? {};
  return {
    lang,
    t: makeTR(T, DATES, lang),
    /** For `set:html`: an element's inner HTML, written in English with its markup. */
    h: html => (html in EL ? EL[html] : html),
    alt: text => (text in ALT ? ALT[text] : text),
    /** What the page's scripts need: `window.HUM_LANG` and `window.TR`. */
    runtime: `window.HUM_LANG=${JSON.stringify(lang)};window.TR=(${String(makeTR)})(${JSON.stringify(T)},${JSON.stringify(DATES)},${JSON.stringify(lang)});`,
  };
}

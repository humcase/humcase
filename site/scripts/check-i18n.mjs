// Every language of the site translates the same texts as French: same keys in EL, T and ALT, the
// same markup in each EL text, the same date rules, and no text left in English.
import * as fr from '../src/i18n/fr.js';
import { LANGS } from '../src/i18n/index.js';

const tags = html => [...html.matchAll(/<\/?([a-z]+)[^>]*>/g)].map(m => m[1]).sort().join(',');
let failures = 0;
const fail = (lang, message) => { failures += 1; console.error(`${lang}: ${message}`); };

for (const lang of Object.keys(LANGS).filter(code => code !== 'en' && code !== 'fr')) {
  const table = await import(`../src/i18n/${lang}.js`);
  for (const part of ['EL', 'T', 'ALT']) {
    const missing = Object.keys(fr[part]).filter(key => !(key in table[part]));
    const extra = Object.keys(table[part]).filter(key => !(key in fr[part]));
    if (missing.length) fail(lang, `${part}: ${missing.length} missing, first: ${JSON.stringify(missing[0])}`);
    if (extra.length) fail(lang, `${part}: ${extra.length} unknown, first: ${JSON.stringify(extra[0])}`);
    const english = Object.entries(table[part]).filter(([key, text]) => key === text && key.length > 12);
    if (english.length) fail(lang, `${part}: ${english.length} left in English, first: ${JSON.stringify(english[0][0])}`);
  }
  for (const [key, html] of Object.entries(table.EL)) if (tags(html) !== tags(key)) fail(lang, `EL markup differs: ${JSON.stringify(key)}`);
  const months = table.DATES?.months ?? {};
  if (!table.DATES || !months.Sep || !months.Oct || !['since', 'done', 'day'].every(rule => table.DATES[rule]?.includes('{d}'))) fail(lang, 'DATES incomplete');
}

// French typography: a no-break space (not a plain one) before : ; ? ! and inside « », in the French
// texts and the French pages.
const plainSpace = /(?<=\S) [:;?!](?=[\s<'"]|$)|« | »/m;
for (const part of ['EL', 'T', 'ALT']) for (const text of Object.values(fr[part])) if (plainSpace.test(text)) fail('fr', `plain space in French typography: ${JSON.stringify(text.slice(0, 80))}`);
const { readdirSync, readFileSync } = await import('node:fs');
const frPages = new URL('../src/pages/fr/', import.meta.url);
for (const file of readdirSync(frPages, { recursive: true }).filter(f => f.endsWith('.astro'))) {
  readFileSync(new URL(file.replaceAll('\\', '/'), frPages), 'utf8').split('\n').forEach((line, i) => {
    if (plainSpace.test(line)) fail('fr', `plain space in French typography: pages/fr/${file}:${i + 1}`);
  });
}

if (failures) { console.error(`${failures} problem(s)`); process.exit(1); }
console.log('Site texts: every language complete.');

// The sitemap search engines read: every page, with its versions in the other languages.
import { LANGS } from '../i18n/index.js';

const SITE = 'https://humcase.com';
const LEGAL_DOCS = ['', 'notice/', 'privacy/', 'terms/', 'license/'];

/** Groups of pages that are the same page in several languages: [hreflang, path][]. */
const groups = [
  Object.entries(LANGS).map(([code, l]) => [code, l.path]),
  ...LEGAL_DOCS.map(doc => [['en', `/legal/${doc}`], ['fr', `/fr/legal/${doc}`]]),
];

const xml = s => s.replace(/&/g, '&amp;');

export function GET() {
  const urls = groups.flatMap(group => group.map(([, path]) => [
    '  <url>',
    `    <loc>${xml(SITE + path)}</loc>`,
    ...group.map(([code, alt]) => `    <xhtml:link rel="alternate" hreflang="${code}" href="${xml(SITE + alt)}"/>`),
    '  </url>',
  ].join('\n')));
  const body = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">',
    ...urls,
    '</urlset>',
    '',
  ].join('\n');
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } });
}

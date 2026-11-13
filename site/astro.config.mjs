// @ts-check
import { defineConfig } from 'astro/config';

// A static site: two pages (English at /, French at /fr/), plain files ready for Cloudflare.
// `site` is the address the canonical and language links point to; the planned domain (internal decisions).
export default defineConfig({
  site: 'https://humcase.com',
  output: 'static',
  trailingSlash: 'ignore',
});

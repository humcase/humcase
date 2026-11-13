# Humcase website

The public page of Humcase, in English (`/`), French (`/fr/`), and in beta Spanish (`/es/`), German (`/de/`), Brazilian Portuguese (`/pt-br/`) and Italian (`/it/`), translated from English without a native review yet. Built with [Astro](https://astro.build) into plain static files, meant for Cloudflare. **Online at https://humcase.com** since 08/10/2026 (Cloudflare Pages project `humcase-site`, deployed by hand: `npm run build`, then `npx wrangler pages deploy dist --project-name humcase-site --branch main`). The links to GitHub and the download buttons (latest GitHub release) are on with `GITHUB = true` in `Landing.astro`; set it to `false` to hide them again.

```bash
npm install
npm run dev       # http://localhost:4321, reloads on save
npm run build     # static files in dist/
npm run preview   # serves dist/ as it will be online
npm run images    # rebuilds the AVIF/WebP copies after a screenshot or a maple frame changes
```

- `src/components/Landing.astro`: the page, written once in English.
- `src/i18n/<language>.js`: each language's texts, keyed by the English ones; a missing text stays in English. The page's scripts translate what they build with the same table (`window.TR`). `npm run check-i18n` checks that every language translates every text of `fr.js` (which also lists the words French writes as English) with the same markup. The screenshots stay English for the beta languages.
- `public/js/`: the scripts of the night maple (`world.js`), the "Your turn" keys and the terminal (`play.js`), the language menu. `src/scripts/keyboard.js`: the 3D keyboard (three.js, bundled; pinned to 0.170.0, whose soft shadows the design uses).
- `public/maple/`: the maple frames rendered in Blender and drawn as pixels; `public/shots/`: the app screenshots, one per language, with example data. The JPEG and the WebP frames are the sources; `npm run images` makes the lighter copies the page serves first. `public/_headers`: cache rules for Cloudflare.
- No third-party request at runtime: fonts and three.js are served by the site itself.

Download: the buttons point to `releases/latest/download/Humcase-setup-x64.exe` on GitHub (`release.yml` publishes the installer under that name) once `GITHUB = true` in `Landing.astro`; until the repository is public they say the download isn't open yet.

## Release news

The sign-up form posts to the Pages Functions in `functions/api/` (shared code in `functions-lib/news.js`): `subscribe` sends a confirmation email through Resend (sender `news@humcase.com`, domain verified in the EU region), `confirm` adds the address to the Resend segment "Humcase release news" and redirects to `/news/`. Nothing is stored before the confirmation; the link is a signed token valid 7 days.

Secrets live in the Pages project (`npx wrangler pages secret put RESEND_API_KEY|CONFIRM_SECRET --project-name humcase-site`, value read from a file or stdin), never in the repository. To try it locally: put both in `site/.dev.vars` (git-ignored), build, then `npx wrangler pages dev dist --port 8788` from `site`.

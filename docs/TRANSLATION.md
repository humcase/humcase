# Translating Humcase

The interface speaks English, French, Spanish, German, Brazilian Portuguese and Italian. English
and French are written by the maintainers; Spanish, German, Brazilian Portuguese and Italian were
translated from English (October 2026) and are marked **beta** in the language menu until a native
speaker reviews them (`beta` in `frontend/src/i18n/language.ts`).

## Files

- `frontend/src/i18n/locales/<language>/<zone>.json`: one file per interface zone. The top-level
  key is the zone's name and never changes.
- `engine.json` in the same folder: texts written by the Rust engine (errors, native menus, brief),
  embedded at compile time (`src-tauri/src/i18n.rs`).
- Adding a language: a folder with the eight files and `index.ts`, an entry in `LANGUAGES`
  (`language.ts`), `resources` (`i18n/index.ts`), `CATALOGS` (`i18n.rs`), the files of
  `locales.test.ts`, and its default case title in `DEFAULT_TITLES` (`utils/title.ts` and
  `db/queries/notes.rs`).

## Rules

1. **Translate from English**, the reference. French is the original intent: read it when the
   English is ambiguous.
2. **One term, one translation**, everywhere: the terms of [GLOSSARY](GLOSSARY.md), with the column
   of each language. A new term goes into the glossary first.
3. **Native, not word for word.** Short sentences, the address of the glossary (German « du »,
   Spanish « tú », Italian « tu », Brazilian Portuguese « você »), the language's own quotation
   marks and typography.
4. **Keep exactly**: keys, `{{variables}}` (never translated, never removed), tags such as `<0>` or
   `<strong>`, Markdown, keyboard shortcuts (`Ctrl+K`), product and brand names (Humcase, Claude
   Code, Codex, Cursor, opencode, Deepgram, Cerebras, Excalidraw, JSON Canvas, MCP), file paths
   and code.
5. **Plurals**: each language has the forms `Intl.PluralRules` gives it (`one`, `many`, `other` in
   Spanish, Italian and Brazilian Portuguese; `one`, `other` in German). `many` is only used for
   millions written in compact form: give it the text of `other`.
6. **Length**: small capitals labels (VOTRE JOURNÉE, TRACES…) and buttons stay short. German is the
   length test: when a word does not fit, choose a shorter one rather than abbreviating.
7. **The agent protocol stays in English**: MCP tool names, `get_handoff`, `note` in the engine's
   protocol texts when the English says "note".

`npm test` in `frontend` checks every language: same keys and variables as French, every plural
form, no empty text, files not left in English, default title recognized.

## Native review

A reviewer reads the interface in their language (Settings → Appearance → Interface language),
corrects the files and the glossary column, then removes `beta` for that language.

# Changelog

## [Unreleased]

### Added
- "Ask Humcase" ("Demander à Humcase"): a panel on the right of the workspace (the Ask button in the top bar, or Ctrl+J; a drawer in a narrow window). A question is answered from the cases of the space shown, sentence by sentence, each sentence with the number of its source; the sources list the case and the sentences of it the answer cites, word for word, and open the case. Humcase finds the cases on the computer (words, then meaning when it is on, then the cases the conversation already cited) and sends the AI chosen in Settings up to nine sentences of each of the seven closest cases and one line for each of the 25 latest cases (title, project, nature, status, decision waiting or not, dates). The code keeps only the sentences that name an excerpt really sent: a sentence without one, or naming one that was not sent, is dropped, and an answer left empty reads "Nothing in your cases on this subject". A follow-up keeps the last three exchanges of the conversation, for the session only. "Keep as a case" files the answer and its sources in the Inbox, marked by a provenance so it is never read as a source later (an answer of the AI is not evidence; the cases it cites are). A next step the AI proposes becomes a sub-case only on click. Without an AI, the closest cases and a "Connect an AI" link; the local server by default (Ollama) is used only when chosen. Each answer says which service received excerpts of how many cases, and each call is recorded in `ai-usage.jsonl` (`ask`, about 2,000 tokens in). The privacy policy of the website lists what is sent.
- Ask Humcase measured: `ask_eval` (ignored test, real calls) asks 32 questions in six languages on invented spaces and counts the right kind of answer, the cases cited against the expected ones, the facts a good answer states, phrases that turn a proposal into a decision, words of the data in the text, sub-cases proposed with and without reason, sentences dropped, tokens and cost (about 0.02 $ a run); `ASK_EVAL_PROMPT` compares another instruction. From the first instruction to the one kept (`ask.v7`, two runs each): facts missing from 6 of 23 to 0 to 2 of 25, sub-cases proposed without reason none, raw words such as "resolved" mostly gone, and no answer saying who proposed something when the case does not name them ("you proposed", seen in the app). The case's state is sent in plain words ("result waiting for the person's decision", "in progress") instead of its status and decision state, which the AI copied; the AI answers the part of a question the cases cover instead of nothing; a long word found nowhere is searched by its start (German compounds such as "Beratungsstunde"); raw states left in the text are removed. Each answer's check is journaled next to the database, numbers only (`ask-journal.jsonl`: sentences kept and dropped, sources, cases sent, sub-case). The provenances Humcase writes (an answer kept, its sources, Sprout) are named in the interface's language.
- Ask Humcase looks further when the first reading is weak: the AI may search the cases with other words, read a whole case or list the cases in a state (read-only tools, four more calls at most), and every excerpt a tool returns is checked like the others. A reading that already holds the question's words is answered in one call, as before. Measured on 40 questions, 8 of them about a case the first reading cannot hold (words that differ, 35 newer cases in front): the right answer on 5 or 4 of those 8 against 2 with one call, the same on the 32 others, about 1.7 calls per question.
- "Think it through" ("Réfléchir avec Humcase"): the second mode of the panel (Ask | Think it through), or the "Think it through" button in a case. Humcase restates what you said, links what your cases already say (goes this way, already decided, to reconcile) with the sentence of the case word for word, proposes a lead when you hesitate, and asks one question at a time. Dictation goes into the answer box, to read before sending. At the end it proposes a summary (direction, kept, set aside, what changed, open questions, actions) that you correct before keeping it: it becomes a sub-case of the case thought about (the Inbox otherwise) with the whole conversation, and the ticked actions become its sub-cases. A link stands only if its sentence was really sent; a kept summary is never read back as a source. A case waiting for the person's decision is sent as a proposal nothing in which is chosen, the points the person already settled are sent apart, and a question that asks one of them again is asked for once more; a link naming only a case's title is dropped. Measured by `reflect_eval` (ignored test, real calls, four conversations) and `reflect_defects` (a proposal presented as chosen: 2 of 9 replies before, 0 of 9 after; an answered question asked again: 1 of 9, then 0 of 9). Recorded as `reflect` in the AI usage.
- "Think it through" asks more specific questions and gives fewer, better leads. Measured by `reflect_quality` (ignored test, real calls): 10 invented situations (two in English) of a French base, a simulated person who answers from a hidden brief, four partner turns and a summary, every reply graded by a strict judge (specific, forward, useful, lead, links, faithful, actionable). From `reflect.v9` to `reflect.v13`: questions that name a figure, option or date of the situation 70 % to 88 %, questions worth asking 32 % to 50 %, links that really bear on what was said 52 % to 64 %; the generic questions of any checklist (what budget, what deadline, which criteria), questions that ask the person to compute, offers of help and a summary asked twice are forbidden; a lead rests on one sentence of a case or one fact the person gave, or there is none. A small space is read whole (the latest cases are sent with their sentences when the words found fewer than six), the reply is written in the language of the person's latest message (an English conversation was answered in French), a currency symbol nobody used and references such as "(case 1, excerpt 3)" are removed from the text, and a question that digs into a point the person settled or postponed is asked again up to twice with the words it repeats forbidden. The summary keeps only what the person chose (not "I could" or "probably"). Started from a case without a word of the person, the first question is about that case itself (what in it is still open for the person), not about another case (`reflect.v14`): questions worth asking 50 % to 65 %, questions that follow the latest answer 93 % to 95 %; on the kitchen tiles, Mistral reached the real hesitation (grey or wood) at once, where no model had before.
- Anthropic (Claude) as a provider (`ANTHROPIC_API_KEY`, default model `claude-haiku-5-5`, Sonnet 5.5 and Opus 5.5 priced too) through the native Messages API, since Anthropic's OpenAI-compatible endpoint ignores the structured output and the reasoning effort: the schema goes in `output_config.format`, the effort in `output_config.effort`, tool calls and results are translated both ways, thinking blocks are sent back unchanged, and a forced final tool that Sonnet 5.5 or Opus 5.5 refuses is asked again with the choice left to the model. Checked against the documented request and answer shapes only: no real call yet.
- With no AI chosen in Settings, Humcase uses the first one whose key is set (in Settings or a Windows environment variable), never the local server by default; Settings shows that one. An AI chosen without a key falls back the same way, and a model chosen for another provider is not sent to it. Before, the dictation's cleanup, Structure and Sprout asked Cerebras and failed without its key.
- Mistral as a provider (`MISTRAL_API_KEY`, default model `mistral-large-4`, priced in the consumption table; it is asked not to reason unless a call asks for it, since its default reasoning outlasted the 90 s timeout on a Think it through reply), and a reply whose `content` is a list of chunks (a `thinking` chunk, then `text` chunks) is read from its text chunks. `reflect_quality` can test another partner (`REFLECT_QUALITY_PARTNER_PROVIDER`, `_MODEL`, `_EFFORT`), stops when the whole bench has spent more than `REFLECT_QUALITY_CAP` dollars and prints the total; `mistral_smoke` checks the shapes of the reply path (strict JSON schema, tool calls) against a provider.
- The panel's width follows its left edge: drag it, or use the arrow keys on it, double-click for the default width; remembered on this computer.

### Changed
- Going public: the website's download buttons point to the installer of the latest GitHub release and its GitHub links show (`GITHUB = true`); the source code is "on GitHub" instead of "on request" (landing page and license pages). The download buttons no longer say "Preview", and the SmartScreen answer says the installer isn't signed yet. The README opens with the logo and three badges (CI, latest release, license).
- Releases: `release.yml` installs the built installer silently on the clean runner, runs the installed `Humcase.exe tools`, checks that `meaning/` came along, opens the window once on the clean profile (screenshot kept as a run artifact), then uninstalls, before publishing. The tag name reaches the scripts through an environment variable instead of being written into them.
- Releases: Linux builds (experimental) are published with each release: `release-linux.yml` builds the AppImage and the `.deb` on Ubuntu 22.04, installs the `.deb` and runs the CLI, opens the AppImage window once under a throwaway home (screenshot kept as a run artifact), then adds both to the release. Search by meaning stays off on Linux.
- CI: `ci.yml` runs on every push to `master` and every pull request again (GitHub-hosted runners are free for a public repository), and its Linux jobs (plugins, website) run on `ubuntu-latest` again: self-hosted runners are not safe for a public repository.

## [0.9.97] - 2026-10-08

First version for the public preview: Settings in six sections, the website and its download from GitHub Releases, search by meaning, Sprout and To tidy, experimental Linux builds, and the fixes of the full tour before going public.

### Added
- Linux builds (experimental): an AppImage and a `.deb`, built locally in Docker with `npm run build-linux` (`tools/linux/`), no CI minutes. On Linux:
  - the data folder is `~/.humcase` (mode 0700) and the AI settings file is written with mode 0600;
  - AI keys are sealed with XChaCha20-Poly1305 under a random key file `ai-config.key` (mode 0600) next to the settings, in place of DPAPI. Same model as DPAPI: it protects the settings file when copied alone, not against a process running under the same account. Keys written in plaintext by older files are migrated as on Windows; without the key file the stored keys must be entered again;
  - a stale AI-settings lock is detected through `/proc/<pid>` (a dead holder's lock is purged, as on Windows);
  - a startup error goes to stderr and, when `zenity` or `kdialog` exists, to a dialog;
  - search by meaning stays off: the worker (`humcase-sense`) is not built for Linux yet, and the app works without it.
- The website (`site/`, Astro), in English and French, and in beta Spanish, German, Brazilian Portuguese and Italian (marked in the language menu, with the app's glossary terms; screenshots stay English for them; `npm run check-i18n` checks every language has every text), from the landing mockup: the night maple drawn as pixels through the cycle of an idea, the "Your turn" keyboard, an agent filing research through MCP, the app's screenshots, privacy, questions. Static pages, no third-party request (fonts and three.js served by the site). The download link and the newsletter sign-up are not wired yet and say so. The headline is "Every decision has roots." ("Chaque décision a ses racines."). Order: hero, the app's home screenshot, "Why Humcase?" (decisions lost in scattered Markdown files and chats, gathered in one place), the tree, agents, the app, privacy, "Your turn", questions. A GitHub icon in the bar, a sentence under it for people who work with AI agents, "Free · Open source" under the button, an invented example everywhere (a freelancer redesigning a client's online shop, screenshots included), a rewritten FAQ, the film grain over the night only (screenshots and text stay clean), French no-break spaces before : ? ! and inside « »; the winget command stays hidden until Humcase is published there. Lighter and smoother: screenshots in AVIF and WebP at two widths (JPEG fallback), the maple frames in AVIF (WebP fallback), the hero fonts preloaded so the headline no longer jumps, the maple data no longer blocks the first paint, the keyboard's shaders compiled in the background, and a `_headers` file with the cache rules for Cloudflare (the scripts and screenshots carry a fingerprint of their content in their address, so a visitor never runs a script older than the page) (`npm run images` rebuilds the image copies). The language menu fades in and the FAQ answers slide open and closed (instant where the browser lacks it, and with reduced motion). The pixel tree keeps its dark wood and deep shade (rounded to black, every other cell vanished and the trunk read as a checkerboard). The hero's tree fills the room under the offer (a fixed size left a wide gap on tall screens and phones). Phones: the French Download button no longer overflows small screens, the bar fits down to 320 px, the chapter copy and the chapter bar hide what scrolls under them, and in "Your turn" the heading and the keys stay put so the tree stays in view while tapping. The privacy block no longer promises that everything works offline: cases do, dictation and AI need the internet.

### Changed
- Repository ready to go public: a short README (screenshot, download link, preview status and the SmartScreen warning, the six languages), the full feature list moved to `docs/FEATURES.md`, a Contributor License Agreement (`CLA.md`, a license grant, not a transfer; every contribution stays available under the AGPL-3.0) signed through CLA Assistant and named in CONTRIBUTING and the pull request template, `.github/CODEOWNERS`, `.claude/` ignored, and a note in AGENTS.md for outside contributors.
- Website texts made exact: the source code is "on request" until the repository is public; cases work offline, dictation and AI services with an API key need the internet; Sprout says it needs an AI service; archiving (not "nothing is deleted") keeps a case readable; an agent can't accept its own work (it can declare a result); the FAQ no longer announces Linux, macOS or paid options, and names Microsoft Defender SmartScreen; "Who did what" becomes "History". The sentence under the headline says what the app does ("Track your ideas, tasks and decisions, and the work your agents hand in."), the menu says "The cycle" instead of "How it grows", and the final call says "Windows 10 and 11." instead of repeating "Free". The privacy policy lists what Sprout sends (up to 4,000 words) and what TypeSafe receives for natures, Sprout and To tidy, and names LM Studio. French: "case" instead of "cas", "clé API", no-break spaces everywhere, checked by `npm run check-i18n`.
- Releases: `release.yml` publishes a release marked latest (no longer a pre-release) and adds the installer under a fixed name, `Humcase-setup-x64.exe`. The website's download buttons point to it on GitHub once the repository is public (`GITHUB = true`); until then they still say the download isn't open yet.
- Interface texts from the first-run tour, by `docs/UI_WRITING.md`: the first steps no longer show the step number twice (the badge only marks a step done); "Paste (Ctrl+V)" in French is "Coller"; the new project dialog has no description line; nature pages (Decisions, Tasks…) say "Nature" above the title and "Across the whole space, every project." under it; the empty trash no longer repeats the 30 days; "Agents and command line only." under Since your last visit; "1 similar case" without "check before continuing"; French Settings say "clé API" and "Humcase fonctionne sans IA".
- Settings: eleven sections become six, General, Dictation, AI services, Suggestions, Agents, Data. Every AI key is in AI services (the TypeSafe key left Natures to check), next to the usage; Dictation links there for its service. Suggestions gathers search by meaning, tidying, natures and the profile, and explains the TypeSafe review once (its model, Jev, is named there). The overview page is gone: Settings open on General. Tokens are explained under the usage numbers, and usage lines name TypeSafe instead of Jev. Settings have one entry, at the foot of the sidebar (the gear of the top bar is gone), and "Back to the app" sits at the foot of the Settings navigation. On a wide screen the content stays next to the navigation (no longer centered with a wide gap), still at most 960 px wide. Settings texts cut to the essential in the six languages: each idea said once, hints that repeated a heading removed (appearance, theme, microphone, agents), what is sent to TypeSafe said once at the top of Suggestions; the agent permission text keeps its exact scope in three sentences.
- Interface texts follow `docs/UI_WRITING.md` (new): exact terms, short, no claims about other companies. Six hints still pointed to Settings sections that no longer exist (Voice and AI, Memory, Profile and suggestions, Data and backups); they point to the new ones. "Smart dictation" is now "Dictate", "Stop dictation" "Stop", and the AI provider line lists the supported services with the exact terms (API key, local) instead of prices and rankings. No em dash in the session brief or the CLI line.
- The website is online at humcase.com (Cloudflare Pages). Its links to GitHub stay hidden until the repository is public.
- The website has a legal section, one page per document in English (`/legal/`) and French (`/fr/legal/`), with the list of documents beside it: legal notice (publisher as an individual entrepreneur (EI) with SIREN, SIRET and city, publication director, host), privacy policy (summary first; site, email, app, optional AI providers, transfers, GDPR rights), terms of use and open source license. The footer links to contact@humcase.com, the privacy policy, the terms and the legal notice; the other languages link to the English pages.
- New logo, the same everywhere: an orange key screen-printed "h", raised on a short shadow, with its indicator light, filling its square like other apps' icons (the app icons edge to edge, without the shadow, as large as their neighbours in the taskbar). It replaces the orange "hmm" key of the app icon and the dark "h" key of the website and the sidebar (app icons for Windows and Linux, sidebar, website favicon, headers and phone icons).
- Website: release news sign-up works, with a confirmation email (double opt-in) sent from news@humcase.com through Resend; the address joins the list only once confirmed, in the language of the page. Pages Functions `site/functions/api/subscribe.js` and `confirm.js`, result page `/news/`, privacy policy updated (English and French).
- Website: the maple no longer breaks when the page loads in an empty viewport (a hidden tab or pane, 0 x 0): its scene divided by zero, the animation stopped with an error and never restarted. The scene is now computed only once the viewport has a size.
- The website has a 404 page in the visitor's language, a sitemap with each page's languages, `robots.txt`, and PNG icons and a web manifest for phones.
- CI (`ci.yml`, by hand) also checks the website: every language has every text (`check-i18n`) and the pages build.
- Humcase reads what a case is and its line of work from its wording, without AI, in the six launch languages (EN, FR, ES, DE, IT, PT-BR):
  - the nature suggested in the editor ("Suggested nature: Task") now comes from cue phrases weighted per nature (task, idea, question, decision, bug, research), a task verb opening the note and a deadline ("before Friday", "d'ici lundi", "bis Freitag"), and is offered only when one nature clearly leads; it is still offered only for a plain thought, never against a nature you chose. It replaces a rule that looked for a few French and English words;
  - Structure and smart dictation in "Automatic" use the development, marketing or research frame when the wording clearly says it (otherwise no frame, as before; reflection stays a frame you choose); agents' `structure_note` does the same with `line_of_work` `auto`, and accepts `personal` (no frame);
  - measured on 360 short notes written in the six languages (half used to set the cues, half held out): on the held-out half, 74 % of natures right and 92 % right when one is offered, against 15 % and 75 % for the former rule; the line of work is offered for 58 % of the notes and right 96 % of the time.
- The Structure button shows a tree (organize into parts) instead of a star; in AI consumption, smart dictation shows two arrows merging.
- Cases page: the list and the board line up with the header, the filters and "Close in meaning" (their scrollbar sits in their right margin), and the row of views, sort and "New case" wraps instead of overflowing on a narrow window.
- Releases publish only the installers: `Humcase.exe` alone would lack search by meaning, and a person never downloads a second file to complete an install.
- Settings: the section menu is wider, so no section name is cut ("Data and backups", "Spaces and activities").
- Search finds what you meant even when one word differs: a case no longer needs every word you typed, nor the exact form of each. A word also finds its near forms ("liens" finds "lien", "perdus" finds "perdu"), and a case holding only some of the words is shown when it is among the best answers; very common words ("the", "de", "la") no longer bring unrelated cases up. Every case the former search found is still found. While a search is typed, the cards and the list show the best match first ("Relevance" in the sort, back to your usual sort when the search is cleared), and so do the shortcut pages (Decisions, Tasks...); agents (`list_notes`, `humcase list --search`) follow the same rule. Measured on 24 cases searched in six languages with their own words: the former search found 98 of 144, this one 144, with two cases listed in the usual search.
- AI rewrites (dictation cleanup, smart dictation, Structure) follow English instructions that never translate: the answer keeps the language of the text, and the interface language only decides when the text does not tell. An English reflection structured in a French interface is no longer turned into French, and the sections of the reflection frame are named in the language of the text. Cleanup now says what cleaning is (transcription errors, punctuation, fillers) and no longer turns an idea, a suggestion or something to look into into a task. Checked on real calls against the French instructions, three passes of 16 French and English samples.
- Code health, after the audit of 03/10:
  - the errors the interface can show on a project, an activity, a JSON Canvas import, an image or a report go through the language files (French in a French interface; unchanged in English for agents and the CLI);
  - the canvas has no search and filters of its own any more (the page's are the only ones), the sidebar keeps only the shadcn parts it uses (no mobile sheet, no Ctrl+B, no cookie), and unused styles, texts, files and dependencies are gone (individual Radix packages, autoprefixer, a second Tauri CLI); the engine's texts are no longer packed into the interface;
  - the case editor and Settings are split into hooks and parts (`components/noteEditor/`, `components/settings/`), with the same behavior and the same safeguards;
  - the remaining French names of the code are English (AI providers, tests, CSS test helpers now shared); the executable is built as `humcase.exe` instead of `app.exe`; CI builds with the locked dependencies and a pinned Rust (1.97.1), and runs by hand only (the same checks run locally before every merge) to save Actions minutes.
- Interface, after the UX audit of 03/10:
  - one header on every page: an eyebrow, the title with its count, and "New case" always at the right of the title (Home, a project, All cases, Today, the Inbox, the shortcuts); on the cases page the view switch, the sort and the archives sit beside it, the same in the canvas, the cards, the list and the board, and the cards no longer repeat a second title. The canvas's "New case" opens its draft in place, inside the case dived into;
  - "New case" creates a case of the nature shown: the nature chosen in the filters, or the shortcut's nature (Decisions create a decision). The shortcut pages get a search and a "New case" (except Documents and Drawings, which are added inside a case);
  - the nature chips come from the kit (`Chip`): the chosen one reads as a raised key, not as a hover; the sub-cases box is neutral, not orange;
  - one vocabulary to remove a case: "Move to the trash" (the editor, the canvas), then "Permanently erase" from the trash; cancelling says "Cancel";
  - every count of the sidebar counts the unresolved cases, sub-cases included (projects, Inbox, Today, shortcuts), in the same style, and says so in its tooltip;
  - Settings: one name per section, the same in the menu, on the overview cards and as the section title; Settings has a gear in the sidebar and the top bar, and "Edit the list" of shortcuts its own pencil; spaces and appearance have their own icons;
  - the nature suggested in the editor is named like everywhere else ("Suggested nature: Task"), and "type" becomes "nature" in the remaining texts;
  - French texts use the typographic apostrophe everywhere ("Aujourd’hui");
  - filing from the Inbox is a line always in view under the top bar, with kit fields, instead of a fold-out;
  - at 720 px the page header no longer leaves an empty band, and the natures scroll on one line;
  - the session brief and the daily point show in the top bar only on a project and its cases;
  - on Home, "Accept" a result is a secondary button (one primary action per page); Archive no longer announces a pressed state on top of its changing label; greyed Export and Import say why; a case without a title is named "Untitled" in the list, the trash and messages.
- Performance, after a measured audit on a space of 417 cases (2 MB of text):
  - the interface reads the database's cases once and narrows them itself for a project, a status, a nature or the sub-cases box: a click on one of them no longer asks the database again, and a reread keeps the cases that did not change, so the views keep what they computed;
  - no more reading of the projects and activities every 5 seconds, which redrew the whole window for nothing; changes made by an agent or the command line still arrive through the database watch, and coming back to the window rereads only if something changed;
  - card excerpts keep their formatting but are prepared once per version of a case, in idle time, and bounded; cards and list rows out of sight are not laid out until they come near;
  - the search waits for a short pause in typing and the board uses the database's answer like the other views;
  - measured: opening the cards view went from about 950 ms to about 55 ms, the list with sub-cases from 390 to 75 ms, the board from 200 to 65 ms, a project from 60 to 10 ms; the cards view no longer freezes twice every 5 seconds while idle.
- Cases page, after a UX audit of every path a person follows:
  - one filter bar above the canvas, the cards, the list and the board: search (with `#tag`), status with its counts, nature, an "Include sub-cases" box that says how many sub-cases it adds, "Today" and "Clear filters". A filter chosen in one view is the one shown in the others; on the canvas it fades the cases it leaves out instead of removing them, and a "Go to the first" button replaces the canvas's own search. Choosing a status or a nature brings the sub-cases along, each one saying where it lives (project › parent cases);
  - "Today" covers the whole space whatever project is open, sub-cases included, with the rule of Home's "To follow up" (in progress, overdue or due today, P0/P1). It narrows any view, the canvas included, keeps its place on a second click and is left from its chip. Home shows four of these cases and a "See all" link to Today;
  - "Back" in a case leads to the page it was opened from and names it ("Back to Home", "Back to Tasks", "Back to Today"), and the sidebar keeps that page highlighted while the case is open;
  - one breadcrumb in the top bar: project, then the parent cases, then the case (they no longer sit above the title); a project's page goes up to "All cases" instead of "My projects", which led to Home;
  - Ctrl+F and the sidebar search look through the whole space; the command palette lists every case of the space, not only the open project's; Ctrl+1 to 4 open the canvas, the cards, the list and the board, in the order of the view switch;
  - "New case" in the list and the board views.
  - the natures stay one-click chips in the filter bar (a second click clears the chosen one), with their counts;
  - the view bar, the filters and the canvas, cards, list or board start on the same page gutter, with gaps taken from the spacing scale; the list and the board scroll inside the room left under the filters, and the list no longer repeats "New case" at its foot.
- Every page has the same frame: the shortcuts by nature (Decisions, Tasks, Documents, Drawings), Home, Inbox, Trash and a project's list no longer keep a width of their own (1100, 1840 or 1248 px centred) and use the full width between the page gutters, like a project. The shortcuts' state filter uses the same control and counts as the cases page. A test fails if a page sets its own width again.
- Words: one name per status everywhere, the shortcut filters included (To do, In progress, Resolved instead of "Decided", "Done", "Closed", "Answered"...); "Kanban" becomes "Tableau" in French; a sketch is a "Dessin" / "Drawing" everywhere, like its shortcut; "Nature" instead of "Type" in the editor and quick capture; "incident" instead of "bug"; the "Brief de reprise" becomes the "Brief de session", "Reprise" being kept for a rework; "the command line" instead of "the CLI".
- Home: the last edited case offered to pick up is never resolved nor archived; Home names the quick capture shortcut (Alt+Shift+N).
- The case's "Activity" and "History" panels are one "History": who did what, then the versions that can be restored, without repeating an agent edit the journal already lists.
- MCP, after an audit against the MCP specification and Anthropic's guidance for agent tools:
  - the server instructions fit under 1,500 characters, so every rule reaches agents (Claude Code reads at most 2,048); the shape of a case lives in the `add_note` description and in the skills;
  - `list_notes` returns paged summaries (`{total, offset, limit, notes, hint}`, 50 by default, at most 200) with a short excerpt and never the raw dictation, ranks searches by relevance, filters by nature, tag, parent, due dates, last change, space and archiving, and refuses an unknown status or nature instead of answering an empty list; `list_sub_cases` is paged the same way;
  - `get_handoff` names every case by its id and shows where a sub-case sits, and lists at most 25 cases to resume while counting the rest;
  - every write returns JSON with the identifiers and the new revision, and the main tools also return structured content with an output schema; every tool has a title and annotations (read-only, destructive, idempotent, open-world) and every parameter is described;
  - protocol version 2025-11-25, and an unknown requested version gets the newest; `update_note_status` records a report without changing the status; `add_note` accepts a due date.
- CLI: `--version` and the exit codes in the help; `list` gains `--project-id`, `--search`, `--limit` and `--json`; `note` shows the case's path, decision and recent activity.
- Claude Code plugin: no pinned version any more, so a skill change reaches installed copies at the next plugin update instead of waiting for a release (a CI job checks the packaging); the executable path is chosen as a file. The skills carry the full agent guidance: duplicates first, sub-cases rather than links, archiving only after proposing it, verified results, the rework loop.
- Home reorganized around the day: **Your day** comes first, then **Since your last visit**, then the case to pick up with what to follow, then the recent cases. A case shows once, in the highest place it appears: a case of the brief is not repeated below it, and the cases to follow are not repeated among the recent cases. When everything is seen, the brief folds to one line and the rest moves up. The "Reports since yesterday" list is gone (a report is read on its case card and in the brief), and so is the projects column, which repeated the sidebar; a space without a project still offers to create the first one. Your day now counts agent writes on sub-cases too. The activities invitation moves to the bottom.
- Humcase keeps all its data in `%USERPROFILE%\.humcase` instead of AppData: database, images, session lock, AI settings, window state and the interface's WebView2 profile (local storage, escrowed drafts included). The Claude desktop app for Windows is an MSIX package and redirects the AppData writes of the processes it launches, so the Humcase MCP server wrote to a private copy the application never saw. On first launch, the AppData folders of earlier versions are copied under the session locks, verified (database integrity, schema version and row counts, every image, every file of the WebView2 profile) and left intact, with a marker file. An agent started by the Claude app never moves data: it asks to open Humcase once.
- The former name "DevNotes" and personal names are gone from the repository, the interface texts and the data files:
  - the database file is now `humcase.db`; on first launch, an existing `devnotes.db` is copied to it under the session lock, verified (integrity, schema version, row counts) and left intact next to it, with a marker file;
  - new archives store `humcase.db` and `humcase-archive://` image references; archives made earlier (`devnotes.db`, `devnotes-archive://`, `.devnotes.zip`) still inspect and restore;
  - interface preferences and escrowed drafts move to `humcase-…` keys; the former values are copied once, never removed;
  - Inbox containers are named `__humcase_inbox_…` (schema v23);
  - the isolated test profile uses `HUMCASE_TEST_DATA_DIR` and a `.humcase-isolated-profile` marker; the plugin no longer ships a personal default path.
- Code, comments, internal messages and public documentation translated to English (#39). Some Rust error messages that were hard-coded in French (image validation, transcription state, restore, data migration marker) now read in English until they move to translations.
- Rust code formatted with `cargo fmt`.
- Light theme with depth: the page and the cards were nearly the same white and the shadows barely visible, so everything melted together on a bright screen. The page stays light, cards are true white with a firmer line and a shadow that reads.
- A box on the canvas ends right after its sub-case count, instead of keeping the full height of its place with an empty space under the list.
- Dependencies updated (October 2026): Tauri 2.12 and its plugins, TypeScript 7, jsdom 30, react-day-picker 10, oxlint, Vite, Vitest, sha2 0.11, tokio-tungstenite 0.30, GitHub Actions v7. `zip` stays pinned at 0.6.6: version 8 silently drops a duplicate entry when reading an archive, which would hide it from the restore guard. The home date and the due-date calendar read the day once and again at midnight instead of on every render.
- The notes written by the `report_codebase_bug` tool use English section headings, and the server instructions and the `write-note` skill give English headings while asking agents to write them in the user's language. The quick capture window title is English in the configuration and follows the interface language at runtime. Delivery scripts, configuration comments and the whole CHANGELOG are in English; `npm run update-exe` also aligns the Claude Code plugin version.
- Dependabot proposes dependency updates once a month, grouped by ecosystem.

### Added
- The interface speaks Spanish, German, Brazilian Portuguese and Italian, marked beta in the language menu until a native speaker reviews them (Settings → Appearance → Interface language). They were translated from English with the glossary (`docs/GLOSSARY.md`, rules in `docs/TRANSLATION.md`); the engine's texts (errors, native menus, brief), the calendar of due dates and the dates of the work profile follow the same language, and the system language picks one of them on first launch.
- Tests check every language against French: same keys and variables, every plural form the language uses, no empty text, no file left in English.
- First steps on Home, until you have written a case and connected an agent, or hide them: what a case is, "Write your first case" and "Connect an agent" ticked from real data (cases written, agents that opened a session), quick capture, how work with an agent goes (you write, it reports, you decide), and a filled example ("Example · Online shop", six cases) to add and remove (archived) again; its cases do not count as your first case. The empty home no longer shows the day, "Since your last visit" or the activities invitation before there is anything to show.
- Settings → Agents explains how to connect one: Claude (Code and app), Codex, Cursor or opencode, with this Humcase's path already written into the command or configuration to copy, then what to ask the agent. It says which agent is connected and when its last session was: Humcase records each MCP session's client name and time in `agents-seen.jsonl` beside the database (names and times only), so an agent that only reads counts too.
- Agents get reminded at the right moment to keep cases tidy:
  - **Creating a case at the top level:** Humcase answers with the open cases of the project whose title is close (`existing_cases`), so the agent continues in the case you dictated instead of opening a new one.
  - **Resolving the last open sub-case:** Humcase points at the parent case (`parent_ready`), to resolve it or say what remains.

  The session rules, the MCP server instructions and the plugin's `organize-cases` skill say the same. On 05/10, 15 cases done but never closed were found, for these two reasons.
- A new case left empty no longer stays behind: it goes to the trash when you leave it (back, another case, another page). Empty means still "New case", no text, nothing dictated, no image or drawing, no sub-case, tag, priority, due date, link or report. The slightest trace keeps it. Humcase checks this at the moment it moves the case, so a write that just landed always keeps it. At launch, empty cases left for more than an hour (a crash, a window closed on them) go to the trash too. They stay there 30 days, like any deleted case.
- How you work: a work profile per space, a case of its Inbox that you edit freely and that every agent receives at the start of a session. Humcase proposes lines for it and you add the ones you want (Add to profile / No, then one button; Undo right after):
  - the decisions in force that are working rules ("Product photos always on a white background", "No client meetings on Fridays"), read by Jev when you let it read your cases; a one-off choice (buying a kiln, a supplier, a date) is not proposed. Measured on 14 decisions in six languages: all 14 sorted right;
  - your lines of work, counted from the wording of your cases, and the kinds of Sprout steps you refuse most of the time, counted on your computer without AI;
  - lines an agent proposes (new MCP tool `propose_profile_rule`), with its reason.
  A Home card counts the lines proposed and leads to the profile page; agents read the profile with the new MCP tool `get_work_profile`, and `get_handoff` carries it under "How the person works".
- What waits for a next step: on Home, at most three open questions, ideas or decisions to take per space, unchanged for five days and without a sub-case, chosen once a day without AI. "Sprout" opens the case and proposes its steps at once (a case too short for it is only opened), "Later" brings it back in a week, "Don't suggest again" stops it.
- Settings → Profile and suggestions: turn the profile proposals and the Home suggestions off, and open the profile of each space.
- To tidy, second part: filing together and checking against decisions. "To tidy" now also lists, first:
  - cases that may go against a decision in force, with the sentence of each that disagrees ("Photograph the collection on the dark wooden table" against "All product photos are taken on a white background"); you answer "I will correct the case" (it opens) or "No contradiction". The case's page shows the same warning, with "It does not" and "See the decision";
  - close cases of one project to file one inside the other ("File « Set Colissimo shipping in Shopify » into « Move the shop to Shopify »"), the direction swappable with one click; a likely duplicate says so, and once filed the copy can go to the trash.
  Close cases are found on your computer by meaning; Jev (when it reads your cases) judges whether two cases belong together and whether a case goes against a decision, up to 12 pairs per request (about $0.0001, recorded as `check_gather`). Without Jev, only duplicates are proposed: nearly the same words (80 % of their words in common, numbers kept whole so two reports of different versions differ; never two resolved cases, which archiving covers), found without AI; closeness by meaning alone cannot tell two cases of one subject from two cases of one field (0.90 to 0.96 for both on a real space). Decisions are never filed with a case: they are compared with cases instead. Measured on 22 pairs in the six languages (`gather_eval`, real calls on a synthetic bench): together 0.80 to 0.94, apart 0.13 at most; conflicts 0.90 to 0.97, the others 0.16 at most. Settings → Memory gains "Propose to file close cases together"; "Compare decisions: replaced or contradicted" covers both checks.
- Project summary: a project's page switches between its cases and its summary: the decisions in force (with their first line; the replaced ones counted as history), the cases in progress, the open questions, what was resolved these two weeks, and a warning when a case may go against a decision, leading to "To tidy". Read from the cases each time it opens. Agents read the same with the new MCP tool `get_project_summary`, and `get_handoff` now lists the decisions in force (at most 10) and the cases that may go against one.
- To tidy ("À ranger"): Humcase proposes to put away what no longer needs your attention, and never tidies alone. On Home, a card says what waits ("2 decisions a newer one may replace, 3 cases resolved more than 4 weeks ago. Nothing is deleted.") and opens the page where you answer:
  - decisions a newer decision may replace, the older one above the newer one with a line of each; you answer "Yes, it is replaced" or "No, both hold". A replaced decision stays readable: its page says "History: replaced on …" with the decision in force one click away and "It still holds" to take the mark back; the newer one says "Replaces …"; search lists replaced decisions after the others; agents read `superseded_by` and `replaces` in `get_note`, and `get_handoff` tags them;
  - cases resolved and untouched for more than 2, 4 or 8 weeks (Settings → Memory, 4 by default, or never), top-level only, without a sub-case still open, checked by default: they are archived (still found under "Archived");
  - one button applies every answer, Undo right after takes them back, and what you refused is not proposed again;
  - when you write a decision that may replace an older one, its page asks right away ("Does it replace the one of 28/09?").
  Pairs of decisions are found on your computer by the words of their titles and by meaning; when Jev reads your cases (Settings → Natures to check) it judges up to 12 pairs in one request whether the newer one replaces the older one, so a decision that only adds to another is not proposed. Without Jev, Humcase proposes no pair itself: words and meaning cannot tell a replacement from an addition (the first pairs proposed that way on a real space were all additions); agents still can, and the pairs are judged once Jev may read. Measured on 15 pairs in the six languages (`tidy_eval`, real calls): Jev reads 0.88 to 0.97 for every replacement, at most 0.12 for a decision that adds to or confirms another, and at most 0.04 for unrelated ones; about $0.00003 per request, recorded in AI consumption (`check_decisions`). Agents read the list with `list_tidy` and propose a replacement with `propose_superseded` (MCP and CLI); only you answer. Answers and marks live in the database (v27: `proposals` kinds `archive` and `supersede`, `case_supersessions`, carried by archives).
- Sprout ("Faire germer"): in a case, Humcase proposes its next steps as sub-cases (an action to carry out, a question to answer, something to look for or compare, a choice still open, an idea), each quoting the passage of the case it comes from, marked in the text with its nature's color. You check, correct the nature or the title, refuse; nothing is created before you answer, and the sub-cases created keep their passage as their provenance (Undo right after takes them back). A case that may already cover a step is shown beside it ("Already exists"): a top-level case of the project can be put here instead, another one stays where it is and the step can be left out or created anyway. Every passage must be in the case word for word, or the proposal is set aside and counted; when Jev reads your cases (Settings → Natures to check), it also checks in one request that each step follows from its passage and that a near case really covers it: a doubt is shown ("To check", left unchecked) and a step it reads as invented is set aside. Proposals wait in the case until you answer (Resume), and agents propose in the same panel with `propose_sub_cases` and read them with `list_proposals` (MCP and CLI); only you answer. One call to the AI chosen in Settings per click, recorded in AI consumption (`germinate`, and `check_proposals` for Jev). Measured on 12 cases in the six languages (`germinate_eval`, real calls): every passage quoted word for word; a recipe gets no step and a one-line case one, where a first version of the instruction padded them to seven; a title names what its passage refers to ("compare the prices of the two" names the two options), so a step reads on its own and its near case is found; about $0.0008 per click with gpt-oss-120b. Proposals and answers live in the database (v26 `proposals`, carried by archives).
- Natures to check: Jev, TypeSafe's model, reads your cases again and Humcase proposes another nature when one fits better ("06 cases may have another nature" on Home, then one case at a time: its nature now, the one proposed and why; Enter changes it, G keeps it, O opens the case). Nothing changes without you, an answer is remembered and a kept nature is never proposed again, nor is a nature you chose in the interface. Off until you turn it on in Settings → Natures to check with a TypeSafe key (or `TYPESAFE_API_KEY`): the title and first 300 words of each case are sent, without keys or email addresses, once per version of the case; a case being written waits two minutes. Every reading is counted in AI consumption. Agents read the proposals with `list_nature_proposals` (MCP and CLI), read-only. Measured on 04/10 (`tools/nature-review`): Jev gives the known nature of 341 of 360 notes written in the six languages; on a real base of 450 cases, neither the wording nor the cases closest in meaning proposed right (Jev agreed with about one proposal in six), so the review waits for Jev's sure readings (both orders of the options, confidence 0.9: 40 proposals out of 450). Answers live in the database (v25 `nature_checks`, carried by archives); readings in `nature-readings.sqlite`, a cache next to it.
- Search by meaning: a search also finds the cases that say the same thing in other words or in another language ("the links disappear when I save" finds "Sauvegarde : les liens se perdent"). Humcase reads your cases with its own model (Granite Embedding 97M multilingual R2, by IBM), on your computer: nothing is sent. It ships with the application, is on by default and reads the cases in the background at low priority (150 cases in under a minute), then only the cases that change. "Close in meaning" sits under the search with the cases the words did not find: when the words found cases, only the cases that stand out clearly are added. Meaning also orders the word results ("Relevance"), on the cases page and the shortcut pages: a case the words found far down comes up when it is close in meaning. When only meaning finds cases, the cards and the list say "No case holds these words" instead of "No case for these filters". Measured through the application on 150 cases per language, six languages, 144 searches worded without the case's words: the case sought is in the first five rows or under "Close in meaning" 94 times (70 with words alone) and first 58 times (36); searches typed with the case's words still find it first (144 of 144); for 60 unrelated searches, "Close in meaning" shows something 13 times. Settings → Search by meaning says where the reading stands and turns it off. Agents ask the same index with `find_by_meaning` (MCP and CLI). The model runs in a separate program (`humcase-sense.exe`, with Microsoft's official ONNX Runtime and the Visual C++ runtime, in `meaning/` next to Humcase.exe): a computer that cannot run it only loses this search. The index (`meaning-index.sqlite`, next to the database) holds only vectors derived from the cases and is rebuilt when deleted.
- A bench of local embedding models before search by meaning (`tools/embed-bench`, not part of the application): time until the first vector, memory, time per case and per query on one thread, and the expected case in the first five results for 288 queries in six languages, said without the case's words or typed with them, within one language (150 cases translated whole) and across languages (446 real cases), against Humcase's current search and a word search where any word counts. Granite Embedding 97M multilingual R2 is kept: 97 of 144 queries said without the case's words within one language (e5-small 86, the current search 0) and 77 across languages (e5-small 52), ready in 0.8 s, 6 ms per query, 182 MB while it works; it will run in a separate worker so a machine that cannot run it only loses this search. The current search misses a third of the cases even with their own words (98 of 144), where counting any word finds them all (`tools/embed-bench/RESULTS.md`).
- Reminders so an agent does not forget to track its work: `Humcase.exe hook <event>`, run by the hooks of Claude Code (CLI and desktop app) and Codex declared in the user's settings (snippets in `docs/AGENT_SETUP.md`). At the start of a session the agent receives the tracking rules and the brief; after a commit or a merge, a reminder to resolve the case with a report once verified; and when a turn changed files while no case was written since the session started, the agent is asked once to update its case or to say the work is not tracked. Humcase never writes in the agent's place, and a failing hook prints nothing.
- Settings → AI consumption: what your dictations and AI calls consumed on this computer, over 7 days, 30 days or this month. It shows the dictated minutes, the AI calls, the tokens sent and received (the reasoning among them), the minutes per day, the detail by use (transcription, smart dictation, Structure, cleanup, agents) and by provider and model, the language of each dictation included. Quantities only: no price is shown, whatever the model.
- What the AI consumes is recorded: every call (dictation cleanup, smart dictation, Structure, structuring asked by an agent) and every dictation session writes a line to `ai-usage.jsonl`, next to the database: the provider and model, the tokens sent and received (the reasoning among them), the minutes of audio, how long it took and an estimated cost from prices dated 03/10/2026 (Cerebras, OpenAI, Deepgram). Numbers only: no text, key or case. The prompt test bench prints the same figures per sample and in total, and can try another model or reasoning effort. Measured on 16 samples: about $0.0006 per call with gpt-oss-120b, against $0.0077 for one minute of dictation; a lower reasoning effort costs 43 % less but lost or added content, and qwen-3.8-27b costs six times more for no better result, so gpt-oss-120b stays as is.
- Filing an Inbox case straight into a case of a project: after the project, choose one of its cases (or its top level).
- The count beside a project in the sidebar says what it counts (open tasks and issues).
- The canvas filters by status (To do, In progress, Resolved) beside the nature filter: matching cases stand out, the others fade but keep their place.
- MCP tools `get_brief` (results waiting for the person's decision, reworks with their reason, agent writes; it only reads), `get_case_activity`, `find_similar_notes`, `archive_case`, `update_project` (rename, color) and `set_case_positions`, and the prompts `resume-work` and `write-note` for clients without the Humcase skills. Deciding on a result, marking as seen and deleting stay with the person.
- `deliver-results` skill (resolve after verification with a report, handle reworks without ever deciding on one's own result), a Codex package of the skills, and `docs/AGENT_SETUP.md` to connect Claude Code, the Claude desktop app, Codex, opencode or Cursor.
- **Shortcuts by nature** in the sidebar, under **All cases** (an arrow folds them): **Decisions**, **Tasks**, **Documents** and **Drawings** gather every case of that nature across the space, sub-cases included, each with where it lives (project › parent cases) and filters by state (to decide / decided, to do / in progress / done, links / files / images). Drawings show as pictures. **Edit the list** adds Ideas, Questions, Research or Issues; the choice and the fold are remembered on this computer. A shortcut moves nothing: a case opens in its place, and the back button returns to the shortcut.
- **Who did what on a case** (schema v24): every write on a case is recorded with its author: you in the interface, the agent named by its MCP client (Claude Code, Codex, OpenCode…), or the CLI (named by `HUMCASE_ACTOR` when set). The database keeps this journal itself, through triggers installed on each connection, so no write path can forget it; consecutive edits of the same author are merged into one entry, and the journal is append-only. A case shows its **last change** (who, when) under its title and, in its **History**, the entries listing creations, edits (with the fields changed), status changes, reports, moves, trash and archive, side by side for you and the agents. Writes made before this version are not attributed, and the History says so.
- **Your decision on a result**, distinct from the status an agent declares: when an agent sets a case to **Resolved**, a band under its title says "Result declared by Claude Code" with **Accept** and **Ask for a rework**. Accepting records the version you reviewed; a later write by an agent shows "Accepted, then changed: review again". Asking for a rework needs a reason, numbers the rework (rework #2…), puts the case back to **To do**, and shows "Rework #1 handed back" when the agent resolves it again. A decision is refused if the case changed since you reviewed it, and only the interface can record one: an agent never decides on its own result. A result and your decision are ordered by the case's version, not the clock, so two writes in the same millisecond keep their order.
- **Since your last visit**, on the home screen: the results declared by agents that wait for your decision (with their report and the buttons), what agents and the CLI wrote that you have not opened yet (sub-cases an agent created are counted under their case), and the reworks waiting for an agent with your reason. Each case says where it is: its project, with its color, then its parent cases ("Distribution › Landing page"), and the results of sub-cases are listed too. Your own writes are not listed and the period is stated: since your last **Accept all**, or the last 24 hours. **Accept all** (after a confirmation) accepts every listed result as shown, like its own Accept; a result that changed since is not accepted and stays to review. Opening a case or deciding on it takes it out of the list. A case the brief shows (result to review, rework waiting) is not repeated among the reports below it. The day timeline counts every agent write of the journal.
- Agents see your decisions: `get_handoff` lists the reworks first ("Rework requested (do these first)") with your reason, and marks each latest decision as awaiting your decision, accepted, or accepted then changed; `get_note` returns `review` (state, rework count, last decision) and the case's `recent_activity`. The server instructions tell agents that resolving declares a result and how to handle a rework.
- A box on the canvas answers when pointed at, like a card: a firmer line, its surface and a shadow.
- Rich contents in cases: inside a case, a new sub-case is **Text**, **Sketch**, **Image** or **Link**, chosen on its draft card.
  - **Sketch**: a full Excalidraw editor (MIT) opens on the whole page, in the interface's language and theme. The drawing is written a moment after each change, when leaving the case, before a backup and when the window closes; a write that fails is kept for recovery like the editor's drafts. The canvas shows a small picture of the drawing, and the document overview too. Excalidraw and its fonts ship inside the app and load only when a sketch is shown: nothing is fetched from the internet (the Chinese, Japanese and Korean font is left out; those scripts use a system font). The drawing is kept in the `.excalidraw` file format: the Rust side refuses any other content for a sketch (editor, agent, structuring), and a sketch over 5 MB is not written.
  - **Image**: choose a PNG or JPEG file, paste an image on the canvas (Ctrl+V) or drop image files on it; the image is copied into Humcase like a pasted image and shows on the card.
  - **Link**: type a web address, or paste or drop one on the canvas; the card shows the site and opens it in the default browser. Only http and https addresses open (checked on the Rust side), never a file or a program. Links of the document overview and of a drawing open in the browser too, instead of replacing the app in its window.
  - Files dropped elsewhere in the window are ignored instead of opening in place of the app.
- Agents ask Humcase to structure: new MCP and CLI tool `structure_note`. An agent gives its line of work (development, marketing, research, reflection or auto) and Humcase structures the case itself with the matching frame of Structurer, through the AI chosen in its settings, and applies it. It needs the same authorization as agent editing and the version read (`expected_revision`); a result that loses a link, an image or most of the words is refused, and the replaced version stays in the case's history, restorable. The server instructions and the `write-note` skill point to it.
- JSON Canvas import and export: **Export** saves the board shown as a `.canvas` file (JSON Canvas 1.0, an open format read by other note-taking and whiteboard apps), and **Import** adds the cases of such a file to the board, all or none. A case with sub-cases goes out as a group and a group comes in as a case holding what is drawn inside it; links, files and texts keep their place; a sketch travels inside its text node, so a Humcase board comes back whole. Edges are not kept. New MCP and CLI tools `export_json_canvas` and `import_json_canvas`.
- Agents organize the work in cases: the MCP server instructions, the tool descriptions and a new `organize-cases` skill of the Claude Code plugin tell an agent to file what belongs to one subject (the idea, its incidents, tasks, questions, research and decisions) inside its case as sub-cases, to propose a parent case and the moves before tidying loose cases, and to keep linked tasks for cross-references between subjects. The `write-note` skill offers the seven natures and files tasks as sub-cases; `resume-work` reads the sub-cases.
- Sub-cases (schema v22): a case can hold other cases, to any depth, each with a content kind (text, image, link, file, sketch, research). The tree is kept by the database: a sub-case stays in its parent's project and a case never goes inside itself or its sub-cases. Trashing a case takes its whole tree, a sub-case comes back from the trash after its parent, and purging a parent keeps its sub-cases as top-level cases. Filing a case from the Inbox takes its whole tree. Archives carry and check the tree; older archives restore with every case at top level.
- Provenance of a case (where it comes from: source, link, agent research), append-only.
- Canvas of cases: the cases page opens on a **Canvas** (new default view, beside Cards, List and Board; the choice is remembered). Cases are cards placed freely: drag one to move it (its place is saved, the case's revision advances but not its modification date), drag the background to pan. A case never placed is laid out automatically in the visible width. **Double-click a case or its "N sub-cases" button to dive in**: its sub-cases form their own canvas, with the path above to climb back; the canvas keeps its place while a case is open in the editor. A click on the title opens the case. **New case**, or a double-click on an empty spot, opens a card right on the canvas to name the case (Enter creates it there, Esc cancels); on All cases, which spans several projects, it keeps the usual flow. Inside a case, its own title, status and text stay shown above its canvas with **Write in the case**, and an empty case offers its first sub-case. Cases never placed keep their order (oldest first), so a new case never pushes the others. **Document overview** shows beside the canvas the case and all its sub-cases as one readable page, each section opening its original. Cards show the content kind (research, image, link, file, sketch), the status, an excerpt and the first local image.
- MCP and CLI: new `set_case_position` tool to place a case on its parent's canvas.
- Natures of a case: each case says what it is, **Thought, Idea, Task, Question, Research, Decision** or Issue, chosen right on its draft card on the canvas (a thought by default) and in the editor and quick capture. The former type "Note" is now shown as **Thought**; existing cases keep their values (`note`, `todo`, `idea`, `bug`), and `question`, `research` and `decision` are new values of the same field (no schema change). The nature shows with its own color on cards, and the canvas can be filtered by nature.
- Canvas for many cases:
  - **zoom** with the wheel or the − and + buttons, back to 100% on the level, and **Show all**, which fits the whole canvas (it starts top left when it cannot all fit). Zoomed out, a card shows its nature and title written large enough to read, and a group becomes one tile with its number of sub-cases by nature. A canvas of more than 24 cases is laid out on a wider grid, so the whole of it is a block and not a strip;
  - **search the canvas**: the matching cases stand out with their count, the others fade, Enter brings the first one to the middle;
  - **groups**: a case with sub-cases shows open on the canvas as a compact list, like a file explorer: up to four sub-cases at the size of a card, the first ten when it has more, two rows high. When a case placed by hand sits right below, the group keeps a card's height and unfolds over it while pointed at or focused, without moving it. Shift + click selects several cases, **Group** puts them inside a new case named on the spot, which takes the place of the first one;
  - **drag a case into another**, like a file into a folder: the case under the pointer lights up, and refuses (red outline) a drop into the dragged case itself, one of its sub-cases or another project. Drop a case on a level of the path to move it up; drag a sub-case out of its group onto an empty spot to put it there. Every move and grouping offers **Undo** for ten seconds;
  - **delete from the canvas**: a click selects a case, **Delete** (or Backspace, or the trash button of the selection bar) asks to move the selection to the trash, naming how many sub-cases go with it, and **Enter** confirms (Enter on Keep keeps), leaving the level drops the question; **Undo** brings everything back to its place. Undoing a grouping keeps the group if another case went into it since;
  - the cases around never move: a case arriving gets a free place, and cases that would slide (into the room left by cases put away, or away from a group growing to two rows) are pinned where they were, in one write.
- MCP and CLI: `note_type` accepts `question`, `research` and `decision`. A case moved to another parent forgets its place on the canvas it left and is placed again where it arrives.
- Sub-cases in the interface: the lists, the board and Home show top-level cases only, each card counting its sub-cases; a search still finds a sub-case and "Today" keeps every urgency. An open case shows its path (clickable ancestors) above the title and, in its Follow-up column, a **Sub-cases** panel: open a sub-case, create one (it opens right away), move the case up one level, and read its provenance (kind, label, link, exact excerpt, agent and date). The editor's pending input is saved before a move. A sub-case added by an agent appears without reopening the case.
- MCP and CLI: `add_note` takes `parent_id` and `content_kind`; new `move_case`, `list_sub_cases` and `add_provenance`; `get_note` returns the case path, direct sub-case count and provenance.
- Connect an AI: Settings → Voice and AI accepts the server address of a local model or custom server (Ollama, LM Studio, any OpenAI-compatible server; `http://localhost:11434` is completed to the API path), **Test connection** lists the provider's models without sending any content, and a short guide suggests free, local and most capable options. The address is stored in `ai-config.json` under the same lock as the keys; older files are read unchanged.
- Without a connected AI, **Structure** is visible but locked with a **Connect an AI** link, and a dictation is appended as dictated instead of failing (raw dictation always kept).
- CLI on par with MCP: `humcase tools` lists every MCP tool and `humcase call <tool> --args '<json>'` runs it with the same safeguards (agent edit permission, expected revision, raw dictation never editable); `handoff` and `projects` shortcuts. The tool list has a single definition shared by MCP and the CLI.
- AGPL-3.0 license, Code of Conduct, security policy, issue and pull request templates.
- CI on pull requests that change code: Rust fmt, clippy and tests; frontend tests, lint and build. Docs-only changes and pushes to master do not run it.

### Fixed
- Today with no case says so in every view: "Nothing urgent today" and what shows up there (cases in progress, overdue, due today or high priority), in place of an empty board of three columns or a faded canvas. Cards and list used a shorter message of their own.
- A new case opens on its title, the default "New case" selected so typing replaces it, and Enter goes on in the text: the first line typed no longer shows twice (as the title and as the first line of the text). An existing case still opens on its text, and a case started in the text or by dictation still takes its title from the first line.
- "Similar case" no longer pairs two cases that share only a common word ("pour", "with") or a single word such as a weekday: a case is similar when it shares two words of the text (the only one when the text has one), common words left out in the six languages (`common_words.rs`, also used by To tidy). The editor and MCP's `find_similar_notes` now run the same check (the editor used the search, which keeps every word when the database is small).
- Search: the filter counts (statuses, natures, sub-cases) count what the search finds, as a click on the filter shows; they kept counting every case.
- A case opens at its top: the editor stays mounted from case to case and kept the previous case's scroll, hiding the title.
- Trash: the count is the number of rows; a sub-case trashed with its case was counted apart ("2" for one row).
- Home with the first steps: "New case" stays in the header once the first case exists (step 1 no longer offers it, and the only other one was at the bottom of the page).
- French counts of a million or more ("1 000 000 cases") showed in English: French has a third plural form for them, now written in every French text.
- "New case" opens with the cursor in the text: you type at once (a case opened from a list stays without focus).
- AI consumption names every operation it records (Sprout and the checks by Jev of proposals, decisions, cases to file together and profile suggestions showed their raw name and a default icon); one list of operations in the engine, and a test fails when one has no label. Transcription at 0 min no longer shows a full bar.
- French texts: "1 projet", "1 case", "1 image" in a backup preview (were plural), "1 lien retiré" / "2 liens retirés", unsaved writes and restore errors without "(s)" nor long dashes.
- Dates and times follow one convention across the interface. In English they used the American one (4:48 PM, month/day: a case due on 7 October read "10/07") while the day ruler counted in 24 hours and the date fields followed Windows. Now the interface language decides, with the computer's region when Windows speaks that language: English on a US Windows keeps 4:48 PM and 10/07, English on any other Windows writes 16:48 and 07/10, French writes 16:48 and 07/10. The day ruler reads "8 AM" only where the clock counts 12 hours, and the current time no longer wraps on two lines.
- Due date: the field is Humcase's own instead of the one Windows draws, so it follows the interface like every other date. In English on a French Windows it read "jj/mm/aaaa"; it now shows dd/mm/yyyy (mm/dd/yyyy on a US computer), takes a typed day in that order (31/12/2026, 31.12.26, 31122026 or 2026-12-31), keeps the saved date when the text is not a real day, and Escape cancels the typing. The calendar starts the week on Monday unless the computer is American.
- Canvas: the help line at the bottom wraps instead of being cut on both sides when it is longer than the canvas (French, narrow windows).
- Cases page: "Include sub-cases" is one choice of yours, unticked at first and kept across launches. Choosing a status (All, To do, In progress, Resolved), a nature or "Clear filters" no longer changes it; before, a status or a nature ticked and locked it. The nature counts follow the box, like the status counts, so a count is what a click shows.
- Jev on a real base (04/10): the natures to check no longer read a task as a decision because it starts with the choice it carries out, a step of a plan as a thought, a lot of a plan as an idea, nor a report as a decision (question `nature.v2`: 45 proposals on a real base of 489 cases become 26, the false ones gone; every case is read again once, about $0.03). The work profile no longer proposes as "rules" the setup of one service (hours, retention), a batch of answers or decisions still to validate (question `profile.v2`), nor a decision To tidy shows as maybe replaced; the rules waiting from the earlier question are read again.
- Found by the audit of lots 1 to 6 (04/10):
  - Sprout: Undo of a created sub-case puts it in the trash with the sub-cases added under it since (it left them live under a case in the trash); the work profile is never offered as a near case.
  - To tidy: accepting a second "newer decision" for a decision already replaced no longer overwrites the first (Undo lost it); filing an open case under a case to archive in the same answer never archives over open work; a proposal that no longer holds (a case reopened, filed, archived or trashed meanwhile) is not shown nor applied, and is counted as "no longer held".
  - To tidy: duplicates found by words alone are "to check" instead of pre-accepted, ignore the headings of a template and the most common words, and need titles on one subject; sketches, images, links and files are never compared by their raw content; the look at every pair runs again only when a case, a proposal or a setting changed (it held the database every two minutes), and closeness is computed on the 400 most recent cases of a project.
  - Work profile: a line already in the profile is never proposed again (after the case it was counted on left the trash for good, a duplicate came back); counted lines are anchored on the profile once it exists; a section written in French is found after switching to English (no second heading); Undo removes the line it added, not an identical one written by the person, and the heading it created; a line the person changed stays answered; the counts read written text only, at most every 10 minutes; a withdrawn rule can be proposed again when its decision is in force again; an agent's line is refused when the person turned proposals off.
  - Home: a case of the Inbox reads "Inbox · Work" instead of its technical name (Nudges, To tidy, the brief); the count of proposed profile lines covers both spaces in the combined view; the cards read again when a setting of the profile, the nudges or To tidy changes; "Sprout" asked from a nudge no longer stays armed for a later opening; the Summary of a project stays chosen.
  - Natures: Jev's reading stops as soon as it is turned off (no case is sent after that, the "Off" state stays), a case Jev fails to read no longer stops the others, and an answer is refused while a restore runs; Enter on a focused button of the review no longer changes the nature; the review asks for the space shown; Settings says that the same access checks Sprout, To tidy and the work profile.
  - Search by meaning stops reading as soon as it is turned off and keeps the "Off" state; a brief names a case that may go against another project's decision even when no decision of its own project is in force; the remaining English errors of To tidy and Sprout go through the language files; Jev is pinned to `jev-1.13.0` everywhere.
- Dictation is no longer always transcribed as French: it follows the interface language, or the language chosen in Settings → Voice and AI → "Dictation language" (English, French, Spanish, German, Italian, Brazilian Portuguese, or several languages in one dictation). An English dictation sent as French came back empty. Transcription moves from Deepgram Nova-2 to Nova-3, which covers these languages; "several languages" costs about 20 % more ($0.0092 instead of $0.0077 per minute, regular rates of 03/10/2026). Checked by streaming spoken French and English through the application's own transcription commands.
- A database held by another program, a read-only data folder, a full disk or a damaged database file is explained in the interface language (with what to do) instead of SQLite's English text.
- Bugs found by the audit of 03/10:
  - a dictation appended to a case while another one was open never gave it its title: the interface sent the title under a name the engine did not read (`titreAuto`). A test now checks every interface call against the engine's command parameters;
  - "New case" now sits on the title's line on every page, also where a description follows the title (Home, the trash); in a narrow window the natures really scroll on one line (the rule lost to the kit's chips);
  - the canvas, a shortcut page and every view before the database answers search like the database: whole words, accents ignored, the dictation and the report included, so the count no longer changes when the answer arrives;
  - a drawing whose write fails is tried again (after a pause, on the next stroke, when the case is left or the window closes) instead of living only in the recovery list, and that list shows it at once in the text editor; when local storage is full, the drawing says it will not survive closing Humcase;
  - filing an Inbox case into a case of a project is one transaction: should the case refuse it, the note stays in the Inbox and the filing can be tried again, instead of landing at the top of the project; a draft of another case no longer blocks the filing;
  - "Back" after the first capture leads to the Inbox it created instead of "The inbox is empty", and no longer restores an activity deleted meanwhile (every project disappeared); opening an Inbox that does not exist yet leaves Today and highlights that Inbox;
  - choosing a status or a nature no longer ticks "Include sub-cases" for good: the box shows ticked and locked, "included by the filter", and the person's own choice comes back when the filter is cleared;
  - a sub-case of the Inbox no longer offers "Move to a project", which was always refused: it follows its parent;
  - MCP and the command line no longer crash on a date that is not ASCII (`updated_since`, `since`);
  - "Empty the trash" erases only the cases it showed: one trashed between the confirmation and the erasure stays;
  - quick capture no longer offers an open drawing as a destination, where adding text always failed.
- The status and search chosen in Cards or List no longer act on the canvas without showing: the canvas shares them, shows them in its own filters and only fades the cases they leave out, instead of removing them from the board.
- Canvas with 1,000 cases: only the cases within one screen of the visible area are rendered (the others are drawn as the board moves to them), so panning, dragging a case and zooming no longer freeze the window (long tasks of 100 to 220 ms before, none after; Show all still takes about 80 ms once).
- Dragging the canvas or a case no longer selects the text of the page.
- The undo offer of the canvas waits while a selection or a confirmation hides it, instead of running out unseen.
- The title of a case is styled again as designed (larger, wrapping): its style targeted a label that the "case" wording had renamed.
- Keyboard keys in tooltips follow the interface language (Shift in English, Maj in French), and a few Rust messages (image format, transcription state, restore) go through translations.
- AI settings: a write waiting for the settings lock no longer fails while other writes keep taking it in turn; it fails only when one holder keeps it for 5 seconds (the intermittent CI failure of `concurrent_writes_do_not_lose_keys`).
- Ctrl+N and Ctrl+F also work with Caps Lock on.
- **Empty the trash** erases only what its confirmation counted: a case trashed after the dialog opened is shown first, with a message, and nothing is erased until a new confirmation.
- Canvas: a position read that started before a move can no longer put the moved case back; a drag or a pan cancelled by the system (touch, pen, gesture) or left by closing the canvas ends cleanly, without writing.
- Deleting a case right after typing in it saves the typing first, instead of reporting a failed save on a case already in the trash.
- CLI: in a terminal, the released `Humcase.exe` now prints its output (it is a Windows GUI program and printed nothing unless its output was redirected); `list` prints `in_progress`, `todo`… instead of internal names, and `add --priority` accepts only P0 to P3, like the MCP.
- CI also runs when the Claude Code plugin or the delivery scripts change.
- Restoring an archive never moves a case's revision backwards: a case whose restored version differs from the one in place takes a revision past both, so an agent holding a revision read before the restore gets a conflict instead of writing over the restored text. A case restored identical keeps its revision (restoring the same archive twice still changes nothing).
- The interface and the MCP server writing at the same moment no longer fail with "database is locked": every write path takes the write lock at its start and waits for the other process instead of failing when it committed in between.
- The automatic purge of the trash reads its 30-day cutoff once, so a case can no longer be erased without its trace when it crosses the cutoff during the purge.
- The MCP server answers a line that is not UTF-8 with a parse error and a request that fails internally with an internal error, and the session goes on; it used to stop, and the agent lost its tools.
- Adding a provenance to a case advances the case's revision, in one transaction, so the interface sees it; label, url and excerpt have a maximum length.
- `report_codebase_bug` refuses a report without `file_path` instead of recording "Unknown".
- The agent editing permission in Settings now says that it also covers the due date, which `update_note` can change.
- A case holding sub-cases could not be deleted ("move them out or delete them first"). Moving it to the trash now takes its whole tree at the same instant (each case's revision advances), the confirmation says how many sub-cases go with it, the trash shows it as one row "with N sub-cases", and restoring it brings back the sub-cases that left with it (not a sub-case trashed on its own before).
- Canvas with 200 cases: panning, zooming and dragging stayed slow, as every pointer move laid out every case again. The canvas now moves directly and renders the cases once the gesture ends, the automatic layout no longer restarts from the first slot for each case, and the excerpts of the cards are computed once per version of a case: no task over 50 ms while panning or dropping a case among 200.
- Canvas: **New case** and **Document overview** sat against the view switcher; they moved to the end of the canvas's own controls. Switching project now opens the canvas at its origin instead of keeping the shift of the previous project.
- A sub-case order test could fail when two cases were created in the same millisecond.
- Quick capture: removed a listener for a "restore finished" signal that nothing ever sent. A successful restore restarts the whole application, and the quick capture already falls back to the Inbox when its remembered project is not in the restored data; a test now covers it.
- Settings showed every AI provider as "no key required": the provider's key variable was renamed on the Rust side only during the English translation (`cle_env` → `key_env`). Both sides now use `key_env`.
- All frontend lint warnings (fast-refresh exports, a ref written during render, a random skeleton width); CI now fails on any lint warning. GitHub Actions moved to their Node 24 versions.

## [0.9.96] - 2026-09-30

Test tooling fix, no effect for the user.

- **Isolated test profile**: backup archives (manual ones and safety ones before a restore) stay in the profile folder's `backups`, never in Documents or on the Desktop. Before, a test recipe could add test archives to the real "Humcase Backups" folder (never overwriting any: an existing archive is refused). A test fails if the protection is removed.
- **Verified**: archive then restore in an isolated profile; both archives are in the profile, the real backup folder is unchanged.

## [0.9.95] - 2026-09-30

Live tracking of projects and areas written by an agent.

- **Interface up to date without reloading**: a project or an area created, renamed, archived, deleted or attached by an MCP agent or the CLI appears while the window stays open. The change fingerprint (`notes_change_marker`) now covers cases, projects, areas and attachments; when it changes, the interface rereads the three lists.
- **Typing preserved**: the draft of the open case is neither emptied nor reloaded by these rereads.
- **Verified**: in an isolated profile, with a case open and being typed in while an MCP agent creates a project and an area, attaches then renames the area; each change is visible within about a second, the typed text and the focus stay, and the case is saved in full. Prepared by a cloud session, reviewed and delivered locally.

## [0.9.94] - 2026-09-30

Batch S2 of the synchronization preparation: no deletion without a trace. No visible change.

- **Deletion traces** (schema v21): deleting a project, an area, a case → task link or a project ↔ area attachment leaves a dated trace, including when the deletion comes from a cascade or a trigger (space change, permanent erasure of a case). Recreating the item erases its trace. The traces of erased cases (`note_tombstones`) are unchanged.
- **Archives**: database format v21, the traces travel in the archive; a restore adds no trace for its own deletions and gives back exactly those of the archive. Archives v9 to v20 remain restorable.
- **Verified**: migration of a copy of the real database (identical data, empty trace table), scenario deletion → archive → deletions → restore in an isolated profile (only the archive's trace remains). Prepared by a cloud session, reviewed and delivered locally.

## [0.9.93] - 2026-09-29

Quality: no more "state modified in an effect" lint warning.

- **Trash**: the first load writes the state only when the response arrives, and not at all if the view was left in the meantime.
- **Narrow window detection** (sidebar): correct value from the first render, read directly from the media query; new test.
- No behavior change expected; tests, lint and build green.

## [0.9.92] - 2026-09-29

Batch S6 of the synchronization preparation: raw dictation becomes an append-only journal. No visible change.

- **Dictation fragments** (schema v20): every dictation append also writes a fragment, in the same transaction as the raw dictation. A fragment can be neither modified nor deleted; only the permanent erasure of the case removes it. At migration, every case carrying a dictation receives a fragment identical to its raw dictation.
- **Raw dictation never replaced**: updating a case now refuses `raw_content` (interface, escrow replay); dictation only gets appended.
- **Archives**: database format v20, fragments archived and restored; archiving and restoring refuse a database whose fragments do not recompose the raw dictation. Archives v9 to v19 remain restorable.
- **Verified**: migration of a copy of the real database (identical data, 9 fragments equal to the 9 dictations), appends through the real path in an isolated profile, direct modifications refused. Prepared by a cloud session, reviewed and delivered locally.

## [0.9.91] - 2026-09-29

Batch S3 of the synchronization preparation: image registry and duplicate-free restore.

- **Archive restore**: an image keeps its original name if it is free or if the file present is identical (same fingerprint). Restoring the same archive twice no longer duplicates images nor rewrites cases. Only a real collision (same name, different content) still renames the image.
  The restore safeguards (safety archive, classification of failures before/during the commit) are unchanged.
- **Image registry** (schema v19): name, fingerprint and size of each image, filled at save, at restore and at startup from the existing folder. An unreadable file is reported, never deleted.
- **Archives**: database format v19; archives v9 to v18 remain restorable.
- **Verified**: real restore in an isolated profile (copy of the database): no image duplicated, content intact. Prepared by a cloud session, reviewed and delivered locally.

## [0.9.90] - 2026-09-29

First batch of the synchronization preparation (S1): projects and areas now have a version, like cases. No visible change.

- **Schema v18**: `revision` and `updated_at` on projects and areas; every modification (name, color, space, archiving, attachments, deletion) advances them, including when a trigger removes attachments. Existing data kept (version 0, creation date).
- **Archives**: database format v18; archives v9 to v17 remain restorable. An earlier version of Humcase refuses a v18 database.
- **Verified** on a copy of the real database: identical data, idempotent migration. Safety copy of the database made before the update.
- Prepared by a cloud session, reviewed and delivered locally. The Windows symbolic-link test is reserved to Windows (test compilation under Linux).

## Synchronization study - 2026-09-29

- Read-only study of what prevents a future synchronization between devices (SYNC_PREPARATION, a private study document): versions, deletions, conflicts, raw dictation, images, local data. Conflict rules chosen by the founder (decision journal). No change to the application.

## [0.9.89] - 2026-09-29

Small fixes, prepared for the first time by a cloud session then verified and delivered locally.

- **Missing API key**: the interface recognizes it through a stable code returned by the engine (`[ai_key_missing]`, removed at display), and no longer by looking for a variable name in the translated text.
- **Board**: column titles follow the interface language (they were frozen at load).
- **Quality**: three state updates moved out of React effects (the "Archived" view, resuming a linked-task draft, the microphone list), no behavior change.
- **Build**: the reproducible build option `/Brepro` is reserved to Windows; it prevented building and testing the engine under Linux (cloud sessions).

## [0.9.88] - 2026-09-29

Vocabulary pass: the central element is now called a **case**.

- **"note" becomes "case"** throughout the interface, in French ("une case", "Nouvelle case", "Toutes les cases") and in English ("New case", "All cases"). The "Note" type remains, next to Task, Issue and Idea.
- **Renamed in French**: "Brief de reprise" (instead of "Brief Handoff"), "Point du jour" (instead of "Daily Standup"), "Mode mini (épinglé)" (no more "Post-it", a 3M trademark).
- **"Incident" type** everywhere in French (the interface used to say "Bug" sometimes, "Problème" other times); the handoff brief names the types in French.
- **Default title**: a case created without a title is called "Nouvelle case" ("New case" in English). Former "Nouvelle note" titles are still recognized: the case still takes the title derived from its text.
- **MCP and CLI unchanged**: tools, fields and agent messages keep the word "note".
- Texts rewritten by a low-cost model (DeepSeek v4.1 Flash via OpenCode, isolated copy of the repository) on written instructions, then reviewed one by one.

## [0.9.87] - 2026-09-29

Engine messages follow the interface language.

- **Engine messages**: errors, backup and restore, AI settings, dictation, handoff brief and daily recap, startup dialogs and the notification icon menu go through the same language files as the interface (`engine.json`, Rust macro `tr!`); French unchanged, native English. At startup, the Windows language; afterwards, the one chosen in the interface.
- **MCP and CLI** stay in English (the agents' contract), whatever the interface language.
- **Fix**: a pending write on a note deleted in the meantime is reported as such ("note not found") instead of a new save failure; the old test compared a text the engine did not produce.
- **Robustness**: the interface no longer recognizes errors by reading their text (restore, duplicate project, missing API key); the engine returns a complete message or an explicit result.
- Still in French: the instructions sent to AI models (to handle separately, with verification) and the native date field (Windows language).

## [0.9.86] - 2026-09-29

Multilingual foundation: the interface exists in French and English.

- **Interface language**: Humcase uses the Windows language if it is available, otherwise English. Settings → Appearance → "Interface language" lets you force one; the change is immediate, including in the quick capture window.
- **French unchanged**: every text goes through language files (i18next) without changing a character. **English** written at the same time following the validated glossary (`docs/GLOSSARY.md`); dates and times follow the language.
- Still in French: error messages produced by the engine (Rust), and the native date field, which follows the Windows language.
- Preventive fix: the "Template" menu no longer depends on a translated label (it would have disappeared in English).
- Glossary and product vision ("let an idea germinate") recorded; 23 tests added (389 in total).

## [0.9.85] - 2026-09-29

Audit batch 2: splitting large files and missing tests. No change of behavior, schema or MCP protocol.

- **Rust**: `commands/mod.rs` (1,566 lines) split into modules by theme (projects, notes, AI, backup, images, dictation); `db/queries.rs` (1,170 lines) becomes `db/queries/` (projects, notes, update, trash, handoff), with the same call paths.
- **Interface**: `App.tsx` goes from 1,223 to 602 lines (logic in `hooks/`, windows and bars as components); `NoteEditor.tsx` from 1,865 to 1,593 lines (pure editable state and display in `components/noteEditor/`). Dictation and editor writes stay in place.
- **Tests**: 5 Rust tests and 30 interface tests added (space preferences, project dialogs, briefs, partial payload and unsaved dictation remainder...); key behaviors were counter-verified by deliberately breaking them.

## [0.9.84] - 2026-09-28

Hygiene after the repository audit (batch 1). No change of schema, MCP protocol or visible feature.

- **Archive restore**: an image that is referenced but missing from the archive produces an error before any write, instead of stopping the application. Dedicated test.
- **Dead code removed**: `get_env_var` command (no longer exposed anything), five unused shadcn components, dependencies `@radix-ui/react-dropdown-menu` and `@radix-ui/react-tooltip` (not imported). `@tiptap/extension-list`, which is used, is now declared.
- **Quality**: clippy warnings fixed (including the opening of lock files, now without explicit truncation), consistent React effect dependencies (`App`, `NoteRelations`), page-title rule deduplicated (displayed values unchanged), HTML pages in `lang="fr"`.
- **Documentation**: handoff, milestones, strategy, roadmap and README realigned on 0.9.84 / schema v17; `AGENTS.md` slimmed down, isolated test recipe and delivery in `docs/CONTRIBUTING.md`; renaming plan archived, old references removed; accent described as orange everywhere; multilingual decision (system language, English by default) recorded; Claude Code plugin version aligned.

## [0.9.83] - 2026-09-28

Find in a note, capture to the open note.

- **Ctrl+F in a note** opens "Find in note" instead of doing nothing: occurrences highlighted (ignoring case and accents), counter, Enter / Shift+Enter to move from one to the next, Escape to return to the text on the chosen occurrence. The selected text becomes the search. Nothing is written in the note. Elsewhere, Ctrl+F remains the notes search.
- **Quick capture to the open note**: when a note is displayed, the capture offers "Open note · <title>" as a destination. The text joins the note's draft (an input not yet saved is kept), then it is saved normally. The capture empties its field only after the main window's confirmation; with no answer, it keeps the text and reports it.
- No change of schema or MCP protocol.

## [0.9.82] - 2026-09-28

Trash.

- **Deleting a note places it in the trash** instead of erasing it. It stays there 30 days with its links, history and images; "Restore" (Trash page) or "Undo" (right after the deletion) bring it back identical, in its project. Before, "Undo" relied on an in-memory copy lost when the message closed.
- **"Trash" page** in the sidebar: deleted notes, days remaining, Restore, Erase permanently (confirmation) and "Empty trash" (confirmation).
- **Automatic erasure** of notes in the trash for more than 30 days, at application launch (never through MCP or the CLI). A minimal trace (identifier, project, dates, no content) is kept for the future synchronization.
- **Protection**: a note in the trash appears in no list, search, count or resume, and cannot be modified (editor, agents, dictation); SQLite refuses the write whatever the path. An agent receives `note is in the trash`.
- **Schema v17** (`deleted_at`, `note_tombstones`); archives keep the trash and its images.

## [0.9.81] - 2026-09-28

Portable and lighter images.

- **Portable reference**: a pasted image is saved in the note as `humcase-image://<name>` instead of a local address specific to the computer and the data folder. The display (editor, cards, tracking) translates it to the application's `images` folder. Prepares synchronization and the web version.
- **Schema migration (v16)**: former internal image addresses (current folder and former `DevNotes` folder) are converted in the content, the raw text and the report of notes; other links are not touched. The revision of the affected notes advances, the modification date stays the same.
- **Compression at save**: a PNG capture becomes a lossless WebP (-52% on average over 90 real captures, identical pixels); a JPEG photo is straightened, stripped of its EXIF metadata and reduced beyond 3840 px. The original file is kept if the result is not lighter or if the image does not decode. Images already saved are not recompressed.
- **Archives**: export and restore go through the portable reference; a reference whose file is missing makes the export fail, as before.
- **Fix**: the paragraph following an image is no longer glued to it at save (`![image](…)Next`).
- MCP: `get_note` and `list_notes` return the portable reference; no tool modified.

## [0.9.80] - 2026-09-28

Smart dictation: the note is structured at the end of the dictation.

- **At the end of a dictation**, a single AI call reworks the whole note: the dictation is integrated at the right place, transcription errors are corrected and the note is structured with the frame chosen in "Structure". The previous version joins the note's history, and a message offers **Undo**. The raw dictated text is always saved before the call.
- **Safeguards**: if the text changed during the call, if the note is no longer displayed, or if the response loses an image, a link or a large part of the note, nothing is replaced and the cleaned dictation is appended at the end, as before.
- **Setting** "Structure the note at the end of the dictation" in Voice and AI, enabled by default.
- **Dictation journal**: `dictation-journal.jsonl` next to the database (dictation, note before, response, duration, provider, error), limited to 2 MB; it never leaves the computer.
- Test profile: the notification icon is titled "Humcase (test)".
- No change of schema or MCP protocol.

## [0.9.79] - 2026-09-28

Live tracking of agents.

- **Fix**: a note created, modified, resolved, archived or deleted by an agent (MCP, CLI) appears in the open application within a few seconds, even if the window stays in the foreground. Before, notes were reread only when the focus came back: a note resolved by an agent stayed displayed as "In progress".
- Every 4 seconds the application compares a fingerprint of the notes in the database (count, revisions, last modification) and rereads the notes only if it changed; nothing is read when the window is hidden.
- No change of schema or MCP protocol.

## [0.9.78] - 2026-09-28

Structure: history, undo and frames.

- **No more stacking**: restructuring a note no longer copies the previous version to the end of the raw text. The replaced version joins the note's **history** ("replaced version"), with a **Restore this version** button. Copies already stacked in the raw text stay intact (the raw text is never erased).
- **Undo**: after "Apply", a message offers to undo for a few seconds; the undo goes through the same check (refused if the text changed in the meantime).
- **Frames** in the structuring window: Automatic (by project), **Reflection summary** (direction, decisions, rejected, evolution of ideas, ideas, open questions, to explore, next actions), Development, Marketing, Research. The choice is remembered; changing frame restarts the proposal.
- Structuring prompt: the name of the project, space or areas is no longer copied into the note, and no qualifier is added.
- The history is titled "History"; structuring versions do not count as agent writes (day timeline, modification alerts).
- No change of schema.

## [0.9.77] - 2026-09-28

Archive a note.

- **Archive / Unarchive** in the note page, next to "Delete". An archived note leaves the cards, the list, the Board, the home, "Today" and the counters; nothing is erased and its status does not change.
- **"Archived · n"** in the notes display bar: shows only the archived notes of the project or space; the view returns to active notes by itself when none remain. Search also finds archived notes.
- Archiving advances the note's revision (agent control), not its modification date. Undoing a deletion also restores the archive state.
- MCP: notes carry `archived_at`; `get_handoff` no longer offers an archived note, or a note of an archived project, "to resume".
- CI: a release run is titled "Release vX.Y.Z" instead of the message of the commit targeted by the tag.
- No change of schema (column added in v15).

## [0.9.76] - 2026-09-28

Manage your projects: rename, color, archive, delete.

- **"Project" window** (a project's "…" button in the sidebar): name, color, areas, **Archive** and **Delete…**. The name and color change everywhere; notes stay in place.
- **Archive** hides the project and its notes from the lists, the home and "All notes", without erasing anything. Archived projects remain openable from the sidebar's "Archived" section, their notes remain findable by search, and "Unarchive" makes them active again.
- **Delete** goes through a confirmation. No note is erased: all join the Inbox of the project's space, with their links to their tasks, and a message says how many.
- **Database v15**: archive columns for projects and notes (the notes one will be used in the next batch), and a safeguard that refuses in the database the deletion of a project that still has notes. Backups follow the new schema; older archives remain restorable.
- **Cursor**: the sidebar buttons (and any clickable element) show the hand again on hover.
- MCP: `list_projects` returns `archived_at`; no new tool. An earlier version of Humcase refuses a v15 database.

## [0.9.75] - 2026-09-28

One Humcase window at a time.

- **Single instance**: relaunching Humcase (shortcut, Start menu, double-click on `Humcase.exe`) brings the already open window to the front, even if hidden in the notification area or minimized, instead of opening a second process. The MCP server (`Humcase.exe mcp`) and the CLI are not concerned: several agents can still work in parallel.
- **Restart after restore**: the instance lock is released before relaunching, so the new window does not take itself for a second instance.
- An isolated test instance (debug build with `DEVNOTES_TEST_DATA_DIR`) can still run alongside the installed application.
- No change of data, schema or MCP protocol.

## Tooling - 2026-09-28

- **Private GitHub release** (`.github/workflows/release.yml`): triggered only by pushing a `v*` tag or by hand, never on every PR. Checks that the tag matches the latest CHANGELOG version and the version files (`scripts/release-check.mjs`), runs the frontend and Rust tests once, builds, then publishes a pre-release visible only to repository members: `Humcase.exe`, NSIS and MSI installers, SHA256 fingerprints, notes taken from the CHANGELOG. The manual run does the same chain without publishing (binaries as artifacts for 7 days).
- No change to the application.

## [0.9.74] - 2026-09-28

End of the Humcase renaming.

- **Archives**: new backups are named `humcase-….humcase.zip` (and `humcase-avant-restauration-….humcase.zip` for the safety backup); `.devnotes.zip` archives remain restorable, the internal format is unchanged.
- **Executable**: only `Humcase.exe` is delivered. The transition copy `DevNotes.exe` is removed; Codex and opencode now launch `Humcase.exe mcp` under the key `humcase`.
- **Log messages** prefixed with `humcase:`.
- **Documentation**: Humcase as the current name (AGENTS, README, docs); the internal identifiers kept (`devnotes.db`, preference keys, events) are flagged as such.
- No change of data, schema or MCP protocol.

## [0.9.73] - 2026-09-28

Technical migration to Humcase, steps 4 and 5: data folder and backups.

- **Data folder**: `%APPDATA%\com.humcase.desktop` (identifier `com.humcase.desktop`). At first launch, the data of the former `DevNotes` folder is **copied** under an exclusive lock: database (consistent SQLite copy, WAL included, integrity check and table counts), images, AI keys, window settings and the window's local storage (theme, spaces, areas, escrowed drafts). The former folder stays intact and receives a `MIGRATED-TO-com.humcase.desktop.txt` file. If the copy fails or another session holds the data, Humcase refuses to start with a message instead of opening an empty database.
- **Backups**: new archives go to "Humcase Backups"; the former ones in "DevNotes Backups" remain restorable. Internal image links are recognized in the former and the new folder.
- **Repository**: renamed `bendee1/humcase` on GitHub (former addresses redirect).
- Trial on a copy of the real data: 269 notes, 3 projects, links and areas identical, integrity "ok"; preferences reread by the window.
- No change of schema or MCP protocol.

## [0.9.72] - 2026-09-28

Technical migration to Humcase, steps 1 and 3 of the renaming plan (without touching the data).

- **Executable**: `Humcase.exe`. An identical copy `DevNotes.exe` is still delivered during the transition for MCP clients already configured (Codex, opencode). `npm run update-exe` replaces both and checks both fingerprints. The product name (installer) becomes Humcase; the CLI presents itself as `humcase`.
- **Claude Code plugin**: now launches `Humcase.exe` by default.
- **Messages**: restore blocks speak of "quick capture" instead of "Quick Capture".
- Data folder, backups and repository unchanged (steps 4 to 6 to come). No change of schema or MCP protocol.

## [0.9.71] - 2026-09-28

"Humcase" Claude Code plugin and writing guidelines for agents.

- **MCP server**: it presents itself under the name "Humcase" (step 2 of the renaming plan) and sends every client its guidelines (`instructions`): reload the context, avoid duplicates, write like "Structure" with the frame of the line of work, link tasks, resolve only after verification with a report. Tool messages cite "Humcase settings".
- **Plugin** `plugins/humcase` (local marketplace `humcase-local` at the repository root): MCP server under the key `humcase`, executable path requested at activation; skills `write-note` (Development, Marketing, Design and research, Personal frames) and `resume-work` (brief, notes in progress, linked tasks).
- Existing clients unchanged: Codex and opencode still launch `DevNotes.exe mcp` under their key `devnotes`.
- No change of data, schema or MCP tool.

## [0.9.70] - 2026-09-28

Batch A - reliability: the 15 findings of the agent review of 0.9.69.

- **Writing**: Ctrl+F in a note no longer leaves the note and no longer steals the focus. The note page's breadcrumb shows the actual project (or "Inbox") instead of "My notes". The note content is in a main landmark (`main`) for screen readers.
- **Inbox**: the project chosen to file a note is reset when changing note, instead of being displayed without being applied.
- **Loading**: the Notes view and the reduced list display "Loading notes…" instead of a misleading empty state.
- **Errors made visible** (they were swallowed): "Your day" timeline when agent writes could not be read; unreadable agent history, distinct from an empty history; unreadable AI provider list, with "Retry"; reloading areas after a creation, a rename, a deletion or a filing; resizing of Mini mode.
- **Waiting**: Handoff Brief and Daily Standup show an indicator and do not restart during generation.
- **Empty states**: the command palette offers "Clear search" and the List view "Clear filters". A microphone without a name is displayed as "Unknown microphone" without empty parentheses.
- **Startup**: if Humcase cannot start (for example a second instance), an error window explains it instead of a silent exit. The report's case could not be reproduced.
- **Kit**: screen sheets no longer repaint kit components (formatting bar, quick capture, settings, dictation card); shadcn tokens and raw colors replaced by theme tokens (relations, history, report, note warnings); unused former sidebar rules removed. `ui/kit-overrides.test.ts` fails if a screen sheet repaints a kit component.
- No change of data, schema or MCP protocol.

## [0.9.69] - 2026-09-28

- **Drop-down lists**: the open list (paragraph style, priority, type, space, areas, settings, windows) takes the kit's design in both themes (raised surface, gray hover, chosen option checked) instead of Windows' white menu with a blue hover. It opens downward and flips only if space is lacking. Keyboard and values unchanged. Quick capture keeps the system menu, which may overflow its small window.
- The paragraph selector of the formatting bar no longer overrides the kit's rule; a test prevents it from coming back.
- No change of data, schema or MCP protocol.

## [0.9.68] - 2026-09-28

Design rework, batch 16 - coverage, accessibility, design system freeze.

- **Coverage**: all screens and states walked through in both themes at 720, 1440, 1920 and 2560 px and in mini, including the first launch with no note. Every screen is built with the kit's components.
- **First launch**: "All notes" in cards no longer announces "This project awaits its first note" outside a project ("No note yet"); the empty List view invites you to create a note instead of a "No note found" line. Filters with no result keep their message.
- **Accessibility**: text contrast ≥ 4.5:1 everywhere in both themes (light warning text and sidebar project counters fixed); visible focus at every tab stop; all animations turned off under "reduce motion", except the loading indicator.
- **Fixes**: Kanban columns squashed at 720 px; Kanban card header that overflowed (it now wraps); paragraph selector without chevron in mini; field in error signaled by the kit; collapsed navigation veil aligned with the windows'; former Tailwind color patch and orphan field component removed.
- **Kit showcase** (development only): `frontend/kit.html` shows each component in each state and both themes; absent from the delivered application, a test guarantees it.
- **Documentation**: `DESIGN_SYSTEM.md` rewritten as a reference; rules for adding a component in `DECISIONS.md`. System frozen: stable and complete, **not aesthetically validated** by the founder.
- No change of data, schema or MCP protocol.

## [0.9.67] - 2026-09-28

Design rework, batch 15 - last kit components.

- **Windows**: a single modal window for project and note creation, a project's areas, the brief and the daily recap, frames, the structure proposal, the report, settings and the palette. Focus, Escape and focus return behave the same everywhere; the report draft, confirmations and restore keep exactly their protections.
- **Badges**: status, priority, type, due date, tags and counters share a single component; counters take the object's small display everywhere.
- **Cards and rows**: note cards, Kanban cards, settings sections, List view, "To continue", reports and home projects on two shared components (card, row). Same hover and same focus everywhere; a row's hover no longer lights up when aiming at a tag.
- **Empty states**: a single empty state, three keys fanned out around the icon (inbox, filters with no result, first project, first note); discreet version for columns, the list, the palette and linked tasks.
- **Messages**: undoing a deletion becomes a pill with a remaining-time ring; read and action errors and settings feedback move to kit messages. Fixes the white-on-white message in light mode.
- **Cleanup**: orphaned CSS removed (former badges, cards, rows, empty states, windows); no more raw Tailwind colors in screens. New kit variants: navigation button, extra-compact button, writing field, tinted icon tile.
- No change of data, schema or MCP protocol.

## [0.9.66] - 2026-09-28

Design rework, batch 14 - refined universe, tooltips with shortcuts.

- **Calmer home**: mono capitals are reserved for the object's landmarks (date, "Your day" screen, counters, cartouche); section titles (To continue, Your projects, Reports...) go back to DM Sans. The "To continue" counter takes the same display as the others. The cartouche strip fits on one line ("Last modified").
- **Compact timeline**: a calm day (three periods or fewer) is displayed on a single line, notes in gray and agent writes in orange with a legend; "Detail by origin" reopens the two lines. The origin is announced in each period's label. Solid orange "now" marker.
- **Note page**: the formatting bar becomes a keyboard tray; an active formatting stays pressed and lights its indicator, like the logo key. Paragraph selector with the kit's chevron.
- **Tooltips**: new kit component (pattern adapted from 21st, cnippet-dev) that names the action and shows its shortcut as small keys; on the formatting bar, the icon buttons of the top bar and "New note" (Ctrl N). Every announced shortcut is checked by a test. The sidebar and palette shortcuts use the same keys.
- **Light mode**: more relief (more readable key edge, two-layer shadow, more recessed LCD screen).
- **PR review** by an agent: no regression on the data safeguards; due date function that became unused removed.
- No change of data, schema or MCP protocol.

## [0.9.65] - 2026-09-28

Design rework, batch 13 - the whole interface on a single component kit (shadcn/Radix).

- **Humcase kit**: buttons, segmented controls, menus, search and fields now have a single definition (`components/ui/`), wired to the state scale. Same hover, same press, same focus everywhere, in both themes. The former competing button sheets are removed.
- **Screens wired**: top bar, view selector, home (areas as tabs, recents filter), notes (status filters with counters, search, sort), list, note page (display, status, priority, type, due date, toolbar, frames, links and linked tasks, deletion confirmation), settings (navigation, fields, alerts), area preferences, sidebar and dialogs.
- **Menus**: sort, note status and frames move to the kit's Radix menu (consistent keyboard, focus and theme).
- **Logo**: "h" and "hmm" legends vectorized from DM Mono; the key no longer depends on the font loading.
- **Name**: quick capture window and menu, CLI help and session messages under the Humcase name. Technical renaming plan (data folder, binary, MCP) drafted for review: `docs/RENAME_PLAN.md`, nothing is applied.
- No change of data or MCP protocol.

## [0.9.64] - 2026-09-28

Design rework, batch 12 - states visible everywhere, Humcase brand.

- **States wired**: a single state scale per theme (hover, press, selection, key face, key edge), read by shadcn (`--accent`, `--sidebar-accent`) and by all the house hover rules. Before, each rule took its own color (white, surface, raised...), invisible in light mode, and several hovers never applied (layered component rules beaten by unlayered overrides).
- **Light mode**: secondary keys on a crisp gray face with a readable edge, visible hover and press; note page fields bordered; sidebar areas filter detached from the background; Sort, Space and Status menus with a neutral hover and a distinct selection.
- **Navigation bar**: hover no longer has the same color as the active item (gray on hover, light raised pill for the active one), in both themes.
- **Humcase**: "h" key light or dark depending on the theme and HUMCASE wordmark (HUM in orange) in the sidebar; window title, mini mode, notification area, settings and resume report under the new name; application icon as an orange "hmm" key ("h" up to 32 px). Technical identifiers unchanged: `DevNotes.exe`, `devnotes.db`, AppData folder, MCP, CLI, archives.
- No change of behavior or data.

## [0.9.63] - 2026-09-28

Design rework, batch 11 - light mode rebuilt on a proven surface recipe (founder's choice).

- **Light mode**: near-white page (#F9F9F8), cards one step lighter (#FDFDFC) with a crisp line (#D0CFCB) and a discreet shadow; gray reserved for the frame and sidebar (#F1F0EF), hovers and the LCD display (#E9E8E6). Slightly firmer ink (#21201C). Radix "Sand" neutrals, no beige. Replaces the medium gray + bright white pair, which looked like sheets lying on a table.
- **Dark mode**: same logic of small luminosity steps, already in place; card line slightly more marked.
- No change of behavior or data.

## [0.9.62] - 2026-09-28

Design rework, batch 10 - screens without pattern, light LCD in light mode (founder's choice).

- **Light mode**: the object's screen (day, palette search, quick capture, brief, counters) becomes a gray recessed "light LCD" display with dark text; black on a light page made too harsh a contrast.
- **Both themes**: the dot pattern disappears; smooth glass with a slight reflection at the top in dark.
- No change of behavior or data.

## [0.9.61] - 2026-09-28

Design rework, batch 9 - floating surfaces join the "game object" universe.

- **Command palette**: the search is written on the object's black screen (orange cursor, focus ring around the screen); silk-screened group headers; chosen item as a neutral raised key with indicator, instead of the orange wash; footer shortcuts as small keys.
- **Windows** (new project, project choice, areas, template, report, structure, brief): panel resting on its edge, silk-screened labels, Cancel and Confirm as keys, drop-down menus with the app's chevron.
- **Handoff Brief / Daily Standup**: hard-coded colors (white text unreadable in light mode) replaced by theme colors; the generated text is displayed on the screen in mono; "Copy & Close" as an action key.
- **Quick capture**: indicator and silk-screened label, input on the black screen, app chevrons; DM Mono font and universe sheet loaded in this window.
- No change of behavior or data.

## [0.9.60] - 2026-09-27

Design rework, batch 8 - universe consistency across all screens.

- **Settings**: same shell as the app (gray frame, rounded panel); sections become real cards again (content no longer sticks to the edge, a defect introduced in 0.9.55); page titles at the app's scale; chosen option neutral (ink outline), only the radio indicator stays orange; active navigation item as a pill.
- **List view and mini mode**: same status filters as the Cards view ("All" instead of "Every"), "New note" button with "+", due date without a blue pill, tags in the cards' design.
- **Note page**: Priority and Type menus in the same design as Status, linked-task counter as a display, "Add a report" as a key, status menu with a neutral selection.
- Inherited purple shadows replaced by neutral shadows.
- No change of data.

## [0.9.59] - 2026-09-27

Design rework, batch 7 - "Braun" light mode, chosen by the founder among three palettes shown on the real home.

- **Neutral light mode**: end of beige. Frame #D6D5CF, stone-gray page #E4E3DE, cards #FBFBF9 clearly detached (gap raised from 1.09 to 1.24:1), menus #FFFFFF, ink #151514, dark secondary texts (#393936 / #52514C); neutral shadows.
- **Black screen**: "Your day" stays a black display in light mode (light text on dark glass), like the filter and Kanban counters.
- **Vivid orange** #C4410C for actions (white text 5.1:1); #A9380A for orange used as text (≥ 4.5:1). Subdued texts ≥ 4.8:1 on all surfaces.
- The "Resume" strip stays neutral in light mode; the current time no longer overlaps the last hour marker.
- Dark mode unchanged. No change of data.

## [0.9.58] - 2026-09-27

Design rework, batch 6 - the "game object" universe reaches all screens (direction validated by the founder).

- **Status filters** as keys with digits: "To do 11", "In progress 2"... Real numbers of the scope's notes, independent of the search; accessible name of the buttons unchanged (test added).
- **Kanban**: silk-screened column headers, counters as displays, dates in mono.
- **Settings**: silk-screened section titles.
- **Resume**: the note in progress carries a label strip at the top, like a cartridge.
- No change of data, model or feature.

## [0.9.57] - 2026-09-27

Design rework, batch 5 - "game object" universe, chosen by the founder among four directions.

- **The app as an object** (handheld console spirit, playful but professional): warm paper and copper kept.
- **Silk-screened labels** in DM Mono (small spaced capitals): home date, sections, sidebar projects, note page sections; digits in mono (counters, priorities, card dates, creation date).
- **Keys**: primary and secondary buttons have a face and an edge, and sink when pressed; the active segment of the view controls is a raised key; so is the brand pill.
- **Your day = the object's screen**: recessed glass with a dot pattern, display-segment bars, pixel icons (note, agent), fixed indicator, and real-digit counters of the day (notes touched, agent writes, last trace). No simulated activity.
- New font dependency `@fontsource/dm-mono` (embedded, offline).
- No change of data, model or feature.

## [0.9.56] - 2026-09-27

Design rework, batch 4 - paper light mode, large-screen scale, finishing touches.

- **"Paper" light mode**: end of all-white. Frame and sidebar #E8E3D9, page #F2EEE7, cards #FAF8F3, menus #FFFDF9, ink #26221D; warm shadows with inner highlight; action #9A4520. Subdued texts ≥ 4.5:1 on all surfaces, including the frame.
- **Interface scale**: on a screen of 2200 physical pixels or more displayed at 100% by Windows, the main window applies the WebView's native zoom (×1.2 on 2560 px, capped at ×1.5), including in half-screen. Native zoom keeps breakpoints consistent with the actually visible width. No zoom added if Windows already scales (≥ 125%); mini mode and quick capture stay at 100%. New permission `core:webview:allow-set-webview-zoom`.
- **Sidebar**: the active item is a raised pill on the frame.
- **Your day**: tracks as tinted rails, dotted markers, thicker blocks, current time as a pill on the ruler.
- Removal of the former unused `Sidebar.tsx` component.
- No change of data, model or feature.

## [0.9.55] - 2026-09-27

Design rework, batch 3 - visual foundations.

- **Depth**: the sidebar rests on a frame, the page becomes a panel with rounded corners; cards, menus and dialogs each have their layered shadow. Light mode is no longer flat.
- **Palette**: a single family of warm neutrals in both themes (end of the blue-gray, neutral grays and copper mix); subdued texts at least 4.5:1 everywhere.
- **Reserved accent**: orange now marks only the primary action, the current time and agent writes. Statuses as neutral pills with a colored icon, P2/P3 neutral, filters and view controls as segmented controls, settings navigation aligned with the sidebar.
- **Typography**: page titles at 30 px weight 600, sections at 15 px, card titles at 16 px; the erroneous "Project" label above "All notes" is removed.
- **Home**: reports grouped in a single card with rows; "Continue" as a secondary button.
- **Finishing touches**: press feedback on buttons, card hover through shadow, shadcn popover animations (`tw-animate-css`), `prefers-reduced-motion` respected.
- No change of data, model or feature.

## [0.9.54] - 2026-09-27

Design rework, batch 2 - shadcn/ui foundation and day timeline.

- **shadcn/ui (Radix) component foundation** in `frontend/src/components/shadcn/`, installed with the official CLI; Phosphor icons (Lucide removed). The shadcn variables read the `--workspace-*` tokens on `.workspace-app`, in both themes; former color classes (`accent`, `text-muted`, `surface`, `text-text`) renamed according to shadcn semantics, with no rendering change.
- **Sidebar** rebuilt on the shadcn Sidebar primitives (still on the left): space selector, views, collapsible inboxes, projects with counters, area filter, sorting and settings kept; sliding panel in a narrow window.
- **Your day** at the top of the home (adapted from Agent Trace, 21st id 26968, MIT): time axis, current time, **Notes** track (creations and last modifications, author not recorded) and **Agents** track (attributed `update_note` writes). Each period opens onto the list of the notes concerned. No simulated duration or progress.
- Read-only `list_agent_revisions_since` on the Rust side, tested; no migration.
- **Linked tasks**: status box that ticks by drawing itself and resolved tasks set back (adapted from AI Task List, 21st id 23793, MIT); "In progress" stays static because it is a declared status.

## [0.9.53] - 2026-09-27

Design rework, batch 1 - resume home. (Number 0.9.53 chosen so as not to confuse it with the undelivered, archived mini trial 0.9.52.)

- Home recomposed on three levels: **resume** (last modified note and notes to continue in a single block), **review** (reports of notes modified since yesterday, otherwise "Latest reports" announced as such; declared status, excerpt and direct opening; explicit period because no "viewed" state exists yet), **browse** (recent notes, projects).
- Factual labels: "Last modified note", relative date from `updated_at`, explicit mention that status and report are declared by their author. No simulated session, validation or progress.
- Numeric summary under the title; P0/P1 priorities are no longer counted as due dates. "To continue" list: status icon, priority and due date visible.
- The resume note is no longer repeated as a card right below. Header without vertical gap in a narrow window.
- Excerpts: Markdown tables read as an enumeration, titles and list items separated; card previews faded instead of being cut in the middle of a line.
- No change to the model, the data or the metadata displayed in cards.

## Documentation maintenance - 2026-09-27

- Kept application 0.9.51; the redesign was stopped at the founder's request, without aesthetic validation.
- Product direction, roadmap, documentation index and handoff reconciled; former documents kept in archives and links fixed.
- Resume prompt moved back into the conversation; the file and references wrongly added to the repository were removed.
- Cross-review of the documentation: MCP contract and location of the voice prompt corrected from the code, duplicate-detection promise toned down, former provider advice and visual prescriptions clearly marked as historical.
- Previous Git states saved; local traces and prototypes moved out of the repository. No new name researched or adopted, no functional change delivered by this cleanup.
- Final branch cleanup: only `master` remains local; the former full state is preserved under an annotated tag. Orphan files of the worktree moved out of the working directories, build junction detached without touching its target. MCP comment aligned with the actual order of validations, no behavior change.

## [0.9.51] - 2026-09-27

- List: content width bounded on large screens, excerpts limited to a readable width and creation placed next to the search with the shared button.
- Mini mode: persistent creation at the bottom kept; tags, statuses, due dates and direct opening unchanged.

## [0.9.50] - 2026-09-27

- Settings: harmonized preference labels, 18 px checkboxes and radio buttons, clickable labels of at least 40 px and visible keyboard focus.
- Native controls, confirmations and disabled states during sensitive operations kept.


> Earlier history (0.9.29 → 0.1.1, early project and audits 2-5) was archived outside this repository.

## [0.9.49] - 2026-09-27

- Light theme: more present gray workspace, distinct navigation, adjusted separators and white cards better detached.
- Light note: delimited white document surface with inner margins adapted to the window. Dark palette kept.

## [0.9.48] - 2026-09-27

- Navigation: the breadcrumb gives way to the actions in a narrow window, without clipping the pin or making "My notes" wrap.
- Settings: all sections stay visible on a grid in a narrow window; overview actions at the design system's standard size.
- Check: fixes from native WebView captures at 720 px, via Playwright/CDP.

## [0.9.47] - 2026-09-27

- Cards: first local image actually present in the content or the report, uncropped thumbnail and fallback if the file is missing. Remote images are not loaded by the previews.
- Editor: side properties wired to the shared dimensions; consolidation of the panel, spacing and border rules, title capped at 36 px.
- Quality: tests of Markdown references, refused image hosts and the fallback after a load error.

## [0.9.46] - 2026-09-27

- Note cards: title and status before the content, preview without imposed height, short cards not stretched by their neighbors; shared spacing and title size across desktop widths.
- Metadata, Markdown preview, report and direct opening kept. Card footer adaptable to project names and due dates.

## [0.9.45] - 2026-09-27

- Foundations: neutral surfaces and copper accent adapted from Darkmatter; tokens shared between the main window, mini and capture. DM Sans and Phosphor kept.
- Controls: standard/compact buttons and fields wired to the shared sizes; primary variants without competing gradients; touch targets adapted to the pointer.
- Typography: stable roles on large screens; existing composition and features kept.
- Documentation: single visual contract and UX contract distinguishing existing features from business data still to be built. Verification and delivery tracked in DevNotes.

## [0.9.44] - 2026-09-27

- Composition: document-style home with compact resume, notes in the foreground, projects and tracking in a secondary column. Redundant accesses to the same report and the duplicated capture block removed; creation always available at the top.
- Cards: document preview, title then metadata, integrated report and discreet footer. Markdown previews, tags, branch, priority, due date and project kept.
- Document: open writing surface, bounded reading width, properties separated by a rule, lighter command bars. Save mechanisms and actions unchanged.
- 21st components: navigation adapted from Dashboard Sidebar (Arun Dass), settings preview adapted from Settings (LN), shared button adapted from Origin UI, due-date calendar adapted from Calendar 5 (Cnippet), linked tasks adapted from the Agent Plan presentation (Isaiah Bjork). Demo contents and actions are replaced by the existing data and callbacks.
- Task details display the persisted report or content; no execution progress is invented.
- Quality: 231 existing tests and 2 calendar tests passed, TypeScript/Vite build passed. Native visual verification and delivery tracked in DevNotes.

## [0.9.43] - 2026-09-27

- Editor: Radix formatting bar directly wired to Tiptap (headings, bold, italic, strikethrough, lists, tasks, quote, code, links, undo/redo). Commands use the existing save circuit.
- Templates: Origin UI menu obtained through 21st, with icons and descriptions, existing text protection kept.
- Menus: link editing in a popover, protocol validation, active states and tooltips; button keyboard navigation handled by Radix.
- Note page: properties aligned in a dedicated surface, commands and fields harmonized with the existing tokens. Phosphor kept.
- Quality: tests of the commands on a real Tiptap document and of their Markdown serialization; paste and save protections kept.

## [0.9.42] - 2026-09-27 - Note page centered on writing
- Title and text joined in a continuous surface, with a useful writing height even when empty. Organization and tracking placed alongside on large screens, below the document with direct access in a narrow window.
- Status, type, priority, due date, tags, branch and reports kept. Filing from the collapsible Inbox; secondary copy and delete commands. Checklist spacing fixed.
- The default title no longer triggers similarity suggestions; obsolete responses are ignored without delaying the save.
- Checks: 227 frontend tests passed, TypeScript/Vite build and `npm run update-exe` delivery passed, SHA256 fingerprint checked. Components inspected in light/dark at 720/1920/2560 px and the note page inspected in the delivered executable. Details and limits recorded in DevNotes.

## [0.9.41] - 2026-09-27 - Home fidelity and shared components

- Home recomposed from the validated visual study: main column with resume, projects and notes; side column for capture and tracking. Area filters linked to the existing context, "To follow" filter for recent notes.
- The resume block uses the real reports and notes: explicit modification date, direct opening of the note and work to follow from the same project. No session brief generated or simulated.
- Cards: status and type at the top for all types, reading at 16/18 px, project and metadata in the footer; inert Markdown, reports, tags, branch, priority and due date kept.
- Shared buttons: iris relief, common borders and keyboard/hover/click states; removal of competing styles between the two button interfaces and of toolbar overrides that masked the variants. Area actions wired to the same components; Template menu aligned with the compact commands.

- Check: 225 frontend tests passed, TypeScript/Vite build and `npm run update-exe` delivery passed, SHA256 of the replacement checked. Visual comparison of the components at 720/1920/2560 px, light/dark, then native check of the home, the direct opening of cards, the toolbar and the area buttons. No change to the schema or to note saving.

## [0.9.40] - 2026-09-27 - Settings and note reading

- Settings in a full-window view, persistent navigation and sections without nested cards; aligned appearance settings and text size preview. Collapsible area suggestions.
- More compact cards, structured and bounded Markdown previews; priority status for tasks/issues, context kept for notes. Images rendered as text and inert links in previews.
- Editor: multiline title on display, grouped metadata, a single main scroll for the document; tighter paragraph and list rhythm. No change to the save mechanisms, the schema or the authorizations.
- Check: 225 frontend tests, TypeScript/Vite build and `npm run update-exe` delivery passed; SHA256 of the replaced binary checked. Inspection of the real components on fictitious data at 720/1920/2560 px, light/dark, then of settings and the editor in the delivered executable. Lint without errors; pre-existing warnings kept.

## [0.9.39] - 2026-09-27 - Design system consolidation

- Common tokens for sizes, spacing, icons, controls, dialogs and depth; documentation in `docs/DESIGN_SYSTEM.md`.
- Buttons and fields shared across settings, reports and capture; tags and priorities harmonized between cards, List and Kanban. Tag suggestions accessible by keyboard.
- Editor: title across the full width, readable commands of a common height, adaptive tracking. Less tight card grid in intermediate windows. Mini mode harmonized, with scrolling that keeps the commands.
- Quick capture aligned with the main window's theme; accessible field names and theme synchronization without losing the draft.
- No change to the SQLite schema, the IPC or the save and dictation mechanisms.
- Frontend check: 224 tests, TypeScript/Vite build passed; lint without errors (three pre-existing warnings). Inspection of the React components with fictitious data at 720, 1100, 1920 and 2560 px, light/dark themes; mini mode 400 × 500 and capture 600 × 220.
- Delivery: `npm run update-exe` passed, replacement and SHA256 checked. Native check of the home, settings, editor, linked tasks, report dialog and quick capture, without modifying the user's notes. Last adjustment of the close button rechecked in the final executable.
- New MCP server 0.9.39 verified directly: 22 tools, reading and confirmed update of the tracking note after delivery.

## [0.9.38] - 2026-09-27 - Pearl/graphite/iris direction in the application

- Light and dark palette, surfaces, borders and shadows harmonized across the existing components; Phosphor icons instead of Lucide.
- Note cards: statuses, types, priorities, due dates and dates kept; saved tags, branch and report preview now visible. Direct opening in the editor kept.
- Home: access to the report of the most recently modified note in the current scope, last modified note of each project and compact project creation. This reminder is neither a session brief nor a proof of execution.
- No change to the SQLite schema, the MCP commands or the save and dictation mechanisms. Spaces, areas, Inbox, search, sort, List and Kanban kept.
- Check: 196 Rust tests and 222 frontend tests; visual check of the React interface with isolated data at 720, 1920 and 2560 px, light/dark themes. Additional check in the delivered `DevNotes.exe`: home, cards in both themes, direct opening of the editor, collapsible report, access to linked tasks and List/Kanban display. Navigation and reading only on existing notes.
- Delivery: `npm run update-exe` passed, replacement and SHA256 checked; new MCP verified in version 0.9.38 (22 tools and reading of the tracking note). Lint without errors, three pre-existing warnings.

## [0.9.37] - 2026-09-26 - MCP wave: note editing by an agent behind human authorization

- **MCP tool `update_note`** (Option B "configuration authorization", notes 8deb5011 /
  ab3ce49d): an agent can modify the title, content, type, tags, priority and due date of an
  existing note **only if the human allowed it** in Settings (new "Agent" section,
  `mcp_allow_note_edit` flag in the database; absent = refused). Without authorization: explicit
  refusal, no write. `raw_content` is refused even when present as `null` (guardian no. 3,
  append-only dictation); `delete_note` is not exposed (pinning test). The dedicated tools
  `set_due_date` and `update_note_status` remain independent of this flag (pre-existing
  behavior, documented in Settings and `docs/MCP_SPEC.md`).
- **Concurrency control**: migration v14 (`notes.revision`, `app_settings`, `note_revisions`).
  The revision is incremented in the same transaction on **every** write path of a note
  (interface, MCP, structuring, move, dictation); the tool requires `expected_revision` and
  refuses a conflict by recalling the current version (reread then replay). Mechanism shared
  with P1-02 (refusal if the note changed since the agent's read).
- **Agent-attributed history**: every agent write archives a JSON snapshot of the **previous
  values** of the modified fields (never `raw_content`) in `note_revisions`, and the
  **archive restore copies this history back** (the authorization flag, however, stays that of
  the live profile: an archive never reactivates editing).
  The note page displays "Agent history" (relative date, modified fields, attribution),
  reloaded on every revision change; an external modification while the note is open is
  visible without re-navigating (defect found during acceptance testing, pinned by a test).
- **Editor coexistence**: dedicated test: title, tags and priority modified by an agent while
  a note is open are adopted by the fields not edited locally, without losing the unsaved
  local input (guardian no. 2).
- Check: 196 Rust tests (+20), 219 frontend tests (+10), lint 0 errors, build OK.
  **Two-client GUI acceptance test passed** on an isolated profile (`DEVNOTES_TEST_DATA_DIR`,
  debug build): authorization enabled through the interface, edit applied by the second MCP
  client, adoption visible in the editor, history displayed; live refusal without
  authorization; revision conflict refused with a replay message; `raw_content` refused.
  Batch review: implementation by DeepSeek V4.1 Flash, adversarial review by MiMo-V2.6-Pro
  (two blockers fixed: Settings text aligned with the flag's real scope, agent history
  preserved by the restore), verification by orchestrator + CDP acceptance test.
- Delivery: DevNotes.exe 0.9.37 replaced and checked by SHA256
  (`40df9c654945e1880cee1f5857538b3dd814c4a13539dde1cea6531d45c6f70b`, after the fixes from
  the MiMo review); MCP server verified by direct call after the replacement.

## [0.9.36] - 2026-09-26 - AI configuration and quick capture made reliable (4 P2 bugs)

- **Quick capture: input always possible even if local storage refuses writes**
  (note 0faa9b51). The memory fallback keeps the draft in the current context instead of
  throwing in `onChange` (the field stopped updating); a visible warning reports the lack
  of persistence ("closing or restarting"). `setLastProjectId`/`getLastProjectId` hardened
  (a cosmetic preference must not unmount the tree from an effect).
- **AI settings: the response of a save no longer destroys a concurrent input**
  (note 2b601abe). Input generation per section (cleanup/transcription): a provider change
  invalidates in-flight responses; the field is emptied only if the user stayed on the same
  input (value-bound); statuses named per section; Save/Clear buttons disabled while in
  flight; a failure to refresh the list is no longer announced as a save failure. The restore
  guard becomes fail-closed if the Quick Capture state is no longer persisted
  (`devnotes:qc-persistence`).
- **AI keys: writes serialized across processes** (note e5e3a86a). `<path>.lock` lock with
  an ownership token (safe release and purge, PID liveness, injectable thresholds), unique
  temporary name, read-modify-write entirely under the lock, plaintext key migration reread
  fresh under the lock, sweep of orphan temporary files. Two-thread concurrency test that
  fails without the lock.
- **AI keys: an unreadable file is no longer destroyed** (note a6fc0e97). Byte-for-byte
  quarantine (`ai-config.json.illisible-…`) of content **proven corrupted**, under the lock,
  before starting again from an empty config; an access error (antivirus, OneDrive,
  permissions) never moves the file and refuses the write instead of overwriting; error
  reasons without a file excerpt (no key in logs); the "keys reset" alert finally reaches
  the interface: never erased by a read, closed by a healthy write, and the `config_alert`
  field is resynchronized Rust/TS (the banner had been dead since the 0fad3bc renaming,
  discovered by the review).
- Check: 176 Rust tests, 209 frontend tests, build and lint passed (0 new warnings).
  Adversarial review by MiMo-V2.6-Pro in two rounds: round 1 "not mergeable" (M1-M5,
  m1-m12: alert erased before display, invisible fallback of the main window, quarantine
  TOCTOU, I/O error treated as corruption, lock theft...), all fixed; round 2 "mergeable
  with reservations" (N1-N4: pre-existing dead banner, poisoned empty lock...), N1, N2, N4
  fixed, N3/N5/N6/N7 documented as reservations in `docs/DECISIONS.md`. Safeguards verified
  by sabotage then restored.
- Delivery: DevNotes.exe 0.9.36 replaced and checked by SHA256
  (`fcdb9aa6575ba99ec7b6fef70161e095a107d917d3b92b44b3c5d72e08c814b8`); MCP server verified
  by direct call after the replacement.

## [0.9.35] - 2026-09-26 - Archive export: bounded scan and non-blocking invalid orphans

- Reservations R1/R2 of wave 2a (note 6a123ad3). The `images` folder is no longer entirely
  accumulated in memory before filtering: a listing of the **names only** feeds the portable
  rewrite, then validation (name/type/size) covers only the **images referenced** by the
  archive's SQLite copy (content + dictated raw text + report; union unchanged).
- An **invalid** orphan (non-UTF-8 name, directory, oversized file) no longer makes an export
  fail if the export does not embed it: it is set aside and logged on stderr (stdout reserved
  for the MCP protocol). The checks on a **referenced** image remain intact: a referenced
  image that is absent, unreadable, not a file, oversized or replaced by a link (rejection of
  reparse points widened along the way; a referenced symlink/junction was previously followed
  at copy time if its target passed the caps) still makes the export fail (archive guardian
  no. 5).
- Check: 159 Rust tests (+6, including a Windows junction created without elevation to really
  exercise the reparse point rejection; tightened assertions to tell link rejection from
  traversal), 198 frontend tests, build OK. Safeguards verified by sabotage then restored.
  Adversarial review (MiMo-V2.6-Pro, read-only): mergeable with reservations; conditions
  executed (link semantics + guard comment fixed); accepted reservations recorded (stderr log
  lost in release UI; memory bound on the scan's names, not on the notes traversal; non-UTF-8
  orphan not covered by a test under Windows).
- Delivery: DevNotes.exe 0.9.35 replaced and checked by SHA256; MCP server verified by
  direct call after the replacement.

## [0.9.34] - 2026-09-26 - Complete space repair for escrowed drafts

- The failure escrow now remembers the **project** of the targeted note (`projectId`) in
  addition to its space: the automatic space repair extends to **all** entries, and no
  longer only to the displayed note, by deducing the real space from the globally loaded
  projects. The draft of a note of another project, whose project changes space without
  the note ever being reopened, is no longer exposed with its full text in the former
  space's panel.
- The displayed note is authoritative for its own entries: a note filed in another project,
  a deleted project or an old entry are repaired on opening, and the project anchor is
  resynchronized, so the repair stays true after the note is closed. An entry without a
  space is **never** pinned (it stays visible everywhere, never unrecoverable). Repair
  grouped into a single storage write, no deletion, never a duplicate: a second failure of
  the same payload refreshes the existing anchor.
- Check: 198 frontend tests (+12), 153 Rust tests, build and lint passed; adversarial
  review in two rounds (Mimo-V2.6-Pro): the first wiring relied on a note list that the
  application never provides (active project only), detected by the review and fixed as a
  repair per captured project; the review then escalated the "note filed in another
  project" case, fixed by making the displayed note take precedence and back-filling the
  anchor. Safeguards verified by sabotage (the repair tests fail if the wiring is
  neutralized). Limits: an entry of a non-displayed note whose project changed without a
  second failure keeps the failure's anchor until the note is reopened (self-healing on
  opening, bounded); old entries without a captured project: repaired on display.
- Delivery: DevNotes.exe 0.9.34 replaced and checked by SHA256
  (`4a77ad4f82463e222d31acc557f2368b07f88734428aacb38112b9986025270d`); MCP server v0.9.34
  verified by direct call.

## [0.9.33] - 2026-09-26 - Reliable restore lock after database replacement

- A restore that committed but whose process could not take the session lock back **no
  longer** reopens the former interface's writes on the replaced database: the write gate
  moves to a dedicated state (writes frozen, honest message about the state of the data) and
  the error invites you to quit DevNotes from the notification icon (Quit) then relaunch,
  since closing the window does not quit the process. A second restore attempt is refused
  with the same truthful wording.
- The restore engine now distinguishes a failure **before** COMMIT (transaction rolled back,
  data proven unchanged, writes reopened) from a failure **during** COMMIT (ambiguous state,
  SQLite may have published, writes frozen). The classification is pinned by tests that fail
  if a refactor moves the COMMIT.
- Session lock: the outcome "operation executed but taking the shared lock back failed" is a
  distinct typed case that keeps the operation's result; the restore error messages are in
  French (including poisoned lock), mention the safety archive when it exists and stay
  truthful once displayed by the interface.
- Check: 153 Rust tests (+16), 186 frontend tests, build and lint passed; adversarial review
  in two rounds (Mimo-V2.6-Pro: initial finding, fixes M1-M5/m1-m5, then COMMIT
  classification and gate wiring); safeguard verified by sabotage (the blocked-case test
  fails if writes are reopened). Limits: a Rust panic during the restore freezes the
  application on "restore in progress" (data fail-safe, relaunch needed); a real unlock()
  failure cannot be injected in a test.
- Delivery: DevNotes.exe 0.9.33 replaced and checked by SHA256
  (`3e259702d9df491531c8ccf726fd5e4cdb164644c855bd9f0248599135c4ec6e`); MCP server v0.9.33
  verified by direct call.

## [0.9.32] - 2026-09-25 - Type accent on the Cards view

- The type pill of cards (Cards view) regains the color accent per type shared with the
  Kanban and the List (single source `getTypeColor`: red = bug, blue = task, amber = idea).
  Secondary by construction, it does not replace the "status first" hierarchy introduced
  in 0.9.30: it marks the nature of the note next to the status badge.
- Check: 137 Rust tests, 185 frontend tests (20 files), `tsc -b` + `vite build` and oxlint
  passed. WebView2/CDP visual acceptance test on the executable: colored pills confirmed on
  Cards (red bug, blue task, amber idea), dark and light theme, status > type hierarchy kept.
- Delivery: DevNotes.exe 0.9.32 replaced and checked by SHA256; MCP server v0.9.32 verified
  by direct call.

## [0.9.31] - 2026-09-25 - Documentation rationalization and language convention

- Engineering documentation is aligned with the delivered state: AGENTS.md stays a short
  guide and codifies the language rule (identifiers, CLI, MCP protocol and schema in
  English; interface and comments in French; MCP errors in English, user errors in French);
  lasting decisions live in DECISIONS.md, changes in the CHANGELOG, milestones in PROGRESS.
  README, PRODUCT_STRATEGY (actual state at 0.9.31) and PRODUCT_ROADMAP are updated.
- The Rust backend is aligned with the convention: all French identifiers (functions,
  helpers, test names) are renamed to English, and the errors of the MCP/queries layer move
  to English (those displayed to the user in Settings and filing stay in French). Behavior
  unchanged.
- Lesson recorded in DECISIONS.md: every DB_SCHEMA_VERSION bump must prove the archive cycle
  (inspection and restore of an archive of the previous schema, data preserved).
- Check: 137 Rust tests, 185 frontend tests, build and lint passed; mechanical renaming
  verified by the compiler. Delivery: DevNotes.exe 0.9.31 replaced and checked by SHA256
  (`bb72e0e7178f54818ad37845b3ad3bcff4ac3c66dc74657188416978b1c13433`); MCP server verified
  by direct call.

## [0.9.30] - 2026-09-25 - Status hierarchy and reliable Kanban

- New shared status badge (icon + label, distinct shapes per state): at the top of the
  Cards (status on the left, compact type on the right), start of line in the List, on every
  Kanban card including the move preview. The type color no longer dominates the card;
  resolved cards keep title and preview at normal contrast (only priority dimmed).
- Single vocabulary: "Done" everywhere (no more "Completed" in both genders), through the
  single source of labels shared by Cards, List, Kanban and the editor.
- Reliable Kanban: the card immediately joins the target column during the write (optimistic
  intent rendered, back to the former column on failure), rapid moves of the same note are
  serialized (the last intent wins), writes register in the save barrier, and the card title
  is an "Open note" button distinct from the drag, reachable by keyboard.
- Check: 137 Rust tests, 185 frontend tests (20 files), build and lint passed; adversarial
  review in two rounds (MiMo-V2.6-Pro + fixes: residual vocabulary, optimistic intent wired
  to the render, drag write in the barrier). WebView2/CDP visual acceptance test on an
  isolated QA database: six combinations (720/1920/2560 px × dark/light) on Cards, List and
  Kanban; full status badge at 720 px and following the scale at 1920/2560; resolved titles
  measured at normal contrast (opacity 1, no strikethrough); grayscale: the three states
  remain distinguishable by shape and label; editor: "Done" confirmed, no "Completed"
  remaining. Delivery: DevNotes.exe 0.9.30 replaced and checked by SHA256; MCP server
  verified by direct call.

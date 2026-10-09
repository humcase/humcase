# AGENTS.md - Development Guide for AI Agents

Welcome to the **Humcase** repository. This document gives any AI agent (Antigravity, Cursor, Claude, Copilot) the exact technical context needed to understand, maintain and evolve this project without breaking backward compatibility.

---

## Project Overview

**Humcase** is the name of the product. The GitHub repository is `humcase/humcase`. The product aims at the creation and continuity of human work, alone or with agents, for several lines of work and Work/Personal uses. It is not an application reserved for developers. It combines:
1. **A native desktop application (Tauri 2.0 + React/Tailwind)** to capture, organize and track notes and tasks.
2. **An stdio MCP (Model Context Protocol) server** built into the same Rust binary (`Humcase.exe mcp`).
3. **A single local SQLite database** (`humcase.db`) acting as the single source of truth shared between the graphical application and the AI. All data lives in `%USERPROFILE%/.humcase` (`src-tauri/src/data_location.rs`), **never in AppData**: the Claude desktop app is an MSIX package and redirects its child processes' AppData writes, so an MCP server writing there would miss the real database. Data of earlier versions is copied from `%APPDATA%/com.humcase.desktop` on first launch and left intact.

---

## Rules for Contributing Agents

Outside contributors: follow [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) and the [CLA](CLA.md); rules 1 to 4 below describe the maintainers' workflow (Humcase tracking, executable delivery).

### Resuming
`internal/` is the maintainers' private repository (product strategy, decision journal, handoff); it is git-ignored and absent for outside contributors, so every `internal/...` pointer below is maintainer-only. When `internal/` exists, read `internal/HANDOFF.md` (current state, planned next steps) first. Public index: `docs/README.md`. Verifications, isolated test profile and detailed delivery: `docs/CONTRIBUTING.md`. The visual universe is a chosen direction, not an aesthetic validation.

### Product direction
Before a functional batch, maintainers read `internal/PRODUCT_STRATEGY.md`.
The foundation must be useful without AI; keep the Work/Personal profiles and activities as customizable organization, without reducing them to frames. Structuring and decision support enrich free capture. MCP/CLI are complementary accesses. Future features must not be announced as delivered. Lasting decisions and the state of the activities batch: `internal/DECISIONS.md`.
Visual tracking of human and agent work remains central: following an idea through to results and decisions. Use without AI complements this collaboration, it does not replace it. Do not simulate progress or confuse a declared status with an observed execution.

### Rule 1: Project tracking EXCLUSIVELY through Humcase (MCP)
**Humcase is the single source of truth for task tracking.** Do not maintain parallel task tracking in `internal/PROGRESS.md`.
Before starting a development task:
- Identify the project concerned.
- Look for an existing note among the notes of type `bug`, `todo` or `idea` (status `open` or `in_progress`).
- **Create the note** (`add_note`) only if no note covers the request.
- Set the note to `in_progress` before starting code modifications.
- *Exception if the MCP server is unavailable*: First diagnose the connection and distinguish an access problem from a server defect. A targeted MCP repair is allowed when a confirmed defect prevents the task. If access remains impossible, report the blockage without claiming to have tracked or resolved the task. Regularize the tracking once restored.

### Rule 2: Rigorous definition of "Done" (`resolved`)
An agent must **NEVER** set a note to `resolved` simply because the code compiles.
- Verify the requested behavior.
- For a bug, reproduce the problem before the fix when possible, and verify it disappears after the fix.
- For a visual change, actually inspect the interface.
- **Resolution action**: Once the verifications have been run, update the note (`update_note_status`) to `resolved`, providing a `resolution_comment` explaining what was done, tested, and any remaining limits.
- **Link with Delivery**: For a task modifying the application, moving to `resolved` also requires the delivery defined in rule 3 to succeed and the new executable to be verified. If an indispensable verification or the delivery fails, keep `in_progress` and report the blockage. If the update interrupts the MCP, restore the connection before resolving the note. Never announce a status update that did not actually succeed.

### Rule 3: Delivery and Recompilation of the Executable
The user expects to use the final compiled application, not development servers (although `npm run tauri dev` is allowed for testing during a task).
- **Once per working session, at its end** (and before resolving a note that needs it), after the relevant verifications, the agent **must build a new executable** of the changes to the frontend or backend. While working, run only the tests of the code touched; the full Verification suite runs once before a merge (founder, 09/10: Rust builds load the machine; `internal/DECISIONS.md`).
- Run the command `npm run update-exe` at the root: it builds the release, replaces `Humcase.exe` and **fails explicitly** (exit code 1) if the build fails or if the replacement did not happen. It checks the SHA256 fingerprint (`Humcase.exe` identical to the release binary) before announcing success.
- *CI and releases*: `ci.yml` (Rust fmt, clippy, tests; frontend test, lint, build; plugin packaging) runs **by hand only** since 03/10 to save the organization's Actions minutes: before every merge, run the checks of the Verification section locally and say so in the PR; run `ci.yml` by hand before a release. Group work into few PRs. `release.yml` builds a release on a `v*` tag (or manually); the tag must equal the latest CHANGELOG version. Only push a tag at the founder's request.
- *Warning on Windows*: If `Humcase.exe` is currently open (graphical interface open or MCP server in use by the agent), replacing the file will fail. **The `update-exe` script does not stop processes**: the agent must close `Humcase.exe` itself, then rerun `npm run update-exe`. Never announce that the executable is up to date if the script failed.
- *MCP*: closing these executables cuts the MCP servers that use this binary, whatever their client. After replacement, verify the new server with a direct MCP call or through the client actually used for the task before updating a note. Do not launch, reload or reconfigure another IDE solely for this verification.

### Rule 4: Delivery documentation (CHANGELOG, AGENTS, README, internal docs)
Documentation is part of delivery: a batch is only "good and up to date" if the code **and** the docs reflect it. Before any `commit`/`push` (and at the very least before announcing a batch as finished):
- **`docs/CHANGELOG.md`**: describe the delivered changes (fixes, features, window, quality) under `## [Unreleased]`; a release renames that section to a dated `## [x.y.z] - date` entry. It is the reference for changes.
- **`internal/PROGRESS.md`** (maintainers): update the date and the corresponding phase milestone. It remains a **presentation of milestones** — task tracking remains exclusively in Humcase (Rule 1).
- **`AGENTS.md`**: keep a short, operational guide. Lasting decisions, invariants not to break and verification lessons are recorded in **`internal/DECISIONS.md`** (dated journal) and simply pointed to from `AGENTS.md`. Do not duplicate there the detail of changes or decisions.
- **`README.md`**: adjust only if the user-visible behavior or the stack changes.
- **Operational procedures** (manual tests, checklists, verification recipes): they live in **Humcase**, not in the repository (note of type `note`, marked `resolved` so it does not appear "to resume"). This repository only keeps software documentation (README, CONTRIBUTING, ARCHITECTURE, MCP spec, glossary, design system, CHANGELOG); product thinking goes to `internal/`.
- **Languages**: **the repository is in English** (decision of 30/09/2026, open source publication): identifiers, comments, commit messages, PRs, issues and documentation. Any new text is written in English. Any remaining French in tracked files is a bug: translate it (exceptions: the locale files of other languages, the glossary's columns of other languages, test fixtures, the code of migrations already applied (never modified), and names stored on disk or in local storage, such as `cles` in `ai-config.json` or the escrowed drafts' fields, which are renamed only with a migration). The conversation with the founder stays in French. CLI, MCP protocol and database schema in English; MCP errors in English (consumed by agents). **Visible interface texts: never hard-coded**, always through i18next (`t('zone.key')`, files `frontend/src/i18n/locales/<language>/<zone>.json`, native English at the same time as French, terms from `docs/GLOSSARY.md`, written by the rules of `docs/UI_WRITING.md`; a new key also goes into es, de, pt-BR and it, as `docs/TRANSLATION.md` says). On the Rust side, a text shown to the user goes through `crate::tr!` (`engine.json` files); MCP and CLI stay in English.
- Never commit manifestly outdated documentation; if it cannot be updated, say so explicitly.

---

## Verification

Rust, in `src-tauri`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Frontend, in `frontend`: `npm test`, `npm run lint -- --deny-warnings`, `npm run build`. Meaning worker, in `sense`: `cargo fmt --check`, `cargo clippy -- -D warnings`. Website, in `site`: `npm run check-i18n`, `npm run build`. These are the CI checks. Every new data-loss safeguard comes with a test that **fails** if it is removed.
Tests on data: isolated profile `HUMCASE_TEST_DATA_DIR` (debug builds only), never the user database; full recipe in `docs/CONTRIBUTING.md`.

---

## Technical Invariants and Database

1. **Type synchronization**: Any model change must be reflected both in `src-tauri/src/models/` and in `frontend/src/types/`.
2. **Schema evolution (Migrations)**: Any SQLite schema change must go through a versioned migration in `src-tauri/src/db/schema.rs` that preserves existing data.
   - Test the creation of a new database and the migration of a representative old database. Automated tests and migration trials use temporary databases or dedicated copies, never the user database directly.
   - Never modify a migration already applied in the code and never reset or delete the user's database to work around a migration problem.
3. **Environment variable security**: Never log or commit secrets (API keys). Do not expose environment variables to the frontend. Prefer authenticated operations on the Rust side; any exposure of a key to the frontend must be explicitly limited and justified.

---

## CLI and MCP Protocol (Model Context Protocol)

Humcase is designed to be manipulated by AI agents:

1. **MCP server**: The binary runs an MCP server in JSON-RPC 2.0 over `stdio` when launched with: `Humcase.exe mcp`. The Claude Code plugin `plugins/humcase` declares it under the key `humcase`; Codex and opencode also declare it under the key `humcase` (snippets: `docs/AGENT_SETUP.md`). Plugin manifests carry no `version` (installs follow the commit); `npm run check-plugins` guards the packaging.
   > **Important for agents**: In MCP mode, `stdout` is exclusively reserved for protocol messages, including error responses. Logs and diagnostics, whatever their level, must be written to `stderr`.

2. **Native CLI**: on par with MCP. `Humcase.exe tools` lists every MCP tool and `Humcase.exe call <tool> --args '<json>'` runs it through the same handler and safeguards (`mcp::tool_definitions` is the single source of the tool list); `handoff`, `projects`, `add`, `list`, `note`, `relations`, `create-linked-task`, `link-task`, `unlink-task`, `status`, `report` are shortcuts. A new capability is added as an MCP tool, so it exists in both. `hook <event>` is run by agent clients' hooks (`agent_hooks.rs`, `docs/AGENT_SETUP.md`): reminders only, never a write, never a failure.

---

## Voice Dictation & AI

Humcase includes a voice dictation feature (Deepgram) and multi-provider LLM cleanup (Cerebras by default).
- Voice cleanup must add **no fact or code absent from the original dictation**.
- Preserve the raw content. Any modification of the system prompt `CLEANUP_SYSTEM_PROMPT` in `src-tauri/src/commands/ai.rs` must be verified on representative examples to ensure the AI does not "RolePlay" and does not hallucinate fake code: `prompt_eval` in `commands/ai.rs` (ignored test, real calls) runs the samples of `commands/prompt_eval_cases.json` and can compare with another version of the prompt, model (`PROMPT_EVAL_MODEL`) or reasoning effort (`PROMPT_EVAL_REASONING`), with tokens and estimated cost per sample.
- Every AI call and dictation session records what it consumed (`ai_usage.rs`, `ai-usage.jsonl` next to the database: provider, model, tokens, audio seconds, estimated cost, numbers only). A new AI call or provider records its usage too, and its prices are added to the dated table from the provider's official pricing page.

---

## Code Organization

`src-tauri/src/`: `main.rs` (default UI, `mcp`, CLI), `lib.rs` (Tauri runner), `db/` (schema, queries by theme in `db/queries/`, sub-cases in `cases.rs`, JSON Canvas in `json_canvas.rs`, who did what and decisions in `journal.rs`), `models/`, `mcp/`, `commands/` (IPC by theme: projects, notes, cases, AI, backup, images, transcription), `backup/` (archives), `data_location.rs` and `data_migration.rs` (data folder and moves from earlier locations), `process_lease.rs` (session lock), `ai_settings.rs` and `providers.rs` (AI keys and providers), `anthropic.rs` (Claude through Anthropic's native Messages API), `images.rs`, `dictation_journal.rs`, `ai_usage.rs` (AI consumption and prices), `agent_hooks.rs` (reminders run by agent clients' hooks), `agents_seen.rs` (agents that opened an MCP session, for Settings → Agents and Home's first steps), `recognize.rs` (nature and line of work read from the wording, without AI), `meaning.rs` (search by meaning: index, background reader, thresholds; the worker is `sense/`, a separate crate assembled into `meaning/` by `npm run build-meaning`), `ask.rs` (the cases read for a question, read-only tools, kept conversations), `assistant.rs` (the assistant: one conversation that answers and proposes actions, with the documentation, a period and "To tidy" as tools, checked the same way), `natures.rs` and `jev.rs` (natures to check, read by TypeSafe's Jev; measures in `tools/nature-review`), `germinate.rs` (Sprout: sub-cases proposed with their passage, near cases, Jev's checks, answers), `tidy.rs` (To tidy: cases to archive, decisions a newer one replaces, Jev's check, answers), `gather.rs` (To tidy: cases to file together, cases against a decision, project summary), `profile.rs` (the work profile of each space, its proposed lines, the Home nudges), `i18n.rs` (engine texts), `test_profile.rs` (isolated profile). `frontend/src/`: `components/` (including `noteEditor/`: the editor's hooks (dictation, Structure, escrow of failed writes), pure functions and parts; `settings/`: Settings' sections and their state hooks; `assistant/`: the assistant window and Mini Humcase; `workspace/`: home, canvas, cards, list; `sketch/`: Excalidraw), `hooks/` (`App` logic), `services/tauriApi.ts`, `types/`, `utils/`. `site/`: the public website (Astro, static, English, French and four beta languages; `site/README.md`). Detail: `docs/ARCHITECTURE.md`.

## Decisions & hard invariants

Lasting decisions, their reasons and verification lessons are recorded in
**`internal/DECISIONS.md`** (dated journal). Do not put them back here: `AGENTS.md` must stay short.

Safeguards which, if broken, cause **data loss** (brief statements; causes, mechanisms
and verification lessons in `internal/DECISIONS.md`):

1. **Tiptap**: every Markdown syntax must have its extension loaded explicitly (`Image`, `Link`,
   `TaskList`/`TaskItem`, `TableKit`), covered by a round-trip test that fails if it is removed.
2. **Editor**: baseline + partial payload (fields not modified locally adopt the external
   value); writes of the same note serialized (`saveChainRef`), including flush on note change,
   on unmount and on window close; flush failure kept in the `utils/pendingWrites.ts` sequestration
   (memory fallback) and replayed with comparison in the same SQLite transaction
   as the write; the sequestration carries the note's project anchor (`projectId`) and its space repair
   never deletes an entry, never pins an entry without a space, and the displayed
   note is authoritative for its own entries; undo isolated per note, preserved when switching List ↔
   Board.
3. **Dictation**: `raw_content` written before the LLM call, never erased or replaced (`update_note` refuses it), atomic SQLite append with its `dictation_fragments` fragment append-only (v20) (including
   the replay of a sequestered fragment); clean shutdown (last fragment awaited, `CloseStream`, bounded
   wait on the Rust side); the accumulated text survives unmounting and window close does not duplicate
   the raw text.
4. **Deletion**: confirmation bound to the identifier of the targeted note; temporary states reset
   on note change. Deleting places the note in the trash (`deleted_at`, v17): reads
   filtered, content frozen by the `trashed_note_is_read_only` trigger; permanent erasure
   only from the trash (confirmed) or after 30 days at interface launch, with a trace
   in `note_tombstones`.
5. **Archive restore**: revalidation of the previewed archive and safety backup before
   a single commit; images present under their new names before the commit; writes
   only reopen if the failure is proven **before** COMMIT (a failure during COMMIT or a failed recovery of the
   shared lock freezes writing until a quit/relaunch via the notification icon);
   the before/during COMMIT classification is pinned by injection tests.
6. **Note page**: click and creation open `NoteEditor` directly; no reading panel nor
   "Edit" step; visual check at 720/1920/2560 px, two themes, narrow window.
7. **Note → task links**: transactional creation, common-project invariant, no cycle;
   links included in archives and deletion undo; the draft sequestration only removes
   entries whose ID was actually displayed or confirmed as saved.
8. **AI keys & quick capture**: `ai-config.json` written under a token-based interprocess lock
   (safe release/purge); quarantine of **proven** corrupted content only, never an
   access error; reset alert never erased by a read; memory fallback of quick
   capture ⇒ fail-closed restore guard. Mechanisms and caveats: `internal/DECISIONS.md`.
9. **MCP agent editing**: `update_note` requires human authorization (`mcp_allow_note_edit`, checked after the refusal of `raw_content`
   and before parsing the other arguments), `expected_revision` mandatory, `raw_content` refused even as `null`,
   `delete_note` not exposed; any new note write increments `revision` in the same
   transaction, and the agent history reloads on revision change. Mechanisms:
   `internal/DECISIONS.md`.
10. **Project deletion**: `notes.project_id` is cascading; notes first join the
   Inbox in the same transaction (links restored, revision advanced) and the v15 trigger refuses
   the deletion of a project that still has notes. Mechanisms: `internal/DECISIONS.md`.

11. **Journal and decisions** (v24): `case_events` is filled only by the per-connection TEMP
   triggers of `db/journal.rs` (UI = human, MCP = agent named at `initialize`, CLI), never by
   callers; append-only except merging consecutive edits and the parent of a sub-case at creation.
   `case_reviews` is append-only and written only when the connection's actor is human; a decision
   needs the reviewed revision. Restore copies both without installing the journal.

UI verification method (WebView2 + CDP) and decision details: `internal/DECISIONS.md`.
Visual foundations, scales and shared components: `docs/DESIGN_SYSTEM.md`.
MCP protocol specification (tools, authorizations, `update_note`): `docs/MCP_SPEC.md`.

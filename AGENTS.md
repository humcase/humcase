# AGENTS.md - Development Guide for AI Agents

**Humcase** (repository `humcase/humcase`) is for the creation and continuity of human work, alone or with agents, across lines of work and Work/Personal uses. It is not reserved for developers. One Rust binary holds:
1. **A desktop app** (Tauri 2 + React/Tailwind) to capture, organize and track cases.
2. **An stdio MCP server** (`Humcase.exe mcp`) and a native CLI.
3. **One local SQLite database** (`humcase.db`), the single source of truth shared by the app and agents. All data lives in `%USERPROFILE%/.humcase` (`src-tauri/src/data_location.rs`), **never in AppData**: the Claude desktop app is an MSIX package that redirects its child processes' AppData writes, so an MCP server writing there would miss the real database.

Where to read next: `docs/README.md` (index), `docs/ARCHITECTURE.md` (components, code map), `docs/CONTRIBUTING.md` (verification, isolated test profile, delivery, AI rules), `docs/MCP_SPEC.md`, `docs/DESIGN_SYSTEM.md`. Outside contributors: [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) and the [CLA](CLA.md); rules 1 to 4 describe the maintainers' workflow.

---

## Rules for Contributing Agents

### Resuming and direction
`internal/` is the maintainers' private repository (git-ignored, absent for outside contributors). When it exists, read `internal/HANDOFF.md` first; before a functional batch, `internal/PRODUCT_STRATEGY.md`; lasting decisions, invariant mechanisms and verification lessons are in `internal/DECISIONS.md`.
- The foundation must be useful without AI; MCP/CLI are complementary accesses. Work/Personal profiles and activities stay customizable organization, not frames.
- Visual tracking of human and agent work stays central: from an idea to results and decisions. Never simulate progress, never announce a future feature as delivered, never confuse a declared status with an observed execution.
- The visual universe is a chosen direction, not an aesthetic validation.

### Rule 1: Tracking only through Humcase (MCP)
Humcase is the single source of truth for task tracking (no parallel tracking in `internal/PROGRESS.md`). Before a task: find the project, look for an open `bug`/`todo`/`idea` note covering it, create one (`add_note`) only if none does, and set it `in_progress` before modifying code.
If the MCP server is unavailable: diagnose the connection (access problem or server defect). A targeted MCP repair is allowed when a confirmed defect blocks the task. Otherwise report the blockage without claiming the task was tracked, and regularize once restored.

### Rule 2: "Done" (`resolved`) means verified
Never resolve because the code compiles. Verify the requested behavior; for a bug, reproduce it before the fix when possible and check it is gone after; for a visual change, actually inspect the interface. Then `update_note_status` → `resolved` with a `resolution_comment`: what was done, tested, and what remains. A task that modifies the app also needs a successful delivery (rule 3) and a verified new executable. If a verification or the delivery fails, keep `in_progress` and report it. Never announce a status update that did not succeed.

### Rule 3: Deliver the executable
The founder uses the compiled app, not dev servers (`npm run tauri dev` is fine while testing).
- **Once per working session, at its end** (and before resolving a note that needs it): `npm run update-exe` at the root. It builds the release, replaces `Humcase.exe`, checks the SHA256 and fails with code 1 otherwise. While working, run only the tests of the code touched; the full Verification suite runs once before a merge (Rust builds load the machine).
- The script stops no process: close `Humcase.exe` yourself (interface and MCP servers using it), then rerun it. Closing it cuts every MCP client of this binary: after replacement, verify the new server with a direct MCP call or the client used for the task, before updating a note. Do not launch or reconfigure another IDE just for that check. Never say the executable is up to date if the script failed.
- CI (`ci.yml`) runs on every PR and push to `master`. Group work into few PRs. `release.yml` builds on a `v*` tag equal to the latest CHANGELOG version; push a tag only at the founder's request. Details: `docs/CONTRIBUTING.md`, Delivery.

### Rule 4: Documentation is part of delivery
A batch is done only when code **and** docs reflect it, before any commit/push announcing it.
- **Where**: documentation and decision records are committed directly on `master`; code goes through one PR per implementation lot.
- **`docs/CHANGELOG.md`**: delivered changes under `## [Unreleased]`; a release renames it `## [x.y.z] - date`.
- **`internal/PROGRESS.md`**: date and phase milestone only (task tracking stays in Humcase).
- **`AGENTS.md`**: stays short and operational; decisions and lessons go to `internal/DECISIONS.md`, pointed to from here. **`README.md`**: only if user-visible behavior or the stack changes.
- **Operational procedures** (manual tests, checklists) live in Humcase (a `note`, marked `resolved`), not in the repository; product thinking goes to `internal/`.
- **Languages**: the repository is in **English** (identifiers, comments, commits, PRs, docs); remaining French in tracked files is a bug, with the exceptions listed in `docs/CONTRIBUTING.md`. The conversation with the founder stays in French. CLI, MCP (errors included) and schema in English. **Visible interface texts are never hard-coded**: i18next (`t('zone.key')`, native English with French, other languages per `docs/TRANSLATION.md`, terms from `docs/GLOSSARY.md`, style from `docs/UI_WRITING.md`); on the Rust side `crate::tr!` (`engine.json`).
- Never commit documentation you know is outdated; if it cannot be updated, say so.

---

## Verification

Rust, in `src-tauri`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Frontend, in `frontend`: `npm test`, `npm run lint -- --deny-warnings`, `npm run build`. Meaning worker, in `sense`: `cargo fmt --check`, `cargo clippy -- -D warnings`. Website, in `site`: `npm run check-i18n`, `npm run build`. These are the CI checks. Every new data-loss safeguard comes with a test that **fails** if it is removed.
Tests on data: isolated profile `HUMCASE_TEST_DATA_DIR` (debug builds only), never the user database; recipe in `docs/CONTRIBUTING.md`.

## Technical invariants

1. **Types**: a model change goes into both `src-tauri/src/models/` and `frontend/src/types/`.
2. **Migrations**: any schema change is a versioned migration in `src-tauri/src/db/schema.rs` that preserves data. Test a new database and the migration of a representative old one, on temporary databases or copies. Never modify an applied migration; never reset or delete the user's database to work around a migration problem.
3. **Secrets**: never log or commit API keys; no environment variables exposed to the frontend; any key exposure to the frontend must be explicitly limited and justified.
4. **MCP stdout** is reserved for protocol messages, errors included; logs and diagnostics go to `stderr`.
5. **CLI on par with MCP**: `mcp::tool_definitions` is the single list of tools; `Humcase.exe call <tool> --args '<json>'` runs any of them through the same handler. A new capability is added as an MCP tool. `hook <event>` (run by agent clients' hooks) only reminds: never a write, never a failure. Plugin manifests carry no `version`; `npm run check-plugins` guards the packaging. Client setup: `docs/AGENT_SETUP.md`.
6. **Dictation and AI**: cleanup adds no fact or code absent from the dictation, the raw content is preserved, every AI call records its usage. Rules for the prompt, its evaluation and prices: `docs/CONTRIBUTING.md`, Voice dictation and AI calls.

## Data-loss safeguards

Brief statements; causes, mechanisms and verification lessons in `internal/DECISIONS.md`. Breaking one causes **data loss**.

1. **Tiptap**: every Markdown syntax has its extension loaded explicitly (`Image`, `Link`, `TaskList`/`TaskItem`, `TableKit`), covered by a round-trip test that fails if it is removed.
2. **Editor**: baseline + partial payload (fields not modified locally adopt the external value); writes of the same note serialized (`saveChainRef`), including flush on note change, on unmount and on window close; flush failure kept in the `utils/pendingWrites.ts` sequestration (memory fallback) and replayed with comparison in the same SQLite transaction as the write; the sequestration carries the note's project anchor (`projectId`) and its space repair never deletes an entry, never pins an entry without a space, and the displayed note is authoritative for its own entries; undo isolated per note, preserved when switching List ↔ Board.
3. **Dictation**: `raw_content` written before the LLM call, never erased or replaced (`update_note` refuses it), atomic SQLite append with its `dictation_fragments` fragment append-only (v20) (including the replay of a sequestered fragment); clean shutdown (last fragment awaited, `CloseStream`, bounded wait on the Rust side); the accumulated text survives unmounting and window close does not duplicate the raw text.
4. **Deletion**: confirmation bound to the identifier of the targeted note; temporary states reset on note change. Deleting places the note in the trash (`deleted_at`, v17): reads filtered, content frozen by the `trashed_note_is_read_only` trigger; permanent erasure only from the trash (confirmed) or after 30 days at interface launch, with a trace in `note_tombstones`.
5. **Archive restore**: revalidation of the previewed archive and safety backup before a single commit; images present under their new names before the commit; writes only reopen if the failure is proven **before** COMMIT (a failure during COMMIT or a failed recovery of the shared lock freezes writing until a quit/relaunch via the notification icon); the before/during COMMIT classification is pinned by injection tests.
6. **Note page**: click and creation open `NoteEditor` directly; no reading panel nor "Edit" step; visual check at 720/1920/2560 px, two themes, narrow window.
7. **Note → task links**: transactional creation, common-project invariant, no cycle; links included in archives and deletion undo; the draft sequestration only removes entries whose ID was actually displayed or confirmed as saved.
8. **AI keys & quick capture**: `ai-config.json` written under a token-based interprocess lock (safe release/purge); quarantine of **proven** corrupted content only, never an access error; reset alert never erased by a read; memory fallback of quick capture ⇒ fail-closed restore guard.
9. **MCP agent editing**: `update_note` requires human authorization (`mcp_allow_note_edit`, checked after the refusal of `raw_content` and before parsing the other arguments), `expected_revision` mandatory, `raw_content` refused even as `null`, `delete_note` not exposed; any new note write increments `revision` in the same transaction, and the agent history reloads on revision change.
10. **Project deletion**: `notes.project_id` is cascading; notes first join the Inbox in the same transaction (links restored, revision advanced) and the v15 trigger refuses the deletion of a project that still has notes.
11. **Journal and decisions** (v24): `case_events` is filled only by the per-connection TEMP triggers of `db/journal.rs` (UI = human, MCP = agent named at `initialize`, CLI), never by callers; append-only except merging consecutive edits and the parent of a sub-case at creation. `case_reviews` is append-only and written only when the connection's actor is human; a decision needs the reviewed revision. Restore copies both without installing the journal.

UI verification method (WebView2 + CDP): `internal/DECISIONS.md`.

---
name: resume-work
description: Reloads the thread of work kept in Humcase before acting (the brief, reworks the person asked for, results waiting for their decision, cases in progress with their sub-cases and linked tasks) and then tells the user where things stand. Use at the start of a session in any project tracked in Humcase, after an interruption or a context reset, before picking up a task, and whenever the user asks where things are, what is next, what was being done, or mentions their cases, tasks or notes, even if Humcase is not named.
---

# Resume work from Humcase

Humcase keeps the thread of the work of the user and their agents. Reread that thread before
acting: a session that starts from scratch redoes finished work, misses the reworks the person
asked for and contradicts decisions already taken.

**Vocabulary.** A **case** is what the tools call a note; a **sub-case** lives inside a case
(`parent_id`); its **nature** is `note_type`; its **report** is `resolution_comment`. Setting a
case to `resolved` delivers a **result**; the person's **decision** then accepts it or asks for a
**rework**, with a reason.

**Tools.** They belong to the Humcase MCP server and are written here `mcp__humcase__<tool>`; when
the server comes from this plugin, Claude Code names them `mcp__plugin_humcase_humcase__<tool>`.
Without MCP, run the same tool from a terminal: `Humcase.exe call <tool> --args '<json>'`
(`Humcase.exe tools` lists them, `Humcase.exe handoff --project-id <id>` prints the brief). A tool
missing from your list belongs to a newer Humcase: use the fallback given next to it.

## 1. Reload the context (read only)

```text
- [ ] Brief: reworks, results waiting for a decision, cases to resume
- [ ] Cases in progress
- [ ] Full content of the cases that matter
- [ ] The subject the user named, if any
```

1. **Brief.** Find the project with `mcp__humcase__list_projects` when it is not known. Then:
   - `mcp__humcase__get_brief` when it exists: the results waiting for the person's decision,
     the reworks still to do with their reason, and what agents did since a given time
     (`{"since": "<ISO time>"}`; without it, the writes the person has not seen yet).
     It only reads: nothing is marked as seen.
   - `mcp__humcase__get_handoff` (`{"project_id": "<id>"}`, or no argument for every project):
     the reworks first ("Rework requested (do these first)"), then the cases to resume by
     priority and due date, overdue items, the decisions in force (respect them; a case that
     "may go against" one needs the person, do not settle it alone), the latest results with
     the person's decision, the last Git branch. Each line carries the case's id: reuse it, do
     not search again.
   - `mcp__humcase__get_project_summary` (`{"project_id": "<id>"}`), when it exists, before
     proposing work in a project: every decision in force, the cases in progress, the open
     questions and what to check.
   - The brief's "How the person works" section (or `mcp__humcase__get_work_profile` when it
     exists) is the person's work profile: follow its rules like decisions in force. When the
     person asks or corrects the same thing twice, or says it applies from now on, propose it
     with `mcp__humcase__propose_profile_rule`; they decide in Humcase.
2. **Cases in progress.** `mcp__humcase__list_notes` with `{"project_id": "<id>", "status":
   "in_progress"}`. The answer is concise by default: `total`, `offset`, `limit` and short rows
   (`id`, `title`, `note_type`, `status`, `priority`, `due_at`, `parent_id`, `updated_at`,
   `revision`, `snippet`). Narrow with filters (`note_type`, `tag`, `parent_id` (`"root"` for
   top-level cases), `due_before`, `updated_since`) rather than paging through everything; page
   with `offset` only when `total` exceeds `limit`. An older Humcase returns a plain array of
   complete notes: read it the same way.
3. **Full content of the cases that matter**: the reworks, the cases in progress the user is
   likely to resume, the subject they named. `mcp__humcase__get_note` (`{"note_id": "<id>"}`)
   gives the content, the report, `review` (`state`, `rework_count`, `last_decision` whose
   `comment` is the person's reason), `case_path` (the cases above it), `sub_case_count` and
   `recent_activity` (who wrote what). Then, when useful:
   - `mcp__humcase__list_sub_cases` when `sub_case_count` > 0: its tasks, issues, questions;
   - `mcp__humcase__get_note_relations`: linked tasks and source cases in other subjects;
   - `mcp__humcase__get_case_activity` when it exists: the case's journal (who did what).
4. **A subject named by the user**: `mcp__humcase__list_notes` with `search_query`.

Change nothing during this reading: no status, no report, no move. A brief must describe the
state as found.

## 2. Tell the user where things stand

A short answer in the user's language, citing case titles (ids only next to them):

- **Reworks requested** (first): the case, the person's reason in their words, the rework number.
- **Results waiting for a decision**: what the person has to accept or send back. Only the
  person decides; say so rather than treating them as done.
- **In progress**: each case with its latest report.
- **Next actions**: open task sub-cases, linked tasks, due dates coming or overdue.
- **To check**: a declared status is not an observed execution. Flag a case marked in
  progress with no recent trace (old `updated_at`, no recent activity).

Report only what the tools returned. When a tool failed, say which one and what is missing.

## 3. Before picking up a case

- When several cases could be resumed, confirm with the user which one. A rework comes first
  unless the user decides otherwise.
- Set the case to in progress before modifying anything:
  `mcp__humcase__update_note_status` `{"note_id": "<id>", "status": "in_progress"}`. Read the
  answer (a newer Humcase returns `{id, status, resolution_comment, revision}`; a refusal comes back as a tool
  error) rather than assuming the write happened.
- Then follow the other skills of this plugin: `deliver-results` to finish a case or handle a
  rework, `write-note` to write content, `organize-cases` to file what you add.

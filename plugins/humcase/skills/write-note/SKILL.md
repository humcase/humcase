---
name: write-note
description: Writes the content of a Humcase case (task, issue, idea, question, research, decision, thought) the way Humcase's Structure button would - faithful to what was said or observed, framed for the user's line of work, headings in the user's language, nothing invented - after checking for an existing case to complete instead. Use whenever an agent creates a case with add_note or adds content to one, such as logging a bug found while coding, turning a conversation or a dictation into a case, writing a task, a decision record, a research summary or a marketing brief, even when the user only says note this, log it, add a task or put that in Humcase.
---

# Write a case in Humcase

Humcase is the space of cases the user shares with their agents. A case written by an agent
should read like one the user ran through **Structure**: faithful, organized, with nothing
invented. The user acts on what the case says, so an invented fact or a lost nuance costs more
than a plain sentence.

**Vocabulary.** A **case** is what the tools call a note; a **sub-case** lives inside a case
(`parent_id`); its **nature** is `note_type`; its **report** is `resolution_comment`.

**Tools.** They belong to the Humcase MCP server and are written here `mcp__humcase__<tool>`; when
the server comes from this plugin, Claude Code names them `mcp__plugin_humcase_humcase__<tool>`.
Without MCP, run the same tool from a terminal: `Humcase.exe call <tool> --args '<json>'`
(`Humcase.exe tools` lists them). A tool missing from your list belongs to a newer Humcase: use
the fallback given next to it.

## 1. Before writing

1. **Project**: `mcp__humcase__list_projects`. No suitable project: the Inbox of the right space,
   `mcp__humcase__get_or_create_inbox` `{"space": "work"}` (or `"private"`).
2. **Existing case**: `mcp__humcase__find_similar_notes` `{"text": "<planned title and a line of
   content>"}` (it returns the five closest cases); without it, `mcp__humcase__list_notes` with a
   `search_query` on two or three key words. When a case already covers the subject, complete it
   (a report, a sub-case) instead of creating a duplicate.
3. **Where it goes**: the case it belongs to becomes its `parent_id` (an issue, a task or a
   research of a subject goes inside that subject's case). Details in the `organize-cases` skill.
4. **Line of work**: `mcp__humcase__list_activities` names the project's areas (Development,
   Marketing, Design and research...; "activities" in the tools). Otherwise infer it from the
   project, or ask.

## 2. Shape of a case

- **Title**: short and specific, what happens or what to do ("PDF export drops images past
  page 10"), never "Note" or "Task".
- **Language**: the content and its headings in the user's language. A French-speaking user gets
  "### Comportement observé", not "### Observed behavior".
- **Fidelity**: keep every fact, name, number, nuance and open question. A hypothesis stays a
  hypothesis ("to be verified"). Add no fact, decision, action or code that was not said or
  observed; leave a section out rather than filling it with a guess.
- **Length**: under about 150 words, paragraphs or a simple list, no headings. Longer, a few
  useful `###` headings, not one per sentence.
- **Decisions and actions**: a decision stays distinct from an action. A checklist only for
  actions actually planned; an action that will be tracked becomes a task sub-case instead (see
  `organize-cases`). Never say the same thing as prose and again as a list.
- **Nature** (`note_type`): `bug` (issue), `todo` (task), `idea`, `question`, `research`,
  `decision`, or `note` (a thought, a reference, a procedure).
- **Priority** `P0` to `P3` only when it is known. **Tags**: a few useful words, without `#`.
  **Due date** (`due_at`, `YYYY-MM-DD`) only when one was given.

### Frame by line of work

Pick the frame of the project's line of work and drop the empty sections:

- **Development**
  - Issue (bug): `### Observed behavior`, `### Expected behavior`, `### Steps to reproduce`,
    `### Environment`, `### Logs` (in a ```text block).
  - Technical task: `### Context`, `### Planned change`, `### Verification`.
  - Decision: `### Status`, `### Context`, `### Decision`, `### Consequences`.
- **Marketing and communication**
  - Brief: `### Goal`, `### Target`, `### Messages`, `### Channels`, `### Deadlines`.
  - Review: `### Results` (observed figures), `### Lessons`, `### Next steps`.
- **Design and research**: `### Question`, `### Observations`, `### Lessons`, `### Leads`.
- **Personal life and free-form notes**: plain prose; a list of actions only when actions were
  stated.
- **Other lines of work**: the headings a person in that line of work would use, with the same
  fidelity rules.

## 3. Create it

```text
mcp__humcase__add_note {
  "parent_id": "<case it belongs to>",      or "project_id" for a top-level case
  "title": "...", "note_type": "bug", "content": "<Markdown>",
  "priority": "P2", "tags": ["export"], "due_at": "2026-11-15"
}
```

`parent_id` puts the case inside its parent's project. A newer Humcase answers
`{id, title, project_id, parent_id, note_type, status, due_at, revision}`; an older one, a sentence
carrying the id. Keep the
id: sub-cases, status and reports need it. `due_at` on creation needs a newer Humcase; otherwise
set it after with `mcp__humcase__set_due_date`.

Where a case comes from (a page, a document, a conversation, your research):
`mcp__humcase__add_provenance` `{"note_id": "<id>", "kind": "source", "label": "...",
"url": "...", "agent": "<your name>"}`, rather than pasting the source in the text.

## 4. Change an existing case

- To complete a case, prefer a sub-case or a report (see `deliver-results`).
- `mcp__humcase__update_note` rewrites title, text, nature, tags, priority or due date only when
  the user allowed agent editing in Humcase's settings. Read the case first and pass its
  `revision` as `expected_revision`; never send `raw_content` (the dictated text is kept as is,
  even `null` is refused). Keep image references `![...](humcase-image://...)` untouched. When
  editing is refused, complete the case with a sub-case or a report instead.
- A raw case (a pasted conversation, a dictation): `mcp__humcase__structure_note`
  `{"note_id": "<id>", "expected_revision": <n>, "line_of_work": "development"}` (or
  `marketing`, `research`, `reflection`, `auto`) has Humcase structure it with the AI the user
  chose. Same authorization as `update_note`; the replaced version stays in the history. Prefer
  it to rewriting the text yourself.

## Example

Title "PDF export: images disappear beyond page 10", `bug`, `P2`, inside the case "PDF export":

```markdown
### Observed behavior
Beyond page 10, the PDF export replaces images with empty frames.

### Expected behavior
Every image of the case appears in the PDF.

### Steps to reproduce
1. Open a 12-page case with images.
2. Export it to PDF.

### Environment
Humcase 0.9.x, Windows 11
```

Then a `todo` sub-case of this issue: "Limit the image size before the PDF export".
To start, finish or report on it, follow the `deliver-results` skill of this plugin.

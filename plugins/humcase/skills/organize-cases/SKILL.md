---
name: organize-cases
description: Files work in Humcase as one case per subject with its sub-cases inside (the idea, its issues, tasks, questions, research, decisions) instead of duplicates or loose cases tied by links, checks for an existing case before anything is created, and moves, regroups or archives cases only after proposing the plan to the user. Use before every add_note, when a project has scattered or duplicate cases on one subject, when choosing between a sub-case and a link, and whenever the user asks to tidy, group, sort, merge, clean up, archive or reorganize their cases, board or canvas.
---

# Organize the work in cases

In Humcase a **case** can hold other cases, to any depth. A subject the user works on is one
case, and what belongs to it lives inside: the idea, the issues found along the way, the
tasks, the questions, the research, the decisions. The user sees it on the canvas as one group
they can open, like a folder; a duplicate or a stray case is noise they have to clean up.

**Vocabulary.** A **case** is what the tools call a note; a **sub-case** lives inside a case
(`parent_id`); its **nature** is `note_type`; its **report** is `resolution_comment`.

**Tools.** They belong to the Humcase MCP server and are written here `mcp__humcase__<tool>`; when
the server comes from this plugin, Claude Code names them `mcp__plugin_humcase_humcase__<tool>`.
Without MCP, run the same tool from a terminal: `Humcase.exe call <tool> --args '<json>'`
(`Humcase.exe tools` lists them). A tool missing from your list belongs to a newer Humcase: use
the fallback given next to it.

## The rule

- **Sub-cases organize**: what belongs to a subject goes *inside* its case
  (`mcp__humcase__add_note` with `parent_id`, or `mcp__humcase__move_case`).
- **Links cross-reference**: `mcp__humcase__create_linked_task` and
  `mcp__humcase__link_note_task` only connect cases of *different* subjects, such as a task that
  serves two cases, or an issue found while doing a task of another subject. Never link what
  can be nested.
- **Nature** (`note_type`): `idea`, `todo` (task), `bug` (issue), `question`, `research`,
  `decision`, `note` (a thought, a reference).

Example, for a developer:

```text
Canvas of cases (idea)                    ← the subject
├── Issue 1: drop lands in the wrong case (bug)
├── Zoom with the wheel (todo)
├── Which grid for 200 cases? (question)
│   └── Layout benchmarks (research)
└── Groups at the size of a card (decision)
```

## 1. Before adding a case: duplicate first, then place

1. **Duplicate check**: `mcp__humcase__find_similar_notes` `{"text": "<planned title and a line of
   content>"}` returns the five closest cases. Without it: `mcp__humcase__list_notes` with
   `{"project_id": "<id>", "search_query": "<two or three key words>"}`. The answer is concise
   (`id`, `title`, `note_type`, `status`, `parent_id`, `snippet`...); open a candidate with
   `mcp__humcase__get_note` for its full text.
2. **The same thing already exists**: complete it (a report, a sub-case) rather than adding a
   duplicate, including a case the user dictated in their own words: track the work there (set it
   `in_progress`, resolve it when done). Tell the user which case you completed. If `add_note`
   answers with `existing_cases`, check them: when one covers the work, continue in it and propose
   to file or archive the case you just created.
5. **Closing a subject**: when `update_note_status` answers with `parent_ready`, every sub-case of
   that parent is resolved. Resolve the parent too if its goal is reached, or record a report on it
   saying what remains.
3. **A case covers the subject**: add inside it. `mcp__humcase__get_note` gives its `case_path`
   (the cases above it) and `sub_case_count`; `mcp__humcase__list_sub_cases`
   (`{"note_id": "<id>"}`) lists what is inside. Pick the most specific case: a benchmark for an
   open question goes under the question. `list_notes` with `{"parent_id": "<id>"}` (or
   `"root"` for top-level cases) narrows a large project.
4. **Nothing fits**: a top-level case in the project. Never invent a parent just to nest.

```text
mcp__humcase__add_note   {"parent_id": "<case>", "title": "...", "note_type": "todo", "content": "..."}
mcp__humcase__move_case  {"note_id": "<case to move>", "parent_id": "<new parent>"}   (null: back to top level)
```

To write the content of the case, follow the `write-note` skill of this plugin.

## 2. Tidying a project

When the user asks to organize, or when you notice **three or more loose cases on one subject**:

1. **Read**: `mcp__humcase__list_notes` with `{"project_id": "<id>", "parent_id": "root"}` for
   the top level (page with `offset` when `total` exceeds `limit`). Group by subject from what
   the cases say, never from a guess; a case you cannot place stays where it is.
2. **Parent**: when the subject already has a case (often the idea), it is the parent. A case
   alone on its subject stays at the top level.
3. **Propose the plan before moving anything**: the parent cases (existing, or new with their
   title and nature), what goes into each, and the duplicates you found. Moves change what the
   user sees on their canvas, so wait for their agreement.
4. **Apply**: create a missing parent with `mcp__humcase__add_note` (usually `idea` or `note`),
   then `mcp__humcase__move_case` each case into it; a case moves with its sub-cases. To lay out
   the board afterwards, `mcp__humcase__set_case_positions` places several cases in one call
   (without it, `mcp__humcase__set_case_position` one at a time); a case never placed is laid
   out by the interface.
5. **Report** what moved where. `move_case` with `parent_id` null takes a case back out.

Limits the server enforces (the error says which): a case stays in its project (a whole tree
changes project with `mcp__humcase__move_note_to_project`), never goes inside itself or its own
sub-cases, and a case in the trash neither moves nor receives sub-cases. Agents cannot delete a
case: propose duplicates for the user to delete in the interface.

## 3. Archiving

Archiving takes a finished or abandoned case off the board and out of the brief without
deleting it.

- **Propose first**: name the cases and why (resolved and accepted long ago, abandoned,
  superseded), and archive only what the user agrees to.
- **Apply**: `mcp__humcase__archive_case` `{"note_id": "<id>"}`; `{"note_id": "<id>",
  "archived": false}` brings a case back. Without that tool (older Humcase), archiving is done in
  the interface: tell the user which cases to archive.
- `mcp__humcase__list_notes` leaves archived cases out unless `include_archived` is true;
  `mcp__humcase__get_handoff` never offers them to resume.

## 4. Existing links

Cases already connected by linked tasks keep their links: never remove a link to tidy up. When
the source and its task belong to the same subject, propose to put the task inside the source
case; the link can stay.

## 5. Projects

Create a project (`mcp__humcase__create_project`) or rename it or change its color
(`mcp__humcase__update_project`, when listed) only at the user's request. A project's space
(`private` or `work`) changes with `mcp__humcase__update_project_space`.

## 6. Where things come from

When a case comes from a source (a page, a document, a conversation, your own research), record
it with `mcp__humcase__add_provenance` (`kind`, then `label`, `url` or exact `excerpt`, and your
agent name) rather than pasting it in the text. A provenance is never edited or removed.

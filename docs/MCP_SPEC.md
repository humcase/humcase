# MCP Specification (Model Context Protocol)

Humcase implements a standard MCP server over `stdio`.

## JSON-RPC lifecycle

The transport is **line-by-line** JSON-RPC 2.0 over `stdout`/`stdin` (one request per line;
in MCP mode, `stdout` is reserved for the protocol, diagnostics go to `stderr`).

- **`ping`** → `{}` (session keep-alive).
- **`initialize`** → returns `protocolVersion`, `serverInfo` (`name`: `Humcase`),
  `instructions` and `capabilities` (`tools` and `prompts`, both `listChanged: false`). If the
  client proposes a supported version (`2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`), it
  is echoed back; otherwise the server answers with the newest one, `2025-11-25`. The client's
  name and the time go to `agents-seen.jsonl` beside the database (`agents_seen.rs`), which
  Settings → Agents and Home's first steps read; nothing else about the session is kept.
  `instructions` (`SERVER_INSTRUCTIONS`, `src-tauri/src/mcp/mod.rs`) stays around 1,600 characters
  (Claude Code reads at most 2,048; a test keeps it under 2,000). It gives any client the essentials:
  reload context through `get_handoff`, look for an existing case (including the ones the person
  dictated) before creating one and continue in it, close a parent when its last sub-case is resolved, file what
  belongs to one subject inside its case (`add_note` with `parent_id`), keep links
  (`create_linked_task` / `link_note_task`) for cross-references between subjects, propose moves and
  archiving to the person first, set statuses only after verification with a report, handle a
  rework (read the reason, fix, resolve again), and the editing authorization. The shape of a note
  is in the `add_note` description; the Claude Code plugin's skills give the detail. Clients
  identify the server by the key in their configuration (for example `humcase`), never by
  `serverInfo.name`.
- **`tools/list`** → every tool with `name`, `title`, `description`, `inputSchema` (every parameter
  described; enumerations for statuses, types, content kinds, spaces and lines of work; a tool
  without parameters sets `additionalProperties: false`) and `annotations` (`title`,
  `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`). `list_notes`, `get_note`,
  `add_note` and `update_note_status` also declare an `outputSchema`.
- **`prompts/list`**, **`prompts/get`** → two prompts for clients that do not load the plugin's
  skills: `resume-work` (argument `project_id`, optional) and `write-note` (argument `subject`,
  optional). A prompt only builds a user message; it reads and writes nothing. An unknown prompt
  is an error `-32602`.
- **Notifications** (requests without `id`, e.g. `notifications/initialized`) → **no response**.
- **Parse error** → error `-32700` with `id: null`.
- **Unknown method** → error `-32601`.
- **Tool error** (`tools/call`) → this is **not** a protocol error: the response is a
  success whose result carries `isError: true` and the message in `content[0].text`, so that
  the agent sees the error. An unknown tool answers `Tool '<name>' not found. Run tools/list to see
  available tools.`

## Supported commands / tools

Every result is carried in `result.content[0].text`. Except `get_handoff` (a Markdown brief), the
text is **compact JSON**: objects for writes (identifiers and the new `revision`), arrays or
objects for reads. The tools that declare an `outputSchema` (`list_notes`, `get_note`, `add_note`,
`update_note_status`) also return the same object as `structuredContent`.

Cases are called notes in tool names: every case is a note.

### Attribution of writes (schema v24)

Every write made through the server is recorded in the case's journal with the agent's name,
taken from `clientInfo.name` at `initialize` (`claude-code` is shown as Claude Code,
`codex-mcp-client` as Codex…; another name is kept as given). Writes through the CLI are
recorded as the CLI, named by the `HUMCASE_ACTOR` environment variable when it is set.

### Annotations

| Tools | readOnly | destructive | idempotent | openWorld |
|---|---|---|---|---|
| `list_*`, `get_*` (except `get_or_create_inbox`), `find_similar_notes`, `find_by_meaning`, `export_json_canvas` | true | false | true | false |
| `delete_activity`, `set_project_activities`, `unlink_note_task` | false | true | true | false |
| `update_note` | false | true | false | false |
| `structure_note` (calls the AI provider chosen in Humcase settings) | false | true | false | true |
| `create_project`, `create_activity`, `import_json_canvas`, `add_provenance`, `create_linked_task`, `add_note`, `report_codebase_bug` | false | false | false | false |
| every other write (`update_project`, `update_project_space`, `rename_activity`, `get_or_create_inbox`, `move_note_to_project`, `move_case`, `archive_case`, `set_case_position`, `set_case_positions`, `link_note_task`, `set_due_date`, `update_note_status`) | false | false | true | false |

### `list_projects`
**Description**: Retrieves all existing projects.
- **Parameters**: None.
- **Return**: Array of `Project` (with counts of associated bugs/todos, `space`: `private`, `work` or `null`, `is_inbox` boolean, `archived_at`: archive date or `null`, `updated_at`: last write in ISO 8601 UTC and `revision`: concurrency version that advances on every write to the project, activity attachments included, 0 at creation). Archived projects are included; archiving and deleting are done in the interface.

### Managing projects and spaces
- `create_project`: `name` required, `color` (`#rrggbb`) and `space` optional. Without a space, the
  project appears in "Unsorted". Returns `{ id, name, color, space }`.
- `update_project`: `project_id` required, `name` and/or `color` (`#rrggbb`); at least one. Renames
  (unique name, 120 characters at most) and recolors an ordinary project; the inbox is refused. A
  bad color is refused before the rename, so nothing is written. Returns the updated `Project`.
  Deleting a project is not exposed.
- `update_project_space`: `project_id` required and `space` required (`private`, `work` or `null`
  to remove the classification). Notes stay attached to the same project. Returns `{ id, space }`.
- The interface's Personal/Work preferences do not limit MCP access. To work in
  a space, select its projects from `list_projects`, then use their `project_id`.

### Activities and inbox

Activities file projects without duplicating notes. Each activity belongs to a single
space (`private` or `work`); a project can belong to several activities of the **same** space.
A project without a space cannot be linked. The inbox is a special project (`is_inbox: true`),
created only at the first capture in the space; it cannot belong to any activity.

- `list_activities`: no parameters; returns `Activity` objects (`id`, `name`, `space`,
  `created_at`, `updated_at`, `revision`, `project_ids`). `revision` (0 at creation) and
  `updated_at` (ISO 8601 UTC) advance on every write to the activity: renaming, adding or removing
  an attached project (whichever side did it, including a space change or the
  deletion of a project). No MCP tool yet requires `revision` as input.
- `create_activity`: `name` (1 to 80 characters after trimming edge whitespace) and `space`
  (`private` or `work`) required; returns the created object. The name is unique per space, case-insensitively.
- `rename_activity`: `activity_id` and `name` required; returns the renamed object. Links remain.
- `delete_activity`: `activity_id` required; deletes the activity and its links, without deleting
  any project or note. Returns `{ id, deleted: true }`.
- `set_project_activities`: `project_id` and array `activity_ids` required; replaces all the
  project's links in one transaction. `expected_activity_ids` is an optional array of the links
  observed earlier: if another client modified them, the operation fails without writing.
  Sending this baseline when editing from `list_activities` avoids erasing a concurrent MCP
  addition. A space change of the project removes its links that became incompatible. Returns
  `{ project_id, activity_ids }`.
- `get_or_create_inbox`: `space` (`private` or `work`) required; returns
  `{ id, space, is_inbox: true }`, the inbox of this space, created on the first call. Use this
  `id` with `add_note` to capture without classification.
- `move_note_to_project`: `note_id` and `project_id` required; moves a note from the inbox
  to an ordinary project of the same space. Content, raw text and other fields are kept. Returns
  `{ id, project_id, revision }`.

Tool names and space values of the protocol remain in English. The visual
Personal/Work preferences are not an MCP access restriction.

### `list_notes`
**Description**: Lists or searches cases, one page at a time, as short summaries.
- **Parameters** (all optional):
  - `search_query` (string): FTS5 search over title, content, raw dictation and report, best
    matches first (title matches weigh more). A case is returned when it holds every word, or when
    it holds some of the rarer words (found in at most a tenth of the cases) and scores at least
    40 % of the best case. A word of five letters or more also finds its near forms: a final s or
    x, then a final e, is dropped and the rest is searched as a prefix (`liens` finds `lien`). A
    word prefixed with `#` filters on that tag (`"#mcp login"`). Duplicate detection
    (`find_similar_notes`) uses a path **without** tag interpretation.
  - `project_id` (string)
  - `status` (string): `open`, `in_progress` or `resolved`. Any other value is an **error**, not an
    empty list.
  - `note_type` (string or array of strings): `note`, `idea`, `todo`, `question`, `research`,
    `decision`, `bug`; an unknown type is an error.
  - `tag` (string): cases carrying this tag (a leading `#` is ignored).
  - `parent_id` (string or `null`): a case id lists its **direct** sub-cases; `"root"` or `null`
    lists top-level cases only; omitted, any case. (`list_sub_cases` gives a whole tree.)
  - `due_before`, `due_after` (`YYYY-MM-DD`): due on or before / on or after this day; a case
    without a due date is left out.
  - `updated_since` (ISO 8601 day or instant): modified at or after it.
  - `include_archived` (boolean, default `false`): archived cases are left out unless `true`.
  - `space` (`private` or `work`): cases of this space's projects.
  - `response_format`: `concise` (default) or `detailed`.
  - `limit` (integer 1–200, default 50), `offset` (integer ≥ 0, default 0).
- **Return**: `{ total, offset, limit, notes, hint }`, also as `structuredContent`. `total` counts
  every match; `hint` is present only when cases were left out
  (`Showing 50 of 122 from offset 0. Refine with search_query, project_id or filters, or call again
  with offset 50.`). Each item of `notes`: `id`, `title`, `note_type`, `status`, `priority`, `tags`
  (array of strings), `due_at`, `parent_id`, `project_id`, `updated_at`, `revision`,
  `archived_at`, `content_kind` and `snippet` (≈200 characters on one line: the passage of the
  content that matched a search, otherwise the start of the content; `null` when empty or for a
  sketch). `detailed` adds `content` (`null` for a sketch) and `resolution_comment`. **No listing
  ever returns `raw_content`**: `get_note` reads a whole case. Notes in the trash appear neither in
  `list_notes` nor in `get_handoff`; ordered by priority then most recent when not searching.

### `get_note`
**Description**: Retrieves the full Markdown content of a note.
- **Parameters**:
  - `note_id` (string, required)
- **Return**: The complete `Note` object with the `content` field, plus the case context
  (schema v22): `parent_id` (`null` for a top-level case), `content_kind`, `case_path` (ancestors
  from the top-level case down to the direct parent, `[{ id, title }]`), `sub_case_count` (live
  direct sub-cases) and `provenance` (oldest first). Since schema v24, also `review` — `state`
  (`none`, `to_review`, `rework_requested`, `accepted`, `accepted_changed`), `rework_count` and
  `last_decision` (the person's latest decision: `decision`, reviewed `revision`, `comment` = the
  reason of a rework, `rework_number`, `created_at`) — and `recent_activity`, the ten latest
  recorded writes (`kind`, `actor`, `fields`, `detail`, `at`). Since schema v27, also
  `superseded_by` (`{ id, title, at }` when the person marked this decision as replaced by a newer
  one, otherwise `null`: read the newer one first) and `replaces` (`[{ id, title }]`, the decisions
  this one replaces). Also returned as `structuredContent`.
- **Images**: a pasted image (portable references, schema v16) appears in the Markdown in the form
  `![text](humcase-image://<name>)`, a portable reference to the application's `images\` folder.
  An agent that rewrites the note (`update_note`) must keep this reference as is.
- **Trash**: a note deleted by the user remains readable through `get_note` with `deleted_at`
  (date; `null` for a live note). Any write on it (`update_note`, `update_note_status`,
  `set_due_date`, links, `move_note_to_project`, `archive_case`) is refused (`note is in the
  trash`). Deletion is still not exposed to agents.

### `find_similar_notes`
**Description**: Cases that look like a text, to check for a duplicate before `add_note`.
- **Parameters**: `text` (string, required: a title or a few lines); `note_id` (optional: that
  case is left out and its project's space limits the search); `space` (optional, `private` or
  `work`, overrides the space of `note_id`); `limit` (integer 1–5, default 5).
- **Return**: `{ notes }`, at most five short summaries (as in `list_notes`, concise), closest
  first. A case is returned when it shares two words of three letters or more with the text (the
  only one when the text has one), common words such as "pour" or "with" left out (`#` is not a
  tag here); live cases only, archived ones included.

### `find_by_meaning`
**Description**: Cases closest in meaning to a text, even without the same words or in another
language, closest first. Runs on the person's computer with Humcase's own model; the interface
reads the cases in the background, so a case created a few seconds ago may be missing.
- **Parameters**: `query` (string, required); `space` (optional, `private` or `work`); `limit`
  (integer 1–20, default 10).
- **Return**: `{ notes }`: short summaries (as in `list_notes`) of the cases that stand out from the
  others, live cases only, with `score` (similarity, 0 to 1), `lead` (how far the case stands out,
  in standard deviations), `strong` (stands out enough to trust beside a word search) and
  `very_close`.
- **Errors**: `search by meaning is turned off in Humcase settings`; `search by meaning is not
  ready: open Humcase so it reads the cases` (no case read yet); `meaning worker not installed`.

### `list_nature_proposals`
**Description**: Cases whose nature reads like another one, surest first, **read only**: it lists
what Jev (TypeSafe) already read and sends nothing. Jev reads the cases in the interface's
background, only when the person turned the review on in Settings and a TypeSafe key is set; the
person answers in the interface (change, keep), never an agent.
- **Parameters**: `space` (optional, `private` or `work`); `limit` (integer 1–100, default 20).
- **Return**: `{ proposals, total, enabled, readable, read }`. Each proposal is `{ note_id, title,
  project_id, revision, nature, proposed, confidence, reasons }`; `reasons` holds `{ kind:
  "reading", confidence }` (Jev's reading, both orders of the options agreeing) and, when the
  wording says it too, `{ kind: "wording", evidence }` (`words`, `task_verb`, `deadline` or
  `question_mark`, quoted as written). A nature is proposed at a confidence of at least 0.9, never
  for a case whose nature the person set or kept. `readable`: live text cases Jev may read;
  `read`: those read as they are now.

### `propose_sub_cases`
**Description**: Proposes the next steps of a case as sub-cases the person accepts, edits or refuses
in Humcase (the Sprout panel, beside Humcase's own proposals). Nothing is created until they
answer; an agent never answers.
- **Parameters**: `note_id` (required); `proposals` (array of 1 to 7, required): `{ title, nature,
  passage, reason? }`, `nature` among `todo`, `question`, `research`, `decision`, `idea`, `passage`
  copied word for word from one line of the case.
- **Return**: `{ proposals, refused }`: the proposals kept (as in `list_proposals`, `proposed_by`
  the agent named at `initialize`) and `{ title, reason }` for each one refused (empty title,
  another nature, passage not in the case). The case's revision advances when one is kept.
- **Errors**: the case is not found, in the trash, not a text case, or under 8 words.

### `list_proposals`
**Description**: Sub-cases proposed for a case (by Humcase or agents) that wait for the person's
answer, Humcase's first. Read only.
- **Parameters**: `note_id` (required).
- **Return**: `{ proposals }`: each `{ id, note_id, nature, title, passage, reason, proposed_by
  (null for Humcase), near, doubt, created_at }`; `near` is `{ id, title, nature, place,
  parent_title }` with `place` `here` (already under this case), `movable` (a top-level case of
  the project the person can put here) or `elsewhere`; `doubt`: Jev doubts the step follows from
  its passage.

### `list_tidy`
**Description**: What waits in the person's "To tidy" list, **read only**, in this order: cases
that may go against a decision in force, older decisions a newer decision may replace, close
cases of one project to file one inside the other, cases resolved long ago to archive. It lists
what the interface last found (it looks again every two minutes while open); nothing is sent.
Only the person answers.
- **Parameters**: `space` (optional, `private` or `work`).
- **Return**: `{ items }`: each `{ id, kind, case, other, passage, other_passage, duplicate,
  doubt, checked, proposed_by, created_at }`; `kind` is `conflict`, `supersede`, `group` or
  `archive`; `case` and `other` are `{ id, title, nature, project_name, date, excerpt }`:
  - `conflict`: `case` may go against the decision `other`; `passage` and `other_passage` are
    the closest sentence of each when the meaning worker found them;
  - `supersede`: `case` is the older decision, `other` the newer one;
  - `group`: `case` would be filed inside `other`; `duplicate`: both say nearly the same thing
    (found by words, without AI);
  - `archive`: `other` is `null`.
  `checked`: Jev judged the pair; `doubt`: to check (Jev hesitates, or nobody judged it);
  `proposed_by`: the agent that proposed it (`null` for Humcase).

### `propose_superseded`
**Description**: Proposes to the person that a newer decision replaces an older one. When they
accept, the older decision stays readable but is marked as history (`get_note` returns
`superseded_by`, `get_handoff` tags it). For a decision that changes, reverses or abandons
another on the same point; not for one that only adds to or confirms it.
- **Parameters**: `note_id` (required, the older decision), `by_id` (required, the newer one).
- **Return**: `{ item }` (as in `list_tidy`). The newer decision's revision advances.
- **Errors**: both must be live decisions of the same space, the older one written first and not
  already marked as replaced.

### `get_project_summary`
**Description**: What holds in a project today and what stays open, read from its cases at the
time of the call (no AI): read it before proposing work in a project.
- **Parameters**: `project_id` (required).
- **Return**: `{ project_id, project_name, decisions, replaced_count, in_progress, open_questions,
  to_check, recently_resolved }`: `decisions` are the live decisions no newer one replaced, newest
  first (`replaced_count` counts the others); `in_progress` and `open_questions` (questions not
  resolved) at most 30 each; `to_check` the waiting `conflict` items (as in `list_tidy`);
  `recently_resolved` the cases resolved in the last 14 days (10 at most). Cases are `{ id,
  title, nature, project_name, date, excerpt }`.
- **Errors**: unknown project.

### `get_work_profile`
**Description**: How the person works: their work profile, one case per space (in its Inbox) that
they write or fill with the lines they accepted (rules, lines of work, what they often refuse in
Sprout), each line with its source. Read it before proposing or doing work for them and respect
it like a decision in force; `get_handoff` carries it too.
- **Parameters**: `space` (`private` or `work`, optional: both otherwise).
- **Return**: `{ profiles: [{ space, note_id, content, revision }], pending }`: `note_id` is null
  while a space has no profile; `pending` lists the lines proposed and waiting for the person
  (`{ id, space, section (rules, work, refused), source (rule, work, refused, agent), text, case,
  detail, proposed_by, chance, created_at }`).
- **Errors**: invalid space.

### `propose_profile_rule`
**Description**: Propose to the person a lasting rule about how they work, for their profile:
something they asked or corrected more than once, or said to apply from now on. Not a one-off
choice. Nothing is written until the person adds it in Humcase.
- **Parameters**: `text` (required, at most 200 characters, one short rule in the person's
  language), `reason` (why, shown to the person), `case_id` (the case it comes from: its space is
  the profile's), `space` (`private` or `work` when there is no `case_id`, default `work`).
- **Return**: `{ id, space, already_proposed }`: the same text already proposed in that space
  (waiting or answered), or already in the profile, returns `already_proposed: true` and
  `id: null`. An error when the person turned line proposals off (Settings → Work profile).
- **Errors**: empty or too long text, unknown case, invalid space.

### `get_case_activity`
**Description**: Who did what on a case, and the person's decisions on its result.
- **Parameters**: `note_id` (required), `limit` (integer 1–300, default 20).
- **Return**: `{ note_id, title, review: { state, rework_count, last_decision }, decisions,
  events, events_total }`. `decisions` lists every decision (`decision`: `accepted` or
  `rework_requested`, `revision`, `comment`, `rework_number`, `created_at`), most recent first;
  `events` the journal (`kind`, `actor`, `actor_kind`: `human`, `agent`, `cli`, `fields`,
  `detail`, `at`), most recent first, at most `limit` of `events_total`.

### `get_brief`
**Description**: What awaits the person, **read only**: nothing is marked as seen.
- **Parameters**: `since` (optional ISO 8601 instant), `limit` (integer 1–300, default 50).
- **Return**: `{ since, since_is_fallback, to_review, reworks_pending, changes, changes_total,
  hint }`. `to_review`: results declared by an agent or the CLI waiting for the person's decision;
  `reworks_pending`: reworks the person asked for that no one handed back yet. Each is
  `{ id, title, project_id, path, status, revision, report, rework_count, declared_by,
  declared_at, last_decision }` (`path`: the cases it sits in, `Parent › Child`; `report`: the
  first 300 characters of the report; `last_decision.comment`: the reason of a rework).
  `changes`: writes of agents and of the CLI, most recent first, each
  `{ note_id, title, kind, actor, actor_kind, fields, detail, at }` — without `since`, those of
  live cases not seen by the person since their last "Accept all" in the interface (the last 24 hours when
  there is none, `since_is_fallback: true`); with `since`, every agent and CLI write since then.
  `hint` says when `changes` was cut to `limit`.

### `add_note`
**Description**: Creates a new case in a project. Its description carries the shape of a note
(title, Markdown in the person's language, facts kept and none added, sections of a bug and of a
decision, checklists only for explicit actions, actions to track as todo sub-cases).
- **Parameters**:
  - `project_id` (string, required unless `parent_id` is given)
  - `parent_id` (string, optional): creates the note as a sub-case inside this case, in the
    parent's project. A given `project_id` must be the parent's; a parent in the trash is refused.
  - `content_kind` (string, optional, with `parent_id` only): `text` (default), `image`, `link`,
    `file`, `sketch` or `research`. The content of each kind:
    - `sketch`: an Excalidraw scene, the `.excalidraw` file format (a JSON object with
      `"type": "excalidraw"` and an `elements` array, at most 5 MB). Without `content`, the sketch
      starts empty. Any other content is refused, at creation and by `update_note`, so a drawing
      is never replaced by text; a sketch is never structured.
    - `link`: Markdown whose first web address is the link, written as `<https://…>` by the
      application (a Markdown link or a bare address is read too); text may follow.
    - `image`: Markdown starting with the image `![title](humcase-image://<name>)`; text may follow.
  - `title` (string, required)
  - `content` (string, optional)
  - `note_type` (string, required): the nature of the case, `note` (shown as Thought), `idea`, `todo`, `question`, `research`, `decision` or `bug`
  - `priority` (string, optional): `P0`, `P1`, `P2` or `P3`
  - `tags` (array of **strings**, optional): a non-text element (`[42]`) is refused — it would crash the interface.
  - `due_at` (string, optional): due day `YYYY-MM-DD` (empty = none); a day that does not exist
    is refused before anything is written.
- **Return**: `{ id, title, project_id, parent_id, note_type, status, due_at, revision }`, also as
  `structuredContent`. A top-level case (no `parent_id`) also gets `existing_cases` (at most 3
  `{ id, title, status, parent_id }`: cases of the same project, not resolved, not archived, whose
  title shares at least two words of 4 letters or more) and a `hint`, when there are any: the work
  may belong to one of them (05/10 sorting: sessions opened their own case and left the person's
  open).

### Sub-cases (introduced in schema v22)

Every case is a note. A top-level case has no parent; a sub-case has one (`parent_id`) and a
content kind (`content_kind`). Depth is unlimited and the structure is a strict tree, checked by
the database whatever the client: a sub-case lives in its parent's project, and a case never goes
inside itself or one of its sub-cases. Sub-cases still appear in `list_notes`.

- `move_case`: `note_id` required, `parent_id` string or `null`/omitted (back to top level). A case that changes parent forgets its place on the canvas it left.
  The case keeps its sub-cases; its revision advances. Refused: a case or parent in the trash, a
  parent in another project, a cycle (`a case cannot go inside itself or its sub-cases`). Returns
  `{ id, parent_id, revision }`.
- `archive_case`: `note_id` required, `archived` optional boolean (default `true`; `false`
  unarchives). Archiving sets a case aside — hidden from boards, from `get_handoff` and, by
  default, from `list_notes` — without erasing anything or changing its status; the revision
  advances, not the modification date. Its description asks to propose archiving to the person
  before tidying. Returns `{ id, archived_at, revision }`.
- `export_json_canvas`: `project_id` required, `case_id` optional (the board inside that case;
  the top-level board of the project otherwise). Returns a JSON Canvas 1.0 document: a case with
  sub-cases is a `group` (its own words in a text node `<case id>-text`), a link case a `link`
  node, a sketch a text node holding its scene in an `excalidraw` code block, every other case a
  `text` node starting with `# title`; note → task links are edges. Coordinates are integers.
- `import_json_canvas`: `canvas` required (the document as an object or its JSON text, at most
  20 MB and 5,000 nodes), with `project_id` (top-level board) or `parent_id` (inside that case).
  All or none: one transaction. A node belongs to the smallest group whose rectangle holds it;
  groups become cases holding their nodes, text nodes cases (a first `#` line is the title, an
  `excalidraw` code block inside a case a sketch), link nodes link cases (a text case holding the
  link at the top level), file nodes text cases naming the file. Positions are kept relative to
  each board; on a board that already holds placed cases, the imported ones arrive below them.
  Edges are counted (`ignored_edges`), not kept. Returns the identifiers created.
- `set_case_position`: `note_id`, `x` and `y` (numbers, canvas pixels from the top-left corner of the parent's canvas, at most 1,000,000 in absolute value) required. Advances the case's revision, not its modification date. A case never placed is laid out automatically by the interface. Returns `{ id, x, y, revision }`.
- `set_case_positions`: `positions` required, an array of 1 to 500 `{ note_id, x, y }`; the same
  rules, all or none in one transaction. Returns `{ placed }`.
- `list_sub_cases`: `note_id` required, `max_depth` optional integer ≥ 1, `response_format`,
  `limit` (1–500, default 100) and `offset` as in `list_notes`. Returns
  `{ total, offset, limit, sub_cases, hint }`: the live sub-cases in tree order (each case followed
  by its own sub-cases), each a summary as in `list_notes` with a `depth` field (1 = direct
  sub-case). A sub-case in the trash is left out with its descendants.
- `add_provenance`: `note_id` and `kind` (short word, 1 to 40 characters: `source`, `link`,
  `research`...) required; `label`, `url`, `excerpt` optional but at least one is needed; `agent`
  optional (the agent's name). Append-only: a provenance is never edited or removed while its
  case exists; it leaves with the case when the case is purged. Returns the created provenance.

Trash and filing (interface): trashing a case takes its live sub-cases with it at the same instant,
and restoring it brings back those sub-cases; a sub-case whose parent is in the trash cannot be
restored before its parent; purging a parent from the trash turns its
remaining sub-cases into top-level cases. `move_note_to_project` files a top-level case with its
whole tree (and linked tasks); a sub-case alone is refused.

### Relations between notes and tasks (introduced in schema v12)

The same tools are exposed by the `Humcase.exe mcp` server to any compatible client. Links
stay in the shared database and are included in archives. Protocol names and parameters
remain in English.

- `get_note_relations`: `note_id` required; returns `{ sources: Note[], tasks: Note[] }`. The objects
  reflect their current statuses and reports.
- `create_linked_task`: `source_note_id` and `title` required, `content` optional; creates a task
  (`todo`) in the source's project and links it in a single transaction; returns
  `{ "task_note_id": "<id>" }`.
- `link_note_task`: `source_note_id` and `task_note_id` required; attaches an existing task of the same
  project. An already existing link is accepted without duplication. Returns
  `{ source_note_id, task_note_id, linked: true }`.
- `unlink_note_task`: same parameters; removes the link without deleting the two notes. Removing an
  absent link has no effect. Returns `{ source_note_id, task_note_id, linked: false }`.

A link cannot point to the same note, cross two projects or create a cycle. A linked task
keeps its link if its type changes afterwards. A linked note cannot change project as long as
its links exist. Statuses and reports remain declarations, with no automatic proof of execution.

### `report_codebase_bug`
**Description**: Automatically creates a **structured** bug note (file, line, error, context) — so that agents can report a defect found in the code without writing the Markdown.
- **Parameters**:
  - `project_id` (string, required)
  - `title` (string, required)
  - `file_path` (string, required)
  - `line_number` (integer ≥ 1, optional): a decimal or a text value is refused
  - `error_message` (string, required)
  - `context` (string, optional)
- **Return**: `{ id, title, project_id, note_type: "bug", priority: "P1", revision }` (tag
  `bug_report`).

### `update_note_status`
**Description**: Updates the status and/or the report of a note. Resolving a note
requires a non-empty report; this status remains a declaration by the caller. The person then
accepts the result or asks for a rework in the interface (schema v24); no tool records that
decision, and a rework puts the note back to `open` with its reason in `get_note.review`.
- **Parameters**:
  - `note_id` (string, required)
  - `status` (string, optional): `open`, `in_progress` or `resolved` (any other value is refused).
    **Omit it to record a report without changing the status** (like the CLI `report`).
  - `resolution_comment` (string): mandatory and non-empty when `status=resolved`; explains what was done and verified. Optional for other statuses.
  - `expected_resolution_comment` (string, optional): value of the report read before the call
    (empty string if there is no report). If it no longer matches the content in the database, the
    transaction is refused without modifying either the status or the report. **Without it, an
    existing report is never replaced**: the call is refused unless the case has no report yet or
    the text is the same (an agent could otherwise erase another agent's or the person's report).
  - `append` (boolean, optional, default `false`): `resolution_comment` is added after the stored
    report, separated by a blank line, instead of replacing it; the stored report is checked again
    in the write's transaction (and against `expected_resolution_comment` when given).
- **Return**: `{ id, status, resolution_comment, revision }`, also as `structuredContent`. When the
  call resolved the last open (non-archived) sub-case of a case that is not resolved, it also
  returns `parent_ready: { id, title, status, hint }`: resolve that case if its goal is reached, or
  report what remains. A
  nonexistent note (`rows_affected == 0`) or a call with neither `status` nor
  `resolution_comment` returns an **error**, never a success.

### `set_due_date`
**Description**: Gives (or removes) a due date on a note, so that an agent can reschedule what it has not finished.
- **Parameters**:
  - `note_id` (string, required)
  - `due_at` (string, required): a **day** in the format `YYYY-MM-DD` (a due date is thought of as a day, not an instant); empty string to remove the due date.
- **Return**: `{ id, due_at, revision }` (`due_at: null` once cleared); an error if the format is not `YYYY-MM-DD` **or if the day does not exist** (e.g. `2026-02-31`; `2028-02-29` is accepted).

### `get_handoff`
**Description**: Assembles a markdown resume brief from the database, to read at the start of a session. It **complements the interface's daily recap** (the "Daily recap" action calls a Tauri command `get_daily_standup`, which is not an MCP tool) by being oriented toward resuming work.
- **Parameters**:
  - `project_id` (string, optional): targeted project; without a value, the brief covers all projects.
- **Brief content**: first, the reworks the person asked for ("Rework requested (do these first)": title, identifier, rework number and the person's reason), then notes to resume (open / in progress, sorted by priority then due date, with status, type, tags and qualified due date), overdue items, decisions in force (at most 10, newest first, then the cases that may go against one: "… may go against the decision … : check with the person"), how the person works (the work profile of the project's space, or of every space, as in `get_work_profile`: its id in brackets, its sections one level down, at most 40 lines; absent while it holds no line), latest decisions (the last five resolved notes with their resolution comment, marked *awaiting the person's decision*, *accepted by the person* or *accepted, changed since* when recorded), last git branch seen in the notes. Notes in the trash and archived ones do not appear in it.
- **Case lines**: every case carries its identifier in brackets after its title, and a sub-case the
  titles of the cases it sits in:
  `- **[P0]** Launch › Pricing [<id>] — in progress, task, due 2026-10-10 (this week) #tag`;
  a latest decision reads `- Title [<id>] → first line of the report`. The interface's brief
  has the same lines.
- **Cap**: the MCP (and CLI `handoff`) brief lists at most **25** cases to resume, the most
  relevant by that order; the others are counted on a last line
  (`- … 7 more to resume, not listed: use list_notes (status, project_id, search_query) to see them`).
  The summary line keeps the full counts. The interface's brief lists every case.
- **Guarantee**: the brief contains **only** what is in the database — no deduction, no invented data.
  It reads titles, statuses and report first lines, never the cases' content.

### `structure_note` (human authorization required)

**Description**: Humcase structures a case itself, like its Structurer button, with the frame of
the agent's line of work, and applies the result at once. It calls the AI provider chosen in
Humcase settings, an external service (`openWorldHint: true`).
- **Parameters**: `note_id` and `expected_revision` (integer, from `get_note`) required;
  `line_of_work` optional, one of the schema's enumeration: `development` (or `dev`), `marketing`,
  `research`, `reflection` (or `thinking`), `personal` (no frame) or `auto` (default: Humcase
  reads the line of work from the case's wording, without AI, and uses the development,
  marketing or research frame when the wording clearly says it, otherwise no frame); `provider` optional (an AI
  provider id of Humcase's settings; the first one connected otherwise). The handler also
  tolerates a few synonyms (`software`, `brainstorm`…), outside the schema.
- **Refusals, in this order, before any AI call**: authorization "Allow the agent to edit cases"
  off; missing arguments; unknown line of work; revision conflict; case in the trash, a sketch
  or an empty case; no AI connected.
- **After the AI call**: the result is refused if it is empty, loses a link or image target, or
  keeps less than 60% of the words of a case of 20 words or more. It is applied by the
  structuring path: the content is compared again in the write transaction (a change during
  the call refuses the write), the replaced version joins the history (source `structure`,
  restorable in the interface) and the revision advances. Returns the structured case.

### `update_note` (human authorization required)

**Description**: Modifies an existing note — title, text, type, tags, priority, due date — with version
control and an attributed log. Unlike the other write tools, this tool is **refused by
default**: the human must have enabled "Allow the agent to edit cases" (Settings →
Agent, flag `mcp_allow_note_edit`). No MCP tool can enable this flag.

- **Parameters**:
  - `note_id` (string, required)
  - `expected_revision` (integer, **required**): the revision read beforehand (`get_note`/`list_notes`
    return `revision`). Any divergence (human write in the meantime) refuses the write and
    reports the current revision; the agent can re-read and replay. A non-integer token (float,
    string) is refused as if absent.
  - `title` (string, optional): non-empty if provided
  - `content` (string, optional)
  - `note_type` (string, optional): `note`, `idea`, `todo`, `question`, `research`, `decision` or `bug`
  - `tags` (array of strings, optional): a non-text element is refused
  - `priority` (string, optional): `P0`, `P1`, `P2`, `P3`, or empty string to remove the priority
  - `due_at` (string, optional): day `YYYY-MM-DD`, or empty string to remove the due date
- **Non-modifiable fields**: `raw_content` causes an explicit refusal **even when present as `null`**
  (append-only dictation). `project_id`, `id` and `created_at` are not among the applied fields:
  the handler ignores these extra arguments, without a dedicated rejection. They therefore cannot be
  modified by this tool. Moving a note remains `move_note_to_project`.
- **Actual order of refusals**: (1) `raw_content` present; (2) authorization refused; (3) typing/enums
  of the arguments; (4) absence of a field to modify and substantive validations (empty title, nonexistent
  due date, tags); (5) nonexistent note; (6) `expected_revision` conflict. Substantive validations
  precede the opening of the transaction; all these refusals apply no modification.
- **History**: on success, the transaction inserts into `note_revisions` a JSON snapshot
  of the **previous** values of the modified fields (`source = 'agent'`), then applies the changes
  and increments `revision`. The interface displays them in the case's history panel. Two
  sources write entries there: agent edits (`agent`) and structuring (`structure`), whether the
  person pressed Structurer or an agent called `structure_note`; the person's direct edits in the
  editor do not.
- **Return**: `{ id, revision }` with the new revision, or an explicit error (in English, like
  the rest of the MCP protocol).

`delete_note` does **not** exist as an MCP tool: this access does not let an agent delete a note.

The dedicated tools `update_note_status` (status, report), `set_due_date` (due date) and
`archive_case` are **independent** of the flag: they remain available without it. The setting
governs only `update_note` and `structure_note`. A due date can therefore be modified by
`set_due_date` without this authorization, or by `update_note` with authorization, expected
revision and history.

### Not exposed to agents

On purpose, no tool decides on a result (accept or ask for a rework: the person only), marks the
brief or a case as seen, deletes, trashes or purges a case, restores an archive or creates a
backup (it needs the exclusive session lock), reads or changes AI keys and settings, deletes a
project, or attaches an image from a file path (it would need a review of which paths an agent
may read).

## CLI

`Humcase.exe` is also a command line on par with MCP: `tools` lists every tool, `call <tool>
--args '<json>'` runs any of them through the same handler and safeguards (`--args -` reads the
arguments from stdin). Shortcuts: `handoff`, `projects`, `add`, `list` (`--status`,
`--project-id`, `--search`, `--limit`, `--json`: the `list_notes` page), `note` (the `get_note`
result), `relations`, `create-linked-task`, `link-task`, `unlink-task`, `status`, `report`.
`hook <event>` (`session-start`, `post-tool`, `stop`) is run by agent clients' hooks, with their JSON on stdin; it always exits `0` (see [AGENT_SETUP](AGENT_SETUP.md)). `--version` prints the version. Exit codes: `0` success, `1` error (invalid value, tool refusal,
not found, database unavailable; message on stderr), `2` invalid command line.

//! Definitions of the MCP tools: name, title, description, input schema, annotations and, for
//! the tools agents read most, an output schema. The single source for `tools/list` and for the
//! CLI (`humcase tools`, `humcase call`), so every tool exists in both.

use serde_json::{json, Value};

/// Behaviour hints of a tool (MCP `ToolAnnotations`).
#[derive(Clone, Copy)]
struct Hints {
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    open_world: bool,
}

/// Reads the database only.
const READ: Hints = Hints {
    read_only: true,
    destructive: false,
    idempotent: true,
    open_world: false,
};
/// Adds something (a case, a project, a provenance...): calling twice adds twice.
const ADD: Hints = Hints {
    read_only: false,
    destructive: false,
    idempotent: false,
    open_world: false,
};
/// Sets a value: calling again with the same arguments changes nothing more.
const SET: Hints = Hints {
    read_only: false,
    destructive: false,
    idempotent: true,
    open_world: false,
};
/// Removes or replaces something: what was there before is lost.
const REPLACE: Hints = Hints {
    read_only: false,
    destructive: true,
    idempotent: true,
    open_world: false,
};

fn tool(name: &str, title: &str, hints: Hints, description: &str, input: Value) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input,
        "annotations": {
            "title": title,
            "readOnlyHint": hints.read_only,
            "destructiveHint": hints.destructive,
            "idempotentHint": hints.idempotent,
            "openWorldHint": hints.open_world,
        }
    })
}

fn with_output(mut tool: Value, output: Value) -> Value {
    tool["outputSchema"] = output;
    tool
}

const NOTE_ID: &str = "Case id (from list_notes, get_handoff, get_brief or add_note)";
const PROJECT_ID: &str = "Project id (from list_projects or get_or_create_inbox)";
const SPACE: &str = "Space of the person's projects: private or work";
const NOTE_TYPES: [&str; 7] = [
    "note", "idea", "todo", "question", "research", "decision", "bug",
];
const STATUSES: [&str; 3] = ["open", "in_progress", "resolved"];
const DAY_PATTERN: &str = r"^\d{4}-\d{2}-\d{2}$";
const DAY_OR_EMPTY_PATTERN: &str = r"^(\d{4}-\d{2}-\d{2})?$";

fn note_id() -> Value {
    json!({ "type": "string", "description": NOTE_ID })
}

fn project_id() -> Value {
    json!({ "type": "string", "description": PROJECT_ID })
}

fn no_parameters() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn page_parameters(default_limit: i64, max: i64) -> (Value, Value, Value) {
    (
        json!({ "type": "integer", "minimum": 1, "maximum": max, "default": default_limit,
                "description": format!("Most cases returned (default {default_limit}, at most {max})") }),
        json!({ "type": "integer", "minimum": 0, "default": 0,
                "description": "Cases to skip, to read the next page (see hint)" }),
        json!({ "type": "string", "enum": ["concise", "detailed"], "default": "concise",
                "description": "concise: summary with a ~200-character snippet; detailed: also content and resolution_comment. Never raw_content: use get_note to read a whole case" }),
    )
}

/// A listed case, as `list_notes` and `list_sub_cases` return it.
fn summary_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "title": { "type": "string" },
            "note_type": { "type": "string", "enum": NOTE_TYPES },
            "status": { "type": "string", "enum": STATUSES },
            "priority": { "type": ["string", "null"] },
            "tags": { "type": "array", "items": { "type": "string" } },
            "due_at": { "type": ["string", "null"] },
            "parent_id": { "type": ["string", "null"] },
            "project_id": { "type": "string" },
            "updated_at": { "type": "string" },
            "revision": { "type": "integer" },
            "archived_at": { "type": ["string", "null"] },
            "content_kind": { "type": "string" },
            "snippet": { "type": ["string", "null"] },
            "content": { "type": ["string", "null"] },
            "resolution_comment": { "type": ["string", "null"] }
        },
        "required": ["id", "title", "note_type", "status", "project_id", "revision"]
    })
}

/// Instructions on the shape of a note, for `add_note` (the server instructions stay short).
const ADD_NOTE_DESCRIPTION: &str = "A case (called note in tool names) is the unit of work in Humcase: a thought (note), idea, task (todo), question, research, decision or bug. Creates one in a project; with parent_id, creates it as a sub-case inside that case (in the parent's project) — the way to file a task, question or incident that belongs to a subject. Check first with find_similar_notes or list_notes that it does not exist.
Writing: short, specific title; Markdown content in the person's language. Keep every fact, name, number, nuance and open question; never add facts, decisions, actions or code that were not observed. Under 150 words use paragraphs or a simple list; longer cases group ideas under a few ### headings in the person's language, fitting their line of work. A bug has the sections Observed behavior, Expected behavior, Steps to reproduce, Environment, Logs; a decision Status, Context, Decision, Consequences. Keep a decision distinct from an action; a checklist only for explicit actions; never the same content as prose and checklist. Actions to track become todo sub-cases, not checklist items. Set priority only when known.
Returns {id, title, project_id, parent_id, note_type, status, due_at, revision}; a top-level case also gets existing_cases and a hint when open cases of the project have a close title.";

/// Definitions of every tool, in `tools/list` order.
pub fn tool_definitions() -> Value {
    let (limit50, offset, format) = page_parameters(50, 200);
    let (limit_sub, _, _) = page_parameters(100, 500);
    json!([
        tool("list_projects", "List projects", READ,
            "List every project (archived ones included) with its id, name, color, space (private, work or null), is_inbox, archived_at, revision and bug/todo counts. Project ids are what the other tools take as project_id.",
            no_parameters()),
        tool("create_project", "Create a project", ADD,
            "Create a project, optionally in the private or work space. Returns {id, name, color, space}. To capture without choosing a project, use get_or_create_inbox instead.",
            json!({ "type": "object", "properties": {
                "name": { "type": "string", "description": "Project name, unique (120 characters at most)" },
                "color": { "type": "string", "pattern": "^#[0-9A-Fa-f]{6}$", "description": "Color as #rrggbb; a default color otherwise" },
                "space": { "type": "string", "enum": ["private", "work"], "description": SPACE }
            }, "required": ["name"] })),
        tool("update_project", "Rename or recolor a project", SET,
            "Rename an ordinary project and/or change its color (the inbox cannot be). Its cases and links are unchanged. Returns the updated project. Deleting or archiving a project is done by the person in Humcase.",
            json!({ "type": "object", "properties": {
                "project_id": project_id(),
                "name": { "type": "string", "description": "New name, unique (120 characters at most)" },
                "color": { "type": "string", "pattern": "^#[0-9A-Fa-f]{6}$", "description": "New color as #rrggbb" }
            }, "required": ["project_id"] })),
        tool("update_project_space", "Set a project's space", SET,
            "Assign a project to the private or work space, or clear the assignment with null. Its cases stay in the project; activity links that no longer fit the space are removed. Returns {id, space}.",
            json!({ "type": "object", "properties": {
                "project_id": project_id(),
                "space": { "type": ["string", "null"], "enum": ["private", "work", null], "description": "private, work, or null to clear" }
            }, "required": ["project_id", "space"] })),
        tool("list_activities", "List activities", READ,
            "List activities (groups of projects within a space) with their id, name, space, revision and linked project_ids.",
            no_parameters()),
        tool("create_activity", "Create an activity", ADD,
            "Create an activity in the private or work space (name unique per space, 1 to 80 characters). Returns the activity.",
            json!({ "type": "object", "properties": {
                "name": { "type": "string", "description": "Activity name" },
                "space": { "type": "string", "enum": ["private", "work"], "description": SPACE }
            }, "required": ["name", "space"] })),
        tool("rename_activity", "Rename an activity", SET,
            "Rename an activity without changing its project links. Returns the activity.",
            json!({ "type": "object", "properties": {
                "activity_id": { "type": "string", "description": "Activity id (from list_activities)" },
                "name": { "type": "string", "description": "New name" }
            }, "required": ["activity_id", "name"] })),
        tool("delete_activity", "Delete an activity", REPLACE,
            "Delete an activity and its project links. Projects and cases are kept. Returns {id, deleted}.",
            json!({ "type": "object", "properties": {
                "activity_id": { "type": "string", "description": "Activity id (from list_activities)" }
            }, "required": ["activity_id"] })),
        tool("set_project_activities", "Replace a project's activities", REPLACE,
            "Replace all the activity links of a project (activities of its own space) in one write. Pass expected_activity_ids, the links read from list_activities, so a concurrent change is refused instead of erased. Returns {project_id, activity_ids}.",
            json!({ "type": "object", "properties": {
                "project_id": project_id(),
                "activity_ids": { "type": "array", "items": { "type": "string" }, "description": "Every activity the project belongs to after the call (ids from list_activities)" },
                "expected_activity_ids": { "type": "array", "items": { "type": "string" }, "description": "Links observed before; the write is refused if they changed" }
            }, "required": ["project_id", "activity_ids"] })),
        tool("get_or_create_inbox", "Get a space's inbox", SET,
            "Get the inbox project of a space, creating it on first use: where to capture a case when no project fits. Returns {id, space, is_inbox}.",
            json!({ "type": "object", "properties": {
                "space": { "type": "string", "enum": ["private", "work"], "description": SPACE }
            }, "required": ["space"] })),
        tool("move_note_to_project", "File a case into a project", SET,
            "A case (called note in tool names) captured in an inbox is filed into an ordinary project of the same space, with its sub-cases and linked tasks; a sub-case alone is refused (use move_case). Returns {id, project_id, revision}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "project_id": project_id()
            }, "required": ["note_id", "project_id"] })),
        with_output(tool("list_notes", "List or search cases", READ,
            "Cases (called notes in tool names): list or search them, filtered, one page at a time. search_query is full-text (best matches first: cases holding every word, near forms included such as lien/liens, and cases holding some of the rarer words that score close to the best); a word starting with # filters on that tag (\"#mcp login\"). Archived cases are left out unless include_archived; cases in the trash never appear. Returns {total, offset, limit, notes, hint}: short summaries (id, title, note_type, status, priority, tags, due_at, parent_id, project_id, updated_at, revision, archived_at, content_kind, snippet); hint says how to see the rest when truncated. Read a whole case with get_note; to check for a duplicate, find_similar_notes is cheaper.",
            json!({ "type": "object", "properties": {
                "search_query": { "type": "string", "description": "Words to find in title, content or report (near forms included; a case missing some words is returned when it scores close to the best); #tag words filter on tags" },
                "project_id": project_id(),
                "status": { "type": "string", "enum": STATUSES, "description": "Only cases with this status" },
                "note_type": { "anyOf": [
                    { "type": "string", "enum": NOTE_TYPES },
                    { "type": "array", "items": { "type": "string", "enum": NOTE_TYPES } }
                ], "description": "Only cases of this type, or of any of these types" },
                "tag": { "type": "string", "description": "Only cases carrying this tag (without #)" },
                "parent_id": { "type": ["string", "null"], "description": "A case id: its direct sub-cases only. \"root\" or null: top-level cases only. Omitted: any case" },
                "due_before": { "type": "string", "pattern": DAY_PATTERN, "description": "Due on or before this day (YYYY-MM-DD)" },
                "due_after": { "type": "string", "pattern": DAY_PATTERN, "description": "Due on or after this day (YYYY-MM-DD)" },
                "updated_since": { "type": "string", "description": "Modified at or after this ISO 8601 instant or day, e.g. 2026-10-01 or 2026-10-01T08:00:00Z" },
                "include_archived": { "type": "boolean", "default": false, "description": "Also list archived cases" },
                "space": { "type": "string", "enum": ["private", "work"], "description": "Only cases of this space's projects" },
                "response_format": format,
                "limit": limit50,
                "offset": offset
            } })),
            json!({ "type": "object", "properties": {
                "total": { "type": "integer", "description": "Cases matching, all pages together" },
                "offset": { "type": "integer" },
                "limit": { "type": "integer" },
                "notes": { "type": "array", "items": summary_schema() },
                "hint": { "type": "string" }
            }, "required": ["total", "offset", "limit", "notes"] })),
        with_output(tool("get_note", "Read a case", READ,
            "A case (called note in tool names) read in full: content, raw_content (the dictation), resolution_comment, revision (pass it as expected_revision to update_note or structure_note), case_path (the cases it sits in), sub_case_count, provenance, review (where the person's decision on its result stands: state none, to_review, rework_requested, accepted or accepted_changed; last_decision.comment is the reason of a rework) and recent_activity (latest writes, who made them).",
            json!({ "type": "object", "properties": { "note_id": note_id() }, "required": ["note_id"] })),
            json!({ "type": "object", "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "content": { "type": "string" },
                "status": { "type": "string", "enum": STATUSES },
                "note_type": { "type": "string", "enum": NOTE_TYPES },
                "revision": { "type": "integer" },
                "parent_id": { "type": ["string", "null"] },
                "project_id": { "type": "string" },
                "resolution_comment": { "type": ["string", "null"] },
                "case_path": { "type": "array", "items": { "type": "object" } },
                "sub_case_count": { "type": "integer" },
                "provenance": { "type": "array" },
                "review": { "type": "object" },
                "recent_activity": { "type": "array" }
            }, "required": ["id", "title", "status", "revision", "project_id"] })),
        tool("find_similar_notes", "Find similar cases", READ,
            "Cases (called notes in tool names) that look like a text, to check for a duplicate before add_note: cases sharing two of its words (common words left out), closest first, at most 5. With note_id, that case is left out and only cases of its space are compared. Returns {notes} as short summaries.",
            json!({ "type": "object", "properties": {
                "text": { "type": "string", "description": "The title, or a few lines, of the case you are about to create" },
                "note_id": { "type": "string", "description": "A case to compare against: left out, and its space limits the search" },
                "space": { "type": "string", "enum": ["private", "work"], "description": "Only compare with cases of this space (default: note_id's space, or every space)" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 5, "default": 5, "description": "Most cases returned (at most 5)" }
            }, "required": ["text"] })),
        tool("find_by_meaning", "Search cases by meaning", READ,
            "Cases (called notes in tool names) closest in meaning to a text, even without the same words or in another language (\"the thing that loses links on save\" finds \"Sauvegarde : les liens se perdent\"), closest first. Runs on this computer with Humcase's own model; the interface reads the cases in the background, so a case created a few seconds ago may be missing. Use list_notes for exact words, tags or filters. Only cases that stand out from the others are returned. Returns {notes}: short summaries with score (similarity, 0 to 1), lead (how far the case stands out, in standard deviations), strong (stands out enough to trust beside a word search) and very_close.",
            json!({ "type": "object", "properties": {
                "query": { "type": "string", "description": "What you are looking for, in any of the six languages, in your own words" },
                "space": { "type": "string", "enum": ["private", "work"], "description": SPACE },
                "limit": { "type": "integer", "minimum": 1, "maximum": 20, "default": 10, "description": "Most cases returned (at most 20)" }
            }, "required": ["query"] })),
        tool("list_nature_proposals", "List natures to check", READ,
            "Cases (called notes in tool names) whose nature reads like another one, surest first. The reader is Jev (TypeSafe), which reads the cases in the background only when the person turned the review on in Humcase settings and set a TypeSafe key; this tool sends nothing, it lists what was read. A nature is proposed when Jev gives it in both orders of the options with a confidence of at least 0.9. A nature the person set or kept is never proposed; the person answers in the interface, an agent only reads. Returns {proposals, total, enabled, readable, read}: each proposal has note_id, title, project_id, revision, nature (now), proposed, confidence and reasons (reading: Jev's confidence; wording: words of the case quoted as written).",
            json!({ "type": "object", "properties": {
                "space": { "type": "string", "enum": ["private", "work"], "description": SPACE },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20, "description": "Most proposals returned (at most 100)" }
            } })),
        tool("propose_sub_cases", "Propose sub-cases for a case", ADD,
            "Propose the next steps of a case as sub-cases the person will accept, edit or refuse in Humcase (the Sprout panel, beside Humcase's own proposals); nothing is created until they answer, and an agent never answers. Each proposal cites the passage of the case it comes from, copied word for word from one line of the case (get_note); a proposal whose passage is not in the case, without a title, or of another nature is refused and returned with the reason. Natures: todo, question, research, decision, idea. A case that already covers a proposal is looked for and shown to the person. 1 to 7 proposals per call. Returns {proposals, refused}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "proposals": { "type": "array", "minItems": 1, "maxItems": 7, "description": "The proposed sub-cases, the most useful first", "items": { "type": "object", "properties": {
                    "title": { "type": "string", "description": "Short title in the language of the case, reusing its words; a question as a question" },
                    "nature": { "type": "string", "enum": ["todo", "question", "research", "decision", "idea"] },
                    "passage": { "type": "string", "description": "The passage of the case it comes from, word for word, from one line" },
                    "reason": { "type": "string", "description": "One short sentence: what in the passage calls for this step" }
                }, "required": ["title", "nature", "passage"] } }
            }, "required": ["note_id", "proposals"] })),
        tool("list_proposals", "List proposals waiting in a case", READ,
            "Sub-cases proposed for a case (by Humcase's Sprout or by agents) that wait for the person's answer, in order. Returns {proposals}: each has id, nature, title, passage, reason, proposed_by (null for Humcase, else the agent), near (a case that may already cover it: id, title, nature, place here|movable|elsewhere) and doubt (Jev doubts it follows from its passage).",
            json!({ "type": "object", "properties": { "note_id": note_id() }, "required": ["note_id"] })),
        tool("list_tidy", "List what Humcase proposes to tidy", READ,
            "What waits in the person's \"To tidy\" list, in this order: cases that may go against a decision in force (kind conflict: case, other = the decision, passage and other_passage = the closest sentence of each when found), older decisions a newer decision may replace (kind supersede, other = the newer one), close cases of one project to file one inside the other (kind group: case is filed inside other; duplicate when both say nearly the same thing), cases resolved long ago to archive (kind archive). Humcase finds the cases on the computer, by words and meaning; Jev (allowed by the person to read their cases) judges each pair, and without Jev only archives and duplicates are proposed, so propose_superseded is how an agent adds a replacement. Read only: it lists what the interface last found (it looks again every two minutes while open). Only the person answers, in Humcase. Returns {items}: each has id, kind, case {id, title, nature, project_name, date, excerpt}, other (same shape, or null for archive), passage, other_passage, duplicate, doubt (to check), checked (Jev judged it), proposed_by (null for Humcase), created_at.",
            json!({ "type": "object", "properties": {
                "space": { "type": "string", "enum": ["private", "work"], "description": SPACE }
            } })),
        tool("get_project_summary", "Read a project's summary", READ,
            "What holds in a project today and what stays open, read from its cases at the time of the call: decisions in force (live decisions no newer one replaced, newest first) and the count of replaced ones, cases in progress, questions not answered, cases that may go against a decision in force (to_check, as in list_tidy), cases resolved in the last two weeks. Read it before proposing work in a project: a case that goes against a decision in force needs the person. Returns {project_id, project_name, decisions, replaced_count, in_progress, open_questions, to_check, recently_resolved}; cases have {id, title, nature, project_name, date, excerpt}.",
            json!({ "type": "object", "properties": { "project_id": project_id() }, "required": ["project_id"] })),
        tool("get_work_profile", "Read how the person works", READ,
            "The person's work profile: one case per space (in its Inbox) where they keep how they work: rules that guide all their work (\"always\", \"never\", a way of doing things), their lines of work, what they often refuse in Sprout. Each line says where it comes from. The person writes it, or adds the lines Humcase and agents propose. Read it before proposing or doing work for them, and respect it like a decision in force; get_handoff carries it too. Returns {profiles: [{space, note_id (null when there is none yet), content (Markdown), revision}], pending (lines proposed and waiting for the person)}.",
            json!({ "type": "object", "properties": {
                "space": { "type": "string", "enum": ["private", "work"], "description": "Space of the profile; both otherwise" }
            } })),
        tool("propose_profile_rule", "Propose a line for the person's work profile", ADD,
            "Propose to the person a lasting rule about how they work, to add to their work profile (get_work_profile): something they asked or corrected more than once, or said to apply from now on (\"always mock up before coding\", \"never show prices\"). Not a one-off choice (a tool, a date, a supplier). One short rule per call, in the person's language. Nothing is written until the person adds it in Humcase; the same rule is not proposed twice. Returns {id, space, already_proposed}.",
            json!({ "type": "object", "properties": {
                "text": { "type": "string", "maxLength": 200, "description": "The rule, short, as it would read in the profile" },
                "reason": { "type": "string", "description": "Why you propose it (what the person said or did), shown to them" },
                "case_id": { "type": "string", "description": "Case it comes from, if any (its space is the profile's)" },
                "space": { "type": "string", "enum": ["private", "work"], "description": "Space of the profile when there is no case_id (default work)" }
            }, "required": ["text"] })),
        tool("propose_superseded", "Propose that a decision replaces an older one", ADD,
            "Propose to the person that a newer decision replaces an older one: when they accept, the older decision stays readable but is marked as history (get_note returns superseded_by; get_handoff tags it). Use it when you notice two decisions on the same point where the newer one changes, reverses or abandons the older one; not for a decision that only adds to or confirms another. Both must be live decisions of the same space, the older one written first and not already replaced. The person answers in Humcase. Returns {item} (as in list_tidy).",
            json!({ "type": "object", "properties": {
                "note_id": { "type": "string", "description": "The older decision, the one replaced" },
                "by_id": { "type": "string", "description": "The newer decision that replaces it" }
            }, "required": ["note_id", "by_id"] })),
        tool("get_case_activity", "Read a case's history", READ,
            "Who did what on a case: its journal (creation, edits, status changes, reports, moves, archive, trash; actor human, agent or cli with a name) most recent first, and every decision of the person on its result (accepted, rework_requested with the reason). Returns {note_id, title, review, decisions, events, events_total}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "limit": { "type": "integer", "minimum": 1, "maximum": 300, "default": 20, "description": "Most journal events returned (default 20)" }
            }, "required": ["note_id"] })),
        tool("move_case", "Move a case", SET,
            "Move a case inside another case of the same project, or back to top level (parent_id null or omitted). The case keeps its sub-cases; a case cannot go inside itself or its sub-cases. Propose a reorganization to the person before making it. Returns {id, parent_id, revision}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "parent_id": { "type": ["string", "null"], "description": "Case to move it into; null or omitted for top level" }
            }, "required": ["note_id"] })),
        tool("archive_case", "Archive or unarchive a case", SET,
            "Archive a case (set it aside: hidden from boards, from get_handoff and from list_notes by default; nothing is erased, the status is kept) or bring it back with archived false. Propose archiving to the person before tidying. Returns {id, archived_at, revision}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "archived": { "type": "boolean", "default": true, "description": "true to archive, false to unarchive" }
            }, "required": ["note_id"] })),
        tool("set_case_position", "Place a case on its board", SET,
            "Place a case on its parent's canvas (the top-level canvas for a top-level case), in canvas pixels from the top-left corner. Cards are 240 px wide; a case never placed is laid out automatically. For several cases use set_case_positions. Returns {id, x, y, revision}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "x": { "type": "number", "description": "Horizontal position in canvas pixels (|x| ≤ 1,000,000)" },
                "y": { "type": "number", "description": "Vertical position in canvas pixels (|y| ≤ 1,000,000)" }
            }, "required": ["note_id", "x", "y"] })),
        tool("set_case_positions", "Place several cases", SET,
            "Place several cases on their boards at once, all or none (same rules as set_case_position). Returns {placed}.",
            json!({ "type": "object", "properties": {
                "positions": { "type": "array", "minItems": 1, "maxItems": 500, "description": "Cases and their positions in canvas pixels",
                    "items": { "type": "object", "properties": {
                        "note_id": note_id(),
                        "x": { "type": "number" },
                        "y": { "type": "number" }
                    }, "required": ["note_id", "x", "y"] } }
            }, "required": ["positions"] })),
        tool("structure_note", "Structure a case with Humcase's AI", Hints { read_only: false, destructive: true, idempotent: false, open_world: true },
            "A case (called note in tool names) is rewritten by Humcase itself, like its Structurer button, with the frame of a line of work, through the AI provider chosen in Humcase settings (an external service). Needs the person's agent editing authorization and expected_revision (from get_note). The result is applied at once; it is refused if it would lose a link, an image or most of the words, and the replaced version stays in the case's history. Returns the structured case.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "expected_revision": { "type": "integer", "description": "The revision read with get_note; refused if the case changed since" },
                "line_of_work": { "type": "string", "enum": ["development", "dev", "marketing", "research", "reflection", "thinking", "personal", "auto"], "default": "auto", "description": "Frame of the structure (dev = development, thinking = reflection; personal = no frame). auto: Humcase reads the line of work from the wording (development, marketing or research), otherwise no frame" },
                "provider": { "type": "string", "description": "AI provider id from Humcase settings; the first one connected otherwise" }
            }, "required": ["note_id", "expected_revision"] })),
        tool("export_json_canvas", "Export a board as JSON Canvas", READ,
            "Export a board of cases as a JSON Canvas 1.0 document (an open format read by other note-taking and whiteboard apps): the top-level board of a project, or the board inside a case (case_id). A case with sub-cases becomes a group; note-to-task links become edges.",
            json!({ "type": "object", "properties": {
                "project_id": project_id(),
                "case_id": { "type": "string", "description": "Export the board inside this case instead of the project's top-level board" }
            }, "required": ["project_id"] })),
        tool("import_json_canvas", "Import a JSON Canvas", ADD,
            "Create cases from a JSON Canvas 1.0 document, all or none: on the top-level board of project_id, or inside the case parent_id. Text nodes become cases (a first # line is the title), groups become cases holding the nodes drawn inside them, link nodes become link cases; edges are not kept. Returns the identifiers created.",
            json!({ "type": "object", "properties": {
                "project_id": { "type": "string", "description": "Project whose top-level board receives the cases (from list_projects)" },
                "parent_id": { "type": "string", "description": "Case to import inside, instead of a top-level board" },
                "canvas": { "type": ["object", "string"], "description": "The JSON Canvas document, as an object or as its JSON text" }
            }, "required": ["canvas"] })),
        tool("list_sub_cases", "List a case's sub-cases", READ,
            "List the live sub-cases of a case in tree order, each a short summary with its depth (1 = direct sub-case), one page at a time. Returns {total, offset, limit, sub_cases, hint}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "max_depth": { "type": "integer", "minimum": 1, "description": "Deepest level listed (1 = direct sub-cases only); every level otherwise" },
                "response_format": format,
                "limit": limit_sub,
                "offset": offset
            }, "required": ["note_id"] })),
        tool("add_provenance", "Record where a case comes from", ADD,
            "Record where a case comes from (source, link, research). Append-only: a provenance is never edited or removed. Give at least a label, a url or an excerpt, and your agent name. Returns the provenance.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "kind": { "type": "string", "description": "Short word: source, link, research..." },
                "label": { "type": "string", "description": "What the source is, e.g. a document or page title" },
                "url": { "type": "string", "description": "Address of the source" },
                "excerpt": { "type": "string", "description": "A short quote from the source" },
                "agent": { "type": "string", "description": "Your agent name" }
            }, "required": ["note_id", "kind"] })),
        tool("get_note_relations", "Read a case's linked tasks", READ,
            "A case (called note in tool names): the cases that link it as a task (sources) and the tasks it links (tasks), with their current status and report. Returns {sources, tasks}.",
            json!({ "type": "object", "properties": { "note_id": note_id() }, "required": ["note_id"] })),
        tool("create_linked_task", "Create a linked task", ADD,
            "Create a todo in the source case's project and link it to the source, atomically. Links cross-reference cases of different subjects; to track an action of the same subject, use add_note with parent_id (a todo sub-case). Returns {task_note_id}.",
            json!({ "type": "object", "properties": {
                "source_note_id": { "type": "string", "description": "Case the task comes from (from list_notes or get_handoff)" },
                "title": { "type": "string", "description": "Task title" },
                "content": { "type": "string", "description": "Task content, Markdown" }
            }, "required": ["source_note_id", "title"] })),
        tool("link_note_task", "Link a task to a case", SET,
            "A case (called note in tool names) gets an existing todo of the same project linked as its task. For cases of the same subject, prefer move_case to put the todo inside the source case. Returns {source_note_id, task_note_id, linked}.",
            json!({ "type": "object", "properties": {
                "source_note_id": { "type": "string", "description": "Case the task belongs to" },
                "task_note_id": { "type": "string", "description": "Existing todo case to link" }
            }, "required": ["source_note_id", "task_note_id"] })),
        tool("unlink_note_task", "Unlink a task", REPLACE,
            "Remove a directed case-to-task link (cases are called notes in tool names) without deleting either case. Never remove a link just to tidy up. Returns {source_note_id, task_note_id, linked}.",
            json!({ "type": "object", "properties": {
                "source_note_id": { "type": "string", "description": "Case the task was linked to" },
                "task_note_id": { "type": "string", "description": "Linked task case" }
            }, "required": ["source_note_id", "task_note_id"] })),
        with_output(tool("add_note", "Create a case", ADD, ADD_NOTE_DESCRIPTION,
            json!({ "type": "object", "properties": {
                "project_id": { "type": "string", "description": "Project id (from list_projects or get_or_create_inbox); required unless parent_id is given" },
                "parent_id": { "type": "string", "description": "Case to create this one inside, as a sub-case (from list_notes or get_note)" },
                "content_kind": { "type": "string", "enum": ["text", "image", "link", "file", "sketch", "research"], "description": "With parent_id only: what the sub-case holds (default text)" },
                "title": { "type": "string", "description": "Short, specific title" },
                "content": { "type": "string", "description": "Markdown content in the person's language" },
                "note_type": { "type": "string", "enum": NOTE_TYPES, "description": "note (a thought, reference or procedure), idea, todo, question, research, decision or bug" },
                "priority": { "type": "string", "enum": ["P0", "P1", "P2", "P3"], "description": "Only when known" },
                "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags, without #" },
                "due_at": { "type": "string", "pattern": DAY_OR_EMPTY_PATTERN, "description": "Due day, YYYY-MM-DD" }
            }, "required": ["title", "note_type"] })),
            json!({ "type": "object", "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "project_id": { "type": "string" },
                "parent_id": { "type": ["string", "null"] },
                "note_type": { "type": "string", "enum": NOTE_TYPES },
                "status": { "type": "string", "enum": STATUSES },
                "due_at": { "type": ["string", "null"] },
                "revision": { "type": "integer" },
                "existing_cases": { "type": "array", "description": "Top-level case only: open cases of the project with a close title, which may already cover this work", "items": { "type": "object" } },
                "hint": { "type": "string" }
            }, "required": ["id", "title", "project_id", "revision"] })),
        tool("report_codebase_bug", "Report a bug found in code", ADD,
            "Create a structured bug case (file, line, error, context; priority P1, tag bug_report) for a defect found in the code. Returns {id, title, project_id, note_type, priority, revision}.",
            json!({ "type": "object", "properties": {
                "project_id": project_id(),
                "title": { "type": "string", "description": "Short title of the bug" },
                "file_path": { "type": "string", "description": "File where the bug is, relative to the repository" },
                "line_number": { "type": "integer", "minimum": 1, "description": "Line in that file" },
                "error_message": { "type": "string", "description": "The error, as observed" },
                "context": { "type": "string", "description": "What led to it, or a proposed fix" }
            }, "required": ["project_id", "title", "file_path", "error_message"] })),
        tool("set_due_date", "Set a case's due date", SET,
            "Set or clear the due date of a case. Returns {id, due_at, revision}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "due_at": { "type": "string", "pattern": DAY_OR_EMPTY_PATTERN, "description": "Due day YYYY-MM-DD; empty string clears it" }
            }, "required": ["note_id", "due_at"] })),
        tool("get_handoff", "Reload the work context", READ,
            "Markdown brief to read at the start of a session: reworks the person asked for first (with their reason), then cases to resume (open and in progress, by priority then due date, at most 25 — the rest is counted), overdue items, decisions in force (at most 10) and cases that may go against one, how the person works (their work profile, as in get_work_profile), latest results with the person's decision on them, last git branch seen. Every case line ends with its id in brackets; a sub-case shows the cases it sits in (Parent › Child).",
            json!({ "type": "object", "properties": {
                "project_id": { "type": "string", "description": "Project to brief (from list_projects); every project otherwise" }
            } })),
        tool("get_brief", "What awaits the person", READ,
            "What happened around the person's decisions, without marking anything as seen: results declared by agents waiting for the person's decision (to_review), reworks the person asked for that no agent handed back yet (reworks_pending, with the reason), and writes of agents and of the CLI (changes) since the person's last visit, or since `since`. Returns {since, since_is_fallback, to_review, reworks_pending, changes, changes_total, hint}.",
            json!({ "type": "object", "properties": {
                "since": { "type": "string", "description": "ISO 8601 instant; lists every agent and CLI write since then instead of those the person has not seen" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 300, "default": 50, "description": "Most changes returned (default 50)" }
            } })),
        with_output(tool("update_note_status", "Set a case's status or report", SET,
            "A case (called note in tool names) gets a status and/or a report (resolution_comment). in_progress when you start; resolved declares a result, needs a non-empty resolution_comment (what was done, how it was verified, what remains) and waits for the person to accept it or ask for a rework. Omit status to record a report without changing the status. Pass expected_resolution_comment, the report read before, to refuse the write if someone changed it. Returns {id, status, resolution_comment, revision}, plus parent_ready when this resolved the last open sub-case of a case that is not resolved yet.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "status": { "type": "string", "enum": STATUSES, "description": "New status; omit to only record the report" },
                "resolution_comment": { "type": "string", "description": "The report: required and non-empty when status is resolved" },
                "expected_resolution_comment": { "type": "string", "description": "The report read before this call (empty string if none); if the stored one differs, nothing is written" }
            }, "required": ["note_id"] })),
            json!({ "type": "object", "properties": {
                "id": { "type": "string" },
                "status": { "type": "string", "enum": STATUSES },
                "resolution_comment": { "type": ["string", "null"] },
                "revision": { "type": "integer" },
                "parent_ready": { "type": "object", "description": "When this resolved the last open sub-case: the parent case {id, title, status, hint}, to resolve or report on" }
            }, "required": ["id", "status", "revision"] })),
        tool("update_note", "Edit a case", Hints { read_only: false, destructive: true, idempotent: false, open_world: false },
            "A case (called note in tool names) gets its title, content, note_type, tags, priority or due date replaced. Disabled unless the person enabled agent case editing in Humcase settings. Pass the revision read with get_note as expected_revision; a stale value is refused with the current revision. raw_content (the dictation) is never editable. The replaced values are kept in the case's history. Returns {id, revision}.",
            json!({ "type": "object", "properties": {
                "note_id": note_id(),
                "expected_revision": { "type": "integer", "description": "The revision read with get_note; the write is refused if it changed" },
                "title": { "type": "string", "description": "New title (not empty)" },
                "content": { "type": "string", "description": "New Markdown content; keep humcase-image:// references as they are" },
                "note_type": { "type": "string", "enum": NOTE_TYPES, "description": "New type" },
                "tags": { "type": "array", "items": { "type": "string" }, "description": "New tags, replacing the old ones" },
                "priority": { "type": "string", "enum": ["P0", "P1", "P2", "P3", ""], "description": "New priority; empty string removes it" },
                "due_at": { "type": "string", "pattern": DAY_OR_EMPTY_PATTERN, "description": "New due day YYYY-MM-DD; empty string removes it" }
            }, "required": ["note_id", "expected_revision"] }))
    ])
}

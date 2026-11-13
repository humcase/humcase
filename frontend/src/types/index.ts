/** What a case is (its nature). `note` is shown as "Thought". */
/** Page shown in the main area. `nature`: a sidebar shortcut by nature. */
export type WorkspacePage = 'home' | 'notes' | 'editor' | 'trash' | 'nature' | 'tidy' | 'profile';

export type NoteType = 'note' | 'idea' | 'todo' | 'question' | 'research' | 'decision' | 'bug';
/** Natures in the order they are offered. */
export const NOTE_TYPES: readonly NoteType[] = ['note', 'idea', 'todo', 'question', 'research', 'decision', 'bug'];
export type NoteStatus = 'open' | 'in_progress' | 'resolved';
/** What the wording of a case says without AI (Rust `recognize`); a part is null when unclear. */
export interface Recognition {
  nature: { value: NoteType; confidence: number } | null;
  line_of_work: { value: 'dev' | 'marketing' | 'research' | 'reflection' | 'personal'; confidence: number } | null;
}
/** Where the reading of the cases for search by meaning stands (Rust `meaning::Status`). */
export interface MeaningStatus {
  state: 'off' | 'unavailable' | 'reading' | 'ready';
  done: number;
  total: number;
  error: string | null;
}
/**
 * A case found by meaning (Rust `meaning::Hit`): its similarity, how far it stands out from the
 * other cases, whether that is enough next to cases found by words (`strong`), and whether it is
 * very close.
 */
export interface MeaningHit {
  note_id: string;
  score: number;
  lead: number;
  strong: boolean;
  very_close: boolean;
}
/** Words of a case that say a nature (Rust `recognize::Evidence`), quoted as written. */
export type NatureEvidence =
  | { kind: 'words'; words: string }
  | { kind: 'task_verb'; word: string }
  | { kind: 'deadline' }
  | { kind: 'question_mark' };
/** Why a nature is proposed (Rust `natures::Reason`): Jev's reading, and the words that say it too. */
export type NatureReason =
  | { kind: 'reading'; confidence: number }
  | { kind: 'wording'; evidence: NatureEvidence };
/** A case whose nature reads like another one (Rust `natures::Proposal`). */
export interface NatureProposal {
  note_id: string;
  title: string;
  project_id: string;
  revision: number;
  nature: NoteType;
  proposed: NoteType;
  reasons: NatureReason[];
  confidence: number;
}
/** Natures to check (Rust `natures::Review`): the surest first, and where Jev's reading stands. */
export interface NatureReview {
  proposals: NatureProposal[];
  total: number;
  enabled: boolean;
  readable: number;
  read: number;
}
/** Jev's reading of the cases in the background (Rust `natures::Status`). */
export interface NatureReadingStatus {
  state: 'off' | 'no_key' | 'reading' | 'ready' | 'key_refused' | 'failed';
  read: number;
  total: number;
  error: string | null;
}
/** A case that may already cover a proposed sub-case (Rust `germinate::NearCase`). */
export interface NearCase {
  id: string;
  title: string;
  nature: NoteType;
  /** `here`: already under this case; `movable`: a top-level case of the project, it can be
   * placed under this case; `elsewhere`: inside another case or project, it stays there. */
  place: 'here' | 'movable' | 'elsewhere';
  parent_title: string | null;
}
/** A sub-case proposed for a case, waiting for the person's answer (Rust `germinate::Proposal`). */
export interface Proposal {
  id: string;
  note_id: string;
  nature: NoteType;
  title: string;
  /** The passage of the case it comes from, word for word. */
  passage: string;
  reason: string | null;
  /** `null` for Humcase, else the agent that proposed it. */
  proposed_by: string | null;
  near: NearCase | null;
  /** Jev doubts the proposal follows from its passage. */
  doubt: boolean;
  created_at: string;
}
/** What a germination gives (Rust `germinate::Germination`). */
export interface Germination {
  proposals: Proposal[];
  /** Proposals dropped: passage not in the case, or read as invented by Jev. */
  dropped: number;
  checked: boolean;
}
export type ProposalChoice = 'create' | 'place' | 'skip' | 'refuse';
export interface ProposalAnswer {
  id: string;
  choice: ProposalChoice;
  nature?: NoteType;
  title?: string;
}
/** What the answers did (Rust `germinate::Outcome`). */
export interface ProposalOutcome {
  created: string[];
  placed: string[];
  skipped: number;
  refused: number;
}
/** A case as "To tidy" shows it (`tidy.rs`). */
export interface CaseRef {
  id: string;
  title: string;
  nature: NoteType;
  project_name: string;
  /** The space of the Inbox when the case is in one (`project_name` is then technical). */
  inbox_space?: ProjectSpace | null;
  /** Written (`created_at`) for a decision, last changed (`updated_at`) for a case to archive. */
  date: string;
  excerpt: string;
}
export type TidyKind = 'archive' | 'supersede' | 'group' | 'conflict';
/**
 * A proposal of "To tidy": archive a case resolved long ago, mark an older decision as replaced,
 * file a case inside a close one (`group`), or check a case that may go against a decision.
 */
export interface TidyItem {
  id: string;
  kind: TidyKind;
  /** The case to archive, the older decision, the case to file, or the case against a decision. */
  case: CaseRef;
  /** The newer decision, the case to file `case` in, or the decision; null for archive. */
  other: CaseRef | null;
  /** Conflict: what `case` says on the point, and what the decision says. */
  passage: string | null;
  other_passage: string | null;
  /** Group: both say nearly the same thing (found by words, without AI). */
  duplicate: boolean;
  /** Jev doubts, or nobody checked. */
  doubt: boolean;
  /** Jev judged the pair. */
  checked: boolean;
  proposed_by: string | null;
  created_at: string;
}
export interface TidyAnswer {
  id: string;
  accept: boolean;
  /** Group: file `other` inside `case` instead. */
  swap?: boolean;
}
export interface TidyOutcome {
  archived: number;
  superseded: number;
  grouped: number;
  to_fix: number;
  refused: number;
  /** Accepted proposals that no longer held when applied (nothing changed). */
  skipped: number;
}
export interface TidySettings {
  /** Weeks before archiving is proposed: 0 (never), 2, 4 or 8. */
  archive_weeks: number;
  /** Compare decisions: replaced by a newer one, or contradicted by a case. */
  supersede: boolean;
  /** Propose to file close cases one inside the other. */
  group: boolean;
}
/** What holds in a project, what moves, what stays open (`gather.rs`). */
export interface ProjectSummary {
  project_id: string;
  project_name: string;
  /** Decisions no newer one replaced, the newest first. */
  decisions: CaseRef[];
  replaced_count: number;
  in_progress: CaseRef[];
  open_questions: CaseRef[];
  /** Cases that may go against a decision in force. */
  to_check: TidyItem[];
  recently_resolved: CaseRef[];
}
/** The work profile of a space (`profile.rs`): a case of its Inbox the person edits freely. */
export interface WorkProfile {
  space: 'private' | 'work';
  /** Null until the first line is added or the person opens it to edit. */
  note_id: string | null;
  content: string;
  revision: number | null;
}
export type ProfileSection = 'rules' | 'work' | 'refused';
/** A line Humcase or an agent proposes to add to the profile. */
export interface ProfileItem {
  id: string;
  space: 'private' | 'work';
  section: ProfileSection;
  /** `rule` (a decision Jev reads as a lasting rule), `work`, `refused` or `agent`. */
  source: 'rule' | 'work' | 'refused' | 'agent';
  text: string;
  /** The decision or case it comes from. */
  case: CaseRef | null;
  /** An agent's reason, or what the line was counted on. */
  detail: string;
  proposed_by: string | null;
  chance: number | null;
  created_at: string;
}
export interface ProfileView {
  profile: WorkProfile;
  pending: ProfileItem[];
}
export interface ProfileAnswer {
  id: string;
  accept: boolean;
}
export interface ProfileOutcome {
  added: number;
  refused: number;
  ids: string[];
  profiles: string[];
}
export interface ProfileSettings {
  /** Humcase proposes lines to add to the profile. */
  propose: boolean;
  /** Home shows the cases that wait for a next step. */
  nudges: boolean;
}
/** A case waiting for a next step, chosen once a day without AI. */
export interface Nudge {
  id: string;
  space: 'private' | 'work' | null;
  case: CaseRef;
  idle_days: number;
  /** Long enough for Sprout; otherwise the case is only opened. */
  can_sprout: boolean;
}
export type NudgeAnswer = 'accepted' | 'skipped' | 'refused';
/** A decision's place in the history. */
export interface Supersession {
  superseded_by: CaseRef | null;
  superseded_at: string | null;
  replaces: CaseRef[];
  /** Proposals waiting where this decision is the newer one. */
  asking: TidyItem[];
}
export type ProjectSpace = 'private' | 'work';
/** What a sub-case holds (v22); a top-level case is `text`. */
export type ContentKind = 'text' | 'image' | 'link' | 'file' | 'sketch' | 'research';

export interface Project {
  id: string;
  name: string;
  color: string;
  created_at: string;
  /** Last write to the project (ISO 8601 UTC); equal to `created_at` until it changes. */
  updated_at: string;
  /** Concurrency version: advances on every write to the project (0 at creation). */
  revision: number;
  space: ProjectSpace | null;
  is_inbox?: boolean;
  /** Archive date (ISO 8601 UTC); absent or null for an active project. */
  archived_at?: string | null;
  open_bugs?: number;
  open_todos?: number;
}

export interface Activity {
  id: string;
  name: string;
  space: ProjectSpace;
  created_at: string;
  /** Last write to the activity (ISO 8601 UTC); equal to `created_at` until it changes. */
  updated_at: string;
  /** Concurrency version: advances on every write to the activity, including attachments. */
  revision: number;
  project_ids: string[];
}

export interface Note {
  id: string;
  project_id: string;
  title: string;
  content: string;
  note_type: NoteType;
  status: NoteStatus;
  resolution_comment?: string | null;
  raw_content?: string | null;
  priority?: string | null;
  git_branch?: string | null;
  tags?: string | null;
  created_at: string;
  updated_at: string;
  /** Due date in `YYYY-MM-DD` format (local day); `null`/absent = no due date. */
  due_at?: string | null;
  /** Concurrency version: incremented on every write (app or agent). */
  revision: number;
  /** Archive date (ISO 8601 UTC); absent or null for an active note. */
  archived_at?: string | null;
  /** Trash date (ISO 8601 UTC); absent or null for a live note. */
  deleted_at?: string | null;
  /** Parent case (v22); absent or null for a top-level case. */
  parent_id?: string | null;
  /** What the case holds (v22); absent in old snapshots, read as `text`. */
  content_kind?: ContentKind;
}

/** Where a case comes from (v22): source, link, agent research. Append-only. */
export interface CaseProvenance {
  id: string;
  note_id: string;
  kind: string;
  label: string | null;
  url: string | null;
  excerpt: string | null;
  /** Agent that recorded it; null when a person did. */
  agent: string | null;
  created_at: string;
}

/** Position of a case on its parent's board (`get_case_positions`). */
export interface CasePosition {
  id: string;
  x: number;
  y: number;
}

/** Where the open case sits and where it comes from (`get_case_context`). */
export interface CaseContext {
  /** Ancestors from the top-level case down to the direct parent; empty for a top-level case. */
  path: { id: string; title: string }[];
  provenance: CaseProvenance[];
}

/**
 * Snapshot of a note's **previous** values before a write attributed to an agent.
 *
 * `fields` is a JSON object of the fields actually modified (title/content/note_type/status/tags/
 * priority/due_at); `raw_content` never appears in it. `revision` is the note's previous version.
 * Type aligned with the Rust struct `NoteRevision` (sync invariant in AGENTS.md).
 */
export interface NoteRevision {
  id: string;
  note_id: string;
  revision: number;
  /** Author of the modification (e.g. `agent`). */
  source: string;
  /** JSON object: values of the fields before the attributed write. */
  fields: string;
  created_at: string;
}

export interface NoteRelations {
  /** Notes that were linked to this task. */
  sources: Note[];
  /** Tasks linked to this note. */
  tasks: Note[];
}


export interface CreateNoteInput {
  project_id: string;
  title: string;
  content?: string;
  note_type: NoteType;
  priority?: string | null;
  git_branch?: string | null;
  tags?: string | null;
  /** Due date `YYYY-MM-DD`; an empty string clears the due date. */
  due_at?: string | null;
  /** Original raw dictation, if the note carries one (also recorded as the first fragment). */
  raw_content?: string | null;
}

export interface UpdateNoteInput {
  status?: NoteStatus;
  content?: string;
  resolution_comment?: string | null;
  title?: string;
  note_type?: NoteType;
  /**
   * Never sent: the engine refuses to replace the raw dictation (batch S6). It is appended with
   * `appendRawContent`, including when the escrow is replayed. Field kept on the Rust side for an explicit refusal.
   */
  raw_content?: never;
  priority?: string | null;
  git_branch?: string | null;
  tags?: string | null;
  /** Due date `YYYY-MM-DD`; an empty string clears the due date. */
  due_at?: string | null;
}

/** Metadata of an AI provider (no key: only the name of the environment variable). */
export interface AiProviderInfo {
  id: string;
  label: string;
  key_env: string;
  default_model: string;
  url_env: string;
  model_env: string;
  needs_key: boolean;
  /** A key is available (entered in the app or via environment variable), never its value. */
  key_defined: boolean;
  key_source: 'app' | 'env' | 'none';
  /** The server address can be entered in the app (local model, custom server). */
  url_editable: boolean;
  /** Server address actually used; empty if none is configured. */
  url: string;
  /** Connected: address known and key present when one is required. */
  ready: boolean;
}

/**
 * An agent that opened a session with this profile, and when last (Settings → Agents, Home's first
 * steps). Type aligned with the Rust struct `agents_seen::AgentSeen` (sync invariant in AGENTS.md).
 */
export interface AgentSeen {
  name: string;
  at_unix_ms: number;
}

/**
 * One AI call or dictation session, as Settings → Consumption reads it: quantities only, never a
 * price. Type aligned with the Rust struct `ai_usage::UsageRecord` (sync invariant in AGENTS.md).
 */
export interface AiUsageRecord {
  at_unix_ms: number;
  /** `transcription`, `integrate`, `structure`, `clean` or `structure_agent`. */
  operation: string;
  provider: string;
  model: string;
  /** Language of a transcription (`multi`: several languages). */
  language: string | null;
  input_tokens: number | null;
  output_tokens: number | null;
  reasoning_tokens: number | null;
  audio_seconds: number | null;
  ok: boolean;
}

export interface AiProvidersInfo {
  cleanup: AiProviderInfo[];
  transcription: AiProviderInfo[];
  /** Readers of the natures to check (TypeSafe's Jev). */
  reading?: AiProviderInfo[];
  /** Reason for the last incident while reading the keys config (corrupted file), if any.
   *  Name aligned with the Rust field `config_alert` (sync invariant in AGENTS.md). */
  config_alert?: string | null;
}

/**
 * One write on a case, recorded by the database with the actor of the connection (v24).
 * Type aligned with the Rust struct `CaseEvent` (sync invariant in AGENTS.md).
 */
export interface CaseEvent {
  id: string;
  note_id: string;
  /** `created`, `edited`, `status`, `report`, `moved`, `trashed`, `restored`, `archived`, `unarchived`. */
  kind: string;
  /** `human` (the interface), `agent` (MCP), `cli`, `unknown`. */
  actor_kind: 'human' | 'agent' | 'cli' | 'unknown' | string;
  /** Agent or CLI name when known; `null` for the person. */
  actor_name?: string | null;
  /** Kind-specific facts: `{from, to}` of a status, the parents of a move... */
  detail: Record<string, unknown> | null;
  /** Fields changed by an `edited` event (consecutive edits of one actor are merged). */
  fields: string[];
  created_at: string;
  last_at: string;
}

/** The person's decision on a case's result (v24), distinct from the declared status. */
export interface CaseReview {
  id: string;
  note_id: string;
  decision: 'accepted' | 'rework_requested';
  /** Revision of the case the person reviewed. */
  revision: number;
  comment?: string | null;
  rework_number?: number | null;
  created_at: string;
}

export type ReviewStateKind = 'none' | 'to_review' | 'rework_requested' | 'accepted' | 'accepted_changed';

/** Where the decision on a case's result stands. */
export interface ReviewState {
  note_id: string;
  state: ReviewStateKind;
  rework_count: number;
  /** Latest recorded move to `resolved` (who declared the result, when). */
  declared?: CaseEvent | null;
  last_review?: CaseReview | null;
}

/** Who did what on a case, the decisions on its result and where they stand. */
export interface CaseJournal {
  events: CaseEvent[];
  reviews: CaseReview[];
  review: ReviewState;
}

export interface BriefCase {
  note_id: string;
  rework_count: number;
  declared?: CaseEvent | null;
  last_review?: CaseReview | null;
}

/** What changed since the person's last visit. */
export interface Brief {
  /** Start of the period (ISO 8601 UTC). */
  since: string;
  /** No "mark all as seen" yet: the period is the last 24 hours. */
  since_is_fallback: boolean;
  to_review: BriefCase[];
  /** Non-human writes not seen yet, most recent first. */
  changes: CaseEvent[];
  reworks_pending: BriefCase[];
}

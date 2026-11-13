/* The assistant window (one conversation that answers, discusses and proposes actions). */

/** A source of a reply: a case (`note_id`) or a section of Humcase's help (`help`, no `note_id`). */
export interface AssistantSourceV2 {
  note_id: string | null;
  title: string;
  /** The sentences cited, word for word. */
  passages: string[];
  help: boolean;
}

/** A piece of the reply's paragraph; `cite` is the 1-based number of its source. */
export interface AssistantPartV2 {
  text: string;
  cite: number | null;
}

/** What the reply proposes; nothing happens before the person clicks. */
export type AssistantActionV2 =
  | { kind: 'create_sub_case'; case_id: string; case_title: string; title: string }
  | { kind: 'archive'; case_id: string; case_title: string }
  /** Open Sprout on the case: its proposals wait there, nothing is written by the click. */
  | { kind: 'germinate'; case_id: string; case_title: string }
  | { kind: 'move'; case_id: string; case_title: string; project_id: string; project_name: string; from_project_id: string }
  | {
    kind: 'tidy'; tidy_id: string; tidy_kind: 'archive' | 'supersede' | 'group' | 'conflict';
    case_id: string; case_title: string; other_id: string | null; other_title: string | null;
    passage: string | null; other_passage: string | null;
  };

export interface AssistantReply {
  /** `no_ai`: no AI connected. */
  kind: 'reply' | 'no_ai';
  parts: AssistantPartV2[];
  /** At most one question, shown highlighted. */
  question: string | null;
  sources: AssistantSourceV2[];
  actions: AssistantActionV2[];
  model_label: string | null;
  /** Sentences dropped by the check (not shown). */
  dropped: number;
}

/** An earlier exchange as the engine reads it (oldest first). */
export interface AssistantPreviousTurnV2 {
  said: string;
  /** The reply's text, its question included. */
  reply: string;
  /** Cases of its sources, help left out. */
  source_ids: string[];
}

/** What the window's small screen shows. */
export interface AssistantStatus {
  model_label: string | null;
  cases: number;
  to_decide: number;
  /** Items of To tidy where two cases disagree (a case against a decision, a replaced decision). */
  conflicts: number;
  /** The other items of To tidy. */
  to_tidy: number;
}

/** How a done action is taken back: data, not a function, so it survives closing the window and
 * the handover between the app and Mini Humcase. */
export type AssistantUndo =
  | { kind: 'delete_note'; id: string }
  | { kind: 'unarchive'; id: string }
  | { kind: 'move'; id: string; project_id: string }
  | { kind: 'untidy'; tidy_id: string };

/** An action of a reply once done: what changed, the case to open, how to undo it. */
export interface AssistantDone {
  text: string;
  open_id?: string;
  undo?: AssistantUndo;
}

/** One exchange of the window. */
export interface AssistantEntry {
  id: number;
  said: string | null;
  reply?: AssistantReply;
  error?: string;
  /** The actions of the reply already done, by index: a closed and reopened window, or the other
   * window, shows them done instead of proposing them again. */
  done?: Record<number, AssistantDone>;
}

/** How many earlier exchanges the engine receives. */
export const PREVIOUS_TURNS = 8;

export function previousTurnsV2(entries: AssistantEntry[]): AssistantPreviousTurnV2[] {
  return entries.filter(entry => entry.reply?.kind === 'reply').slice(-PREVIOUS_TURNS).map(entry => {
    const reply = entry.reply!;
    const text = reply.parts.map(part => part.text).join('');
    return {
      said: entry.said ?? '',
      reply: reply.question ? `${text} ${reply.question}`.trim() : text,
      source_ids: reply.sources.filter(source => !source.help && source.note_id).map(source => source.note_id!),
    };
  });
}

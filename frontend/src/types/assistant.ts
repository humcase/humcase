/** A case an answer of "Ask Humcase" relies on, with the sentences of it the answer cites,
 * word for word (its title is not repeated among them). */
export interface AssistantSource {
  note_id: string;
  title: string;
  passages: string[];
}

/** An action the answer proposes; nothing happens until the person clicks. */
export interface AssistantAction {
  kind: 'create_sub_case';
  title: string;
  parent_id: string;
  parent_title: string;
}

/** A piece of the answer; `cite` is the 1-based number of the source it comes from. */
export interface AssistantPart {
  text: string;
  cite?: number;
}

export interface AssistantAnswer {
  /** `answer`: written from the cases; `not_found`: nothing in the cases; `no_ai`: no AI connected,
   * the closest cases only. */
  kind: 'answer' | 'not_found' | 'no_ai';
  parts: AssistantPart[];
  sources: AssistantSource[];
  action?: AssistantAction | null;
  /** What left the computer: the AI service and how many case excerpts it received. */
  sent?: { service: string; excerpts: number } | null;
  /** Sentences of the AI dropped because the excerpt they named was not sent. */
  dropped?: number;
}

/** One exchange of the conversation. The conversation lives for the session; an answer worth
 * keeping becomes a case. */
export interface AssistantTurn {
  id: number;
  question: string;
  answer?: AssistantAnswer;
  error?: string;
}

/** A previous exchange as the engine reads it: the answer as plain text and the cases it cited. */
export interface AssistantPreviousTurn {
  question: string;
  answer: string;
  source_ids: string[];
}

export function previousTurn(turn: AssistantTurn): AssistantPreviousTurn {
  return {
    question: turn.question,
    answer: turn.answer?.parts.map(part => part.text).join('') ?? '',
    source_ids: turn.answer?.sources.map(source => source.note_id) ?? [],
  };
}

/** Answers a question with the previous exchanges of the conversation. */
export type AssistantEngine = (question: string, previous: AssistantTurn[]) => Promise<AssistantAnswer>;

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

/** One exchange of the window. */
export interface AssistantEntry {
  id: number;
  said: string | null;
  reply?: AssistantReply;
  error?: string;
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

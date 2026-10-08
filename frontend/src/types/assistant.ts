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

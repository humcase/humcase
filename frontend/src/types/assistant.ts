/** A case an answer of "Ask Humcase" relies on, with the passage it quotes word for word. */
export interface AssistantSource {
  noteId: string;
  title: string;
  passage: string;
}

/** An action the answer proposes; nothing happens until the person clicks. */
export interface AssistantAction {
  kind: 'create_sub_case';
  title: string;
  parentId: string;
  parentTitle: string;
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
  action?: AssistantAction;
  /** What left the computer: the AI service and how many case excerpts it received. */
  sent?: { service: string; excerpts: number };
}

/** One exchange of the conversation. The conversation lives for the session; an answer worth
 * keeping becomes a case. */
export interface AssistantTurn {
  id: number;
  question: string;
  answer?: AssistantAnswer;
  error?: string;
}

/** Answers a question with the previous exchanges of the conversation. */
export type AssistantEngine = (question: string, previous: AssistantTurn[]) => Promise<AssistantAnswer>;

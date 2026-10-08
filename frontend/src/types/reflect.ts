import type { AssistantSource } from './assistant';

/** What a case of the space says about the idea: it goes the same way, it was already decided,
 * or it may contradict what the person just said. */
export interface ReflectLink {
  kind: 'supports' | 'decided' | 'conflicts';
  source: AssistantSource;
}

/** One reply of the thinking partner: it restates, links what the cases already say, and asks one
 * question; when the person is stuck, it proposes a lead. */
export interface ReflectReply {
  restated?: string;
  links: ReflectLink[];
  lead?: string;
  question?: string;
  /** The partner thinks a direction emerged and offers a summary. */
  offer_summary?: boolean;
  /** What left the computer: the AI service and how many case excerpts it received. */
  sent?: { service: string; excerpts: number } | null;
}

export interface ReflectTurn {
  id: number;
  /** What the person said (typed or dictated); empty on the partner's opening. */
  said?: string;
  reply?: ReflectReply;
  error?: string;
}

/** The summary proposed at the end; the person corrects it before keeping it as a case. */
export interface ReflectSummary {
  title: string;
  direction: string;
  kept: string[];
  rejected: string[];
  evolution: string[];
  questions: string[];
  /** Actions with no case yet: each becomes a sub-case when ticked. */
  actions: { title: string; create: boolean }[];
  /** Existing cases the direction waits on (a decision to take). */
  waiting_on: { note_id: string; title: string }[];
  /** What left the computer: the AI service and how many case excerpts it received. */
  sent?: { service: string; excerpts: number } | null;
}

/** A previous exchange as the engine reads it: what the person said, or the partner's reply as
 * plain text with the cases it linked. */
export interface ReflectPreviousTurn {
  said?: string;
  reply?: string;
  source_ids: string[];
}

export function reflectPreviousTurns(turns: ReflectTurn[]): ReflectPreviousTurn[] {
  return turns.filter(turn => turn.said || turn.reply).map(turn => turn.said
    ? { said: turn.said, source_ids: [] }
    : {
      reply: [turn.reply?.restated, turn.reply?.lead, turn.reply?.question].filter(Boolean).join(' '),
      source_ids: turn.reply?.links.map(link => link.source.note_id) ?? [],
    });
}

export interface ReflectEngine {
  reply: (subject: { id: string; title: string } | null, turns: ReflectTurn[]) => Promise<ReflectReply>;
  summarize: (subject: { id: string; title: string } | null, turns: ReflectTurn[]) => Promise<ReflectSummary>;
}

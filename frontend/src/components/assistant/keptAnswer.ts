import type { AssistantAnswer } from '../../types/assistant';

/** The text of a case kept from an answer: the answer with its citation numbers, then its sources
 * and the passages they quote. */
export function keptAnswerMarkdown(answer: AssistantAnswer): string {
  const text = answer.parts.map(part => part.text + (part.cite ? ` [${part.cite}]` : '')).join('');
  const sources = answer.sources
    .map((source, index) => `${index + 1}. ${source.title}${source.passage ? `: “${source.passage}”` : ''}`)
    .join('\n');
  return [text, sources && `### Sources\n${sources}`].filter(Boolean).join('\n\n');
}

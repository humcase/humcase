import type { AssistantAnswer } from '../../types/assistant';

/** The text of a case kept from an answer: the answer with its citation numbers, then its sources
 * and the passages they quote (`sourcesHeading` and `source` give the words of the interface's
 * language). */
export function keptAnswerMarkdown(answer: AssistantAnswer, sourcesHeading: string, source: (title: string, passage: string) => string): string {
  const text = answer.parts.map(part => part.text + (part.cite ? ` [${part.cite}]` : '')).join('');
  const sources = answer.sources
    .map((item, index) => `${index + 1}. ${item.passages.length ? source(item.title, item.passages.join(' … ')) : item.title}`)
    .join('\n');
  return [text, sources && `### ${sourcesHeading}\n${sources}`].filter(Boolean).join('\n\n');
}

import type { ReflectSummary, ReflectTurn } from '../../types/reflect';

export type ReflectLabel = `section.${'direction' | 'kept' | 'rejected' | 'evolution' | 'questions'}` | 'you' | 'partner' | 'conversation';

/** The kept summary as the case's text: the corrected sections, then the whole conversation, so
 * nothing said is lost. `label` gives the section and speaker names in the interface language. */
export function reflectSummaryMarkdown(summary: ReflectSummary, turns: ReflectTurn[], label: (key: ReflectLabel) => string): string {
  const list = (items: string[]) => items.map(item => item.trim()).filter(Boolean).map(item => `- ${item}`).join('\n');
  const sections = (['kept', 'rejected', 'evolution', 'questions'] as const)
    .map(key => [label(`section.${key}`), list(summary[key])] as const)
    .filter(([, body]) => body);
  const conversation = turns.flatMap(turn => [
    turn.said ? `**${label('you')}** : ${turn.said}` : '',
    turn.reply ? `**${label('partner')}** : ${[turn.reply.restated, turn.reply.lead, turn.reply.question].filter(Boolean).join(' ')}` : '',
  ]).filter(Boolean).join('\n\n');
  return [
    `### ${label('section.direction')}\n${summary.direction.trim()}`,
    ...sections.map(([heading, body]) => `### ${heading}\n${body}`),
    `### ${label('conversation')}\n${conversation}`,
  ].join('\n\n');
}

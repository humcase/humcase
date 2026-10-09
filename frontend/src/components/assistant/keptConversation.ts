import type { AssistantEntry } from '../../types/assistant';

/** The sources of a kept conversation: every case cited, once (help sections left out). */
export function conversationSources(entries: AssistantEntry[]): { title: string; passages: string[] }[] {
  const byKey = new Map<string, { title: string; passages: string[] }>();
  for (const entry of entries) {
    for (const source of entry.reply?.sources ?? []) {
      if (source.help) continue;
      const key = source.note_id ?? source.title;
      const known = byKey.get(key);
      if (known) known.passages.push(...source.passages.filter(passage => !known.passages.includes(passage)));
      else byKey.set(key, { title: source.title, passages: [...source.passages] });
    }
  }
  return [...byKey.values()];
}

/**
 * The text of a case kept from the assistant's conversation: each exchange in order (what the
 * person said, then the reply with its citation numbers and its question), then the sources and
 * the passages they quote. `labels` give the words of the interface's language.
 */
export function keptConversationMarkdown(entries: AssistantEntry[], labels: {
  you: string;
  assistant: string;
  sources: string;
  source: (title: string, passage: string) => string;
}): string {
  const sources = conversationSources(entries);
  const number = (entry: AssistantEntry, cite: number) => {
    const source = entry.reply?.sources[cite - 1];
    if (!source || source.help) return '';
    const index = sources.findIndex(item => item.title === source.title);
    return index < 0 ? '' : ` [${index + 1}]`;
  };
  const blocks: string[] = [];
  for (const entry of entries) {
    if (entry.said) blocks.push(`**${labels.you}:** ${entry.said}`);
    const reply = entry.reply;
    if (!reply || reply.kind !== 'reply') continue;
    const text = reply.parts.map(part => part.text + (part.cite ? number(entry, part.cite) : '')).join('');
    blocks.push(`**${labels.assistant}:** ${[text, reply.question].filter(Boolean).join(' ')}`);
  }
  const list = sources
    .map((item, index) => `${index + 1}. ${item.passages.length ? labels.source(item.title, item.passages.join(' … ')) : item.title}`)
    .join('\n');
  return [...blocks, list && `### ${labels.sources}\n${list}`].filter(Boolean).join('\n\n');
}

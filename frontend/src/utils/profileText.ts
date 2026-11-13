export type ProfileBlock = { kind: 'heading'; text: string } | { kind: 'line'; text: string; source: string | null } | { kind: 'text'; text: string };

/** The profile's Markdown as the page shows it: section headings, lines with their source, text. */
export function readProfile(content: string): ProfileBlock[] {
  const blocks: ProfileBlock[] = [];
  for (const raw of content.split('\n')) {
    const line = raw.trim();
    if (!line) continue;
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    if (heading) { blocks.push({ kind: 'heading', text: heading[1] }); continue; }
    const item = /^(?:[-*+]|\d+[.)])\s+(?:\[[ xX]\]\s+)?(.*)$/.exec(line);
    if (item) {
      const sourced = /^(.*\S)\s+\(([^()]+)\)$/.exec(item[1]);
      blocks.push(sourced ? { kind: 'line', text: sourced[1], source: sourced[2] } : { kind: 'line', text: item[1], source: null });
      continue;
    }
    blocks.push({ kind: 'text', text: line });
  }
  return blocks;
}

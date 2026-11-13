import { unified } from 'unified';
import remarkParse from 'remark-parse';
import i18n from '../i18n';
import { displayImage, SCHEMA_IMAGE } from './imageRefs';

export interface NoteImage { src: string; alt: string }
interface MarkdownNode {
  type: string;
  url?: string;
  alt?: string | null;
  identifier?: string;
  children?: MarkdownNode[];
}
const parser = unified().use(remarkParse);

/** Previews only reuse local images: portable reference or local protocol subject
 * to the Tauri scope. No remote image is loaded for a preview. */
function isLocalAsset(value: string): boolean {
  if (value.startsWith(SCHEMA_IMAGE)) return true;
  try {
    const url = new URL(value);
    return !url.username && !url.password && !url.port && (
      (url.protocol === 'http:' && url.hostname === 'asset.localhost') ||
      (url.protocol === 'asset:' && url.hostname === 'localhost')
    );
  } catch { return false; }
}

export function firstLocalNoteImage(markdown: string): NoteImage | undefined {
  // Every Markdown image, inline or by reference, starts with `![`: without one, no parsing.
  if (!markdown.includes('![')) return undefined;
  const root = parser.parse(markdown) as MarkdownNode;
  const definitions = new Map<string, string>();
  const images: MarkdownNode[] = [];
  const visit = (node: MarkdownNode) => {
    if (node.type === 'definition' && node.identifier && node.url && !definitions.has(node.identifier)) {
      definitions.set(node.identifier, node.url);
    }
    if (node.type === 'image' || node.type === 'imageReference') images.push(node);
    node.children?.forEach(visit);
  };
  visit(root);
  for (const node of images) {
    const src = node.url ?? definitions.get(node.identifier ?? '');
    if (src && isLocalAsset(src)) return { src: displayImage(src), alt: node.alt?.trim() || i18n.t('notes.image.alt') };
  }
}

import Image from '@tiptap/extension-image';
import { displayImage, referenceFromDisplay } from '../utils/imageRefs';

/**
 * Image whose source stays the portable reference `humcase-image://<name>` in the document (hence
 * in the saved Markdown); only the HTML rendering translates it into a local URL. An old local URL
 * of an internal image reread from the Markdown or pasted as HTML becomes a portable reference again.
 */
export const ImagePortable = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      src: {
        default: null,
        parseHTML: (element: HTMLElement) => {
          const src = element.getAttribute('src');
          return src ? referenceFromDisplay(src) : null;
        },
        renderHTML: (attributes: { src?: string | null }) =>
          attributes.src ? { src: displayImage(attributes.src) } : {},
      },
    };
  },

  /* tiptap-markdown serializes the image as an inline node (without closing the block) whereas this node
   * is a block: the next paragraph ended up glued to the image (`![a](…)Suite`). */
  addStorage() {
    return {
      markdown: {
        serialize(state: MarkdownSerializerState, node: { attrs: { src?: string | null; alt?: string | null; title?: string | null } }) {
          const { src, alt, title } = node.attrs;
          const target = (src ?? '').replace(/[()]/g, '\\$&');
          const titleAttr = title ? ` "${title.replace(/"/g, '\\"')}"` : '';
          state.write(`![${state.esc(alt ?? '')}](${target}${titleAttr})`);
          state.closeBlock(node);
        },
        parse: {},
      },
    };
  },
});

interface MarkdownSerializerState {
  write(content: string): void;
  esc(text: string): string;
  closeBlock(node: unknown): void;
}

import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useTranslation } from 'react-i18next';
import { ArrowSquareOut } from '@phosphor-icons/react';
import type { Note } from '../../types';
import { directSubCases } from '../../utils/subCases';
import { transformerUrlMarkdown } from '../../utils/imageRefs';
import { webAddress } from '../../utils/caseContent';
import { api } from '../../services/tauriApi';
import SketchThumbnail from '../sketch/SketchThumbnail';
import { Button } from '../ui/Button';

interface CaseDocumentProps {
  /** Cases to gather, each followed by all its sub-cases. */
  roots: Note[];
  allNotes: Note[];
  onOpenNote: (note: Note) => void;
}

/**
 * Document overview: a case and its sub-cases in one readable page, in tree order. Read only:
 * each section opens the original case to edit it.
 */
export default function CaseDocument({ roots, allNotes, onOpenNote }: CaseDocumentProps) {
  const { t } = useTranslation();
  // A link opens in the browser: followed in place, it would replace the app in its window.
  const openOutside = (event: { preventDefault: () => void }, href: string | undefined) => {
    event.preventDefault();
    const url = href ? webAddress(href) : null;
    if (url) void api.openLink(url).catch(() => {});
  };
  const section = (note: Note, depth: number, seen: Set<string>) => {
    // A strict tree has no cycle; the guard keeps a corrupted list from looping.
    if (seen.has(note.id)) return null;
    seen.add(note.id);
    const Heading = (['h2', 'h3', 'h4', 'h5', 'h6'] as const)[Math.min(depth, 4)];
    return (
      <section key={note.id} className="case-document__section" data-depth={depth}>
        <div className="case-document__heading">
          <Heading>{note.title || t('common.untitled')}</Heading>
          <Button type="button" variant="ghost" size="icon-sm" aria-label={t('workspace.board.openOriginal', { title: note.title || t('common.untitled') })} title={t('workspace.board.openOriginalShort')} onClick={() => onOpenNote(note)}><ArrowSquareOut size={14} /></Button>
        </div>
        {note.content_kind === 'sketch'
          ? <SketchThumbnail content={note.content} label={t('workspace.board.sketchPicture', { title: note.title || t('common.untitled') })} />
          : note.content.trim() && <div className="case-document__content prose prose-sm max-w-none"><ReactMarkdown skipHtml remarkPlugins={[remarkGfm]} urlTransform={transformerUrlMarkdown} components={{ a: ({ href, children }) => <a href={href} onClick={event => openOutside(event, href)}>{children}</a> }}>{note.content}</ReactMarkdown></div>}
        {directSubCases(note.id, allNotes).map(child => section(child, depth + 1, seen))}
      </section>
    );
  };
  const seen = new Set<string>();
  return (
    <aside className="case-document" aria-label={t('workspace.board.documentLabel')}>
      {roots.length ? roots.map(root => section(root, 0, seen)) : <p className="case-document__empty">{t('workspace.board.documentEmpty')}</p>}
    </aside>
  );
}

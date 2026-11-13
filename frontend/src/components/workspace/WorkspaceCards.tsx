import { notePreview } from '../../utils/notePresentation';
import { FileText, GitBranch, SquaresFour } from '@phosphor-icons/react';
import type { Note, Project } from '../../types';
import { formatShortDate, parseTimestamp } from '../../utils/datetime';
import { DueBadge, PriorityBadge, StatusBadge, TagBadge, TypeBadge } from '../ui/Badge';
import { readTags } from '../../utils/tags';
import { memo, useMemo, useState, type ReactNode } from 'react';
import { renderExcerpt } from './cardExcerpts';
import { useTranslation } from 'react-i18next';
import { firstLocalNoteImage, type NoteImage } from '../../utils/noteImage';
import { Card } from '../ui/Card';
import './note-cards.css';

const MarkdownExcerpt = memo(function MarkdownExcerpt({ text }: { text: string }) {
  return renderExcerpt(text);
});

export function NoteThumbnail({ image }: { image: NoteImage }) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  if (failed) return <span className="workspace-note-card__image-unavailable">{t('workspace.cards.imageUnavailable', { alt: image.alt })}</span>;
  return <figure className="workspace-note-card__thumbnail">
    <img src={image.src} alt={image.alt} loading="lazy" decoding="async" onError={() => setFailed(true)} />
  </figure>;
}

export function NoteCard({ note, project, onOpen, compact = false, subCaseCount = 0, location }: {
  note: Note;
  project?: Project;
  onOpen: (note: Note) => void;
  compact?: boolean;
  /** Live sub-cases of the case; shown only when there are some. */
  subCaseCount?: number;
  /** Where the case lives, for a case shown outside its place. */
  location?: ReactNode;
}) {
  const { t } = useTranslation();
  // Computed once per version of the case (a reread keeps the same object when nothing changed).
  const preview = useMemo(() => notePreview(note), [note]);
  const reportTitle = t('workspace.cards.report');
  const report = useMemo(() => note.resolution_comment?.trim()
    ? notePreview({ ...note, title: reportTitle, content: note.resolution_comment })
    : '', [note, reportTitle]);
  const tags = readTags(note.tags);
  const image = useMemo(() => firstLocalNoteImage(note.content) ?? firstLocalNoteImage(note.resolution_comment ?? ''), [note.content, note.resolution_comment]);
  return (
    <Card as="article" interactive className={`workspace-note-card workspace-note-card--refined${compact ? ' workspace-note-card--compact' : ''}`}>
      {location && <p className="workspace-note-card__location">{location}</p>}
      <h3 className="workspace-note-card__title">
        <button type="button" onClick={() => onOpen(note)} aria-label={t('workspace.cards.open', { title: note.title || t('workspace.cards.untitledLower') })}>
          {note.title || t('common.untitled')}
        </button>
      </h3>
      <div className="workspace-note-card__top">
        <StatusBadge status={note.status} size="md" />
        <TypeBadge type={note.note_type} size="md" />
      </div>
      {image && <NoteThumbnail key={`${note.id}:${image.src}`} image={image} />}
      <div className="workspace-note-card__document">
      {preview && <div className="workspace-note-card__preview" aria-label={t('workspace.cards.previewLabel')}><MarkdownExcerpt text={note.content} /></div>}
      {report && <div className="workspace-note-card__report"><span><FileText size={14} aria-hidden="true" />{t('workspace.cards.report')}</span><div className="workspace-note-card__report-content"><MarkdownExcerpt text={note.resolution_comment ?? ''} /></div></div>}
        {!preview && !report && !image && <span className="workspace-note-card__empty">{t('workspace.cards.empty')}</span>}
      </div>
      {(tags.length > 0 || note.git_branch) && <div className="workspace-note-card__context">
        {tags.slice(0, 2).map((tag, index) => <TagBadge key={`${tag}-${index}`} tag={tag} />)}
        {tags.length > 2 && <span title={tags.slice(2).join(', ')}>+{tags.length - 2}</span>}
        {note.git_branch && <span className="workspace-note-card__branch" title={note.git_branch}><GitBranch size={14} aria-hidden="true" />{note.git_branch}</span>}
      </div>}
      <div className="workspace-note-card__bottom">
        {project && <span className="workspace-note-card__project"><span className="workspace-note-card__project-dot" style={{ backgroundColor: project.color || undefined }} />{project.name}</span>}
        <div className="workspace-note-card__footer-meta">
          {subCaseCount > 0 && <span className="workspace-note-card__sub-cases"><SquaresFour size={14} aria-hidden="true" />{t('notes.cases.subCasesBadge', { count: subCaseCount })}</span>}
          {note.priority && <PriorityBadge priority={note.priority} status={note.status} />}
          {note.due_at && <DueBadge due={note.due_at} />}
          <time className="workspace-note-card__date" dateTime={parseTimestamp(note.updated_at)?.toISOString()}>{formatShortDate(note.updated_at)}</time>
        </div>
      </div>
    </Card>
  );
}

import { useLayoutEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { createPortal } from 'react-dom';
import { ArrowDown, ArrowLeft } from '@phosphor-icons/react';
import type { Note } from '../../types';
import { formatDateTime, formatRelative } from '../../utils/datetime';
import { CasePath } from '../NoteSubCases';
import { CaseReviewBand } from '../review/CaseReviewBand';
import { actorLabel } from '../review/journalText';
import type { CaseJournal } from '../../types';
import { Button } from '../ui/Button';

/**
 * The top of the case: save state, path of parent cases, title, the decision band, when it was
 * made and last changed, and the cases with a similar text.
 */
export function NoteEditorHeader({
  note, isMiniMode, onBackToList, pathSlot, allNotes, onOpenNote, title, onTitleChange, onTitleBlur, isSaving, hasSaved,
  journal, onReviewed, onJumpToDetails, similarNotes, focusTitle = false, onTitleEnter,
}: {
  note: Note;
  isMiniMode?: boolean;
  onBackToList?: () => void;
  /** Topbar place for the parent cases; without it they show above the title. */
  pathSlot: HTMLElement | null;
  allNotes: Note[];
  /** Opens a parent case through the editor's guarded opener; absent: no path. */
  onOpenNote?: (target: Note) => Promise<void>;
  title: string;
  onTitleChange: (title: string) => void;
  onTitleBlur: (title: string) => void;
  isSaving: boolean;
  hasSaved: boolean;
  journal: CaseJournal | null;
  onReviewed: () => void;
  onJumpToDetails: () => void;
  similarNotes: Note[];
  /** A new case: the title takes the focus, its default text selected so typing replaces it. */
  focusTitle?: boolean;
  /** Enter in the title: the writing goes on in the text. */
  onTitleEnter?: () => void;
}) {
  const { t } = useTranslation();
  const titleRef = useRef<HTMLTextAreaElement>(null);
  // Once per case, when the field holds the case's title (it loads after the first render).
  const focusedFor = useRef<string | null>(null);
  const titleLoaded = title === note.title;
  useLayoutEffect(() => {
    if (!focusTitle || !titleLoaded || focusedFor.current === note.id || !titleRef.current) return;
    focusedFor.current = note.id;
    titleRef.current.focus();
    titleRef.current.select();
  }, [focusTitle, titleLoaded, note.id]);
  const lastChange = journal?.events[0];
  return (
    <div className="note-editor-header p-6 border-b border-border flex items-start justify-between gap-4">
      <div className="flex-1 min-w-0 flex flex-col">
        <div className="note-editor-save-state">
          <span className="text-muted-foreground" role="status">
            {isSaving ? t('editor.header.saving') : hasSaved ? t('editor.header.saved') : ""}
          </span>
        </div>
        {isMiniMode && onBackToList && (
          <Button variant="ghost" size="sm" onClick={onBackToList} className="mb-2 self-start">
            <ArrowLeft size={15} aria-hidden="true" />{t('editor.header.back')}
          </Button>
        )}

        {/* The parent cases go in the topbar breadcrumb when there is one, through the editor's
            guarded opener (a dictation in progress or an unsaved draft keeps the case open). */}
        {onOpenNote && (pathSlot
          ? createPortal(<CasePath key={`path-${note.id}`} note={note} allNotes={allNotes} onOpenNote={onOpenNote} crumbs />, pathSlot)
          : <CasePath key={`path-${note.id}`} note={note} allNotes={allNotes} onOpenNote={onOpenNote} />)}
        <textarea
          ref={titleRef}
          rows={1}
          aria-label={t('editor.header.titleLabel')}
          value={title}
          onChange={(e) => onTitleChange(e.target.value.replace(/[\r\n]+/g, ' '))}
          onKeyDown={event => {
            if (event.key !== 'Enter' || event.nativeEvent.isComposing) return;
            event.preventDefault();
            onTitleEnter?.();
          }}
          onBlur={(e) => onTitleBlur(e.target.value)}
          className={`note-editor-title font-bold bg-transparent border-none focus:outline-none focus:ring-1 focus:ring-primary rounded px-1 -ml-1 ${isMiniMode ? 'text-lg' : 'text-2xl'}`}
          placeholder={t('editor.header.titlePlaceholder')}
        />

        <CaseReviewBand note={note} journal={journal} onReviewed={onReviewed} />
        {!isMiniMode && <p className="note-editor-created">{t('editor.header.created', { date: formatDateTime(note.created_at) })}{lastChange && <> · <span title={formatDateTime(lastChange.last_at)}>{t('notes.activity.lastChange', { actor: actorLabel(lastChange, t), when: formatRelative(lastChange.last_at) })}</span></>}</p>}
        {!isMiniMode && <Button variant="ghost" size="sm" className="note-editor-details-jump" onClick={onJumpToDetails}>{t('editor.header.details')}<ArrowDown size={15} aria-hidden="true" /></Button>}
        {similarNotes.length > 0 && (
          <details className="note-editor-similar mt-3">
            <summary>{t('editor.header.similar', { count: similarNotes.length })}</summary>
            <ul className="note-editor-similar__list list-disc list-inside">
              {similarNotes.map(sn => <li key={sn.id} className="truncate">{sn.title}</li>)}
            </ul>
          </details>
        )}
      </div>
    </div>
  );
}

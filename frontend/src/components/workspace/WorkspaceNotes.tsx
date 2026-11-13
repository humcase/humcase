import { Plus, Note as StickyNote } from '@phosphor-icons/react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../ui/EmptyState';
import type { Note } from '../../types';
import { NoteCard } from './WorkspaceCards';
import { Button } from '../ui/Button';

export interface WorkspaceNotesProps {
  /** Number of live sub-cases per case identifier. */
  subCaseCounts?: ReadonlyMap<string, number>;
  /** Where a case lives, shown when the cards hold sub-cases. */
  locate?: (note: Note) => ReactNode;
  notes: Note[];
  onNote: (note: Note) => void;
  onCreateNote: () => void;
  /** Cases of the scope before the filters. */
  totalCount?: number;
  /** A search or a filter of the page narrows the cards. */
  filtered?: boolean;
  onClearFilters?: () => void;
  /** Title of the empty state when the scope has no note yet (outside filters). */
  firstNoteTitle?: string;
}

/** Cards of the cases page; the search and filters live in the page's filter bar. */
export function WorkspaceNotes({ notes, onNote, onCreateNote, totalCount, filtered = false, onClearFilters, firstNoteTitle, subCaseCounts, locate }: WorkspaceNotesProps) {
  const { t } = useTranslation();
  const hasAnyNotes = (totalCount ?? notes.length) > 0;
  const filteredOut = notes.length === 0 && (hasAnyNotes || filtered);

  return <main className="workspace-page workspace-notes">
    <section className="workspace-section" aria-label={t('workspace.notes.sectionLabel')}>
      {notes.length ? <div className="workspace-note-grid">{notes.map(note => <NoteCard key={note.id} note={note} onOpen={onNote} subCaseCount={subCaseCounts?.get(note.id)} location={locate?.(note)} />)}</div> : <EmptyState icon={<StickyNote />} headingLevel={2} title={filteredOut ? t('workspace.notes.noneFound') : firstNoteTitle ?? t('workspace.notes.firstNoteTitle')} description={filteredOut ? t('workspace.notes.descFiltered') : t('workspace.notes.descFirst')} action={filteredOut && onClearFilters ? <Button type="button" variant="secondary" onClick={onClearFilters}>{t('workspace.notes.clearFilters')}</Button> : <Button type="button" variant="secondary" onClick={onCreateNote}><Plus size={17} aria-hidden="true" />{t('workspace.notes.createNote')}</Button>} />}
    </section>
  </main>;
}

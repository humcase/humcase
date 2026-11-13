import { X } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { SegmentedControl } from '../ui/SegmentedControl';
import NoteSortMenu from './NoteSortMenu';
import type { NoteSort } from '../../utils/noteSort';
import type { NotesViewMode as ViewMode } from '../../utils/viewMode';

interface NotesViewbarProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  noteSort: NoteSort;
  onNoteSortChange: (sort: NoteSort) => void;
  /** A search is typed: the sort offers "Relevance". */
  searching?: boolean;
  archivedNotesCount: number;
  showArchived: boolean;
  onToggleArchived: () => void;
}

/** Display bar of the cases page: the view (Ctrl+1 to 4), the sort and the archived cases. */
export default function NotesViewbar({
  viewMode, onViewModeChange, noteSort, onNoteSortChange, searching, archivedNotesCount, showArchived, onToggleArchived,
}: NotesViewbarProps) {
  const { t } = useTranslation();
  return (
    <div className="workspace-viewbar" aria-label={t('workspace.viewbar.label')}>
      <SegmentedControl label={t('workspace.viewbar.mode')} size="sm" value={viewMode} onValueChange={onViewModeChange} items={[{ value: 'plan', label: t('workspace.viewbar.plan') }, { value: 'cards', label: t('workspace.viewbar.cards') }, { value: 'list', label: t('workspace.viewbar.list') }, { value: 'kanban', label: t('workspace.viewbar.kanban') }]} />
      {(viewMode === 'cards' || viewMode === 'list') && <NoteSortMenu value={noteSort} onChange={onNoteSortChange} searching={searching} />}
      {(archivedNotesCount > 0 || showArchived) && <Button variant={showArchived ? 'secondary' : 'ghost'} size="sm" aria-pressed={showArchived} title={showArchived ? t('workspace.viewbar.backToActive') : t('workspace.viewbar.showArchived')} onClick={onToggleArchived}>{t('workspace.viewbar.archived', { n: archivedNotesCount })}{showArchived && <X size={14} aria-hidden="true" />}</Button>}
    </div>
  );
}

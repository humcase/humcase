import { useTranslation } from 'react-i18next';
import { Button } from './ui/Button';
import { Hint } from './ui/Hint';
import { formatChanged, formatDateTime } from '../utils/datetime';
import { statusLabel, ORDERED_STATUSES } from '../utils/statuses';
import { readTags } from '../utils/tags';

import { Note as NoteIcon, Plus } from '@phosphor-icons/react';
import type { ReactNode } from 'react';
import type { Note } from '../types';
import { notePreview } from '../utils/notePresentation';
import { DueBadge, PriorityBadge, StatusBadge, TagBadge, TypeBadge } from './ui/Badge';
import { List, ListRow } from './ui/List';
import { EmptyState } from './ui/EmptyState';
import './workspace/note-views.css';
import { SearchField } from './ui/SearchField';
import { SegmentedControl } from './ui/SegmentedControl';

interface NoteListProps {
  notes: Note[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  /** Creation delegated to the application: same path as Ctrl+N (the note is then selected). */
  onCreateNote: () => void;
  searchQuery: string;
  onSearchQueryChange: (query: string) => void;
  statusFilter?: string;
  onStatusFilterChange?: (s: string) => void;
  hideFilters?: boolean;
  emptyMessage?: string;
  isMiniMode?: boolean;
  /** Where a case lives, shown when the list holds sub-cases. */
  locate?: (note: Note) => ReactNode;
}

export default function NoteList({ notes, activeNoteId, onSelectNote, onCreateNote, searchQuery, onSearchQueryChange, statusFilter, onStatusFilterChange, hideFilters = false, emptyMessage, isMiniMode, locate }: NoteListProps) {
  const { t } = useTranslation();
  const handleAddNote = () => {
    // Delegated to the application (Ctrl+N): creation in the active project, or in the Inbox without a project.
    // No local guard requires a project here, otherwise capture would stay impossible.
    onCreateNote();
  };

  

  

  return (
    // In mini mode, the list takes the full width: a fixed 320 px width left an
    // empty band on the right in a 400 px window. `pr-14` reserves room for the pin.
    <div className={`workspace-note-list ${isMiniMode ? 'workspace-note-list--mini w-full' : 'w-80 max-[900px]:w-64'} bg-card flex flex-col border-r border-border h-full overflow-hidden shrink-0`}>
      {!hideFilters && <div className={`p-4 flex flex-col gap-3 ${isMiniMode ? 'pr-14' : ''}`}>
        <div className="workspace-list-toolbar">
        <SearchField id="search-input" label={t('notes.list.searchLabel')} className="workspace-list-search" type="text" placeholder={t('notes.list.searchPlaceholder')} value={searchQuery} onChange={(e) => onSearchQueryChange(e.target.value)} />

        {!isMiniMode && <Hint label={t('notes.list.newNote')} shortcut="Control+N"><Button onClick={handleAddNote} aria-keyshortcuts="Control+N"><Plus size={19} aria-hidden="true" />{t('notes.list.newNote')}</Button></Hint>}
        </div>

        {onStatusFilterChange && (
          <SegmentedControl className="workspace-list-filters max-w-full overflow-x-auto scrollbar-none" size={isMiniMode ? 'sm' : 'default'} label={t('notes.list.filterLabel')} value={statusFilter || 'all'} onValueChange={value => onStatusFilterChange(value === 'all' ? '' : value)} items={['all', ...ORDERED_STATUSES].map(s => ({ value: s, label: s === 'all' ? t('notes.list.filterAll') : statusLabel(s) }))} />
        )}
      </div>}
      
      <div className="workspace-list-items flex-1 overflow-y-auto">
        {notes.length === 0 ? (
          !emptyMessage && !searchQuery && !statusFilter
            // No note in the scope (first launch, new project): same invitation as the cards.
            ? <EmptyState size="sm" icon={<NoteIcon />} title={t('notes.list.emptyTitle')} description={t('notes.list.emptyDescription')} action={!isMiniMode && <Button type="button" variant="secondary" onClick={handleAddNote}><Plus size={17} aria-hidden="true" />{t('notes.list.createNote')}</Button>} />
            // Search or filter without results: a way out, as in cards ("Clear filters").
            : <EmptyState size="sm" description={emptyMessage ?? (searchQuery && !hideFilters ? t('notes.list.noResult') : t('notes.list.noneFound'))} action={!emptyMessage && !hideFilters && (searchQuery || statusFilter) && <Button type="button" variant="secondary" size="sm" onClick={() => { onSearchQueryChange(''); onStatusFilterChange?.(''); }}>{t('notes.list.clearFilters')}</Button>} />
        ) : (
          <List className="workspace-list" label={t('notes.list.listLabel')}>{notes.map(n => {
            // Count tags **after decoding**: `n.tags.length` measures the JSON, and a
            // single-character tag (`["a"]`, 5 characters) was then never displayed.
            const tags = readTags(n.tags);
            const preview = notePreview(n);
            return (
              <ListRow
                key={n.id}
                size={isMiniMode ? 'md' : 'lg'}
                className="workspace-list-row"
                onClick={() => onSelectNote(n.id)}
                actionLabel={t('notes.list.openNote', { title: n.title || t('common.untitled') })}
                actionTitle={formatDateTime(n.updated_at)}
                active={activeNoteId === n.id}
                titleAs="h3"
                overline={<><StatusBadge status={n.status} size="sm" />{n.priority && <PriorityBadge priority={n.priority} status={n.status} />}<TypeBadge type={n.note_type} /></>}
                title={n.title}
                description={preview || undefined}
                meta={<>{locate?.(n)}<span>{formatChanged(n.updated_at)}</span>{n.due_at && <DueBadge due={n.due_at} />}</>}
              >
                {tags.length > 0 && (
                  <span className="workspace-list-row__tags">
                    {tags.map((tag) => (
                      <TagBadge key={tag} tag={tag} label={t('notes.list.searchTag', { tag })} onClick={() => onSearchQueryChange("#" + tag)} />
                    ))}
                  </span>
                )}
              </ListRow>
            );
          })}</List>
        )}
      </div>

      {/* The cases page has its own "New case" in the view bar. */}
      {isMiniMode && <div className="p-4 border-t border-border">
        <Button
          onClick={handleAddNote}
          title={t('notes.list.createTitle')}
          className="w-full"
        >
          <Plus size={17} aria-hidden="true" />
          {t('notes.list.newNote')}
        </Button>
      </div>}
    </div>
  );
}

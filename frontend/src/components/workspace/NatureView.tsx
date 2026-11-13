import { useState, type ElementType } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowRight, File, Image, LinkSimple } from '@phosphor-icons/react';
import type { Note, Project } from '../../types';
import { useMeaningHits } from '../../hooks/useMeaning';
import { useSearchIds } from '../../hooks/useSearchIds';
import { useSupersededIds } from '../../hooks/useTidy';
import { caseMatches } from '../../utils/caseBoard';
import { searchRank } from '../../utils/searchRank';
import { inShortcut, shortcutGroups, shortcutNature, type NatureShortcut } from '../../utils/natureShortcuts';
import { TYPE_ICONS } from '../../utils/notePresentation';
import { statusLabel } from '../../utils/statuses';
import { DueBadge, StatusBadge } from '../ui/Badge';
import { EmptyState } from '../ui/EmptyState';
import { List, ListRow } from '../ui/List';
import { SearchField } from '../ui/SearchField';
import { SegmentedControl } from '../ui/SegmentedControl';
import SketchThumbnail from '../sketch/SketchThumbnail';
import { CaseLocation } from './CaseLocation';
import { NewCaseButton, PageHeader } from './PageHeader';
import { SHORTCUT_ICONS } from './shortcutIcons';
import './natureView.css';

const KIND_ICONS: Partial<Record<NonNullable<Note['content_kind']>, ElementType>> = { link: LinkSimple, file: File, image: Image };

const byModification = (a: Note, b: Note) => b.updated_at.localeCompare(a.updated_at);

/**
 * Every case of one nature across the space (a sidebar shortcut). Each row says where the case
 * lives and opens it there: the view gathers, it never moves a case.
 */
export function NatureView({ shortcut, notes, projects, onNote, onCreate }: {
  shortcut: NatureShortcut;
  /** Visible cases of the space, sub-cases included. */
  notes: Note[];
  projects: Project[];
  onNote: (note: Note) => void;
  /** Creates a case of the shortcut's nature (not offered for documents and drawings). */
  onCreate?: () => void;
}) {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<{ shortcut: NatureShortcut; group: string; query: string }>({ shortcut, group: 'all', query: '' });
  // A filter belongs to its shortcut: changing shortcut starts again from "All", without a search.
  const group = filter.shortcut === shortcut ? filter.group : 'all';
  const query = filter.shortcut === shortcut ? filter.query : '';
  const live = notes.filter(note => !note.deleted_at && !note.archived_at);
  const byId = new Map(notes.map(note => [note.id, note]));
  const projectById = new Map(projects.map(project => [project.id, project]));
  const cases = live.filter(note => inShortcut(shortcut, note)).sort(byModification);
  const groups = shortcutGroups(shortcut);
  const inGroup = group === 'all' ? cases : cases.filter(groups.find(item => item.id === group)?.match ?? (() => true));
  // The database's answer once it arrives, best match first (it also finds cases holding only some
  // of the words), reordered with search by meaning as a second opinion (utils/searchRank).
  const searchIds = useSearchIds(query);
  const meaningHits = useMeaningHits(query);
  const supersededIds = useSupersededIds(notes);
  const rank = searchIds ? searchRank(searchIds, meaningHits, supersededIds) : null;
  const shown = !query.trim() ? inGroup
    : rank ? inGroup.filter(note => rank.has(note.id)).sort((a, b) => (rank.get(a.id) ?? 0) - (rank.get(b.id) ?? 0))
    : inGroup.filter(note => caseMatches(note, query, null));
  const label = t(`workspace.shortcuts.${shortcut}`);
  const Icon = SHORTCUT_ICONS[shortcut];
  const title = (note: Note) => note.title || t('common.untitled');

  return (
    <main className="workspace-page nature-view" aria-labelledby="nature-view-heading">
      <PageHeader eyebrow={t('workspace.shortcuts.eyebrow')} title={label} titleId="nature-view-heading" icon={<Icon size={24} aria-hidden="true" />} count={cases.length}
        description={t('workspace.shortcuts.hint')} action={onCreate && shortcutNature(shortcut) && <NewCaseButton onClick={onCreate} />} />

      {cases.length > 0 && <div className="nature-view__filters">
        <SearchField id="nature-view-search" label={t('workspace.shortcuts.searchLabel', { label })} placeholder={t('workspace.shortcuts.searchPlaceholder', { label: label.toLocaleLowerCase() })} className="nature-view__search" type="search"
          value={query} onChange={event => setFilter({ shortcut, group, query: event.target.value })}
          onKeyDown={event => { if (event.key === 'Escape' && query) { event.preventDefault(); setFilter({ shortcut, group, query: '' }); } }} />
        {groups.length > 0 && <SegmentedControl className="nature-view__filter" label={t('workspace.shortcuts.filter', { label })} value={group} onValueChange={value => setFilter({ shortcut, group: value, query })}
        items={[{ value: 'all', label: t('workspace.shortcuts.groups.all'), count: cases.length }, ...groups.map(item => ({ value: item.id, label: item.status ? statusLabel(item.status) : t(`workspace.shortcuts.groups.${item.label}`), count: cases.filter(item.match).length }))]} />}
      </div>}

      {shown.length === 0
        ? <EmptyState icon={<Icon />} title={t('workspace.shortcuts.emptyTitle')} description={t(cases.length ? 'workspace.shortcuts.emptyFiltered' : 'workspace.shortcuts.emptyDesc')} />
        : shortcut === 'drawings'
          ? <ul className="nature-view__drawings">{shown.map(note => <li key={note.id}>
              <button type="button" className="nature-view__drawing" onClick={() => onNote(note)} aria-label={t('workspace.shortcuts.open', { title: title(note) })}>
                <span className="nature-view__picture"><SketchThumbnail content={note.content} label={t('workspace.shortcuts.drawingPicture', { title: title(note) })} /></span>
                <span className="nature-view__drawing-text"><strong>{title(note)}</strong><CaseLocation note={note} byId={byId} projectById={projectById} /></span>
              </button>
            </li>)}</ul>
          : <List variant="divided" className="nature-view__list">{shown.map(note => {
              const RowIcon = (shortcut === 'documents' && note.content_kind && KIND_ICONS[note.content_kind]) || TYPE_ICONS[note.note_type] || Icon;
              return <ListRow key={note.id} onClick={() => onNote(note)} actionLabel={t('workspace.shortcuts.open', { title: title(note) })}
                leading={<span className="ui-icon-tile" aria-hidden="true"><RowIcon size={18} /></span>}
                overline={<CaseLocation note={note} byId={byId} projectById={projectById} />}
                title={title(note)}
                meta={<><StatusBadge status={note.status} />{note.due_at && <DueBadge due={note.due_at} />}</>}
                trailing={<ArrowRight size={16} aria-hidden="true" />} />;
            })}</List>}
    </main>
  );
}

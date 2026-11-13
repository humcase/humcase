import { CalendarDots, X } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { NOTE_TYPES, type NoteStatus, type NoteType } from '../../types';
import { ORDERED_STATUSES, statusLabel } from '../../utils/statuses';
import { TYPE_ICONS, TYPE_LABELS } from '../../utils/notePresentation';
import { Count } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Chip, ChipGroup } from '../ui/Chip';
import { Hint } from '../ui/Hint';
import { SearchField } from '../ui/SearchField';
import { SegmentedControl } from '../ui/SegmentedControl';

export interface CaseFilterBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  /** Says where the search looks when it is not the whole space. */
  searchPlaceholder?: string;
  /** Empty: every status. */
  status: NoteStatus | '';
  onStatusChange: (status: NoteStatus | '') => void;
  /** Cases of the view in each status, before the status and the search apply. */
  statusCounts: Record<NoteStatus | 'all', number>;
  /** Empty: every nature. */
  nature: NoteType | '';
  onNatureChange: (nature: NoteType | '') => void;
  natureCounts: ReadonlyMap<NoteType, number>;
  /** Whether sub-cases are listed beside the top-level cases; null when the view has no such choice. */
  subCases: boolean | null;
  /** Sub-cases of the scope the box adds, so the person knows what ticking it changes. */
  subCaseCount: number;
  onSubCasesChange: (subCases: boolean) => void;
  /** "Today" narrows the view to the cases to follow up. */
  today: boolean;
  onLeaveToday: () => void;
  onClear: () => void;
}

/**
 * The filters of the cases page, the same above the canvas, the cards, the list and the board, so a
 * filter chosen in one view is visible in the others. Every active filter can be seen and cleared.
 */
export function CaseFilterBar({
  query, onQueryChange, searchPlaceholder, status, onStatusChange, statusCounts, nature, onNatureChange, natureCounts,
  subCases, subCaseCount, onSubCasesChange, today, onLeaveToday, onClear,
}: CaseFilterBarProps) {
  const { t } = useTranslation();
  const active = Boolean(query.trim() || status || nature || today);
  const natures = NOTE_TYPES.filter(type => natureCounts.has(type) || type === nature);
  return (
    <div className="case-filter-bar" role="search" aria-label={t('workspace.filters.label')}>
      <SearchField id="search-input" label={t('workspace.filters.searchLabel')} className="case-filter-bar__search" type="search"
        placeholder={searchPlaceholder ?? t('workspace.filters.searchPlaceholder')} value={query} onChange={event => onQueryChange(event.target.value)}
        onKeyDown={event => { if (event.key === 'Escape' && query) { event.preventDefault(); onQueryChange(''); } }} />
      <SegmentedControl className="case-filter-bar__status" label={t('workspace.filters.status')} value={status || 'all'}
        onValueChange={value => onStatusChange(value === 'all' ? '' : value)}
        items={(['all', ...ORDERED_STATUSES] as const).map(value => ({ value, label: value === 'all' ? t('workspace.filters.allStatuses') : statusLabel(value), count: statusCounts[value] }))} />
      {subCases !== null && (
        // A box, not a button: ticked or not, its state reads at a glance.
        // Ticked and locked while a status or a nature includes them; the person's own choice
        // comes back when that filter is cleared.
        <Hint label={t('workspace.filters.subCasesHint')}>
          <label className="case-filter-bar__check">
            <input type="checkbox" checked={subCases} onChange={event => onSubCasesChange(event.target.checked)} />
            {t('workspace.filters.subCases')}<Count value={subCaseCount} />
          </label>
        </Hint>
      )}
      {today && (
        <Button type="button" variant="secondary" size="sm" title={t('workspace.filters.leaveToday')} onClick={onLeaveToday}>
          <CalendarDots size={14} aria-hidden="true" />{t('workspace.filters.today')}<X size={13} aria-hidden="true" />
        </Button>
      )}
      {active && <Button type="button" variant="ghost" size="sm" onClick={onClear}><X size={14} aria-hidden="true" />{t('workspace.filters.clear')}</Button>}
      {/* Natures in one click, like on the canvas before: a second click on the chosen one clears it. */}
      {natures.length > 0 && (
        <ChipGroup className="case-filter-bar__natures" label={t('workspace.filters.nature')}>
          <Chip selected={!nature} onClick={() => onNatureChange('')}>{t('workspace.filters.allNatures')}</Chip>
          {natures.map(type => {
            const Icon = TYPE_ICONS[type] ?? TYPE_ICONS.note;
            return (
              <Chip key={type} data-type={type} selected={nature === type} icon={<Icon aria-hidden="true" />} count={natureCounts.get(type) ?? 0} onClick={() => onNatureChange(nature === type ? '' : type)}>
                {TYPE_LABELS[type]}
              </Chip>
            );
          })}
        </ChipGroup>
      )}
    </div>
  );
}

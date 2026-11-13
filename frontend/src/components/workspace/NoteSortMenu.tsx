import { CaretDown as ChevronDown } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import type { NoteSort } from '../../utils/noteSort';
import { Button } from '../ui/Button';
import { Menu, MenuRadioGroup } from '../ui/Menu';
import './note-sort-menu.css';

/** The order of the cases; "Relevance" is offered while a search is typed. */
export default function NoteSortMenu({ value, onChange, searching = false }: { value: NoteSort; onChange: (value: NoteSort) => void; searching?: boolean }) {
  const { t } = useTranslation();
  const options: { value: NoteSort; label: string }[] = [
    ...(searching ? [{ value: 'relevance' as const, label: t('workspace.sort.relevance') }] : []),
    { value: 'attention', label: t('workspace.sort.attention') },
    { value: 'recent', label: t('workspace.sort.recent') },
    { value: 'priority', label: t('workspace.sort.priority') },
    { value: 'title', label: t('workspace.sort.title') },
  ];
  const selected = options.find(option => option.value === value)?.label ?? options[0].label;
  return <div className="workspace-sort">
    <span className="workspace-sort__caption">{t('workspace.sort.caption')}</span>
    <Menu align="end" label={t('workspace.sort.menuLabel')} trigger={
      <Button variant="field" size="sm" className="workspace-sort__trigger" aria-label={t('workspace.sort.triggerLabel', { selected })}>
        <span>{selected}</span><ChevronDown size={15} aria-hidden="true" />
      </Button>
    }>
      <MenuRadioGroup label={t('workspace.sort.menuLabel')} value={value} onValueChange={onChange} options={options} />
    </Menu>
  </div>;
}

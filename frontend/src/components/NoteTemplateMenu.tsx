import { useTranslation } from 'react-i18next';
import { CaretDown, Bug, ListChecks, Scales, FileText } from '@phosphor-icons/react';
import { Button } from './ui/Button';
import { Menu, MenuItem } from './ui/Menu';

// Note template menu, on the kit menu (Radix DropdownMenu).
export function NoteTemplateMenu({ onSelect }: { onSelect: (value: string) => void }) {
  const { t } = useTranslation();
  return <Menu label={t('editor.templates.menuLabel')} className="note-template-menu" trigger={
    <Button size="sm" variant="outline" aria-label={t('editor.templates.button')}><FileText size={16} /><span>{t('editor.templates.buttonText')}</span><CaretDown size={13} /></Button>
  }>
    {[
      { id: 'bug', name: t('editor.templates.bug.name'), detail: t('editor.templates.bug.detail'), Icon: Bug },
      { id: 'pr', name: t('editor.templates.pr.name'), detail: t('editor.templates.pr.detail'), Icon: ListChecks },
      { id: 'adr', name: t('editor.templates.adr.name'), detail: t('editor.templates.adr.detail'), Icon: Scales },
    ].map(({ id, name, detail, Icon }) => <MenuItem key={id} icon={<Icon size={19} />} detail={detail} onSelect={() => onSelect(id)}>{name}</MenuItem>)}
  </Menu>;
}

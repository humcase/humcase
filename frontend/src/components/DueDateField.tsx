// CalendarDropdown adapted directly from Cnippet Calendar 5 (21st id 25124).
// https://21st.dev/@cnippet-dev/components/v-calendar-5
import { useState, type ChangeEvent, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { DayPicker, type DropdownProps } from 'react-day-picker';
import { de, enGB, enUS, es, fr, it, ptBR } from 'react-day-picker/locale';
import { Popover, Select } from 'radix-ui';
import { CalendarDots, CaretDown, CaretLeft, CaretRight, Check } from '@phosphor-icons/react';
import 'react-day-picker/style.css';
import { Button } from './ui/Button';
import { useToday } from '../hooks/useToday';
import { currentDateLocale } from '../i18n';
import { dayPattern, fullDay, parseDay } from '../utils/dueDates';
import './due-date-field.css';

function CalendarDropdown(props: DropdownProps) {
  const { options, value, onChange, 'aria-label': ariaLabel, disabled } = props;
  const handleValueChange = (newValue: string) => {
    if (onChange && newValue) onChange({ target: { value: newValue } } as ChangeEvent<HTMLSelectElement>);
  };
  const items = options?.map(option => ({ disabled: option.disabled, label: option.label, value: option.value.toString() })) ?? [];
  return <Select.Root disabled={disabled} value={value?.toString()} onValueChange={handleValueChange}>
    <Select.Trigger aria-label={ariaLabel} className="due-calendar-select"><Select.Value /><Select.Icon><CaretDown size={13} /></Select.Icon></Select.Trigger>
    <Select.Content position="popper" className="due-calendar-options"><Select.Viewport>{items.map(item => <Select.Item className="due-calendar-option" key={item.value} value={item.value} disabled={item.disabled}><Select.ItemText>{item.label}</Select.ItemText><Select.ItemIndicator><Check size={14} /></Select.ItemIndicator></Select.Item>)}</Select.Viewport></Select.Content>
  </Select.Root>;
}
function fromLocalDay(value: string) { if (!value) return undefined; const [y,m,d] = value.split('-').map(Number); const date = new Date(y,m-1,d); return Number.isNaN(date.getTime()) ? undefined : date; }
function toLocalDay(date: Date) { return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`; }
export function DueDateField({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const { t } = useTranslation();
  // The calendar follows the dates shown (months, days, start of week): see `dateLocale`.
  const dates = currentDateLocale();
  const locale = ({ fr, es, de, it, pt: ptBR } as Record<string, typeof enGB>)[dates.split('-')[0]] ?? (dates === 'en-US' ? enUS : enGB);
  const [open,setOpen] = useState(false);
  // What is being typed; `null` shows the saved day. Not the native date field: Windows draws it in its own language.
  const [draft,setDraft] = useState<string | null>(null);
  const selected = fromLocalDay(value);
  const year = useToday().getFullYear();
  const commit = () => {
    if (draft === null) return;
    const day = draft.trim() ? parseDay(draft) : '';
    setDraft(null);
    // A text that is not a day keeps the saved one.
    if (day !== null && day !== value) onChange(day);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') { event.preventDefault(); commit(); }
    if (event.key === 'Escape' && draft !== null) { event.preventDefault(); event.stopPropagation(); setDraft(null); }
  };
  return <div className="due-date-field">
    <input type="text" inputMode="numeric" autoComplete="off" spellCheck={false} className="ui-field" aria-label={t('editor.dueDate.label')}
      placeholder={dayPattern({ day: t('editor.dueDate.dayLetter'), month: t('editor.dueDate.monthLetter'), year: t('editor.dueDate.yearLetter') })}
      value={draft ?? (value ? fullDay(value) : '')} onChange={event => setDraft(event.target.value)} onBlur={commit} onKeyDown={onKeyDown} />
    <Popover.Root open={open} onOpenChange={setOpen}><Popover.Trigger asChild><Button type="button" variant="ghost" size="icon" aria-label={t('editor.dueDate.choose')}><CalendarDots size={18} aria-hidden="true" /></Button></Popover.Trigger>
      <Popover.Content className="due-calendar-popover" sideOffset={8} align="end" aria-label={t('editor.dueDate.calendar')}>
        <DayPicker key={value || 'empty'} mode="single" selected={selected} defaultMonth={selected} locale={locale} captionLayout="dropdown" startMonth={new Date(Math.min(year-10,selected?.getFullYear() ?? year),0)} endMonth={new Date(Math.max(year+20,selected?.getFullYear() ?? year),11)} components={{ Dropdown: CalendarDropdown, Chevron: ({orientation}) => orientation === 'left' ? <CaretLeft size={18} /> : <CaretRight size={18} /> }} onSelect={date => { if(date) { onChange(toLocalDay(date)); setOpen(false); } }} />
        <div className="due-calendar-footer"><Button type="button" variant="ghost" size="sm" onClick={() => { onChange(toLocalDay(new Date())); setOpen(false); }}>{t('editor.dueDate.today')}</Button><Popover.Close asChild><Button type="button" variant="ghost" size="sm">{t('common.close')}</Button></Popover.Close></div>
      </Popover.Content>
    </Popover.Root>
  </div>;
}

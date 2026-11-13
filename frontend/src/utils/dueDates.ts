import i18n, { currentDateLocale } from '../i18n';
import type { Note } from '../types';
import { todayLocalIso } from './datetime';

/**
 * Due dates. A due date is a **day** (`YYYY-MM-DD`), not an instant: a note is scheduled
 * "for Friday", not "at 14:37 UTC". All comparisons are therefore made between local
 * days, which avoids the time-zone shifts the timestamp had before batch B8.
 */
export type DueState = 'overdue' | 'today' | 'thisWeek' | 'upcoming';

function inNDays(now: Date, days: number): string {
  return todayLocalIso(new Date(now.getTime() + days * 24 * 60 * 60 * 1000));
}

export function dueState(due?: string | null, now: Date = new Date()): DueState | null {
  if (!due) return null;
  const day = todayLocalIso(now);
  if (due < day) return 'overdue';
  if (due === day) return 'today';
  return due <= inNDays(now, 7) ? 'thisWeek' : 'upcoming';
}

const LABEL_KEYS = {
  overdue: 'notes.due.overdue',
  today: 'notes.due.today',
  thisWeek: 'notes.due.thisWeek',
  upcoming: 'notes.due.upcoming',
} as const satisfies Record<DueState, string>;

export function dueLabel(due?: string | null, now?: Date): string {
  const state = dueState(due, now);
  return state ? i18n.t(LABEL_KEYS[state]) : '';
}

/** A `YYYY-MM-DD` day at local noon, or `null` when the value is not one. */
function localDay(due: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(due);
  return match ? new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]), 12) : null;
}

/** `2026-09-25` → `25/09`, or `09/25` on a US computer (compact display on cards); see `dateLocale`. */
export function shortDay(due?: string | null): string {
  const day = due ? localDay(due.slice(0, 10)) : null;
  return day ? day.toLocaleDateString(currentDateLocale(), { day: '2-digit', month: '2-digit' }) : '';
}

/** `2026-09-25` → `25/09/2026`, or `09/25/2026` on a US computer; an unexpected value is returned as is. */
export function fullDay(due: string): string {
  const day = localDay(due);
  return day ? day.toLocaleDateString(currentDateLocale(), { day: '2-digit', month: '2-digit', year: 'numeric' }) : due.split('-').reverse().join('/');
}

type DayPart = 'day' | 'month' | 'year';

/** Order of day, month and year in the dates shown (`dateLocale`), with the separator between them. */
function dayLayout(): { order: DayPart[]; separator: string } {
  const parts = new Intl.DateTimeFormat(currentDateLocale(), { day: '2-digit', month: '2-digit', year: 'numeric' })
    .formatToParts(new Date(2026, 11, 31));
  const order = parts.map(part => part.type).filter((type): type is DayPart => type === 'day' || type === 'month' || type === 'year');
  return { order, separator: parts.find(part => part.type === 'literal')?.value ?? '/' };
}

/** What the empty due date field shows, in the same order as the dates: `dd/mm/yyyy`, `jj/mm/aaaa`, `mm/dd/yyyy`. */
export function dayPattern(letters: Record<DayPart, string>): string {
  const { order, separator } = dayLayout();
  return order.map(part => letters[part].repeat(part === 'year' ? 4 : 2)).join(separator);
}

/**
 * A day typed in the due date field, read in the order of the dates shown (`31/12/2026`, `31.12.26`,
 * `31122026`, or `12/31/2026` on a US computer), or `2026-12-31`. Returns `YYYY-MM-DD`, or `null`
 * when the text is not a day that exists.
 */
export function parseDay(text: string): string | null {
  const trimmed = text.trim();
  let groups = trimmed.split(/\D+/).filter(Boolean);
  let order = dayLayout().order;
  if (groups.length === 1 && groups[0].length === 8) {
    let at = 0;
    groups = order.map(part => { const width = part === 'year' ? 4 : 2; const group = groups[0].slice(at, at + width); at += width; return group; });
  }
  if (groups.length !== 3) return null;
  if (groups[0].length === 4) order = ['year', 'month', 'day'];
  const value = Object.fromEntries(order.map((part, index) => [part, Number(groups[index])])) as Record<DayPart, number>;
  const year = groups[order.indexOf('year')].length <= 2 ? 2000 + value.year : value.year;
  const date = new Date(year, value.month - 1, value.day, 12);
  if (year < 1000 || date.getFullYear() !== year || date.getMonth() !== value.month - 1 || date.getDate() !== value.day) return null;
  return todayLocalIso(date);
}

/** Due date overdue or today. */
export function isUrgent(due?: string | null, now: Date = new Date()): boolean {
  const state = dueState(due, now);
  return state === 'overdue' || state === 'today';
}

/**
 * Notes to handle today: due date overdue or today, or high priority (P0/P1) still
 * open. This is the definition of the "Today" view in the sidebar.
 */
export function notesOfTheDay(notes: Note[], now: Date = new Date()): Note[] {
  return notes.filter(n => {
    if (n.status === 'resolved') return false;
    return isUrgent(n.due_at, now) || n.priority === 'P0' || n.priority === 'P1';
  });
}

/**
 * Cases to follow up, the one definition behind Home's "To follow up" and the Today view: still
 * open, neither archived nor in the trash, and either in progress, due today or overdue, or of high
 * priority (P0/P1). Sub-cases count like any case. Due dates first (earliest first), then the
 * most recently changed.
 */
export function casesToFollow(notes: Note[], now: Date = new Date()): Note[] {
  const live = notes.filter(note => !note.deleted_at && !note.archived_at);
  const urgent = new Set(notesOfTheDay(live, now).map(note => note.id));
  return live
    .filter(note => urgent.has(note.id) || note.status === 'in_progress')
    .sort((a, b) => (a.due_at || '9999').localeCompare(b.due_at || '9999') || b.updated_at.localeCompare(a.updated_at));
}

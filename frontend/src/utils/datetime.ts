import i18n, { currentDateLocale } from '../i18n';

/**
 * Humcase timestamps.
 *
 * The database now stores explicit ISO 8601 **UTC** dates (`2026-09-21T15:38:27.596Z`,
 * see migration v7). Migrated databases may contain the old format produced by
 * `DEFAULT CURRENT_TIMESTAMP`: `YYYY-MM-DD HH:MM:SS`, in UTC but **without a time zone**.
 * `new Date("2026-09-21 14:50:57")` is then interpreted as *local* time by the JS engine
 * (offset shift, +2 h in summer in Paris): this module normalizes before parsing.
 */

/** Converts a timestamp (old or new format) to a local `Date`, or `null` if unreadable. */
export function parseTimestamp(value?: string | null): Date | null {
  if (!value) return null;
  const trimmed = value.trim();
  if (!trimmed) return null;

  // "YYYY-MM-DD HH:MM[:SS[.sss]]" or "YYYY-MM-DDTHH:MM..." without a time zone → explicit UTC.
  const match = /^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?)$/.exec(trimmed);
  const iso = match ? `${match[1]}T${match[2]}Z` : trimmed;

  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? null : date;
}

/** "21 sept. 2026, 17:38" (French locale; local time of the machine). */
export function formatDateTime(value?: string | null): string {
  return parseTimestamp(value)?.toLocaleString(currentDateLocale()) ?? '';
}

/** "21 sept." (French locale), for lists; "13 déc. 2025" for a day of another year. */
export function formatShortDate(value?: string | null, now: Date = new Date()): string {
  const date = parseTimestamp(value);
  if (!date) return '';
  const year = date.getFullYear() === now.getFullYear() ? {} : { year: 'numeric' as const };
  return date.toLocaleDateString(currentDateLocale(), { day: '2-digit', month: 'short', ...year });
}

/** "il y a 5 min", "il y a 3 h", "il y a 2 j" (French locale), then a short date beyond a month. */
export function formatRelative(value?: string | null, now: Date = new Date()): string {
  const date = parseTimestamp(value);
  if (!date) return '';

  const minutes = Math.floor((now.getTime() - date.getTime()) / 60000);
  if (minutes < 1) return i18n.t('notes.time.justNow');
  if (minutes < 60) return i18n.t('notes.time.minutesAgo', { n: minutes });

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return i18n.t('notes.time.hoursAgo', { n: hours });

  const days = Math.floor(hours / 24);
  if (days < 31) return i18n.t('notes.time.daysAgo', { n: days });

  return formatShortDate(value, now);
}

/** "il y a 3 j · 06 oct.", or the date alone when it is already the relative form (beyond a month). */
export function formatChanged(value?: string | null, now: Date = new Date()): string {
  const relative = formatRelative(value, now);
  const day = formatShortDate(value, now);
  return relative === day ? day : `${relative} · ${day}`;
}

/** Local date of the day in `YYYY-MM-DD` format (standup titles must not be in UTC). */
export function todayLocalIso(now: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/**
 * True if the value is a **real** calendar day `YYYY-MM-DD` (mirror of `dates::est_jour_iso`
 * on the Rust side): `2026-02-31` does not exist and must be rejected, `2028-02-29` (leap year) accepted.
 */
export function isIsoDay(value?: string | null): boolean {
  if (!value || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split('-').map(Number);
  if (month < 1 || month > 12 || day < 1) return false;
  const leapYear = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  const daysInMonth = [31, leapYear ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  return day <= daysInMonth;
}


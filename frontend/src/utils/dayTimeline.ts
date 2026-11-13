import { currentDateLocale } from '../i18n';
import type { Note, NoteRevision } from '../types';
import { parseTimestamp } from './datetime';

/**
 * Timeline of the day: recorded facts only.
 * - `created` / `modified` come from `created_at` / `updated_at`: unattributed (human or agent),
 *   and `updated_at` only keeps the **last** modification of the note.
 * - `agent` comes from `note_revisions`: attributed agent write (`update_note` via MCP).
 * No working time is invented: a block groups nearby instants, not a session.
 */
export type DayEventKind = 'created' | 'modified' | 'agent';
export type DayLane = 'notes' | 'agents';

export interface DayEvent {
  at: Date;
  noteId: string;
  kind: DayEventKind;
}

export interface DayCluster {
  id: string;
  lane: DayLane;
  start: Date;
  end: Date;
  events: DayEvent[];
}

export interface DayTimelineModel {
  dayStart: Date;
  axisStart: Date;
  axisEnd: Date;
  hours: number[];
  clusters: DayCluster[];
  counts: { notesTouched: number; agentWrites: number; last: Date | null };
}

/** Two instants less than 30 min apart belong to the same block. */
export const CLUSTER_GAP_MS = 30 * 60_000;
/** A modification coinciding with an agent write (same note, ±5 s) is not counted twice. */
const SAME_WRITE_MS = 5_000;
/** A modification within the minute following creation adds nothing to the timeline. */
const CREATION_ECHO_MS = 60_000;

export function buildDayTimeline(notes: Note[], revisions: NoteRevision[], now: Date): DayTimelineModel {
  const dayStart = new Date(now); dayStart.setHours(0, 0, 0, 0);
  const dayEnd = new Date(dayStart); dayEnd.setDate(dayEnd.getDate() + 1);
  const inDay = (date: Date | null): date is Date => Boolean(date && date >= dayStart && date < dayEnd && date <= now);
  const noteIds = new Set(notes.map(note => note.id));

  const agentEvents: DayEvent[] = [];
  for (const revision of revisions) {
    const at = parseTimestamp(revision.created_at);
    if (!inDay(at) || !noteIds.has(revision.note_id) || revision.source !== 'agent') continue;
    agentEvents.push({ at, noteId: revision.note_id, kind: 'agent' });
  }

  const noteEvents: DayEvent[] = [];
  for (const note of notes) {
    const created = parseTimestamp(note.created_at);
    const updated = parseTimestamp(note.updated_at);
    if (inDay(created)) noteEvents.push({ at: created, noteId: note.id, kind: 'created' });
    if (!inDay(updated)) continue;
    if (created && updated.getTime() - created.getTime() < CREATION_ECHO_MS) continue;
    const sameAsAgent = agentEvents.some(event => event.noteId === note.id && Math.abs(event.at.getTime() - updated.getTime()) <= SAME_WRITE_MS);
    if (!sameAsAgent) noteEvents.push({ at: updated, noteId: note.id, kind: 'modified' });
  }

  const clusters = [...cluster('notes', noteEvents), ...cluster('agents', agentEvents)];
  const all = [...noteEvents, ...agentEvents].sort((a, b) => a.at.getTime() - b.at.getTime());
  const first = all[0]?.at;
  const last = all[all.length - 1]?.at ?? null;

  const hourOf = (date: Date) => date.getHours() + date.getMinutes() / 60;
  const startHour = Math.max(0, Math.min(8, first ? Math.floor(hourOf(first)) : 8));
  const endHour = Math.min(24, Math.max(20, Math.ceil(hourOf(now) + 0.25)));
  const axisStart = new Date(dayStart); axisStart.setHours(startHour);
  const axisEnd = new Date(dayStart); axisEnd.setHours(endHour);
  const step = endHour - startHour > 14 ? 3 : 2;
  const hours: number[] = [];
  for (let hour = startHour; hour <= endHour; hour += step) hours.push(hour);

  return {
    dayStart, axisStart, axisEnd, hours, clusters,
    counts: { notesTouched: new Set(all.map(event => event.noteId)).size, agentWrites: agentEvents.length, last },
  };
}

function cluster(lane: DayLane, events: DayEvent[]): DayCluster[] {
  const sorted = [...events].sort((a, b) => a.at.getTime() - b.at.getTime());
  const out: DayCluster[] = [];
  for (const event of sorted) {
    const current = out[out.length - 1];
    if (current && event.at.getTime() - current.end.getTime() <= CLUSTER_GAP_MS) {
      current.events.push(event);
      current.end = event.at;
    } else {
      out.push({ id: `${lane}-${event.at.getTime()}-${event.noteId}`, lane, start: event.at, end: event.at, events: [event] });
    }
  }
  return out;
}

export const formatClock = (date: Date) => date.toLocaleTimeString(currentDateLocale(), { hour: '2-digit', minute: '2-digit' });

/** True when the date locale counts hours from 1 to 12 (`en-US`): the ruler then reads `8 AM`, like the clock. */
export const uses12HourClock = () =>
  ['h11', 'h12'].includes(new Intl.DateTimeFormat(currentDateLocale(), { hour: 'numeric' }).resolvedOptions().hourCycle ?? '');

/** A full hour of the ruler on a 12-hour clock: `8 AM`. */
export const formatHourMark = (date: Date) => date.toLocaleTimeString(currentDateLocale(), { hour: 'numeric' });

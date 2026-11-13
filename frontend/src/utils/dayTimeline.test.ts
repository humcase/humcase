import { describe, expect, it } from 'vitest';
import type { Note, NoteRevision } from '../types';
import { buildDayTimeline } from './dayTimeline';

const at = (hh: number, mm = 0) => { const date = new Date(2026, 8, 27, hh, mm); return date.toISOString(); };
const now = new Date(2026, 8, 27, 17, 10);

const note = (id: string, created: string, updated: string): Note => ({
  id, project_id: 'p', title: id, content: '', note_type: 'note', status: 'open', resolution_comment: null,
  raw_content: null, priority: null, git_branch: null, tags: '[]', created_at: created, updated_at: updated,
  due_at: null, revision: 1,
} as Note);
const revision = (noteId: string, created: string, source = 'agent'): NoteRevision => ({
  id: `${noteId}-${created}`, note_id: noteId, revision: 0, source, fields: '{}', created_at: created,
});

describe('Day timeline — recorded traces only', () => {
  it('places an agent write on the Agents lane without counting it again as a modification', () => {
    const model = buildDayTimeline([note('a', at(9), at(15, 30))], [revision('a', at(15, 30))], now);
    const lanes = model.clusters.map(cluster => [cluster.lane, cluster.events.map(event => event.kind)]);
    expect(lanes).toEqual([['notes', ['created']], ['agents', ['agent']]]);
    expect(model.counts).toMatchObject({ notesTouched: 1, agentWrites: 1 });
  });

  it('ignores the modification that immediately follows creation', () => {
    const model = buildDayTimeline([note('a', at(10), new Date(2026, 8, 27, 10, 0, 30).toISOString())], [], now);
    expect(model.clusters.flatMap(cluster => cluster.events.map(event => event.kind))).toEqual(['created']);
  });

  it('excludes the previous day, the future, non-agent sources and notes out of scope', () => {
    const yesterday = new Date(2026, 8, 26, 16).toISOString();
    const model = buildDayTimeline(
      [note('a', yesterday, yesterday), note('b', yesterday, at(18))],
      [revision('a', yesterday), revision('zzz', at(11)), revision('a', at(12), 'human')],
      now,
    );
    expect(model.clusters).toEqual([]);
    expect(model.counts.notesTouched).toBe(0);
  });

  it('groups traces less than 30 minutes apart, separates the others', () => {
    const notes = [note('a', at(9), at(9)), note('b', at(9, 20), at(9, 20)), note('c', at(11), at(11))];
    const model = buildDayTimeline(notes, [], now);
    expect(model.clusters.map(cluster => cluster.events.length)).toEqual([2, 1]);
    expect(model.clusters[0].end.getTime() - model.clusters[0].start.getTime()).toBe(20 * 60_000);
  });

  it('extends the axis to include a morning trace and the current time', () => {
    const late = new Date(2026, 8, 27, 21, 40);
    const model = buildDayTimeline([note('a', at(6, 15), at(6, 15))], [], late);
    expect(model.axisStart.getHours()).toBe(6);
    expect(model.axisEnd.getHours()).toBe(22);
  });
});

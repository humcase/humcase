// Presentation adapted from Agent Trace / n1m4mz, 21st id 26968 (MIT license).
// https://21st.dev/@n1m4mz/components/agent-trace
// Kept: label column, graduated ruler, solid bars, diamond time marker, shadcn
// tokens. Removed: the animated replay and the playback cursor: the timeline shows the day
// as actually recorded; the marker is the current time, never a simulated progress.
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowRight, ArrowsInLineVertical, ArrowsOutLineVertical } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { PixelIcon, type PixelGlyph } from '../ui/PixelIcon';
import { Notice } from '../ui/Toast';
import type { Note, NoteRevision } from '../../types';
import { cn } from '@/lib/utils';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/shadcn/popover';
import { buildDayTimeline, formatClock, formatHourMark, uses12HourClock, type DayCluster, type DayEventKind, type DayLane } from '../../utils/dayTimeline';

/** Beyond this number of periods, the timeline opens by itself in two lines (Notes, Agents). */
const COMPACT_MAX_CLUSTERS = 3;

export function DayTimeline({ notes, revisions, revisionsFailed = false, onNote }: {
  notes: Note[];
  revisions: NoteRevision[];
  /** Reading of agent writes failed: the Agents track is incomplete, not empty. */
  revisionsFailed?: boolean;
  onNote: (note: Note) => void;
}) {
  const { t } = useTranslation();
  const rootRef = useRef<HTMLElement>(null);
  const [now, setNow] = useState(() => new Date());
  const [portal, setPortal] = useState<HTMLElement | null>(null);
  // null: follow the day's activity; otherwise, an explicit choice made in this session.
  const [detailChoice, setDetailChoice] = useState<boolean | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 60_000);
    return () => window.clearInterval(timer);
  }, []);
  // Theme tokens are declared on `.workspace-app`: the popover is rendered there.
  useEffect(() => { setPortal(rootRef.current?.closest<HTMLElement>('.workspace-app') ?? null); }, []);

  const model = useMemo(() => buildDayTimeline(notes, revisions, now), [notes, revisions, now]);
  const noteById = useMemo(() => new Map(notes.map(note => [note.id, note])), [notes]);
  const span = model.axisEnd.getTime() - model.axisStart.getTime();
  const pos = (date: Date) => Math.min(100, Math.max(0, ((date.getTime() - model.axisStart.getTime()) / span) * 100));
  const twelveHour = uses12HourClock();
  const nowPos = pos(now);
  // Calm day: a single line where the color distinguishes the origin (gray: notes, orange: agents).
  const detailed = detailChoice ?? model.clusters.length > COMPACT_MAX_CLUSTERS;
  const lanes: { id: DayLane; label: string; hint: string; glyph: PixelGlyph }[] = [
    { id: 'notes', label: t('workspace.timeline.laneNotes'), hint: t('workspace.timeline.laneNotesHint'), glyph: 'note' },
    { id: 'agents', label: t('workspace.timeline.laneAgents'), hint: t('workspace.timeline.laneAgentsHint'), glyph: 'agent' },
  ];
  const rows = detailed
    ? lanes.map(lane => ({ ...lane, clusters: model.clusters.filter(cluster => cluster.lane === lane.id) }))
    : [{ id: 'all', label: t('workspace.timeline.allLane'), hint: t('workspace.timeline.allLaneHint'), glyph: 'note' as PixelGlyph, clusters: model.clusters }];

  const summary = model.counts.notesTouched === 0
    ? (revisionsFailed ? t('workspace.timeline.summaryNoneFailed') : t('workspace.timeline.summaryNone'))
    : [
        t('workspace.timeline.notesTouched', { count: model.counts.notesTouched }),
        model.counts.agentWrites > 0 && t('workspace.timeline.agentWrites', { count: model.counts.agentWrites }),
        model.counts.last && t('workspace.timeline.lastTrace', { time: formatClock(model.counts.last) }),
      ].filter(Boolean).join(' · ');

  // Same box for the ruler, the markers and the blocks (Agent Trace principle).
  const track = 'absolute inset-y-0 left-[var(--gutter)] right-4';

  return (
    <section ref={rootRef} aria-labelledby="day-timeline-heading" className="day-timeline @container/day mb-8 bg-card text-card-foreground overflow-hidden rounded-[var(--workspace-radius-card)] border border-[var(--workspace-card-border)] shadow-[var(--workspace-shadow-card)]">
      <header className="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 pt-3.5 pb-3">
        <span aria-hidden="true" className="objet-led" />
        <h2 id="day-timeline-heading" className="objet-label text-foreground">{t('workspace.timeline.title')}</h2>
        {model.counts.notesTouched === 0
          ? <p className="text-muted-foreground text-[13px]">{summary}</p>
          : <>
            <p className="sr-only">{summary}</p>
            {/* Object counters: real numbers of the day, nothing estimated. */}
            <dl aria-hidden="true" className="objet-counters ml-auto">
              <div><dt>{t('workspace.timeline.counterNotes')}</dt><dd>{String(model.counts.notesTouched).padStart(2, '0')}</dd></div>
              <div data-accent={model.counts.agentWrites > 0 || undefined}><dt>{t('workspace.timeline.counterAgents')}</dt><dd>{String(model.counts.agentWrites).padStart(2, '0')}</dd></div>
              {model.counts.last && <div><dt>{t('workspace.timeline.counterLast')}</dt><dd>{formatClock(model.counts.last)}</dd></div>}
            </dl>
          </>}
      </header>
      {revisionsFailed && <Notice tone="danger" className="mx-3 mb-3">{t('workspace.timeline.failedNotice')}</Notice>}

      {/* The object's screen: dot pattern, recessed glass. */}
      <div className="day-timeline__screen relative mx-3 [--gutter:5.5rem] @md/day:[--gutter:8.5rem]">
        {/* Time ruler */}
        <div className="day-timeline__ruler relative h-8" aria-hidden="true">
          <div className={track}>
            <span className="bg-primary text-primary-foreground absolute top-1.5 z-10 -translate-x-1/2 whitespace-nowrap rounded-[3px] px-1.5 py-[3px] font-mono text-[11px] font-medium leading-none tabular-nums shadow-sm" style={{ left: `${nowPos}%` }}>{formatClock(now)}</span>
            {model.hours.map((hour, index) => {
              const at = new Date(model.dayStart); at.setHours(hour);
              const edge = index === model.hours.length - 1 && pos(at) > 96 ? '-translate-x-full' : '-translate-x-1/2';
              if (Math.abs(pos(at) - nowPos) < 9) return null; // the current time takes its place
              return <span key={hour} className={cn('text-muted-foreground absolute top-2.5 whitespace-nowrap font-mono text-[11px] leading-none tabular-nums', edge)} style={{ left: `${pos(at)}%` }}>{twelveHour ? formatHourMark(at) : t('workspace.timeline.hourLabel', { hour: String(hour).padStart(2, '0') })}</span>;
            })}
          </div>
        </div>

        <div className="relative py-1">
          <div aria-hidden="true" className={track}>
            {model.hours.map(hour => {
              const at = new Date(model.dayStart); at.setHours(hour);
              return <span key={hour} className="absolute inset-y-0 w-px border-l border-dashed border-border/70" style={{ left: `${pos(at)}%` }} />;
            })}
          </div>

          <ul aria-label={t('workspace.timeline.listLabel')} className="relative">
            {rows.map(row => {
              return <li key={row.id} className={cn('relative flex items-center', detailed ? 'h-12' : 'h-11')}>
                <span className="text-foreground flex w-[var(--gutter)] shrink-0 items-center gap-2.5 pl-4 pr-2" title={row.hint}>
                  {detailed
                    ? <PixelIcon glyph={row.glyph} size={16} className={cn('hidden shrink-0 @md/day:block', row.id === 'agents' ? 'text-primary' : 'text-muted-foreground')} />
                    : <span aria-hidden="true" className="day-timeline__legend hidden @md/day:flex"><i /><i data-agent /></span>}
                  <span className="objet-label truncate">{row.label}</span>
                  <span className="sr-only">. {row.hint}. {t('workspace.timeline.periods', { count: row.clusters.length })}.</span>
                </span>
                <div className={cn(track, 'left-[var(--gutter)]')}>
                  <span aria-hidden="true" className="day-timeline__rail absolute inset-x-0 top-1/2 h-7 -translate-y-1/2 rounded-[5px]" />
                  {row.clusters.map(cluster => <DayBlock key={cluster.id} cluster={cluster} left={pos(cluster.start)} width={pos(cluster.end) - pos(cluster.start)} noteById={noteById} onNote={onNote} container={portal} />)}
                </div>
              </li>;
            })}
          </ul>

          {/* Now: the current time, not a progress. */}
          <div aria-hidden="true" className={cn(track, 'pointer-events-none')}>
            <div className="absolute inset-y-0" style={{ left: `${nowPos}%` }}>
              <span className="bg-primary absolute inset-y-0 w-[1.5px] -translate-x-[0.25px]" />
              <span className="bg-primary absolute -top-1 size-2 -translate-x-[3.5px] rotate-45 rounded-[1px]" />
            </div>
          </div>
        </div>
      </div>

      <footer className="text-muted-foreground flex flex-wrap items-center justify-between gap-2 px-4 pt-2.5 pb-3 text-[12px]">
        <span>{t('workspace.timeline.footer')}</span>
        <span className="sr-only">{t('workspace.timeline.currentTime', { time: formatClock(now) })}</span>
        {model.clusters.length > 0 && <Button type="button" variant="ghost" size="xs" className="day-timeline__detail" onClick={() => setDetailChoice(!detailed)}>
          {detailed ? <ArrowsInLineVertical size={15} aria-hidden="true" /> : <ArrowsOutLineVertical size={15} aria-hidden="true" />}
          {detailed ? t('workspace.timeline.singleLine') : t('workspace.timeline.splitBySource')}
        </Button>}
      </footer>
    </section>
  );
}

function DayBlock({ cluster, left, width, noteById, onNote, container }: {
  cluster: DayCluster; left: number; width: number;
  noteById: Map<string, Note>; onNote: (note: Note) => void; container: HTMLElement | null;
}) {
  const { t } = useTranslation();
  const kindLabel: Record<DayEventKind, string> = {
    created: t('workspace.timeline.kindCreated'),
    modified: t('workspace.timeline.kindModified'),
    agent: t('workspace.timeline.kindAgent'),
  };
  const range = formatClock(cluster.start) === formatClock(cluster.end) ? formatClock(cluster.start) : `${formatClock(cluster.start)} – ${formatClock(cluster.end)}`;
  const noteCount = new Set(cluster.events.map(event => event.noteId)).size;
  const agent = cluster.lane === 'agents';
  const laneLabel = agent ? t('workspace.timeline.laneAgents') : t('workspace.timeline.laneNotes');
  return <Popover>
    <PopoverTrigger asChild>
      <button
        type="button"
        aria-label={t('workspace.timeline.blockLabel', { lane: laneLabel, range, traces: t('workspace.timeline.traceCount', { count: cluster.events.length }), notes: t('workspace.timeline.noteCount', { count: noteCount }) })}
        className="group/block focus-visible:ring-ring/50 absolute top-1/2 flex h-7 min-w-3 -translate-y-1/2 cursor-pointer items-center rounded-md outline-none focus-visible:ring-[3px]"
        style={{ left: `${left}%`, width: `${width}%` }}
      >
        <span className={cn('day-timeline__bar block h-3.5 w-full min-w-3 rounded-[3px] transition-[transform,opacity] duration-150 ease-out group-hover/block:scale-y-125 group-data-[state=open]/block:scale-y-125', agent ? 'bg-primary' : 'bg-foreground/45')} />
      </button>
    </PopoverTrigger>
    <PopoverContent container={container} align="start" collisionPadding={16} className="w-80 max-w-[calc(100vw-32px)] p-0">
      <p className="border-border text-muted-foreground border-b px-3 py-2 font-mono text-[11.5px] uppercase tracking-[0.06em] tabular-nums">{range} · {laneLabel}</p>
      <ul className="max-h-64 overflow-y-auto py-1">
        {cluster.events.map((event, index) => {
          const note = noteById.get(event.noteId);
          if (!note) return null;
          return <li key={`${event.noteId}-${event.kind}-${index}`}>
            <button type="button" onClick={() => onNote(note)} className="hover:bg-accent focus-visible:bg-accent flex w-full items-center gap-3 px-3 py-2 text-left outline-none">
              <span className="text-muted-foreground w-11 shrink-0 font-mono text-[12px] tabular-nums">{formatClock(event.at)}</span>
              <span className="min-w-0 flex-1">
                <span className="text-foreground block truncate text-[13px] font-medium">{note.title || t('common.untitled')}</span>
                <span className="text-muted-foreground block text-[12px]">{kindLabel[event.kind]}</span>
              </span>
              <ArrowRight size={14} aria-hidden="true" className="text-muted-foreground shrink-0" />
            </button>
          </li>;
        })}
      </ul>
    </PopoverContent>
  </Popover>;
}

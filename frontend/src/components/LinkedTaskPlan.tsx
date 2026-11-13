// Presentation adapted from Agent Plan / Isaiah Bjork (21st id 2127) and AI Task List / educalvolpz
// (21st id 23793, MIT license): checkbox that ticks by drawing itself, resolved tasks recessed.
// States come from the persisted notes; "In progress" is a declared status, hence no
// progress animation (AI Task List's animated underline is not reused).
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CaretRight, X } from '@phosphor-icons/react';
import type { NoteStatus } from '../types';
import type { Note } from '../types';
import { fullDay } from '../utils/dueDates';
import { statusLabel } from '../utils/statuses';
import { Button } from './ui/Button';
import './linked-task-plan.css';

/** Status box: empty (to do), half full (in progress, declared), checked (resolved). */
function TaskBox({ status }: { status: NoteStatus }) {
  return <svg className="linked-task-plan__status" viewBox="0 0 14 14" width="16" height="16" aria-hidden="true">
    <rect x="1" y="1" width="12" height="12" rx="3.5" fill="none" stroke="currentColor" strokeWidth="1.4" />
    {status === 'in_progress' && <path d="M 4 10 L 10 4 L 10 10 Z" fill="currentColor" opacity="0.55" />}
    {status === 'resolved' && <path className="linked-task-plan__check" d="M 3.5 7.5 L 6 10 L 10.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" pathLength={1} strokeDasharray="1 1" />}
  </svg>;
}

export function LinkedTaskPlan({ tasks, disabled, onOpen, onUnlink }: {
  tasks: Note[]; disabled: boolean; onOpen: (task: Note) => void; onUnlink: (task: Note) => void;
}) {
  const { t } = useTranslation();
  const [expandedTasks, setExpandedTasks] = useState<Record<string, boolean>>({});
  return <ul className="linked-task-plan">{tasks.map(task => {
    const expanded = Boolean(expandedTasks[task.id]);
    const detail = task.resolution_comment?.trim() || task.content.trim();
    return <li key={task.id} className="linked-task-plan__item" data-status={task.status}>
      <div className="linked-task-plan__row">
        <TaskBox status={task.status} />
        <button type="button" className="linked-task-plan__title" disabled={disabled} onClick={() => onOpen(task)}>{task.title || t('common.untitled')}</button>
        <Button type="button" variant="ghost" size="icon-sm" className="linked-task-plan__toggle" aria-label={t('notes.plan.detailsOf', { title: task.title || t('common.untitled') })} aria-expanded={expanded} aria-controls={`task-detail-${task.id}`} onClick={() => setExpandedTasks(previous => ({ ...previous, [task.id]: !previous[task.id] }))}><CaretRight size={14} /></Button>
        <Button type="button" variant="ghost" size="icon-sm" data-tone="danger" className="note-relations-unlink" disabled={disabled} aria-label={t('notes.plan.unlinkTaskLabel', { title: task.title || t('common.untitled') })} title={t('notes.plan.unlinkTitle')} onClick={() => onUnlink(task)}><X size={14} /></Button>
      </div>
      <div className="linked-task-plan__meta">{statusLabel(task.status)}{task.due_at && <span>{t('notes.plan.dueOn', { date: fullDay(task.due_at) })}</span>}</div>
      {expanded && <div className="linked-task-plan__detail" id={`task-detail-${task.id}`}>
        <span>{task.resolution_comment?.trim() ? t('notes.plan.report') : t('notes.plan.taskContent')}</span>
        <p>{detail || t('notes.plan.noDetail')}</p>
      </div>}
    </li>;
  })}</ul>;
}

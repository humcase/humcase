import type { HTMLAttributes, ReactNode } from 'react';
import { CheckCircle, Circle, CircleDashed, Clock } from '@phosphor-icons/react';
import type { Note, NoteStatus, NoteType } from '../../types';
import { cn } from '../../lib/utils';
import i18n from '../../i18n';
import { statusLabel } from '../../utils/statuses';
import { dueState, shortDay, dueLabel } from '../../utils/dueDates';
import { TYPE_ICONS, TYPE_LABELS } from '../../utils/notePresentation';

export type BadgeTone = 'neutral' | 'danger' | 'warning' | 'success' | 'accent';
export type BadgeVariant = 'soft' | 'outline' | 'plain' | 'screen';

/**
 * Humcase kit — information badge (0.9.67). A single definition for statuses, priorities,
 * types, due dates, tags and counters.
 * - soft: discreet filled badge; outline: bordered (tags); plain: icon + text without background;
 *   screen: small LCD display for numbers.
 * - tone: color never carries the meaning alone (the label or the icon says it too).
 */
export function Badge({ tone = 'neutral', variant = 'soft', size = 'sm', icon, className, children, ...props }: HTMLAttributes<HTMLSpanElement> & {
  tone?: BadgeTone;
  variant?: BadgeVariant;
  size?: 'sm' | 'md';
  icon?: ReactNode;
}) {
  return (
    <span className={cn('ui-badge', className)} data-tone={tone} data-variant={variant} data-size={size} {...props}>
      {icon}
      {children !== undefined && children !== null && <span className="ui-badge__label">{children}</span>}
    </span>
  );
}

/** Number on the object's display (header, filter and column counters). */
export function Count({ value, className, label }: { value: ReactNode; className?: string; label?: string }) {
  return <Badge variant="screen" className={className} aria-label={label} aria-hidden={label ? undefined : true}>{value}</Badge>;
}

/**
 * Status: the icon shape is enough to tell states apart in grayscale (empty circle,
 * interrupted circle, check mark); the icon color only reinforces. Label: `utils/statuses.ts`.
 */
const STATUS_ICONS: Record<NoteStatus, typeof Circle> = { open: Circle, in_progress: CircleDashed, resolved: CheckCircle };
export function StatusBadge({ status, size = 'sm', className, iconOnly = false }: { status: NoteStatus | string; size?: 'sm' | 'md'; className?: string; iconOnly?: boolean }) {
  const Icon = STATUS_ICONS[status as NoteStatus] ?? Circle;
  const label = statusLabel(status);
  return (
    <Badge variant={iconOnly ? 'plain' : 'soft'} size={size} className={cn('ui-status', className)} data-status={status}
      icon={<Icon size={size === 'md' ? 15 : 13} aria-hidden="true" />}>
      {iconOnly ? <span className="ui-sr-only">{label}</span> : label}
    </Badge>
  );
}

/** Priority: only P0 and P1 carry a color; a resolved note keeps its priority recessed. */
export function PriorityBadge({ priority, status }: Pick<Note, 'priority' | 'status'>) {
  if (!priority) return null;
  const resolved = status === 'resolved';
  const tone: BadgeTone = resolved ? 'neutral' : priority === 'P0' ? 'danger' : priority === 'P1' ? 'warning' : 'neutral';
  return <Badge tone={tone} className="ui-priority" data-priority={priority} data-resolved={resolved || undefined}>{priority}</Badge>;
}

/** Note type: icon and label, without background. */
export function TypeBadge({ type, size = 'sm' }: { type: NoteType; size?: 'sm' | 'md' }) {
  const Icon = TYPE_ICONS[type] ?? TYPE_ICONS.note;
  return <Badge variant="plain" size={size} className="ui-type" data-type={type} icon={<Icon size={size === 'md' ? 16 : 14} aria-hidden="true" />}>{TYPE_LABELS[type] ?? TYPE_LABELS.note}</Badge>;
}

/** Due date: "Overdue · 27/09"; only the overdue and today's due dates are colored. */
export function DueBadge({ due, withDay = true, className }: { due: string; withDay?: boolean; className?: string }) {
  const state = dueState(due);
  const tone: BadgeTone = state === 'overdue' ? 'danger' : state === 'today' ? 'warning' : 'neutral';
  return (
    <Badge variant="plain" tone={tone} className={cn('ui-due', className)} data-due={state ?? undefined} title={i18n.t('common.dueTitle', { date: due })}
      icon={<Clock size={13} aria-hidden="true" />}>
      {dueLabel(due)}{withDay && <> · {shortDay(due)}</>}
    </Badge>
  );
}

/** Tag: "#site". Bordered; `onClick` makes it a button (filter by tag). */
export function TagBadge({ tag, onClick, label, className }: { tag: string; onClick?: () => void; label?: string; className?: string }) {
  if (onClick) return <button type="button" className={cn('ui-badge ui-tag', className)} data-variant="outline" data-tone="neutral" data-size="sm" onClick={onClick} title={tag} aria-label={label}><span className="ui-badge__label">#{tag}</span></button>;
  return <Badge variant="outline" className={cn('ui-tag', className)} title={tag}>#{tag}</Badge>;
}

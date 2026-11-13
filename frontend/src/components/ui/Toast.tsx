import type { CSSProperties, ReactNode } from 'react';
import { CheckCircle, Info, WarningCircle } from '@phosphor-icons/react';
import { cn } from '../../lib/utils';

export type FeedbackTone = 'neutral' | 'success' | 'danger';

const ICONS = { neutral: Info, success: CheckCircle, danger: WarningCircle } as const;

/**
 * Humcase kit — floating message (0.9.67). Pill placed above the screen; the caller keeps
 * the timer and the logic (cancel, clear). `duration` is display only: a ring
 * empties around the action during this delay (frozen if the user reduces motion).
 * Ring idea: "Undo Pill" (21st.dev), rewritten in CSS, without dependency.
 */
export function Toast({ tone = 'neutral', action, duration, className, children }: {
  tone?: FeedbackTone;
  action?: ReactNode;
  duration?: number;
  className?: string;
  children: ReactNode;
}) {
  const Icon = ICONS[tone];
  return (
    <div className={cn('ui-toast', className)} data-tone={tone} role={tone === 'danger' ? 'alert' : 'status'}
      style={duration ? { '--ui-toast-duration': `${duration}ms` } as CSSProperties : undefined}>
      <Icon size={17} aria-hidden="true" className="ui-toast__icon" />
      <span className="ui-toast__message">{children}</span>
      {action && <span className="ui-toast__action">
        {duration && <svg className="ui-toast__timer" viewBox="0 0 36 36" aria-hidden="true"><circle cx="18" cy="18" r="16" pathLength="100" /></svg>}
        {action}
      </span>}
    </div>
  );
}

/** Inline message in the flow (error of an action, confirmation of a setting). */
export function Notice({ tone = 'neutral', className, children, live = true }: {
  tone?: FeedbackTone;
  className?: string;
  children: ReactNode;
  live?: boolean;
}) {
  const Icon = ICONS[tone];
  return (
    <div className={cn('ui-notice', className)} data-tone={tone} role={live ? (tone === 'danger' ? 'alert' : 'status') : undefined}>
      <Icon size={16} aria-hidden="true" className="ui-notice__icon" />
      <div className="ui-notice__message">{children}</div>
    </div>
  );
}

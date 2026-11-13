import type { ReactNode } from 'react';
import { cn } from '../../lib/utils';

/**
 * Humcase kit — list and row (0.9.67).
 * - List `boxed`: a card cut into rows separated by a rule; `divided`: bare rows
 *   separated by a rule; `plain`: bare rows. When bare, rows overflow their padding so
 *   that the text stays aligned with the section title.
 * - ListRow: leading icon, overline (badges), title, description, metadata, trailing element. `onClick` sets a
 *   button that covers the whole row; inner buttons (tags) stay clickable on top.
 */
export function List({ variant = 'boxed', label, className, children }: {
  variant?: 'boxed' | 'divided' | 'plain';
  label?: string;
  className?: string;
  children: ReactNode;
}) {
  return <ul className={cn('ui-list', className)} data-variant={variant} aria-label={label}>{children}</ul>;
}

export function ListRow({ title, titleAs: Title = 'strong', overline, description, meta, leading, trailing, onClick, actionLabel, actionTitle, active = false, size = 'md', className, children }: {
  title: ReactNode;
  overline?: ReactNode;
  titleAs?: 'strong' | 'h3' | 'h4';
  description?: ReactNode;
  meta?: ReactNode;
  leading?: ReactNode;
  trailing?: ReactNode;
  onClick?: () => void;
  actionLabel?: string;
  actionTitle?: string;
  active?: boolean;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
  children?: ReactNode;
}) {
  return (
    <li className={cn('ui-list-row', className)} data-size={size} data-active={active || undefined}>
      {onClick && <button type="button" className="ui-list-row__action" onClick={onClick} aria-label={actionLabel}
        title={actionTitle} aria-current={active ? 'true' : undefined} />}
      {leading && <span className="ui-list-row__leading">{leading}</span>}
      <div className="ui-list-row__body">
        {overline && <span className="ui-list-row__overline">{overline}</span>}
        <Title className="ui-list-row__title">{title}</Title>
        {description && <span className="ui-list-row__description">{description}</span>}
        {children}
        {meta && <span className="ui-list-row__meta">{meta}</span>}
      </div>
      {trailing && <span className="ui-list-row__trailing">{trailing}</span>}
    </li>
  );
}

import type { ReactNode } from 'react';
import { cn } from '../../lib/utils';

/**
 * Humcase kit — empty state (0.9.67). Three fanned keys carry the icon: the object waits
 * to be pressed. Fan idea: "Empty" (cnippet, 21st.dev), redrawn as kit keys.
 * - `md`: empty area of a page or section (dotted border, fan, action);
 * - `sm`: discreet line in a column or narrow panel (text only).
 */
export function EmptyState({ icon, title, description, action, size = 'md', headingLevel = 3, className }: {
  icon?: ReactNode;
  title?: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  size?: 'sm' | 'md';
  headingLevel?: 2 | 3 | 4;
  className?: string;
}) {
  const Heading = `h${headingLevel}` as 'h2' | 'h3' | 'h4';
  return (
    <div className={cn('ui-empty', className)} data-size={size}>
      {size === 'md' && icon && <span className="ui-empty__keys" aria-hidden="true">
        <span className="ui-empty__key" data-side="left" />
        <span className="ui-empty__key" data-side="right" />
        <span className="ui-empty__key">{icon}</span>
      </span>}
      {title && <Heading className="ui-empty__title">{title}</Heading>}
      {description && <p className="ui-empty__description">{description}</p>}
      {action && <div className="ui-empty__action">{action}</div>}
    </div>
  );
}

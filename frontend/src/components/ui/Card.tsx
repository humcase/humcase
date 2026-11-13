import type { ElementType, HTMLAttributes, Ref } from 'react';
import { cn } from '../../lib/utils';

export type CardTone = 'default' | 'sunken' | 'dashed';
export type CardPadding = 'none' | 'sm' | 'md' | 'lg';

/**
 * Humcase kit — card (0.9.67). Single source of a card's surface: border, radius, background,
 * shadow, hover and focus. Screens only handle the layout of their content.
 * - interactive: raised hover and focus ring when an inner element has keyboard focus;
 * - active: selected card (open note);
 * - tone: `sunken` for a tray (column), `dashed` for an invitation to create.
 */
export function Card({ as: Tag = 'div', interactive = false, active = false, tone = 'default', padding = 'md', className, children, ref, ...props }: HTMLAttributes<HTMLElement> & {
  as?: ElementType;
  interactive?: boolean;
  active?: boolean;
  tone?: CardTone;
  padding?: CardPadding;
  ref?: Ref<HTMLElement>;
  type?: 'button' | 'submit';
}) {
  return (
    <Tag ref={ref} className={cn('ui-card', className)} data-tone={tone} data-padding={padding}
      data-interactive={interactive || undefined} data-active={active || undefined} {...props}>
      {children}
    </Tag>
  );
}

import type { ComponentPropsWithRef, ReactNode } from 'react';
import { cn } from '../../lib/utils';
import { Count } from './Badge';
import './chip.css';

/**
 * Humcase kit — a filter chosen in one click, cleared by a second (`aria-pressed`). The chosen
 * chip is a raised key, like the active segment of `SegmentedControl`, so it never reads as a
 * hover. `data-type` on a chip tints its icon with the nature's color.
 */
export function Chip({ selected, icon, count, children, className, ...props }: Omit<ComponentPropsWithRef<'button'>, 'type'> & {
  selected: boolean;
  icon?: ReactNode;
  /** Counter in a small display (hidden from screen readers, like the segments'). */
  count?: number | null;
}) {
  return (
    <button type="button" aria-pressed={selected} className={cn('ui-chip', className)} {...props}>
      {icon}
      <span>{children}</span>
      {count !== undefined && count !== null && <Count value={count} className="ui-chip__count" />}
    </button>
  );
}

/** A labelled row of chips that wraps. */
export function ChipGroup({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return <div role="group" aria-label={label} className={cn('ui-chips', className)}>{children}</div>;
}

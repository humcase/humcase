import { ToggleGroup } from 'radix-ui';
import type { ReactNode } from 'react';
import { cn } from '../../lib/utils';
import { Count } from './Badge';
import './segmented.css';

export interface SegmentedItem<T extends string> {
  value: T;
  label: ReactNode;
  /** Counter displayed in a small display (hidden from screen readers). */
  count?: number | null;
  icon?: ReactNode;
  ariaLabel?: string;
}

/**
 * Humcase kit — exclusive choice (Radix ToggleGroup "single"): keyboard arrows, a single tab
 * stop, radio role. A click on the active segment never deselects it.
 * Variants: "track" (keys on a rail) and "tabs" (underlined tabs).
 */
export function SegmentedControl<T extends string>({ label, value, onValueChange, items, variant = 'track', size = 'default', className }: {
  label: string;
  value: T;
  onValueChange: (value: T) => void;
  items: SegmentedItem<T>[];
  variant?: 'track' | 'tabs';
  size?: 'default' | 'sm';
  className?: string;
}) {
  return (
    <ToggleGroup.Root
      type="single"
      aria-label={label}
      value={value}
      onValueChange={next => { if (next) onValueChange(next as T); }}
      className={cn('ui-segmented', className)}
      data-variant={variant}
      data-size={size}
    >
      {items.map(item => (
        <ToggleGroup.Item key={item.value} value={item.value} aria-label={item.ariaLabel} className="ui-segmented__item">
          {item.icon}
          <span>{item.label}</span>
          {item.count !== undefined && item.count !== null && <Count value={item.count} className="ui-segmented__count" />}
        </ToggleGroup.Item>
      ))}
    </ToggleGroup.Root>
  );
}

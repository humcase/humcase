import { MagnifyingGlass } from '@phosphor-icons/react';
import type { ComponentPropsWithRef } from 'react';
import { cn } from '../../lib/utils';
import './controls.css';

/** Humcase kit — search field: magnifier, accessible label, same design as the other fields. */
export function SearchField({ id, label, className, ...props }: ComponentPropsWithRef<'input'> & { id: string; label: string }) {
  return (
    <label className={cn('ui-search', className)} htmlFor={id}>
      <MagnifyingGlass size={17} aria-hidden="true" />
      <span className="ui-sr-only">{label}</span>
      <input id={id} {...props} />
    </label>
  );
}

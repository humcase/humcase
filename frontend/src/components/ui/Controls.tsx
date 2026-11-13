import type { ComponentPropsWithRef } from 'react';
import './controls.css';

/** Humcase kit — native fields: they keep the events, refs and attributes of the existing flows.
 *  Buttons come from ui/Button (single source). */
export function TextField({ className = '', ...props }: ComponentPropsWithRef<'input'>) {
  return <input {...props} className={`ui-field ${className}`} />;
}

export function SelectField({ className = '', ...props }: ComponentPropsWithRef<'select'>) {
  return <select {...props} className={`ui-field ${className}`} />;
}

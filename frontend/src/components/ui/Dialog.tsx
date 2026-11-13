// Structure taken from the shadcn Dialog (originui variant, 21st id 365): Radix Dialog, header,
// body, footer. Adapted: Humcase casing (raised surface, key edge), theme tokens,
// closing always left to the calling code to preserve its safeguards (draft, write in progress).
import { Dialog as RDialog } from 'radix-ui';
import { X } from '@phosphor-icons/react';
import { useState, type ReactNode } from 'react';
import { cn } from '../../lib/utils';
import { Button } from './Button';
import './dialog.css';

export type DialogSize = 'sm' | 'md' | 'lg' | 'xl' | 'page' | 'palette';

/**
 * Humcase kit — modal window (Radix Dialog): confined focus, Escape, focus returned to the
 * trigger, inert background.
 *
 * `onClose` is the only way to close: Escape and the close button call it, and it
 * decides (it may refuse during a write). The dialog never closes by itself.
 * A click outside does not close, except `closeOnOutside` (command palette).
 * The content must contain a `DialogTitle` (or receive `label` for a hidden title).
 */
export function Dialog({ open = true, onClose, size = 'md', label, describedBy, closeOnOutside = false, className, overlayClassName, children }: {
  open?: boolean;
  onClose: () => void;
  size?: DialogSize;
  /** Hidden accessible title, when the dialog does not display a DialogTitle. */
  label?: string;
  describedBy?: string;
  closeOnOutside?: boolean;
  className?: string;
  overlayClassName?: string;
  children: ReactNode;
}) {
  // Rendered in `.workspace-app`: the dialog inherits the theme (light or dark).
  const [container] = useState(() => (typeof document !== 'undefined' ? document.querySelector<HTMLElement>('.workspace-app') : null));
  return (
    <RDialog.Root open={open} onOpenChange={next => { if (!next) onClose(); }}>
      <RDialog.Portal container={container ?? undefined}>
        <RDialog.Overlay className={cn('ui-dialog-overlay', overlayClassName)} data-size={size} />
        <RDialog.Content
          className={cn('ui-dialog', className)}
          data-size={size}
          aria-modal="true"
          aria-describedby={describedBy}
          onEscapeKeyDown={event => { event.preventDefault(); onClose(); }}
          onPointerDownOutside={event => { event.preventDefault(); if (closeOnOutside) onClose(); }}
          onInteractOutside={event => event.preventDefault()}
        >
          {label && <RDialog.Title className="ui-sr-only">{label}</RDialog.Title>}
          {children}
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}

/** Dialog title: Radix gives it the identifier that names the dialog; do not replace it. */
export function DialogTitle({ className, children }: { className?: string; children: ReactNode }) {
  return <RDialog.Title className={cn('ui-dialog__title', className)}>{children}</RDialog.Title>;
}

/** Header: title, optional description and optional close button (same `onClose`). */
export function DialogHeader({ title, description, onClose, closeLabel = 'Fermer', closeDisabled, className, children }: {
  title: ReactNode;
  description?: ReactNode;
  onClose?: () => void;
  closeLabel?: string;
  closeDisabled?: boolean;
  className?: string;
  children?: ReactNode;
}) {
  return (
    <header className={cn('ui-dialog__header', className)}>
      <div className="ui-dialog__heading">
        <DialogTitle>{title}</DialogTitle>
        {description && <div className="ui-dialog__description">{description}</div>}
        {children}
      </div>
      {onClose && <Button type="button" variant="ghost" size="icon-sm" className="ui-dialog__close" aria-label={closeLabel} onClick={onClose} disabled={closeDisabled}><X size={18} /></Button>}
    </header>
  );
}

export function DialogBody({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn('ui-dialog__body', className)}>{children}</div>;
}

/** Footer: actions aligned to the right, optional `note` on the left (e.g. "Close keeps…"). */
export function DialogFooter({ note, className, children }: { note?: ReactNode; className?: string; children: ReactNode }) {
  return <footer className={cn('ui-dialog__footer', className)}>{note && <small className="ui-dialog__note">{note}</small>}{children}</footer>;
}

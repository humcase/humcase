import { DropdownMenu } from 'radix-ui';
import { Check } from '@phosphor-icons/react';
import { useRef, useState, type ReactNode } from 'react';
import { cn } from '../../lib/utils';
import './menu.css';

/**
 * Humcase kit — dropdown menu (Radix DropdownMenu): keyboard, type-ahead search, focus returned to the
 * trigger. The content is rendered in the trigger's `.workspace-app` container,
 * to inherit the theme. After a mouse close, focus does not return to the button.
 */
export function Menu({ trigger, children, align = 'start', label, className, onOpenChange }: {
  /** Trigger element (a kit Button), passed as `asChild`. */
  trigger: ReactNode;
  children: ReactNode;
  align?: 'start' | 'end' | 'center';
  label?: string;
  className?: string;
  onOpenChange?: (open: boolean) => void;
}) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [container, setContainer] = useState<HTMLElement | undefined>();
  const closeFromPointer = useRef(false);
  return (
    <DropdownMenu.Root onOpenChange={open => {
      if (open) { closeFromPointer.current = false; setContainer(anchor.current?.closest('.workspace-app') as HTMLElement | undefined); }
      onOpenChange?.(open);
    }}>
      <span ref={anchor} className="ui-menu-anchor"><DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger></span>
      <DropdownMenu.Portal container={container}>
        <DropdownMenu.Content
          className={cn('ui-menu', className)}
          sideOffset={6}
          align={align}
          collisionPadding={12}
          onPointerDown={() => { closeFromPointer.current = true; }}
          onPointerDownOutside={() => { closeFromPointer.current = true; }}
          onCloseAutoFocus={event => { if (closeFromPointer.current) { event.preventDefault(); closeFromPointer.current = false; } }}
        >
          {label && <DropdownMenu.Label className="ui-menu__label">{label}</DropdownMenu.Label>}
          {children}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function MenuItem({ children, onSelect, icon, detail, tone }: { children: ReactNode; onSelect: () => void; icon?: ReactNode; detail?: ReactNode; tone?: 'danger' }) {
  return (
    <DropdownMenu.Item className="ui-menu__item" data-tone={tone} onSelect={onSelect}>
      {icon}
      <span className="ui-menu__text"><span>{children}</span>{detail && <small>{detail}</small>}</span>
    </DropdownMenu.Item>
  );
}

/** Exclusive choice in a menu (sort, order…): menuitemradio role, check mark on the selected option. */
export function MenuRadioGroup<T extends string>({ value, onValueChange, options, label }: { value: T; onValueChange: (value: T) => void; options: { value: T; label: ReactNode }[]; label?: string }) {
  return (
    <DropdownMenu.RadioGroup value={value} onValueChange={next => onValueChange(next as T)} aria-label={label}>
      {options.map(option => (
        <DropdownMenu.RadioItem key={option.value} value={option.value} className="ui-menu__item">
          <span className="ui-menu__text"><span>{option.label}</span></span>
          <DropdownMenu.ItemIndicator className="ui-menu__check"><Check size={15} aria-hidden="true" /></DropdownMenu.ItemIndicator>
        </DropdownMenu.RadioItem>
      ))}
    </DropdownMenu.RadioGroup>
  );
}

/** Heading of a group of items, inside the menu (not focusable). */
export function MenuLabel({ children }: { children: ReactNode }) {
  return <DropdownMenu.Label className="ui-menu__label">{children}</DropdownMenu.Label>;
}

export function MenuSeparator() {
  return <DropdownMenu.Separator className="ui-menu__separator" />;
}

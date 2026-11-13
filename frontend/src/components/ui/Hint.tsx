// Pattern adapted from "Keyboard Shortcut Toolbar" / cnippet-dev, 21st id 23618:
// https://21st.dev/@cnippet-dev/components/v-tooltip-7
// Kept: Radix tooltip that names the action and shows its shortcut as keys.
// Adapted: keys in the Humcase design (face and edge), theme tokens, Phosphor, no lucide.
import { Tooltip } from 'radix-ui';
import { createContext, useContext, useState, type ReactElement, type ReactNode } from 'react';
import { shortcutKeys } from './shortcutKeys';
import './hint.css';

/** A screen-printed keyboard key ("Ctrl", "B"…). Decorative: the shortcut is also announced by aria-keyshortcuts. */
export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="ui-kbd">{children}</kbd>;
}

/**
 * Humcase kit — tooltip of a control: its name and, if any, its shortcut as keys.
 * The child (a kit button) must carry its own accessible name; the tooltip shows it
 * on mouse and keyboard focus. `shortcut` follows the aria-keyshortcuts syntax ("Control+B").
 * In a toolbar, wrap the row in `HintGroup`: after a first tooltip,
 * the neighbors open without delay.
 */
export function Hint({ label, shortcut, side = 'bottom', children }: { label: string; shortcut?: string; side?: 'top' | 'bottom' | 'left' | 'right'; children: ReactElement }) {
  const content = <HintRoot label={label} shortcut={shortcut} side={side}>{children}</HintRoot>;
  return useContext(GroupContext) ? content : <Tooltip.Provider delayDuration={450} skipDelayDuration={200}>{content}</Tooltip.Provider>;
}

const GroupContext = createContext(false);
export function HintGroup({ children }: { children: ReactNode }) {
  return <Tooltip.Provider delayDuration={450} skipDelayDuration={200}><GroupContext.Provider value>{children}</GroupContext.Provider></Tooltip.Provider>;
}

function HintRoot({ label, shortcut, side, children }: { label: string; shortcut?: string; side: 'top' | 'bottom' | 'left' | 'right'; children: ReactElement }) {
  // Rendered in `.workspace-app` to inherit the theme tokens.
  const [container, setContainer] = useState<HTMLElement | null>(null);
  return (
    <Tooltip.Root onOpenChange={open => { if (open && !container) setContainer(document.querySelector<HTMLElement>('.workspace-app')); }}>
      <Tooltip.Trigger asChild>{children}</Tooltip.Trigger>
      <Tooltip.Portal container={container ?? undefined}>
        <Tooltip.Content className="ui-hint" side={side} sideOffset={7} collisionPadding={10}>
          <span>{label}</span>
          {shortcut && <span className="ui-hint__keys" aria-hidden="true">{shortcutKeys(shortcut).map(key => <Kbd key={key}>{key}</Kbd>)}</span>}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

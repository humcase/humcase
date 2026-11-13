import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus } from '@phosphor-icons/react';
import { Count } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Hint } from '../ui/Hint';

/**
 * The header of every page: an eyebrow, the title with its count, a line of context, and the page
 * action on the right ("New case" wherever a case can be created). One place, one style, so the
 * person finds the same thing in the same spot on Home, a project, Today, the Inbox, a shortcut,
 * the trash, and in each of the four views.
 */
export function PageHeader({ eyebrow, title, titleId, icon, count, countLabel, description, action, className }: {
  eyebrow?: ReactNode;
  title: ReactNode;
  titleId?: string;
  icon?: ReactNode;
  count?: number | null;
  /** Read in place of the count, which is a small display hidden from screen readers. */
  countLabel?: string;
  description?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <header className={`workspace-page__header${className ? ` ${className}` : ''}`}>
      <div className="workspace-page__intro">
        {eyebrow && <span className="workspace-page__eyebrow">{eyebrow}</span>}
        {/* The count sits beside the title, not in it: the heading keeps the page's name. The
            action shares the title's line, whatever the description below says. */}
        <div className="workspace-page__title">
          <h1 id={titleId}>{icon}{title}</h1>
          {count !== undefined && count !== null && <Count value={count} label={countLabel} className="workspace-page__count" />}
          {action && <div className="workspace-page__action">{action}</div>}
        </div>
        {description && (typeof description === 'string' ? <p>{description}</p> : description)}
      </div>
    </header>
  );
}

/** "New case", the primary action of a page, with its shortcut. */
export function NewCaseButton({ onClick, label }: { onClick: () => void; label?: string }) {
  const { t } = useTranslation();
  const text = label ?? t('workspace.notes.newNote');
  return (
    <Hint label={text} shortcut="Control+N">
      <Button type="button" onClick={onClick} aria-keyshortcuts="Control+N"><Plus size={19} aria-hidden="true" />{text}</Button>
    </Hint>
  );
}

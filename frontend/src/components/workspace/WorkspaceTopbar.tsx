import { CalendarDots as CalendarDays, CircleNotch, FileText, List as Menu, Moon, PushPin as Pin, Sun } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Hint } from '../ui/Hint';
import type { Project, WorkspacePage } from '../../types';

interface WorkspaceTopbarProps {
  workspacePage: WorkspacePage;
  noteProject: Project | undefined;
  activeProject: Project | undefined;
  activeNoteTitle: string | undefined;
  pageTitle: string;
  briefPending: 'handoff' | 'standup' | null;
  workspaceTheme: 'dark' | 'light';
  onOpenNav: () => void;
  onBackToNotes: () => void;
  /** "All cases", the level above a project. */
  onAllNotes: () => void;
  /** Receives the place where the editor puts the open case's parent cases. */
  onPathSlot: (element: HTMLElement | null) => void;
  /** The project's briefs apply here: its cases page or one of its cases (not Home, the trash, a shortcut, the Inbox). */
  showBriefs: boolean;
  onHandoff: () => void;
  onStandup: () => void;
  onToggleTheme: () => void;
  onToggleMini: () => void;
  /** Opens or closes "Ask Humcase"; absent: no button. */
}

/** Top bar of the normal mode: breadcrumb, briefs, theme, settings and Mini mode. */
export default function WorkspaceTopbar({
  workspacePage, noteProject, activeProject, activeNoteTitle, pageTitle, briefPending, workspaceTheme,
  onOpenNav, onBackToNotes, onAllNotes, onPathSlot, showBriefs, onHandoff, onStandup, onToggleTheme, onToggleMini,
}: WorkspaceTopbarProps) {
  const { t } = useTranslation();
  return (
    <header className="workspace-topbar">
      <div className="workspace-topbar-left">
        <Button variant="ghost" size="icon-sm" className="workspace-icon-action workspace-mobile-menu" onClick={onOpenNav} aria-label={t('workspace.topbar.openNav')}><Menu size={19} /></Button>
        {/* One breadcrumb: a project's page goes up to all cases; a case shows its project (or the
            Inbox), then its parent cases (put here by the editor), then itself. */}
        <nav className="workspace-topbar-trail" aria-label={t('workspace.topbar.trail')}>
          {workspacePage === 'editor' && <Button variant="ghost" size="sm" className="workspace-topbar-crumb" onClick={onBackToNotes}>{noteProject?.is_inbox ? t('workspace.topbar.inbox') : noteProject?.name ?? t('workspace.sidebar.allNotes')}</Button>}
          {workspacePage === 'notes' && activeProject && <Button variant="ghost" size="sm" className="workspace-topbar-crumb" onClick={onAllNotes}>{t('workspace.sidebar.allNotes')}</Button>}
          {workspacePage === 'editor' && <span className="workspace-topbar-parents" ref={onPathSlot} />}
          {(workspacePage === 'editor' || (workspacePage === 'notes' && activeProject)) && <span aria-hidden="true">/</span>}
          <strong aria-current="page">{workspacePage === 'editor' ? activeNoteTitle || pageTitle : pageTitle}</strong>
        </nav>
      </div>
      <div className="workspace-topbar-actions">
        {showBriefs && <Hint label={briefPending === 'handoff' ? t('workspace.topbar.handoffPending') : t('workspace.topbar.handoff')}><Button variant="ghost" size="icon-sm" className="workspace-icon-action" onClick={onHandoff} aria-label={t('workspace.topbar.handoff')} aria-busy={briefPending === 'handoff' || undefined} disabled={briefPending !== null}>{briefPending === 'handoff' ? <CircleNotch size={18} className="animate-spin" /> : <FileText size={18} />}</Button></Hint>}
        {showBriefs && <Hint label={briefPending === 'standup' ? t('workspace.topbar.standupPending') : t('workspace.topbar.standup')}><Button variant="ghost" size="icon-sm" className="workspace-icon-action" onClick={onStandup} aria-label={t('workspace.topbar.standup')} aria-busy={briefPending === 'standup' || undefined} disabled={briefPending !== null}>{briefPending === 'standup' ? <CircleNotch size={18} className="animate-spin" /> : <CalendarDays size={18} />}</Button></Hint>}
        <Hint label={workspaceTheme === 'dark' ? t('workspace.topbar.lightTheme') : t('workspace.topbar.darkTheme')}><Button variant="ghost" size="icon-sm" className="workspace-icon-action" onClick={onToggleTheme} aria-label={workspaceTheme === 'dark' ? t('workspace.topbar.lightTheme') : t('workspace.topbar.darkTheme')}>{workspaceTheme === 'dark' ? <Sun size={18} /> : <Moon size={18} />}</Button></Hint>
        <Hint label={t('workspace.topbar.mini')}><Button variant="ghost" size="icon-sm" className="workspace-icon-action" onClick={onToggleMini} aria-label={t('workspace.topbar.mini')}><Pin size={18} /></Button></Hint>
      </div>
    </header>
  );
}

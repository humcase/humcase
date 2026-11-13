import { useState, type CSSProperties, type ElementType } from 'react';
import { useTranslation } from 'react-i18next';
import './app-sidebar.css';
import { Briefcase as BriefcaseBusiness, CalendarDots as CalendarDays, CaretDown as ChevronDown, DotsThree, FolderOpen, House as Home, Tray as Inbox, SquaresFour as LayoutGrid, Plus, MagnifyingGlass as Search, GearSix as Settings, PencilSimple, Trash, X } from '@phosphor-icons/react';
import type { Activity, Project, WorkspacePage } from '../types';
import { availableScopes, type ProjectSpace, type SpacePreferences, type SpaceScope } from '../utils/spaces';
import { cn } from '@/lib/utils';
import { TRASH_RETENTION_DAYS } from '../utils/trash';
import { HumcaseKey } from './brand/HumcaseKey';
import { SHORTCUT_ICONS } from './workspace/shortcutIcons';
import { NATURE_SHORTCUTS, readShortcutsOpen, readShownShortcuts, saveShortcutsOpen, saveShownShortcuts, type NatureShortcut } from '../utils/natureShortcuts';
import { Button } from './ui/Button';
import { Kbd } from './ui/Hint';
import { shortcutKeys } from './ui/shortcutKeys';
import {
  Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupAction, SidebarGroupContent, SidebarGroupLabel,
  SidebarHeader, SidebarMenu, SidebarMenuAction, SidebarMenuButton, SidebarMenuItem,
  SidebarMenuSub, SidebarMenuSubButton, SidebarMenuSubItem, SidebarProvider,
} from '@/components/shadcn/sidebar';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/shadcn/collapsible';

// Sidebar built on the shadcn/ui Sidebar primitives (Radix). Organization taken from
// Dashboard Sidebar / Arun Dass (21st id 14941) and Animated Sidebar / starc007 (21st id 29334):
// space selector at the top, views, inboxes, projects, settings at the bottom.
// The `.workspace-app` grid places the bar; the provider stays in `display: contents`.

/** Count of a sidebar entry: its unresolved cases, the same rule and style everywhere, said in the tooltip. */
function NavCount({ value }: { value?: number }) {
  const { t } = useTranslation();
  if (!value) return null;
  const rule = t('workspace.sidebar.unresolvedCount', { count: value });
  // Decorative beside the entry's name, which stays the name of the page; the rule is in the tooltip.
  return <span className="workspace-nav-count" title={rule} aria-hidden="true">{value}</span>;
}

/** Menu item: comfortable height (36 px) and truncated label. */
function NavItem({ title, icon: Icon, active, onSelect, badge, shortcut, description }: {
  title: string; icon: ElementType; active?: boolean; onSelect: () => void;
  badge?: number; shortcut?: string; description?: string;
}) {
  return <SidebarMenuItem>
    <SidebarMenuButton className="h-9" isActive={active} aria-current={active ? 'page' : undefined} title={description} onClick={onSelect}>
      <Icon aria-hidden="true" />
      <span className="flex-1 truncate">{title}</span>
      {shortcut && <span className="ui-kbd-group" aria-hidden="true">{shortcutKeys(shortcut).map(key => <Kbd key={key}>{key}</Kbd>)}</span>}
      <NavCount value={badge} />
    </SidebarMenuButton>
  </SidebarMenuItem>;
}

interface WorkspaceSidebarProps {
  projects: Project[];
  inboxProjects: Project[];
  activities: Activity[];
  activeActivityId: string | null;
  onActivity: (id: string | null) => void;
  /** Opens the "Project" window: name, color, areas, archiving, deletion. */
  onProjectSettings: (project: Project) => void;
  /** Archived projects of the space: hidden from the list, openable from "Archived". */
  archivedProjects: Project[];
  page: WorkspacePage;
  activeProjectId: string | null;
  todayActive: boolean;
  todayCount: number;
  mobileOpen: boolean;
  onCloseMobile: () => void;
  onHome: () => void;
  onAllNotes: () => void;
  /** Live cases of each shortcut by nature, in the current space. */
  shortcutCounts?: Partial<Record<NatureShortcut, number>>;
  /** Unresolved cases of each project, sub-cases included. */
  projectCounts?: ReadonlyMap<string, number>;
  /** Shortcut shown, when the page is a shortcut's view. */
  activeShortcut?: NatureShortcut | null;
  /** Opens a shortcut's view; without it, the shortcuts are not offered. */
  onShortcut?: (shortcut: NatureShortcut) => void;
  onToday: () => void;
  onProject: (id: string) => void;
  onInbox: (space: ProjectSpace) => void;
  /** Space whose Inbox is shown before it exists (no project yet), to highlight it. */
  inboxPreview?: ProjectSpace | null;
  onNewProject: () => void;
  onSearch: () => void;
  onSettings: () => void;
  /** Opens the trash (deleted notes, restorable for 30 days). */
  onTrash: () => void;
  spacePreferences: SpacePreferences;
  onScope: (scope: SpaceScope) => void;
  unassignedCount: number;
  onAssignProject: (id: string, space: ProjectSpace) => void;
}

const fieldClass = 'ui-field ui-field--sm w-full';
const NO_COUNTS: ReadonlyMap<string, number> = new Map();

export default function WorkspaceSidebar({
  projects, inboxProjects, activities, activeActivityId, onActivity, onProjectSettings, archivedProjects, page, activeProjectId, todayActive, todayCount, mobileOpen, onCloseMobile,
  onHome, onAllNotes, onToday, onProject, onInbox, inboxPreview = null, onNewProject, onSearch, onSettings, onTrash,
  spacePreferences, onScope, unassignedCount, onAssignProject, shortcutCounts = {}, projectCounts = NO_COUNTS, activeShortcut = null, onShortcut,
}: WorkspaceSidebarProps) {
  const { t } = useTranslation();
  // Shortcuts by nature fold under "All cases"; which ones show and the fold are remembered.
  const [shortcutsOpen, setShortcutsOpen] = useState(readShortcutsOpen);
  const [shown, setShown] = useState(readShownShortcuts);
  const [customizing, setCustomizing] = useState(false);
  const toggleShortcuts = () => { const next = !shortcutsOpen; setShortcutsOpen(next); saveShortcutsOpen(next); };
  const toggleShown = (key: NatureShortcut) => {
    const next = NATURE_SHORTCUTS.filter(item => item === key ? !shown.includes(key) : shown.includes(item));
    setShown(next);
    saveShownShortcuts(next);
  };
  const spaceLabel = (scope: SpaceScope) => t(scope === 'private' ? 'workspace.sidebar.scopePrivate' : scope === 'work' ? 'workspace.sidebar.scopeWork' : scope === 'all' ? 'workspace.sidebar.scopeAll' : 'workspace.sidebar.scopeUnassigned');
  const allActive = page === 'notes' && activeProjectId === null && !todayActive && !inboxPreview;
  const scopedActivities = activities.filter(activity => spacePreferences.scope === 'all' || activity.space === spacePreferences.scope);
  const inboxFor = (space: ProjectSpace) => inboxProjects.find(item => item.space === space);
  const inboxActive = (space: ProjectSpace) => {
    const project = inboxFor(space);
    if (!project) return inboxPreview === space;
    return activeProjectId === project.id && (page === 'notes' || page === 'editor');
  };
  const openInbox = (space: ProjectSpace) => { const project = inboxFor(space); if (project) onProject(project.id); else onInbox(space); };
  const ScopeIcon = spacePreferences.scope === 'work' ? BriefcaseBusiness : spacePreferences.scope === 'private' ? Home : LayoutGrid;

  return (
    <SidebarProvider className="contents" style={{ '--sidebar-width': 'var(--workspace-sidebar-width)' } as CSSProperties}>
      {mobileOpen && <button className="workspace-nav-backdrop" aria-label={t('workspace.sidebar.closeNav')} onClick={onCloseMobile} />}
      <aside className={cn('app-sidebar', mobileOpen && 'app-sidebar--open')} aria-label={t('workspace.sidebar.mainNav')}>
        <Sidebar className="w-full">
          <SidebarHeader className="gap-4 px-3 pt-4 pb-2">
            <div className="flex items-center gap-2.5 px-1">
              <HumcaseKey size={34} />
              <span className="objet-brand-name text-sidebar-foreground" aria-label="Humcase"><b>hum</b>case</span>
              <Button type="button" variant="ghost" size="icon-sm" className="app-sidebar__close ml-auto" aria-label={t('workspace.sidebar.closeNav')} onClick={onCloseMobile}><X size={18} /></Button>
            </div>
            {/* Native disclosure: works with the keyboard, without a portal, and stays testable. */}
            <details className="app-sidebar__spaces group/spaces relative" key={spacePreferences.scope}>
              <summary aria-label={t('workspace.sidebar.changeSpace', { space: spaceLabel(spacePreferences.scope) })} className="border-sidebar-border hover:bg-sidebar-accent text-sidebar-foreground flex h-9 cursor-pointer list-none items-center gap-2 rounded-md border px-2.5 text-sm font-medium group-open/spaces:border-primary [&::-webkit-details-marker]:hidden">
                <ScopeIcon size={16} aria-hidden="true" />
                <span className="flex-1 truncate">{spaceLabel(spacePreferences.scope)}</span>
                <ChevronDown size={14} aria-hidden="true" className="transition-transform group-open/spaces:rotate-180" />
              </summary>
              <div className="bg-popover text-popover-foreground border-border absolute inset-x-0 top-[calc(100%+4px)] z-30 flex flex-col gap-0.5 rounded-md border p-1 shadow-md" role="group" aria-label={t('workspace.sidebar.yourSpaces')}>
                {availableScopes(spacePreferences).map(scope => <button type="button" key={scope} aria-current={spacePreferences.scope === scope ? 'true' : undefined} onClick={() => onScope(scope)} className="hover:bg-accent hover:text-accent-foreground aria-[current=true]:bg-accent aria-[current=true]:text-accent-foreground rounded-sm px-2 py-1.5 text-left text-sm">
                  {spaceLabel(scope)}{scope === 'unassigned' && unassignedCount > 0 ? ` · ${unassignedCount}` : ''}
                </button>)}
              </div>
            </details>
          </SidebarHeader>

          <SidebarContent className="gap-0">
            <SidebarGroup>
              <SidebarGroupContent>
                <nav aria-label={t('workspace.sidebar.views')}><SidebarMenu>
                  <NavItem title={t('workspace.sidebar.search')} icon={Search} shortcut="Control+F" onSelect={onSearch} />
                  <NavItem title={t('workspace.sidebar.home')} icon={Home} active={page === 'home'} onSelect={onHome} />
                  <SidebarMenuItem>
                    <SidebarMenuButton className="h-9 pr-8" isActive={allActive} aria-current={allActive ? 'page' : undefined} onClick={onAllNotes}>
                      <LayoutGrid aria-hidden="true" /><span className="flex-1 truncate">{t('workspace.sidebar.allNotes')}</span>
                    </SidebarMenuButton>
                    {onShortcut && <SidebarMenuAction className="top-2!" aria-expanded={shortcutsOpen} aria-controls="nature-shortcuts" aria-label={t(shortcutsOpen ? 'workspace.shortcuts.toggleClose' : 'workspace.shortcuts.toggleOpen')} title={t(shortcutsOpen ? 'workspace.shortcuts.toggleClose' : 'workspace.shortcuts.toggleOpen')} onClick={toggleShortcuts}>
                      <ChevronDown aria-hidden="true" className={cn('transition-transform', !shortcutsOpen && '-rotate-90')} />
                    </SidebarMenuAction>}
                    {onShortcut && shortcutsOpen && <nav id="nature-shortcuts" aria-label={t('workspace.shortcuts.nav')}><SidebarMenuSub>
                      {shown.map(key => {
                        const Icon = SHORTCUT_ICONS[key];
                        const active = page === 'nature' && activeShortcut === key;
                        return <SidebarMenuSubItem key={key}>
                          <SidebarMenuSubButton asChild isActive={active}><button type="button" className="w-full justify-start text-left" aria-current={active ? 'page' : undefined} onClick={() => onShortcut(key)}>
                            <Icon aria-hidden="true" /><span className="flex-1 truncate">{t(`workspace.shortcuts.${key}`)}</span>
                            <NavCount value={shortcutCounts[key]} />
                          </button></SidebarMenuSubButton>
                        </SidebarMenuSubItem>;
                      })}
                      <SidebarMenuSubItem>
                        <SidebarMenuSubButton asChild><button type="button" className="w-full justify-start text-left text-muted-foreground" aria-expanded={customizing} onClick={() => setCustomizing(open => !open)}>
                          <PencilSimple aria-hidden="true" /><span className="flex-1 truncate">{t('workspace.shortcuts.customize')}</span>
                        </button></SidebarMenuSubButton>
                      </SidebarMenuSubItem>
                      {customizing && <li className="app-sidebar__shortcut-choice"><fieldset>
                        <legend>{t('workspace.shortcuts.customizeTitle')}</legend>
                        {NATURE_SHORTCUTS.map(key => <label key={key}><input type="checkbox" checked={shown.includes(key)} onChange={() => toggleShown(key)} />{t(`workspace.shortcuts.${key}`)}</label>)}
                      </fieldset></li>}
                    </SidebarMenuSub></nav>}
                  </SidebarMenuItem>
                  <NavItem title={t('workspace.sidebar.today')} icon={CalendarDays} active={todayActive && page === 'notes'} badge={todayCount} description={t('workspace.sidebar.todayDesc')} onSelect={onToday} />
                  {spacePreferences.scope !== 'unassigned' && (spacePreferences.scope === 'all'
                    ? <Collapsible asChild className="group/inbox">
                        <SidebarMenuItem>
                          <CollapsibleTrigger asChild>
                            <SidebarMenuButton className="h-9"><Inbox aria-hidden="true" /><span>{t('workspace.sidebar.inboxes')}</span><ChevronDown aria-hidden="true" className="ml-auto transition-transform group-data-[state=open]/inbox:rotate-180" /></SidebarMenuButton>
                          </CollapsibleTrigger>
                          <CollapsibleContent asChild><nav aria-label={t('workspace.sidebar.inboxes')}>
                            <SidebarMenuSub>
                              {(['private', 'work'] as ProjectSpace[]).map(space => <SidebarMenuSubItem key={space}>
                                <SidebarMenuSubButton asChild isActive={inboxActive(space)}><button type="button" className="w-full" onClick={() => openInbox(space)}><span className="flex-1 truncate text-left">{t(space === 'private' ? 'common.space.private' : 'common.space.work')}</span><NavCount value={projectCounts.get(inboxFor(space)?.id ?? '')} /></button></SidebarMenuSubButton>
                              </SidebarMenuSubItem>)}
                            </SidebarMenuSub>
                          </nav></CollapsibleContent>
                        </SidebarMenuItem>
                      </Collapsible>
                    : <NavItem title={t('workspace.sidebar.inbox')} icon={Inbox} active={inboxActive(spacePreferences.scope as ProjectSpace)} badge={projectCounts.get(inboxFor(spacePreferences.scope as ProjectSpace)?.id ?? '')} onSelect={() => openInbox(spacePreferences.scope as ProjectSpace)} />)}
                </SidebarMenu></nav>
              </SidebarGroupContent>
            </SidebarGroup>

            <SidebarGroup>
              <SidebarGroupLabel>{t('workspace.sidebar.projects')}</SidebarGroupLabel>
              <SidebarGroupAction aria-label={t('workspace.sidebar.newProject')} title={t('workspace.sidebar.newProject')} onClick={onNewProject}><Plus aria-hidden="true" /></SidebarGroupAction>
              <SidebarGroupContent className="flex flex-col gap-2">
                {scopedActivities.length > 0 && <label className="px-0.5">
                  <span className="sr-only">{t('workspace.sidebar.area')}</span>
                  <select aria-label={t('workspace.sidebar.filterByArea')} className={fieldClass} value={activeActivityId ?? ''} onChange={event => onActivity(event.target.value || null)}>
                    <option value="">{t('workspace.sidebar.allAreas')}</option>
                    {scopedActivities.map(activity => <option key={activity.id} value={activity.id}>{activity.name}</option>)}
                  </select>
                </label>}
                <nav aria-label={t('workspace.sidebar.projects')}><SidebarMenu>
                  {projects.map(project => {
                    const active = activeProjectId === project.id && (page === 'notes' || page === 'editor');
                    const open = projectCounts.get(project.id) ?? 0;
                    return <SidebarMenuItem key={project.id}>
                      <SidebarMenuButton className="h-9" isActive={active} aria-current={active ? 'page' : undefined} onClick={() => onProject(project.id)}>
                        <span className="grid size-5 shrink-0 place-items-center rounded-[5px] text-white" style={{ backgroundColor: project.color || 'var(--workspace-sidebar-control-selected)' }}><FolderOpen size={13} aria-hidden="true" /></span>
                        <span className="flex-1 truncate">{project.name}</span>
                        <NavCount value={open} />
                      </SidebarMenuButton>
                      {!project.is_inbox && <SidebarMenuAction className="top-2!" aria-label={t('workspace.sidebar.projectSettings', { name: project.name })} title={t('workspace.sidebar.projectSettingsTitle', { name: project.name })} onClick={() => onProjectSettings(project)}><DotsThree weight="bold" aria-hidden="true" /></SidebarMenuAction>}
                      {project.space == null && <select aria-label={t('workspace.sidebar.assign', { name: project.name })} className={cn(fieldClass, 'mt-1 ml-7 w-auto')} defaultValue="" onChange={event => { if (event.target.value) onAssignProject(project.id, event.target.value as ProjectSpace); event.target.value = ''; }}>
                        <option value="">{t('workspace.sidebar.assignPlaceholder')}</option><option value="private">{t('common.space.private')}</option><option value="work">{t('common.space.work')}</option>
                      </select>}
                    </SidebarMenuItem>;
                  })}
                </SidebarMenu></nav>
                {archivedProjects.length > 0 && <details className="workspace-archived-projects">
                  <summary>{t('workspace.sidebar.archived')} <span className="workspace-nav-count">{archivedProjects.length}</span></summary>
                  <nav aria-label={t('workspace.sidebar.archivedNav')}><SidebarMenu>
                    {archivedProjects.map(project => {
                      const active = activeProjectId === project.id && (page === 'notes' || page === 'editor');
                      return <SidebarMenuItem key={project.id}>
                        <SidebarMenuButton className="h-9" isActive={active} aria-current={active ? 'page' : undefined} onClick={() => onProject(project.id)}>
                          <span className="grid size-5 shrink-0 place-items-center rounded-[5px] text-white opacity-60" style={{ backgroundColor: project.color || 'var(--workspace-sidebar-control-selected)' }}><FolderOpen size={13} aria-hidden="true" /></span>
                          <span className="flex-1 truncate">{project.name}</span>
                        </SidebarMenuButton>
                        <SidebarMenuAction className="top-2!" aria-label={t('workspace.sidebar.projectSettings', { name: project.name })} title={t('workspace.sidebar.archivedProjectTitle', { name: project.name })} onClick={() => onProjectSettings(project)}><DotsThree weight="bold" aria-hidden="true" /></SidebarMenuAction>
                      </SidebarMenuItem>;
                    })}
                  </SidebarMenu></nav>
                </details>}
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>

          <SidebarFooter className="border-sidebar-border border-t px-2 py-2">
            <SidebarMenu>
              <NavItem title={t('workspace.sidebar.trash')} icon={Trash} active={page === 'trash'} description={t('workspace.sidebar.trashDesc', { days: TRASH_RETENTION_DAYS })} onSelect={onTrash} />
              <NavItem title={t('workspace.sidebar.settings')} icon={Settings} onSelect={onSettings} />
            </SidebarMenu>
          </SidebarFooter>
        </Sidebar>
      </aside>
    </SidebarProvider>
  );
}

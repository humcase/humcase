import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import WorkspaceSidebar from './components/WorkspaceSidebar';
import { TrashView, WorkspaceNotes } from './components/workspace';
import { HomePage } from './components/workspace/HomePage';
import { InboxPreview } from './components/workspace/InboxPreview';
import WorkspaceTopbar from './components/workspace/WorkspaceTopbar';
import NotesViewbar from './components/workspace/NotesViewbar';
import { CaseFilterBar } from './components/workspace/CaseFilterBar';
import InboxPlacement from './components/workspace/InboxPlacement';
import { NewCaseButton, PageHeader } from './components/workspace/PageHeader';
import BriefDialog from './components/workspace/BriefDialog';
import ProjectCreateDialog from './components/workspace/ProjectCreateDialog';
import ProjectSettingsDialog from './components/workspace/ProjectSettingsDialog';
import NoteTargetDialog from './components/workspace/NoteTargetDialog';
import { Button } from './components/ui/Button';
import { Hint } from './components/ui/Hint';
import { Toast } from './components/ui/Toast';
import { EmptyState } from './components/ui/EmptyState';
import { CalendarDots } from '@phosphor-icons/react';
import NoteList from './components/NoteList';
import NoteEditor from './components/NoteEditor';
import SettingsModal from './components/SettingsModal';
import CommandPalette from './components/CommandPalette';
import KanbanBoard from './components/KanbanBoard';
import { api } from './services/tauriApi';
import { useWorkspaceChangeWatch } from './hooks/useWorkspaceChangeWatch';
import { deriveCaseView } from './utils/caseView';
import { resolveActiveNote } from './utils/selection';
import { sortNotes, type NoteSort } from './utils/noteSort';
import { isNoteStatus } from './utils/statuses';
import { defaultTitle } from './utils/title';
import { getLastProjectId, getMiniMode, setLastProjectId } from './utils/prefs';
import type { Note, NoteStatus, NoteType, WorkspacePage } from './types';
import { NatureView } from './components/workspace/NatureView';
import { TidyView } from './components/workspace/Tidy';
import { ProfilePage } from './components/workspace/Profile';
import { ProjectSummaryView } from './components/workspace/ProjectSummary';
import { SegmentedControl } from './components/ui/SegmentedControl';
import { CaseLocation } from './components/workspace/CaseLocation';
import { shortcutCounts as countShortcuts, shortcutNature, unresolvedByProject, type NatureShortcut } from './utils/natureShortcuts';
import './project-settings.css';
import { ArrowLeft, ArrowsOut as Maximize2 } from '@phosphor-icons/react';
import { prepareSnapshot, type BackupPreparation } from './utils/backupBarrier';
import SketchEditor from './components/sketch/SketchEditor';
import { projectInScope, type ProjectSpace } from './utils/spaces';
import { useActionToast } from './hooks/useActionToast';
import { useActivities } from './hooks/useActivities';
import { useSpaceActions, useSpacePreferences } from './hooks/useSpacePreferences';
import { useSubCasesChoice } from './hooks/useSubCasesChoice';
import { useNotesLoading } from './hooks/useNotesLoading';
import { useProjectsLoading } from './hooks/useProjectsLoading';
import { useMiniWindow } from './hooks/useMiniWindow';
import { useBriefs } from './hooks/useBriefs';
import { useProjectDialogs } from './hooks/useProjectDialogs';
import { useInboxMove } from './hooks/useInboxMove';
import { useGlobalShortcuts } from './hooks/useGlobalShortcuts';
import { useCurrentDay } from './hooks/useCurrentDay';
import { useOpenNoteAnnouncement, useQuickCapturePersistenceWatch, useQuickCaptureShortcut, useRefreshOnFocus } from './hooks/useWindowEvents';
import './workspace-theme.css';
import './components/ui/kit.css';
import './workspace-shell.css';
import './workspace-mini.css';
import './spaces.css';
import './activities-persistent.css';
import './foundations.css';
import './game-object.css';
import { OPEN_SETTINGS_EVENT } from './hooks/useAiReady';
import CaseBoard from './components/workspace/CaseBoard';
import { warmCardExcerpts } from './components/workspace/cardExcerpts';
import { readProjectSummary, readViewMode, saveProjectSummary, saveViewMode, type NotesViewMode } from './utils/viewMode';
import { MeaningResults } from './components/workspace/MeaningResults';
import { useMeaningHits, useMeaningStatus } from './hooks/useMeaning';
import { meaningRows } from './utils/meaningRows';
import { searchRank as rankSearch } from './utils/searchRank';
import { useSupersededIds } from './hooks/useTidy';
import { currentDateLocale } from './i18n';

/** Where a case was opened from: "Back" leads there and the sidebar keeps it highlighted. */
interface ReturnPoint {
  page: WorkspacePage;
  projectId: string | null;
  today: boolean;
  shortcut: NatureShortcut | null;
  activityId: string | null;
  inbox: ProjectSpace | null;
}

function App() {
  const { t } = useTranslation();
  const { spacePreferences, setSpacePreferences, spaceEpochRef } = useSpacePreferences();
  const [inboxPreviewSpace, setInboxPreviewSpace] = useState<ProjectSpace | null>(null);
  const [activeProjectId, setActiveProjectId] = useState<string | null>(() => getMiniMode() ? getLastProjectId() : null);
  const [workspacePage, setWorkspacePage] = useState<WorkspacePage>('home');
  // Shortcut by nature shown.
  const [natureView, setNatureView] = useState<NatureShortcut | null>(null);
  // Page the open case came from; null when it was opened from nowhere in particular.
  const [returnTo, setReturnTo] = useState<ReturnPoint | null>(null);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const [noteTargetDialogOpen, setNoteTargetDialogOpen] = useState(false);
  const [noteTargetId, setNoteTargetId] = useState('');
  const [noteTargetPending, setNoteTargetPending] = useState(false);
  const [activeNoteId, setActiveNoteId] = useState<string | null>(null);

  const [globalError, setGlobalError] = useState<string | null>(null);
  const [workspaceTheme, setWorkspaceTheme] = useState<'dark' | 'light'>(() => {
    try { return localStorage.getItem('humcase-workspace-theme') === 'light' ? 'light' : 'dark'; }
    catch { return 'dark'; }
  });
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  // Settings open on Agents (Home's first steps, "Connect an agent").
  const [focusAgentSettings, setFocusAgentSettings] = useState(false);
  const [focusAiSettings, setFocusAiSettings] = useState(false);
  // "Connect an AI" in the editor opens Settings on the Voice and AI section.
  useEffect(() => {
    const open = () => { setFocusAiSettings(true); setIsSettingsOpen(true); };
    window.addEventListener(OPEN_SETTINGS_EVENT, open);
    return () => window.removeEventListener(OPEN_SETTINGS_EVENT, open);
  }, []);
  const [focusActivitySettings, setFocusActivitySettings] = useState(false);
  const prepareEditorRef = useRef<(() => Promise<BackupPreparation>) | null>(null);
  const registerEditorPreparation = useCallback((prepare: (() => Promise<BackupPreparation>) | null) => {
    prepareEditorRef.current = prepare;
  }, []);
  // The sketch editor registers apart: it lives beside the text editor, which stays mounted.
  const prepareSketchRef = useRef<(() => Promise<BackupPreparation>) | null>(null);
  const registerSketchPreparation = useCallback((prepare: (() => Promise<BackupPreparation>) | null) => {
    prepareSketchRef.current = prepare;
  }, []);
  const prepareEditors = useCallback(async (): Promise<BackupPreparation> => {
    for (const prepare of [prepareSketchRef.current, prepareEditorRef.current]) {
      const result = prepare ? await prepare() : { ready: true as const };
      if (!result.ready) return result;
    }
    return { ready: true };
  }, []);
  const prepareBackup = useCallback(() => prepareSnapshot(prepareEditors), [prepareEditors]);
  // Filing a case waits for the editors too, but only the case's own kept writes stop it: a draft
  // of another case, of another space, has nothing to do with this move.
  const prepareMove = useCallback((noteId: string) => prepareSnapshot(prepareEditors, new Set([noteId])), [prepareEditors]);
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(false);
  // Place of the open case's ancestors in the topbar breadcrumb (filled by the editor).
  const [topbarPathSlot, setTopbarPathSlot] = useState<HTMLElement | null>(null);

  const [viewMode, setViewMode] = useState<NotesViewMode>(readViewMode);
  // A project's page shows its cases or its summary (remembered).
  const [projectSummary, setProjectSummary] = useState(readProjectSummary);
  // The canvas creates in place (a draft in its next free spot, inside the case dived into): the
  // page's "New case" calls it while the canvas is shown.
  const [canvasCreate, setCanvasCreate] = useState<{ create: () => void; label: string } | null>(null);
  // Cases dived into on the canvas, kept while a case is open in the editor.
  const [boardPath, setBoardPath] = useState<string[]>([]);
  const [noteSort, setNoteSort] = useState<NoteSort>('attention');
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<string>('');
  // Filters of the cases page shared by every view (canvas, cards, list, board).
  const [natureFilter, setNatureFilter] = useState<NoteType | ''>('');
  // "Include sub-cases": one remembered choice, which the status, the nature and "Clear filters" leave alone.
  const subCases = useSubCasesChoice();
  const subCasesShown = subCases.shown;
  // Notes view: show only archived notes ("Archived" button of the display bar).
  const [showArchived, setShowArchived] = useState(false);
  const [todayView, setTodayView] = useState(false);
  // Creation lock: two quick Ctrl+N must not create two notes.
  const creationEnCoursRef = useRef(false);
  // Error of a one-off action (Handoff, Standup, creation): visible and non-blocking.
  const { actionError, toneAction, reportAction } = useActionToast();
  // Is a modal open? Global shortcuts must not act behind it.
  const modalOpenRef = useRef(false);

  const { projectNotes, setProjectNotes, searchIds, initialNotesSettled, loadTokenRef, loadNotes } =
    useNotesLoading({ searchQuery, spacePreferences, setGlobalError });
  const { projects, setProjects, initialProjectsSettled, loadProjects } = useProjectsLoading({ setGlobalError });
  const { activities, setActivities, activeActivityId, setActiveActivityId, loadActivities, refreshActivities, createActivity, renameActivity, deleteActivity } =
    useActivities({ reportAction, setGlobalError });

  // Projects of the space, recomputed only when the projects, the space or the activity change: the
  // lists below are keyed on these identities.
  const { scopeProjects, inboxProjects, visibleProjects, archivedProjects, archivedProjectIds, visibleProjectIds, unassignedCount } = useMemo(() => {
    const inScope = projects.filter(project => projectInScope(project, spacePreferences.scope));
    const archived = inScope.filter(project => !project.is_inbox && project.archived_at);
    return {
      scopeProjects: inScope,
      inboxProjects: inScope.filter(project => project.is_inbox),
      visibleProjects: inScope.filter(project => !project.is_inbox && !project.archived_at && (!activeActivityId || activities.some(activity => activity.id === activeActivityId && activity.project_ids.includes(project.id)))),
      archivedProjects: archived,
      archivedProjectIds: new Set(archived.map(project => project.id)),
      visibleProjectIds: new Set(inScope.map(project => project.id)),
      unassignedCount: projects.filter(project => !project.is_inbox && project.space == null).length,
    };
  }, [projects, spacePreferences.scope, activeActivityId, activities]);

  const { briefPending, showStandup, setShowStandup, showHandoff, setShowHandoff, handoffText, standupText, copyError, closeTextModal, openHandoff, openStandup, copyAndClose } =
    useBriefs({ activeProjectId, visibleProjectIds, spaceEpochRef, reportAction });

  // Reset of navigation and lists on every space change.
  const resetWorkspace = useCallback(() => {
    setActiveActivityId(null);
    setInboxPreviewSpace(null);
    setActiveProjectId(null);
    setActiveNoteId(null);
    setProjectNotes([]);
    loadTokenRef.current.fresh();
    setTodayView(false);
    setSearchQuery('');
    setStatusFilter('');
    setWorkspacePage('home');
    setMobileNavOpen(false);
    setIsCommandPaletteOpen(false);
    setShowHandoff(false);
    setShowStandup(false);
    setLastProjectId(null);
  }, [setActiveActivityId, setProjectNotes, loadTokenRef, setShowHandoff, setShowStandup]);
  const { applySpacePreferences, changeScope, changeSpaceUsage, changeCombinedView } =
    useSpaceActions({ spacePreferences, setSpacePreferences, spaceEpochRef, resetWorkspace });
  const assignProject = useCallback(async (id: string, space: ProjectSpace) => {
    try {
      await api.updateProjectSpace(id, space);
      setProjects(await api.getProjects());
      await loadActivities();
      if (activeProjectId === id) {
        setActiveNoteId(null);
        setActiveProjectId(null);
        setWorkspacePage('home');
      }
    } catch (e) { reportAction(t('workspace.app.assignError', { error: String(e) })); }
  }, [activeProjectId, reportAction, loadActivities, setProjects, t]);

  useEffect(() => {
    if (!initialProjectsSettled || !activeProjectId) return;
    if (projects.some(project => project.id === activeProjectId && projectInScope(project, spacePreferences.scope))) return;
    // Synchronize navigation with an externally reassigned/deleted project (MCP refresh).
    // oxlint-disable-next-line react/set-state-in-effect
    setActiveProjectId(null);
    setActiveNoteId(null);
    setWorkspacePage('home');
  }, [initialProjectsSettled, activeProjectId, projects, spacePreferences.scope]);

  const hereRef = useRef<ReturnPoint>({ page: 'home', projectId: null, today: false, shortcut: null, activityId: null, inbox: null });
  useEffect(() => {
    hereRef.current = { page: workspacePage, projectId: activeProjectId, today: todayView, shortcut: workspacePage === 'nature' ? natureView : null, activityId: activeActivityId, inbox: inboxPreviewSpace };
  });
  // Opening a case from a page remembers that page; going from case to case keeps the first one.
  const rememberOrigin = useCallback(() => {
    if (hereRef.current.page !== 'editor') setReturnTo(hereRef.current);
  }, []);

  const selectProject = useCallback((id: string | null) => {
    setTodayView(false);
    setInboxPreviewSpace(null);
    if (id && projects.some(project => project.id === id && project.is_inbox)) setActiveActivityId(null);
    setActiveProjectId(id);
    setLastProjectId(id);
    // The selected note belonged to the old project: do not leave the editor on a note
    // that cannot be found in the new list.
    setActiveNoteId(null);
    setWorkspacePage('notes');
    setViewMode(readViewMode());
    setBoardPath([]);
    setMobileNavOpen(false);
  }, [projects, setActiveActivityId]);

  const goHome = useCallback(() => {
    setInboxPreviewSpace(null);
    setActiveNoteId(null);
    setActiveProjectId(null);
    setTodayView(false);
    setWorkspacePage('home');
    setMobileNavOpen(false);
  }, []);

  const goTrash = useCallback(() => {
    setInboxPreviewSpace(null);
    setActiveNoteId(null);
    setTodayView(false);
    setWorkspacePage('trash');
    setMobileNavOpen(false);
  }, []);

  const goTidy = useCallback(() => {
    setInboxPreviewSpace(null);
    setActiveNoteId(null);
    setTodayView(false);
    setWorkspacePage('tidy');
    setMobileNavOpen(false);
  }, []);

  // The work profile shown: the space in view, or the one chosen on its page when both show.
  const [profileSpaceChoice, setProfileSpaceChoice] = useState<ProjectSpace | null>(null);
  const profileSpaces: ProjectSpace[] = spacePreferences.scope === 'work' || spacePreferences.scope === 'private' ? [spacePreferences.scope]
    : spacePreferences.usage === 'both' ? ['work', 'private'] : [spacePreferences.usage];
  const profileSpace = profileSpaceChoice && profileSpaces.includes(profileSpaceChoice) ? profileSpaceChoice : profileSpaces[0];
  const goProfile = useCallback((space?: ProjectSpace) => {
    // From Settings, the profile of a space not in view: that space comes into view first.
    if (space && (spacePreferences.scope === 'work' || spacePreferences.scope === 'private') && spacePreferences.scope !== space) changeScope(space);
    setProfileSpaceChoice(space ?? null);
    setInboxPreviewSpace(null);
    setActiveNoteId(null);
    setTodayView(false);
    setWorkspacePage('profile');
    setMobileNavOpen(false);
  }, [spacePreferences.scope, changeScope]);
  // A nudge's "Sprout": the case opens and proposes its steps at once.
  const [germinateNoteId, setGerminateNoteId] = useState<string | null>(null);

  // Returns whether the case opened (a case out of the space's scope does not).
  const openNote = useCallback((note: Note): boolean => {
    if (!projects.some(project => project.id === note.project_id && projectInScope(project, spacePreferences.scope))) return false;
    // Any case opened disarms a Sprout asked for another opening (re-armed by the nudge after).
    setGerminateNoteId(null);
    if (hereRef.current.page !== 'editor') setReturnTo(hereRef.current);
    setActiveActivityId(null);
    setInboxPreviewSpace(null);
    setActiveProjectId(note.project_id);
    setLastProjectId(note.project_id);
    setActiveNoteId(note.id);
    setWorkspacePage('editor');
    setViewMode(readViewMode());
    setMobileNavOpen(false);
    return true;
  }, [projects, spacePreferences.scope, setActiveActivityId]);
  const clearGerminateNoteId = useCallback(() => setGerminateNoteId(null), []);
  // A case the person just created opens with the cursor in its text, once.
  const [focusNoteId, setFocusNoteId] = useState<string | null>(null);
  if (focusNoteId !== null && (activeNoteId !== focusNoteId || workspacePage !== 'editor')) setFocusNoteId(null);
  // "Edit" on the profile page: its case (created the first time) opens in the editor.
  const editProfile = useCallback(async (space: ProjectSpace) => {
    try {
      const id = await api.openWorkProfile(space);
      await loadNotes();
      openNote(await api.getNote(id));
    } catch (e) { reportAction(t('workspace.profile.openFailed', { error: String(e) })); }
  }, [loadNotes, openNote, reportAction, t]);

  const {
    projectDialogOpen, projectDraftName, projectDraftError, projectPending, projectDraftSpace, projectDraftActivities,
    projectActivityDialogId, projectActivityPending, projectActivityError, projectNameDraft, projectColorDraft, setProjectColorDraft,
    projectActivityDraft, projectDeleteConfirm,
    openProjectDialog, closeProjectDialog, closeProjectActivityDialog, openProjectSettings,
    saveProjectSettings, toggleProjectArchived, deleteProject, createProject,
    changeDraftName, changeDraftSpace, toggleDraftActivity, changeProjectName, toggleProjectActivity, askDeleteProject, cancelDeleteProject,
  } = useProjectDialogs({
    projects, activities, setActivities, activeActivityId, refreshActivities, reportAction, loadProjects, loadNotes,
    activeProjectId, setActiveProjectId, setActiveNoteId, setWorkspacePage, selectProject, spacePreferences, applySpacePreferences,
  });
  const closeNoteTargetDialog = useCallback(() => {
    if (!noteTargetPending) setNoteTargetDialogOpen(false);
  }, [noteTargetPending]);
  useEffect(() => {
    modalOpenRef.current = isSettingsOpen || isCommandPaletteOpen || showStandup || showHandoff || projectDialogOpen || noteTargetDialogOpen || projectActivityDialogId !== null;
  }, [isSettingsOpen, isCommandPaletteOpen, showStandup, showHandoff, projectDialogOpen, noteTargetDialogOpen, projectActivityDialogId]);

  const { isMiniMode, toggleMiniMode } = useMiniWindow(reportAction);

  const createNoteInProject = useCallback(async (projectId: string, noteType: NoteType = 'note') => {
    if (!projects.some(project => project.id === projectId && projectInScope(project, spacePreferences.scope))) return;
    if (creationEnCoursRef.current) return;
    creationEnCoursRef.current = true;
    const epoch = spaceEpochRef.current;
    try {
      const id = await api.createNote({
        project_id: projectId,
        title: defaultTitle(),
        note_type: noteType,
      });
      if (spaceEpochRef.current !== epoch) return;
      rememberOrigin();
      setActiveProjectId(projectId);
      setLastProjectId(projectId);
      setTodayView(false);
      setActiveNoteId(id);
      setFocusNoteId(id);
      setWorkspacePage('editor');
      setNoteTargetDialogOpen(false);
      setMobileNavOpen(false);
      void loadNotes();
    } catch (e) {
      console.error(e);
      reportAction(t('workspace.app.createNoteError'));
    } finally {
      creationEnCoursRef.current = false;
    }
  }, [loadNotes, reportAction, projects, spacePreferences.scope, spaceEpochRef, rememberOrigin, t]);

  const createInboxNote = useCallback(async (space: ProjectSpace, noteType: NoteType = 'note') => {
    if (creationEnCoursRef.current) return;
    creationEnCoursRef.current = true;
    try {
      const inboxId = await api.getOrCreateInbox(space);
      const id = await api.createNote({ project_id: inboxId, title: defaultTitle(), note_type: noteType });
      rememberOrigin();
      setActiveActivityId(null);
      setInboxPreviewSpace(null);
      await loadProjects();
      setActiveProjectId(inboxId);
      setLastProjectId(inboxId);
      setActiveNoteId(id);
      setFocusNoteId(id);
      setTodayView(false);
      setWorkspacePage('editor');
      setMobileNavOpen(false);
      void loadNotes();
    } catch (cause) { reportAction(t('workspace.app.captureError', { error: String(cause) })); }
    finally { creationEnCoursRef.current = false; }
  }, [loadProjects, loadNotes, reportAction, setActiveActivityId, rememberOrigin, t]);

  // "New case" creates a case of the nature the page shows: the nature filter of the cases page,
  // or the shortcut's nature (Decisions create a decision); a thought otherwise.
  const creationNature: NoteType = workspacePage === 'nature' && natureView ? shortcutNature(natureView) ?? 'note'
    : workspacePage === 'notes' && natureFilter ? natureFilter : 'note';
  const handleCreateNote = useCallback(() => {
    if (activeProjectId && workspacePage !== 'nature' && projects.some(project => project.id === activeProjectId && !project.is_inbox)) { void createNoteInProject(activeProjectId, creationNature); return; }
    const activeInbox = workspacePage === 'nature' ? undefined : projects.find(project => project.id === activeProjectId && project.is_inbox);
    const space: ProjectSpace = activeInbox?.space ?? inboxPreviewSpace ?? (spacePreferences.scope === 'private' || spacePreferences.scope === 'work' ? spacePreferences.scope : spacePreferences.usage === 'private' ? 'private' : 'work');
    void createInboxNote(space, creationNature);
  }, [activeProjectId, workspacePage, creationNature, projects, createNoteInProject, createInboxNote, inboxPreviewSpace, spacePreferences.scope, spacePreferences.usage]);

  // Notes, projects and activities written elsewhere (MCP agent, CLI, quick capture): reread as soon
  // as the database fingerprint changes, checked every few seconds and at once on return to the
  // foreground.
  const checkChanges = useWorkspaceChangeWatch({ readFingerprint: api.notesChangeMarker, loadNotes, loadProjects, loadActivities });
  useRefreshOnFocus(checkChanges);

  useQuickCapturePersistenceWatch();

  const changeViewMode = useCallback((mode: NotesViewMode) => { setViewMode(mode); saveViewMode(mode); }, []);
  const { setSearchFocusPending } = useGlobalShortcuts({
    handleCreateNote, workspacePage, projectNotesCount: projectNotes.length, modalOpenRef,
    onSearch: () => openSearchRef.current(), onViewMode: mode => { setWorkspacePage('notes'); changeViewMode(mode); }, setIsCommandPaletteOpen,
  });

  useQuickCaptureShortcut();

  // The active note is resolved **independently of the filter**: the palette and creation can
  // select a note missing from the filtered list (e.g. "Resolved" filter), which must still
  // open in the editor.
  const viewArchived = showArchived && workspacePage === 'notes';
  const searching = searchQuery.trim() !== '';
  // While a search is typed, the cases come best match first, until another sort is chosen for this
  // search; clearing the search returns to the usual sort.
  const [searchSort, setSearchSort] = useState<NoteSort | null>(null);
  const [sortedSearch, setSortedSearch] = useState(searching);
  if (sortedSearch !== searching) {
    setSortedSearch(searching);
    setSearchSort(null);
  }
  const shownSort: NoteSort = searching ? searchSort ?? 'relevance' : noteSort;
  // Search by meaning: the cases closest to the search that the words did not find, and a second
  // opinion on the order of those they found (utils/searchRank).
  const meaningHits = useMeaningHits(workspacePage === 'notes' ? searchQuery : '');
  // Decisions marked as replaced come after the others ("To tidy").
  const supersededIds = useSupersededIds(projectNotes);
  const searchRank = useMemo(() => searchIds ? rankSearch(searchIds, meaningHits, supersededIds) : null, [searchIds, meaningHits, supersededIds]);
  const [meaningStatus] = useMeaningStatus(searching && workspacePage === 'notes');
  // "Today" follows the date: the lists are derived again when the day changes.
  const currentDay = useCurrentDay();
  const activityProjectIds = useMemo(() => activeActivityId ? new Set(visibleProjects.map(project => project.id)) : null, [activeActivityId, visibleProjects]);
  // Every list of the page, computed once per change of the cases or the filters (utils/caseView).
  const caseView = useMemo(() => deriveCaseView({
    notes: projectNotes, visibleProjectIds, archivedProjectIds, activityProjectIds, activeProjectId, activeNoteId,
    query: searchQuery.trim(), searchIds, status: statusFilter, nature: natureFilter, includeSubCases: subCasesShown, today: todayView, showArchived: viewArchived, now: currentDay,
  }), [projectNotes, visibleProjectIds, archivedProjectIds, activityProjectIds, activeProjectId, activeNoteId, searchQuery, searchIds, statusFilter, natureFilter, subCasesShown, todayView, viewArchived, currentDay]);
  const { allScoped, projectCases, spaceCases, topLevelProjectCases, homeCases, scopedNotes, archivedNotesCount, subCaseCounts, todayCases, todayIds, displayedNotes, statusCounts, natureCounts, subCaseTotal, caseById } = caseView;
  const meaningCases = useMemo(() => new Map(projectCases.map(note => [note.id, note])), [projectCases]);
  // No archived note left (the last one was unarchived): back to active notes rather than an empty view.
  // Adjusted during render (not in an effect): React recomputes immediately, without showing the
  // empty view. The state goes back to false and stays there, even if a note is archived afterwards.
  if (showArchived && archivedNotesCount === 0 && initialNotesSettled) setShowArchived(false);
  const activeNote = resolveActiveNote(activeNoteId, [allScoped, scopedNotes]);

  // Quick capture (0.9.83): the displayed note is announced to the capture window, which offers it
  // as a destination. Announced on every change and on request (capture opened afterwards).
  // A drawing is not offered: a text cannot be added to it (the engine refuses what is not a drawing).
  const announcedNote = (workspacePage === 'editor' || isMiniMode) && activeNote && !activeNote.deleted_at && activeNote.content_kind !== 'sketch' ? activeNote : null;
  useOpenNoteAnnouncement(announcedNote?.id ?? null, announcedNote?.title ?? '');

  // The cards page opens without parsing: excerpts are prepared in the idle moments of the window.
  useEffect(() => warmCardExcerpts(allScoped), [allScoped]);
  const notesForView = useMemo(() => viewMode === 'kanban' ? displayedNotes : sortNotes(displayedNotes, shownSort, searchRank), [viewMode, displayedNotes, shownSort, searchRank]);
  const boardRootCases = useMemo(() => sortNotes(topLevelProjectCases, noteSort), [topLevelProjectCases, noteSort]);
  const shortcutCounts = useMemo(() => countShortcuts(spaceCases), [spaceCases]);
  const projectCounts = useMemo(() => unresolvedByProject(projectNotes), [projectNotes]);
  const homeActivities = useMemo(() => activities.filter(activity => spacePreferences.scope === 'all' || activity.space === spacePreferences.scope), [activities, spacePreferences.scope]);
  // A case shown outside its place (a sub-case, or every project at once) says where it lives.
  const projectById = useMemo(() => new Map(projects.map(project => [project.id, project])), [projects]);
  const locate = (note: Note) => note.parent_id || !activeProjectId ? <CaseLocation note={note} byId={caseById} projectById={projectById} /> : null;
  const filtersActive = Boolean(searching || statusFilter || natureFilter || todayView);
  // The words found nothing but "Close in meaning" shows cases: the views say so instead of "none".
  const onlyMeaning = searching && notesForView.length === 0 && meaningRows(meaningHits, meaningCases, searchIds).length > 0;
  const clearFilters = () => { setSearchQuery(''); setStatusFilter(''); setNatureFilter(''); setTodayView(false); };
  // A status or a nature looks for cases wherever they are filed: sub-cases come along.
  const chooseStatus = (status: NoteStatus | '') => { setStatusFilter(status); };
  const chooseNature = (nature: NoteType | '') => { setNatureFilter(nature); };
  const activeProject = scopeProjects.find(project => project.id === activeProjectId);
  const settingsProject = projects.find(project => project.id === projectActivityDialogId);
  const noteProject = projects.find(project => project.id === activeNote?.project_id);
  const structuringContextLabel = noteProject ? [
    noteProject.space === 'work' ? t('common.space.work') : noteProject.space === 'private' ? t('common.space.private') : t('workspace.sidebar.scopeUnassigned'),
    noteProject.name,
    ...activities.filter(activity => activity.space === noteProject.space && activity.project_ids.includes(noteProject.id)).map(activity => activity.name),
  ].join(' · ') : undefined;
  const inboxPreviewTitle = inboxPreviewSpace ? t('common.inboxOf', { space: t(inboxPreviewSpace === 'work' ? 'common.space.work' : 'common.space.private') }) : '';
  const summaryAvailable = Boolean(activeProject && !activeProject.is_inbox && !todayView);
  const showSummary = projectSummary && summaryAvailable;
  const pageTitle = workspacePage === 'nature' && natureView ? t(`workspace.shortcuts.${natureView}`) : workspacePage === 'home' ? t('workspace.sidebar.home') : workspacePage === 'trash' ? t('workspace.sidebar.trash') : workspacePage === 'tidy' ? t('workspace.tidy.title') : workspacePage === 'profile' ? t('workspace.profile.title') : inboxPreviewSpace ? inboxPreviewTitle : todayView ? t('workspace.sidebar.today') : activeProject?.name ?? t('workspace.sidebar.allNotes');
  // What the cases page shows, above its title: the project, the Inbox, the day or the space.
  const casesEyebrow = todayView ? currentDay.toLocaleDateString(currentDateLocale(), { weekday: 'long', day: 'numeric', month: 'long' })
    : activeProject?.is_inbox ? t('workspace.app.freeCapture')
    : activeProject ? t('workspace.app.projectEyebrow')
    : t(spacePreferences.scope === 'private' ? 'workspace.sidebar.scopePrivate' : spacePreferences.scope === 'work' ? 'workspace.sidebar.scopeWork' : spacePreferences.scope === 'all' ? 'workspace.sidebar.scopeAll' : 'workspace.sidebar.scopeUnassigned');
  const casesCount = todayView ? todayCases.length : topLevelProjectCases.length;
  const { moveTargetId, moveParentId, movePending, moveError, chooseTarget, chooseParent, moveNote } = useInboxMove({
    moveScope: workspacePage === 'editor' ? activeNoteId : null,
    prepareMove, spaceEpochRef, setActiveProjectId, loadNotes, loadProjects,
  });
  // The case's project (topbar crumb), or where the case was opened from (back button).
  const backToProject = () => {
    setActiveNoteId(null);
    setTodayView(false);
    setWorkspacePage('notes');
  };
  const goBack = () => {
    setActiveNoteId(null);
    if (!returnTo) { setWorkspacePage('notes'); return; }
    // The page may have changed meanwhile: an activity deleted would hide every project, and the
    // Inbox shown before it existed is now a project of its own (the first capture creates it).
    const activityId = returnTo.activityId && activities.some(activity => activity.id === returnTo.activityId) ? returnTo.activityId : null;
    const inbox = returnTo.inbox ? projects.find(project => project.is_inbox && project.space === returnTo.inbox) : undefined;
    const projectId = inbox?.id ?? returnTo.projectId;
    setActiveActivityId(activityId);
    setInboxPreviewSpace(inbox ? null : returnTo.inbox);
    setActiveProjectId(projectId);
    setLastProjectId(projectId);
    setTodayView(returnTo.today);
    if (returnTo.shortcut) setNatureView(returnTo.shortcut);
    setWorkspacePage(returnTo.page === 'editor' ? 'notes' : returnTo.page);
  };
  const returnProject = returnTo?.projectId ? projects.find(project => project.id === returnTo.projectId) : undefined;
  const returnLabel = !returnTo ? null
    : returnTo.page === 'home' ? t('workspace.sidebar.home')
    : returnTo.page === 'nature' && returnTo.shortcut ? t(`workspace.shortcuts.${returnTo.shortcut}`)
    : returnTo.page === 'trash' ? t('workspace.sidebar.trash')
    : returnTo.page === 'tidy' ? t('workspace.tidy.title')
    : returnTo.page === 'profile' ? t('workspace.profile.title')
    : returnTo.today ? t('workspace.sidebar.today')
    : returnTo.inbox ? t('common.inboxOf', { space: t(returnTo.inbox === 'work' ? 'common.space.work' : 'common.space.private') })
    : returnProject ? (returnProject.is_inbox ? t('workspace.topbar.inbox') : returnProject.name)
    : t('workspace.sidebar.allNotes');
  // In a case, the sidebar keeps the page it was opened from highlighted.
  const navPoint: ReturnPoint = workspacePage === 'editor' && returnTo ? returnTo : { page: workspacePage, projectId: activeProjectId, today: todayView, shortcut: natureView, activityId: activeActivityId, inbox: inboxPreviewSpace };
  const showShortcut = (shortcut: NatureShortcut) => {
    setInboxPreviewSpace(null);
    setActiveNoteId(null);
    setTodayView(false);
    setNatureView(shortcut);
    setWorkspacePage('nature');
    setMobileNavOpen(false);
  };
  // "Today" covers the whole space and narrows the current view; a second click keeps it. It
  // starts from its own rule: an earlier search, status or nature would hide part of it.
  const showToday = () => {
    setSearchQuery('');
    setStatusFilter('');
    setNatureFilter('');
    setActiveNoteId(null);
    setInboxPreviewSpace(null);
    setActiveProjectId(null);
    setLastProjectId(null);
    setTodayView(true);
    setWorkspacePage('notes');
    setBoardPath([]);
    setMobileNavOpen(false);
  };
  // The search looks through the whole space: every case, in the current view.
  const openSearch = () => {
    setActiveNoteId(null);
    setInboxPreviewSpace(null);
    setActiveProjectId(null);
    setTodayView(false);
    setWorkspacePage('notes');
    setBoardPath([]);
    setMobileNavOpen(false);
    setSearchFocusPending(true);
  };
  const openSearchRef = useRef(openSearch);
  useEffect(() => { openSearchRef.current = openSearch; });
  const changeWorkspaceTheme = (next: 'dark' | 'light') => {
    try { localStorage.setItem('humcase-workspace-theme', next); } catch { /* session-only */ }
    setWorkspaceTheme(next);
  };
  const activeSketch = activeNote?.content_kind === 'sketch' ? activeNote : null;
  const toggleWorkspaceTheme = () => changeWorkspaceTheme(workspaceTheme === 'dark' ? 'light' : 'dark');

  return (
    <div className={`workspace-app relative${isMiniMode ? ' workspace-mini' : ''}`} data-theme={workspaceTheme}>
      {globalError && (
        // Non-blocking warning: the application stays usable and the periodic refresh
        // clears the banner as soon as a read succeeds.
        <Toast tone="danger">{t('workspace.app.readError', { error: globalError })}</Toast>
      )}
      {actionError && (
        <Toast tone={toneAction}>{actionError}</Toast>
      )}
      {!isMiniMode && (
        <WorkspaceSidebar
          projects={visibleProjects}
          inboxProjects={inboxProjects}
          activities={activities}
          activeActivityId={activeActivityId}
          onActivity={id => { setActiveActivityId(id); setActiveProjectId(null); setActiveNoteId(null); setWorkspacePage('home'); setMobileNavOpen(false); }}
          onProjectSettings={openProjectSettings}
          archivedProjects={archivedProjects}
          spacePreferences={spacePreferences}
          onScope={changeScope}
          unassignedCount={unassignedCount}
          onAssignProject={assignProject}
          page={navPoint.page}
          activeProjectId={navPoint.projectId}
          todayActive={navPoint.today}
          todayCount={todayCases.length}
          mobileOpen={mobileNavOpen}
          onCloseMobile={() => setMobileNavOpen(false)}
          onHome={goHome}
          onAllNotes={() => selectProject(null)}
          onToday={showToday}
          onProject={id => selectProject(id)}
          onInbox={space => { setTodayView(false); setInboxPreviewSpace(space); setActiveProjectId(null); setActiveNoteId(null); setWorkspacePage('notes'); setBoardPath([]); setMobileNavOpen(false); }}
          inboxPreview={navPoint.page === 'notes' ? navPoint.inbox : null}
          onNewProject={() => { openProjectDialog(); setMobileNavOpen(false); }}
          onSearch={openSearch}
          onSettings={() => { setFocusActivitySettings(false); setIsSettingsOpen(true); setMobileNavOpen(false); }}
          onTrash={goTrash}
          shortcutCounts={shortcutCounts}
          projectCounts={projectCounts}
          activeShortcut={navPoint.shortcut ?? (navPoint.page === 'nature' ? natureView : null)}
          onShortcut={showShortcut}
        />
      )}
      <div className="workspace-main">
        {!isMiniMode && (
          <WorkspaceTopbar
            workspacePage={workspacePage}
            noteProject={noteProject}
            activeProject={activeProject}
            activeNoteTitle={activeNote?.title}
            pageTitle={pageTitle}
            briefPending={briefPending}
            workspaceTheme={workspaceTheme}
            onOpenNav={() => setMobileNavOpen(true)}
            onBackToNotes={backToProject}
            onAllNotes={() => selectProject(null)}
            onPathSlot={setTopbarPathSlot}
            showBriefs={Boolean(activeProject && !activeProject.is_inbox && !todayView && (workspacePage === 'notes' || workspacePage === 'editor'))}
            onHandoff={openHandoff}
            onStandup={openStandup}
            onToggleTheme={toggleWorkspaceTheme}
            onToggleMini={toggleMiniMode}
          />
        )}
        {/* A sub-case follows its parent: only a top-level case of the Inbox is filed. */}
        {activeNote && !activeNote.parent_id && activeProject?.is_inbox && !isMiniMode && workspacePage === 'editor' && <InboxPlacement
          projects={projects}
          inboxProject={activeProject}
          targetId={moveTargetId}
          onTargetChange={chooseTarget}
          cases={allScoped}
          parentId={moveParentId}
          onParentChange={chooseParent}
          pending={movePending}
          error={moveError}
          onMove={() => void moveNote(activeNote, activeProject)}
        />}
        {isMiniMode && (
          <div className="workspace-topbar"><strong>Humcase</strong><Hint label={t('workspace.app.backToNormal')}><Button variant="ghost" size="icon-sm" className="workspace-icon-action" onClick={toggleMiniMode} aria-label={t('workspace.app.backToNormal')}><Maximize2 size={18} /></Button></Hint></div>
        )}
        {/* The editor stays mounted (write safeguards); when visible, it is the main region. */}
        <div className="workspace-editor-page" role={workspacePage === 'editor' || (isMiniMode && activeNoteId) ? 'main' : undefined} aria-label={t('workspace.app.noteRegion')} style={workspacePage === 'editor' || (isMiniMode && activeNoteId) ? undefined : { display: 'none' }}>
          {workspacePage === 'editor' && !isMiniMode && <Button variant="ghost" size="sm" className="workspace-editor-back" onClick={goBack}><ArrowLeft size={16} />{returnLabel ? t('workspace.app.backTo', { label: returnLabel }) : t('workspace.app.backToNotes')}</Button>}
          <div className="workspace-editor-slot">
            {activeSketch && (workspacePage === 'editor' || isMiniMode) && (
              <SketchEditor key={activeSketch.id} note={activeSketch} theme={workspaceTheme} projects={projects}
                onNoteUpdated={() => { void loadNotes(); }} onRegisterPrepareBackup={registerSketchPreparation} />
            )}
            {/* A sketch is never opened in the text editor: its Markdown would replace the drawing. */}
            <div style={{ display: activeSketch ? 'none' : 'contents' }}>
              <NoteEditor
                note={activeSketch ? null : activeNote}
                structuringContextLabel={structuringContextLabel}
                onRegisterPrepareBackup={registerEditorPreparation}
                onNoteUpdated={() => { void loadNotes(); void loadProjects(); }}
                isMiniMode={isMiniMode}
                pathSlot={workspacePage === 'editor' && !isMiniMode ? topbarPathSlot : null}
                allNotes={projectCases}
                projects={projects}
                onOpenRelatedNote={async target => { await loadNotes(); openNote(target); }}
                germinateOnOpen={workspacePage === 'editor' && germinateNoteId === activeNoteId ? germinateNoteId : null}
                onGerminateStarted={clearGerminateNoteId}
                focusOnOpen={workspacePage === 'editor' && focusNoteId !== null && focusNoteId === activeNoteId}
                onBackToList={goBack}
              />
            </div>
          </div>
        </div>
        {isMiniMode ? (
          !activeNoteId && (!(initialProjectsSettled && initialNotesSettled) ? <div className="workspace-loading" role="status">{t('workspace.app.loadingNotes')}</div> : <div className="workspace-mini-list">
            <NoteList
              notes={displayedNotes}
              activeNoteId={activeNoteId}
              onSelectNote={id => { setActiveNoteId(id); setWorkspacePage('editor'); }}
              onCreateNote={handleCreateNote}
              searchQuery={searchQuery}
              onSearchQueryChange={setSearchQuery}
              statusFilter={statusFilter}
              onStatusFilterChange={setStatusFilter}
              isMiniMode
            />
          </div>)
        ) : (
          <>
            {workspacePage === 'home' && <HomePage ready={initialProjectsSettled && initialNotesSettled}
              unassignedCount={spacePreferences.scope === 'unassigned' ? 0 : unassignedCount} onViewUnassigned={() => changeScope('unassigned')}
              activities={homeActivities} activeActivityId={activeActivityId} onActivity={id => { setActiveActivityId(id); setActiveProjectId(null); setActiveNoteId(null); }}
              projects={visibleProjects} locateProjects={projects} notes={homeCases} allNotes={spaceCases} onNote={note => openNote(note)} onSeeAllFollowup={showToday}
              onCreateProject={openProjectDialog} onCreateNote={handleCreateNote} subCaseCounts={subCaseCounts} onNotesChanged={() => { void loadNotes(); }} onTidy={goTidy}
              onConnectAgent={() => { setFocusAgentSettings(true); setIsSettingsOpen(true); }} onDataChanged={() => { void loadProjects(); void loadNotes(); }}
              profileSpace={profileSpace} profileSpaces={profileSpaces} onProfile={() => goProfile()} onGerminate={id => { const note = spaceCases.find(candidate => candidate.id === id); if (note && openNote(note)) setGerminateNoteId(id); }}
              inviteActivities={activities.length === 0} onCustomize={() => { setFocusActivitySettings(true); setIsSettingsOpen(true); }} />}
            {workspacePage === 'notes' && inboxPreviewSpace && <InboxPreview title={inboxPreviewTitle} onCreate={handleCreateNote} />}
            {workspacePage === 'nature' && natureView && <div className="workspace-scroll"><NatureView shortcut={natureView} notes={spaceCases} projects={projects} onNote={note => openNote(note)} onCreate={handleCreateNote} /></div>}
            {workspacePage === 'tidy' && <div className="workspace-scroll"><TidyView notes={spaceCases} onNote={note => openNote(note)} onChanged={() => { void loadNotes(); }} /></div>}
            {workspacePage === 'profile' && <div className="workspace-scroll"><ProfilePage space={profileSpace} spaces={profileSpaces} onSpace={setProfileSpaceChoice} notes={spaceCases} onNote={note => openNote(note)} onEdit={editProfile} /></div>}
            {workspacePage === 'trash' && <TrashView projects={projects} onChanged={() => void loadNotes()} onMessage={reportAction} />}
            {workspacePage === 'notes' && !inboxPreviewSpace && <div className="workspace-scroll workspace-cases-page">
              {/* One header for the four views: the title, its count and "New case" always at its right. */}
              <PageHeader className="workspace-cases-page__header" eyebrow={casesEyebrow} title={pageTitle} count={casesCount} countLabel={t('workspace.notes.count', { count: casesCount })}
                action={<>
                  {summaryAvailable && <SegmentedControl label={t('workspace.summary.switch')} size="sm" value={showSummary ? 'summary' : 'cases'}
                    onValueChange={value => { setProjectSummary(value === 'summary'); saveProjectSummary(value === 'summary'); }}
                    items={[{ value: 'cases', label: t('workspace.summary.cases') }, { value: 'summary', label: t('workspace.summary.title') }]} />}
                  {!showSummary && <NotesViewbar
                    viewMode={viewMode}
                    onViewModeChange={changeViewMode}
                    noteSort={shownSort}
                    onNoteSortChange={searching ? setSearchSort : setNoteSort}
                    searching={searching}
                    archivedNotesCount={archivedNotesCount}
                    showArchived={showArchived}
                    onToggleArchived={() => setShowArchived(current => !current)}
                  />}
                  <NewCaseButton onClick={viewMode === 'plan' && canvasCreate ? canvasCreate.create : handleCreateNote} label={viewMode === 'plan' ? canvasCreate?.label : undefined} />
                </>} />
              {showSummary && activeProject ? <ProjectSummaryView projectId={activeProject.id} notes={spaceCases} onNote={note => openNote(note)} onTidy={goTidy} /> : <>
              <CaseFilterBar
                query={searchQuery}
                onQueryChange={setSearchQuery}
                searchPlaceholder={activeProject && !todayView ? t('workspace.filters.searchIn', { project: activeProject.is_inbox ? t('workspace.topbar.inbox') : activeProject.name }) : undefined}
                status={isNoteStatus(statusFilter) ? statusFilter : ''}
                onStatusChange={chooseStatus}
                statusCounts={statusCounts}
                nature={natureFilter}
                onNatureChange={chooseNature}
                natureCounts={natureCounts}
                subCases={viewMode === 'plan' || todayView ? null : subCasesShown}
                subCaseCount={subCaseTotal}
                onSubCasesChange={subCases.choose}
                today={todayView}
                onLeaveToday={() => setTodayView(false)}
                onClear={clearFilters}
              />
              {searching && initialNotesSettled && (
                <MeaningResults hits={meaningHits} status={meaningStatus} cases={meaningCases} wordIds={searchIds}
                  onOpen={note => openNote(note)} locate={locate} />
              )}
              {/* First read in progress: a loading placeholder, never a fake empty state. */}
              {!(initialProjectsSettled && initialNotesSettled) ? (
                <div className="workspace-loading" role="status">{t('workspace.app.loadingNotes')}</div>
              ) : todayView && todayCases.length === 0 && !searching && !statusFilter && !natureFilter ? (
                // One message for every view: an empty board or canvas does not say what Today holds.
                <div className="workspace-page workspace-notes"><section className="workspace-section">
                  <EmptyState icon={<CalendarDots size={20} />} headingLevel={2} title={t('workspace.app.todayEmptyTitle')} description={t('workspace.app.todayEmptyText')} />
                </section></div>
              ) : viewMode === 'plan' ? (
                <CaseBoard
                  key={activeProjectId ?? 'all'}
                  rootCases={boardRootCases}
                  allNotes={projectCases}
                  query={searchQuery}
                  status={isNoteStatus(statusFilter) ? statusFilter : null}
                  nature={natureFilter || null}
                  focusIds={todayView ? todayIds : null}
                  searchIds={searchIds}
                  projectId={activeProjectId}
                  creationProjectId={activeProjectId && projects.some(project => project.id === activeProjectId && !project.is_inbox) ? activeProjectId : null}
                  path={boardPath}
                  onPathChange={setBoardPath}
                  onOpenNote={openNote}
                  onCreateRootCase={handleCreateNote}
                  onRegisterCreate={setCanvasCreate}
                  onChanged={() => { void loadNotes(); }}
                />
              ) : onlyMeaning && viewMode !== 'kanban' ? (
                <p className="workspace-meaning__no-words" role="status">{t('workspace.meaning.noWords')}</p>
              ) : viewMode === 'cards' ? (
                <WorkspaceNotes
                  notes={notesForView}
                  onNote={openNote}
                  onCreateNote={handleCreateNote}
                  totalCount={todayView ? todayCases.length : topLevelProjectCases.length}
                  filtered={filtersActive}
                  onClearFilters={clearFilters}
                  subCaseCounts={subCaseCounts}
                  locate={locate}
                  firstNoteTitle={activeProject ? undefined : t('workspace.home.emptyNoneTitle')}
                />
              ) : viewMode === 'list' ? (
                <div className="workspace-legacy-view"><NoteList
                  notes={notesForView}
                  activeNoteId={activeNoteId}
                  onSelectNote={id => { const target = allScoped.find(note => note.id === id); if (target) openNote(target); }}
                  onCreateNote={handleCreateNote}
                  searchQuery={searchQuery}
                  onSearchQueryChange={setSearchQuery}
                  statusFilter={statusFilter}
                  onStatusFilterChange={setStatusFilter}
                  hideFilters
                  locate={locate}
                  emptyMessage={filtersActive ? t('workspace.filters.noneFound') : undefined}
                /></div>
              ) : (
                <div className="workspace-legacy-view"><KanbanBoard
                  notes={displayedNotes}
                  activeNoteId={activeNoteId}
                  onSelectNote={id => { const target = allScoped.find(note => note.id === id); if (target) openNote(target); }}
                  onNotesUpdate={loadNotes}
                /></div>
              )}
              </>}
            </div>}
          </>
        )}
      </div>

      {projectDialogOpen && <ProjectCreateDialog
        name={projectDraftName}
        onNameChange={changeDraftName}
        space={projectDraftSpace}
        onSpaceChange={changeDraftSpace}
        selectedActivities={projectDraftActivities}
        onToggleActivity={toggleDraftActivity}
        activities={activities}
        usage={spacePreferences.usage}
        error={projectDraftError}
        pending={projectPending}
        onClose={closeProjectDialog}
        onSubmit={() => void createProject()}
      />}
      {noteTargetDialogOpen && <NoteTargetDialog
        projects={visibleProjects}
        targetId={noteTargetId}
        onTargetChange={setNoteTargetId}
        pending={noteTargetPending}
        onClose={closeNoteTargetDialog}
        onSubmit={async () => { setNoteTargetPending(true); await createNoteInProject(noteTargetId); setNoteTargetPending(false); }}
      />}
      {projectActivityDialogId && settingsProject && <ProjectSettingsDialog
        project={settingsProject}
        activities={activities}
        deleteConfirm={projectDeleteConfirm}
        nameDraft={projectNameDraft}
        onNameChange={changeProjectName}
        colorDraft={projectColorDraft}
        onColorChange={setProjectColorDraft}
        activityDraft={projectActivityDraft}
        onToggleActivity={toggleProjectActivity}
        error={projectActivityError}
        pending={projectActivityPending}
        onClose={closeProjectActivityDialog}
        onSave={() => void saveProjectSettings()}
        onToggleArchived={() => void toggleProjectArchived()}
        onAskDelete={askDeleteProject}
        onCancelDelete={cancelDeleteProject}
        onDelete={() => void deleteProject()}
      />}
      {isSettingsOpen && <SettingsModal onClose={() => { setIsSettingsOpen(false); setFocusAiSettings(false); setFocusAgentSettings(false); }} focusAgent={focusAgentSettings} prepareBackup={prepareBackup} spacePreferences={spacePreferences} onSpaceUsage={changeSpaceUsage} onCombinedView={changeCombinedView} activities={activities} onCreateActivity={createActivity} onRenameActivity={renameActivity} onDeleteActivity={deleteActivity} focusActivities={focusActivitySettings} focusAi={focusAiSettings} theme={workspaceTheme} onThemeChange={changeWorkspaceTheme} onOpenProfile={space => { setIsSettingsOpen(false); goProfile(space); }} />}

      {(showStandup || showHandoff) && (
        <BriefDialog
          kind={showHandoff ? 'handoff' : 'standup'}
          text={showHandoff ? handoffText : standupText}
          copyError={copyError}
          onClose={closeTextModal}
          onCopy={() => void copyAndClose()}
        />
      )}

      <CommandPalette
        isOpen={isCommandPaletteOpen}
        onClose={() => setIsCommandPaletteOpen(false)}
        projects={visibleProjects}
        notes={allScoped}
        onSelectProject={(id) => {
          selectProject(id);
          setActiveNoteId(null);
        }}
        onSelectNote={(id) => {
          const note = allScoped.find(n => n.id === id) ?? scopedNotes.find(n => n.id === id);
          if (note) openNote(note);
          else void api.getNote(id).then(openNote).catch(() => reportAction(t('workspace.app.noteNotFound')));
        }}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />
    </div>
  );
}

export default App;

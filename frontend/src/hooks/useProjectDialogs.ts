import { useCallback, useState, type Dispatch, type SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../services/tauriApi';
import { setLastProjectId } from '../utils/prefs';
import { normalizeSpacePreferences, type ProjectSpace, type SpacePreferences } from '../utils/spaces';
import type { Activity, Project, WorkspacePage } from '../types';

interface UseProjectDialogsOptions {
  projects: Project[];
  activities: Activity[];
  setActivities: Dispatch<SetStateAction<Activity[]>>;
  activeActivityId: string | null;
  refreshActivities: () => void;
  reportAction: (message: string, tone?: 'danger' | 'success') => void;
  loadProjects: () => Promise<void>;
  loadNotes: () => Promise<void>;
  activeProjectId: string | null;
  setActiveProjectId: Dispatch<SetStateAction<string | null>>;
  setActiveNoteId: Dispatch<SetStateAction<string | null>>;
  setWorkspacePage: Dispatch<SetStateAction<WorkspacePage>>;
  selectProject: (id: string | null) => void;
  spacePreferences: SpacePreferences;
  applySpacePreferences: (next: SpacePreferences) => void;
}

/**
 * "New project" and "Project" windows (name, color, activities, archiving, deletion).
 * Writes of a project are saved separately; the state of the windows lives here.
 */
export function useProjectDialogs({
  projects, activities, setActivities, activeActivityId, refreshActivities, reportAction, loadProjects, loadNotes,
  activeProjectId, setActiveProjectId, setActiveNoteId, setWorkspacePage, selectProject, spacePreferences, applySpacePreferences,
}: UseProjectDialogsOptions) {
  const { t } = useTranslation();
  const [projectDialogOpen, setProjectDialogOpen] = useState(false);
  const [projectDraftName, setProjectDraftName] = useState('');
  const [projectDraftError, setProjectDraftError] = useState<string | null>(null);
  const [projectPending, setProjectPending] = useState(false);
  const [projectDraftSpace, setProjectDraftSpace] = useState<ProjectSpace>('private');
  const [projectDraftActivities, setProjectDraftActivities] = useState<string[]>([]);
  const [projectActivityDialogId, setProjectActivityDialogId] = useState<string | null>(null);
  const [projectActivityDraft, setProjectActivityDraft] = useState<string[]>([]);
  const [projectActivityBaseline, setProjectActivityBaseline] = useState<string[]>([]);
  const [projectActivityPending, setProjectActivityPending] = useState(false);
  const [projectActivityError, setProjectActivityError] = useState<string | null>(null);
  // "Project" window (opened by `projectActivityDialogId`): name, color, archiving, deletion.
  const [projectNameDraft, setProjectNameDraft] = useState('');
  const [projectColorDraft, setProjectColorDraft] = useState('');
  const [projectDeleteConfirm, setProjectDeleteConfirm] = useState(false);

  // Opening "New project": the proposed space follows the usage, the active activity is pre-checked.
  const openProjectDialog = () => {
    setProjectDraftSpace(spacePreferences.scope === 'work' || spacePreferences.usage === 'work' ? 'work' : 'private');
    setProjectDraftActivities(activeActivityId ? [activeActivityId] : []);
    setProjectDialogOpen(true);
  };

  const closeProjectDialog = useCallback(() => {
    if (projectPending) return;
    setProjectDialogOpen(false);
    setProjectDraftError(null);
  }, [projectPending]);
  const closeProjectActivityDialog = useCallback(() => { if (!projectActivityPending) { setProjectActivityDialogId(null); setProjectActivityError(null); } }, [projectActivityPending]);
  const openProjectSettings = useCallback((project: Project) => {
    if (project.is_inbox) return;
    const ids = project.space ? activities.filter(activity => activity.space === project.space && activity.project_ids.includes(project.id)).map(activity => activity.id) : [];
    setProjectActivityDraft(ids);
    setProjectActivityBaseline(ids);
    setProjectNameDraft(project.name);
    setProjectColorDraft(project.color);
    setProjectDeleteConfirm(false);
    setProjectActivityDialogId(project.id);
    setProjectActivityError(null);
  }, [activities]);
  const saveProjectSettings = useCallback(async () => {
    const project = projects.find(item => item.id === projectActivityDialogId);
    if (!project || projectActivityPending) return;
    const name = projectNameDraft.trim();
    if (!name) { setProjectActivityError(t('workspace.projectDialogs.nameRequired')); return; }
    setProjectActivityPending(true);
    setProjectActivityError(null);
    // Each setting is saved separately: on failure, the message says which one.
    let step = t('workspace.projectDialogs.stepRename');
    try {
      if (name !== project.name) await api.renameProject(project.id, name);
      step = t('workspace.projectDialogs.stepColor');
      if (projectColorDraft !== project.color) await api.setProjectColor(project.id, projectColorDraft);
      const changedActivities = projectActivityDraft.length !== projectActivityBaseline.length || projectActivityDraft.some(id => !projectActivityBaseline.includes(id));
      step = t('workspace.projectDialogs.stepAreas');
      if (project.space && changedActivities) {
        await api.setProjectActivities(project.id, projectActivityDraft, projectActivityBaseline);
        setActivities(current => current.map(activity => ({ ...activity, project_ids: projectActivityDraft.includes(activity.id) ? [...new Set([...activity.project_ids, project.id])] : activity.project_ids.filter(id => id !== project.id) })));
        refreshActivities();
      }
      setProjectActivityDialogId(null);
    } catch (cause) {
      // The engine already returns its message in the interface language (e.g. name already taken).
      const message = String(cause);
      setProjectActivityError(t('workspace.projectDialogs.saveError', { step: step, message }));
    } finally {
      setProjectActivityPending(false);
      void loadProjects();
    }
  }, [projects, projectActivityDialogId, projectActivityPending, projectNameDraft, projectColorDraft, projectActivityDraft, projectActivityBaseline, setActivities, refreshActivities, loadProjects, t]);
  const toggleProjectArchived = useCallback(async () => {
    const project = projects.find(item => item.id === projectActivityDialogId);
    if (!project || projectActivityPending) return;
    const archive = !project.archived_at;
    setProjectActivityPending(true);
    setProjectActivityError(null);
    try {
      await api.setProjectArchived(project.id, archive);
      setProjectActivityDialogId(null);
      reportAction(t(archive ? 'workspace.projectDialogs.archived' : 'workspace.projectDialogs.unarchived', { name: project.name }), 'success');
    } catch (cause) {
      setProjectActivityError(t(archive ? 'workspace.projectDialogs.archiveError' : 'workspace.projectDialogs.unarchiveError', { error: String(cause) }));
    } finally {
      setProjectActivityPending(false);
      void loadProjects();
    }
  }, [projects, projectActivityDialogId, projectActivityPending, reportAction, loadProjects, t]);
  const deleteProject = useCallback(async () => {
    const project = projects.find(item => item.id === projectActivityDialogId);
    if (!project || projectActivityPending) return;
    setProjectActivityPending(true);
    setProjectActivityError(null);
    try {
      const moved = await api.deleteProject(project.id);
      setProjectActivityDialogId(null);
      if (activeProjectId === project.id) { setActiveProjectId(null); setActiveNoteId(null); setWorkspacePage('home'); }
      reportAction(moved === 0 ? t('workspace.projectDialogs.deleted', { name: project.name }) : t('workspace.projectDialogs.deletedMoved', { name: project.name, count: moved }), 'success');
      void loadNotes();
      refreshActivities();
    } catch (cause) {
      setProjectActivityError(t('workspace.projectDialogs.deleteError', { error: String(cause) }));
    } finally {
      setProjectActivityPending(false);
      void loadProjects();
    }
  }, [projects, projectActivityDialogId, projectActivityPending, activeProjectId, setActiveProjectId, setActiveNoteId, setWorkspacePage, reportAction, loadNotes, loadProjects, refreshActivities, t]);

  const createProject = useCallback(async () => {
    const name = projectDraftName.trim();
    if (!name || projectPending) {
      if (!name) setProjectDraftError(t('workspace.projectDialogs.createNameRequired'));
      return;
    }
    setProjectPending(true);
    setProjectDraftError(null);
    try {
      const id = await api.createProject(name, undefined, projectDraftSpace);
      if (projectDraftActivities.length) {
        try { await api.setProjectActivities(id, projectDraftActivities, []); setActivities(current => current.map(activity => projectDraftActivities.includes(activity.id) ? { ...activity, project_ids: [...new Set([...activity.project_ids, id])] } : activity)); refreshActivities(); }
        catch (cause) { reportAction(t('workspace.projectDialogs.areasNotSaved', { error: String(cause) })); }
      }
      await loadProjects();
      setProjectDraftName('');
      setProjectDraftActivities([]);
      setProjectDialogOpen(false);
      if (spacePreferences.scope === projectDraftSpace || spacePreferences.scope === 'all') selectProject(id);
      else {
        applySpacePreferences(normalizeSpacePreferences({ ...spacePreferences, scope: projectDraftSpace }));
        setActiveProjectId(id);
        setLastProjectId(id);
        setWorkspacePage('notes');
      }
    } catch (e) {
      setProjectDraftError(t('workspace.projectDialogs.createError', { error: String(e) }));
    } finally {
      setProjectPending(false);
    }
  }, [projectDraftName, projectPending, projectDraftSpace, projectDraftActivities, setActivities, loadProjects, refreshActivities, selectProject, spacePreferences, applySpacePreferences, setActiveProjectId, setWorkspacePage, reportAction, t]);

  // "New project" inputs: any change clears the error or the activities that became invalid.
  const changeDraftName = (name: string) => { setProjectDraftName(name); setProjectDraftError(null); };
  const changeDraftSpace = (space: ProjectSpace) => { setProjectDraftSpace(space); setProjectDraftActivities([]); };
  const toggleDraftActivity = (id: string) => setProjectDraftActivities(current => current.includes(id) ? current.filter(item => item !== id) : [...current, id]);
  // Inputs of the "Project" window.
  const changeProjectName = (name: string) => { setProjectNameDraft(name); setProjectActivityError(null); };
  const toggleProjectActivity = (id: string) => setProjectActivityDraft(current => current.includes(id) ? current.filter(item => item !== id) : [...current, id]);
  const askDeleteProject = () => { setProjectDeleteConfirm(true); setProjectActivityError(null); };
  const cancelDeleteProject = () => { setProjectDeleteConfirm(false); setProjectActivityError(null); };

  return {
    projectDialogOpen, projectDraftName, projectDraftError, projectPending, projectDraftSpace, projectDraftActivities,
    projectActivityDialogId, projectActivityDraft, projectActivityPending, projectActivityError,
    projectNameDraft, projectColorDraft, setProjectColorDraft, projectDeleteConfirm,
    openProjectDialog, closeProjectDialog, closeProjectActivityDialog, openProjectSettings,
    saveProjectSettings, toggleProjectArchived, deleteProject, createProject,
    changeDraftName, changeDraftSpace, toggleDraftActivity, changeProjectName, toggleProjectActivity, askDeleteProject, cancelDeleteProject,
  };
}

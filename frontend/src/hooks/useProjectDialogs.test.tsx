import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useState } from 'react';
import { useProjectDialogs } from './useProjectDialogs';
import { api } from '../services/tauriApi';
import { normalizeSpacePreferences } from '../utils/spaces';
import type { Activity, Project } from '../types';

vi.mock('../services/tauriApi', () => ({
  api: {
    deleteProject: vi.fn(),
    setProjectArchived: vi.fn(),
    renameProject: vi.fn(),
    setProjectColor: vi.fn(),
    setProjectActivities: vi.fn(),
    createProject: vi.fn(),
  },
}));

const mockApi = vi.mocked(api);
const project: Project = { id: 'p1', name: 'Site vitrine', color: '#3b82f6', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0, space: 'work' };

function mountHook(options: { activeProjectId?: string | null } = {}) {
  const reportAction = vi.fn();
  const loadNotes = vi.fn(async () => {});
  const loadProjects = vi.fn(async () => {});
  const refreshActivities = vi.fn();
  const setActiveProjectId = vi.fn();
  const setActiveNoteId = vi.fn();
  const setWorkspacePage = vi.fn();
  const rendered = renderHook(() => {
    const [activities, setActivities] = useState<Activity[]>([]);
    return useProjectDialogs({
      projects: [project], activities, setActivities, activeActivityId: null, refreshActivities, reportAction, loadProjects, loadNotes,
      activeProjectId: options.activeProjectId ?? null, setActiveProjectId, setActiveNoteId, setWorkspacePage,
      selectProject: vi.fn(), spacePreferences: normalizeSpacePreferences({}), applySpacePreferences: vi.fn(),
    });
  });
  return { ...rendered, reportAction, loadNotes, loadProjects, refreshActivities, setActiveProjectId, setActiveNoteId, setWorkspacePage };
}

describe('project deletion', () => {
  beforeEach(() => vi.clearAllMocks());

  it.each([
    [0, '« Site vitrine » est supprimé.'],
    [1, '« Site vitrine » est supprimé : 1 case a rejoint la Réception.'],
    [3, '« Site vitrine » est supprimé : 3 cases ont rejoint la Réception.'],
  ])('announces %i note(s) sent to the Inbox', async (moved, message) => {
    mockApi.deleteProject.mockResolvedValue(moved);
    const hook = mountHook();
    act(() => hook.result.current.openProjectSettings(project));
    expect(hook.result.current.projectActivityDialogId).toBe('p1');
    await act(async () => { await hook.result.current.deleteProject(); });
    expect(hook.reportAction).toHaveBeenCalledWith(message, 'success');
    expect(hook.result.current.projectActivityDialogId).toBeNull();
    expect(hook.loadNotes).toHaveBeenCalledTimes(1);
    expect(hook.loadProjects).toHaveBeenCalledTimes(1);
    expect(hook.refreshActivities).toHaveBeenCalledTimes(1);
  });

  it('leaves the deleted project when it was open', async () => {
    mockApi.deleteProject.mockResolvedValue(2);
    const hook = mountHook({ activeProjectId: 'p1' });
    act(() => hook.result.current.openProjectSettings(project));
    await act(async () => { await hook.result.current.deleteProject(); });
    expect(hook.setActiveProjectId).toHaveBeenCalledWith(null);
    expect(hook.setActiveNoteId).toHaveBeenCalledWith(null);
    expect(hook.setWorkspacePage).toHaveBeenCalledWith('home');
  });

  it('keeps the window open and states that no note was erased on failure', async () => {
    mockApi.deleteProject.mockRejectedValue(new Error('boom'));
    const hook = mountHook();
    act(() => hook.result.current.openProjectSettings(project));
    await act(async () => { await hook.result.current.deleteProject(); });
    expect(hook.result.current.projectActivityDialogId).toBe('p1');
    expect(hook.result.current.projectActivityError).toMatch(/Suppression impossible : .*boom.*Aucune case n’a été effacée\./);
    expect(hook.reportAction).not.toHaveBeenCalled();
    expect(hook.loadProjects).toHaveBeenCalledTimes(1);
  });
});

describe('Project window', () => {
  beforeEach(() => vi.clearAllMocks());

  it('refuses an empty name without saving anything', async () => {
    const hook = mountHook();
    act(() => hook.result.current.openProjectSettings(project));
    act(() => hook.result.current.changeProjectName('   '));
    await act(async () => { await hook.result.current.saveProjectSettings(); });
    expect(hook.result.current.projectActivityError).toBe('Le projet doit avoir un nom.');
    expect(mockApi.renameProject).not.toHaveBeenCalled();
  });

  it('names the failing step and translates the name duplicate', async () => {
    mockApi.renameProject.mockRejectedValue('un autre projet porte déjà ce nom');
    const hook = mountHook();
    act(() => hook.result.current.openProjectSettings(project));
    act(() => hook.result.current.changeProjectName('Autre nom'));
    await act(async () => { await hook.result.current.saveProjectSettings(); });
    expect(hook.result.current.projectActivityError).toMatch(/^Renommage impossible : un autre projet porte déjà ce nom\./);
    expect(hook.result.current.projectActivityDialogId).toBe('p1');
  });

  it('announces archiving with the success tone', async () => {
    mockApi.setProjectArchived.mockResolvedValue(undefined);
    const hook = mountHook();
    act(() => hook.result.current.openProjectSettings(project));
    await act(async () => { await hook.result.current.toggleProjectArchived(); });
    expect(mockApi.setProjectArchived).toHaveBeenCalledWith('p1', true);
    expect(hook.reportAction).toHaveBeenCalledWith('« Site vitrine » est archivé : retrouvez-le sous « Archivés », ses cases sont conservées.', 'success');
  });

  it('does not open the window for the Inbox', () => {
    const hook = mountHook();
    act(() => hook.result.current.openProjectSettings({ ...project, id: 'inbox', is_inbox: true }));
    expect(hook.result.current.projectActivityDialogId).toBeNull();
  });
});

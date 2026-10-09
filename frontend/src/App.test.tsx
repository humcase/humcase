import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import App from './App';
import { api } from './services/tauriApi';
import type { Activity, Note, Project } from './types';
import { addPendingWrite, clearPendingWrites } from './utils/pendingWrites';

const notesEnBase = vi.hoisted(() => ({ current: [] as Note[] }));
const storedProjects = vi.hoisted(() => ({ current: [] as Project[] }));
const storedActivities = vi.hoisted(() => ({ current: [] as Activity[] }));
const focusListener = vi.hoisted(() => ({
  current: null as null | ((event: { payload: boolean }) => void),
}));

vi.mock('./services/tauriApi', () => ({
  api: {
    getProjects: vi.fn(),
    getActivities: vi.fn(),
    createActivity: vi.fn(),
    renameActivity: vi.fn(),
    deleteActivity: vi.fn(),
    setProjectActivities: vi.fn(),
    getOrCreateInbox: vi.fn(),
    moveNoteToProject: vi.fn(),
    getHandoff: vi.fn(),
    getNotes: vi.fn(),
    getNoteRelations: vi.fn(),
    getCaseContext: vi.fn(),
    getCasePositions: vi.fn(),
    setCasePosition: vi.fn(),
    createSubCase: vi.fn(),
    moveCase: vi.fn(),
    createProject: vi.fn(),
    createNote: vi.fn(),
    listAiProviders: vi.fn(async () => ({ cleanup: [], transcription: [] })),
    updateProjectSpace: vi.fn(),
    renameProject: vi.fn(),
    setProjectColor: vi.fn(),
    setProjectArchived: vi.fn(),
    deleteProject: vi.fn(),
    setNoteArchived: vi.fn(),
    getTrashedNotes: vi.fn(),
    restoreFromTrash: vi.fn(),
    notesChangeMarker: vi.fn(async () => 'stable'),
    meaningStatus: vi.fn(async () => ({ state: 'ready', done: 0, total: 0, error: null })),
    searchByMeaning: vi.fn(async () => []),
    assistantStatus: vi.fn(async () => ({ model_label: 'Claude Haiku 5.5', cases: 3, to_decide: 0, to_tidy: 0 })),
    assistantReply: vi.fn(),
  },
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onFocusChanged: async (listener: (event: { payload: boolean }) => void) => {
      focusListener.current = listener;
      return () => { focusListener.current = null; };
    },
    onScaleChanged: async () => () => {},
    onCloseRequested: async () => () => {},
  }),
  LogicalSize: class {},
  PhysicalSize: class {},
}));

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
  register: async () => {},
  unregister: async () => {},
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: { getByLabel: async () => null },
}));

// Listening to the Quick Capture persistence state (M2) must not try to reach Tauri in tests.
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}));

vi.mock('./utils/prefs', () => ({
  // Home's first steps are covered by WelcomeGuide.test: these tests see the usual home.
  getWelcomeHidden: () => true,
  setWelcomeHidden: () => {},
  getExampleProjectId: () => null,
  setExampleProjectId: () => {},
  MIN_FONT_SCALE: 0.75,
  MAX_FONT_SCALE: 1.5,
  getEditorFontScale: () => 1,
  setEditorFontScale: (scale: number) => scale,
  getAiCleanProvider: () => 'cerebras',
  getAiSttProvider: () => 'deepgram',
  getAiModel: () => '',
  getDictationAutoStructure: () => true,
  setDictationAutoStructure: () => {},
  getDictationLanguage: () => 'interface',
  setDictationLanguage: () => {},
  dictationLanguageCode: (preference: string, language: string) => preference === 'interface' ? language : preference,
  DICTATION_LANGUAGES: [],
  setAiCleanProvider: () => {},
  setAiSttProvider: () => {},
  setAiModel: () => {},
  getLastProjectId: () => 'p1',
  setLastProjectId: () => {},
  getMiniMode: () => false,
  getMiniSize: () => ({ width: 400, height: 500 }),
  setMiniMode: () => {},
  setMiniSize: () => {},
  getAssistantWindow: () => null,
  setAssistantWindow: () => {},
}));

vi.mock('./components/NoteEditor', () => ({
  default: ({ note, allNotes, onOpenRelatedNote }: { note: Note | null; allNotes: Note[]; onOpenRelatedNote: (target: Note) => void }) => <>
    <div data-testid="editor-note">{note?.id}</div>
    {note && allNotes.filter(child => child.parent_id === note.id).map(child => <button key={child.id} onClick={() => onOpenRelatedNote(child)}>Sous-case {child.title}</button>)}
  </>,
}));
vi.mock('./components/TiptapEditor', () => ({ default: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => <textarea aria-label="Contenu de la note" value={value} onChange={event => onChange(event.target.value)} /> }));
vi.mock('./components/CommandPalette', () => ({ default: ({ isOpen, projects, notes }: { isOpen: boolean; projects: Project[]; notes: Note[] }) => isOpen ? <div data-testid="palette">{projects.map(project => <span key={project.id}>{project.name}</span>)}{notes.map(note => <span key={note.id}>{note.title}</span>)}</div> : null }));
vi.mock('./components/KanbanBoard', () => ({
  default: ({ notes }: { notes: Note[] }) => (
    <div data-testid="kanban">{notes.map(note => <span key={note.id}>{note.title}</span>)}</div>
  ),
}));

const mockApi = vi.mocked(api);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const makeNote = (id: string, title: string, status: Note['status'], priority: string | null): Note => ({
  id,
  project_id: 'p1',
  title,
  content: '',
  note_type: 'note',
  status,
  resolution_comment: null,
  raw_content: null,
  priority,
  git_branch: null,
  tags: null,
  created_at: '2026-09-22T10:00:00.000Z',
  updated_at: '2026-09-22T10:00:00.000Z',
  due_at: null,
  revision: 0,
});

async function openProjectList() {
  const projects = await screen.findByRole('navigation', { name: 'Projets' });
  fireEvent.click(within(projects).getByRole('button', { name: 'Projet' }));
  fireEvent.click(await screen.findByRole('radio', { name: 'Liste' }));
  await screen.findByRole('button', { name: 'Ouvrir la case Urgente' });
}

beforeEach(() => {
  clearPendingWrites();
  vi.clearAllMocks();
  localStorage.removeItem('humcase-spaces-v1');
  localStorage.removeItem('humcase-activities-v1');
  // Most tests check the cards; the canvas (default view) has its own tests.
  localStorage.setItem('humcase-view-mode', 'cards');
  localStorage.removeItem('humcase-include-sub-cases');
  mockApi.createProject.mockReset();
  mockApi.createNote.mockReset();
  storedActivities.current = [];
  mockApi.getActivities.mockImplementation(async () => storedActivities.current);
  mockApi.createActivity.mockImplementation(async (name, space) => { const activity: Activity = { id: `a${storedActivities.current.length + 1}`, name, space, created_at: '2026-09-24T10:00:00Z', updated_at: '2026-09-24T10:00:00Z', revision: 0, project_ids: [] }; storedActivities.current = [...storedActivities.current, activity]; return activity; });
  mockApi.renameActivity.mockImplementation(async (id, name) => { const renamed = { ...storedActivities.current.find(activity => activity.id === id)!, name }; storedActivities.current = storedActivities.current.map(activity => activity.id === id ? renamed : activity); return renamed; });
  mockApi.deleteActivity.mockImplementation(async id => { storedActivities.current = storedActivities.current.filter(activity => activity.id !== id); });
  mockApi.setProjectActivities.mockImplementation(async (projectId, ids) => { storedActivities.current = storedActivities.current.map(activity => ({ ...activity, project_ids: ids.includes(activity.id) ? [...new Set([...activity.project_ids, projectId])] : activity.project_ids.filter(id => id !== projectId) })); });
  mockApi.updateProjectSpace.mockImplementation(async (id, space) => { storedProjects.current = storedProjects.current.map(project => project.id === id ? { ...project, space } : project); });
  storedProjects.current = [
    { id: 'p1', name: 'Projet', color: '#123456', space: 'work', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
  ];
  notesEnBase.current = [
    { ...makeNote('urgence', 'Urgente', 'open', 'P0'), tags: '["bug"]' },
    makeNote('archive', 'Ordinaire', 'resolved', null),
  ];
  mockApi.getProjects.mockImplementation(async () => storedProjects.current);
  mockApi.getNotes.mockImplementation(async (projectId, status, search) =>
    notesEnBase.current.filter(note =>
      (!projectId || note.project_id === projectId) &&
      (!status || note.status === status) &&
      (!search || (
        search.startsWith('#')
          ? JSON.parse(note.tags ?? '[]').includes(search.slice(1))
          : note.title.toLowerCase().includes(search.toLowerCase())
      ))
    )
  );
  mockApi.getNoteRelations.mockResolvedValue({ sources: [], tasks: [] });
  mockApi.getCaseContext.mockResolvedValue({ path: [], provenance: [] });
  mockApi.getCasePositions.mockResolvedValue([]);
});

describe('App — raccordement du nouvel espace', () => {
  it('sorts All by To do then keeps the sort choice between cards and list', async () => {
    notesEnBase.current = [
      { ...makeNote('resolved', 'Résolue', 'resolved', 'P0'), updated_at: '2026-09-25T12:00:00Z' },
      { ...makeNote('progress', 'En cours', 'in_progress', 'P0'), updated_at: '2026-09-25T11:00:00Z' },
      { ...makeNote('open', 'À traiter', 'open', 'P3'), updated_at: '2026-09-25T10:00:00Z' },
    ];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Toutes les cases' }));
    const cardTitles = () => within(screen.getByRole('main')).getAllByRole('button', { name: /^Ouvrir / }).map(button => button.textContent);
    expect(cardTitles()).toEqual(['À traiter', 'En cours', 'Résolue']);

    fireEvent.keyDown(screen.getByRole('button', { name: 'Trier les cases : À faire d’abord' }), { key: 'Enter' });
    fireEvent.click(screen.getByRole('menuitemradio', { name: 'Récentes' }));
    expect(cardTitles()).toEqual(['Résolue', 'En cours', 'À traiter']);

    fireEvent.click(screen.getByRole('radio', { name: 'Liste' }));
    expect(screen.getAllByRole('button', { name: /^Ouvrir la case / }).map(button => button.getAttribute('aria-label')))
      .toEqual(['Ouvrir la case Résolue', 'Ouvrir la case En cours', 'Ouvrir la case À traiter']);
  });

  it('shows top-level cases only, with their sub-case count, and finds a sub-case by search (lot 5)', async () => {
    notesEnBase.current = [
      makeNote('voyage', 'Voyage', 'open', null),
      { ...makeNote('hotels', 'Hôtels', 'open', null), parent_id: 'voyage' },
      { ...makeNote('hotel-x', 'Hôtel X', 'open', null), parent_id: 'hotels' },
    ];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Toutes les cases' }));
    const main = screen.getByRole('main');
    await within(main).findByRole('button', { name: 'Ouvrir Voyage' });
    expect(within(main).queryByRole('button', { name: 'Ouvrir Hôtels' })).toBeNull();
    expect(within(main).queryByRole('button', { name: 'Ouvrir Hôtel X' })).toBeNull();
    expect(within(main).getByText('1 sous-case')).toBeTruthy();

    fireEvent.change(screen.getByRole('searchbox', { name: 'Rechercher une case' }), { target: { value: 'Hôtel X' } });
    await within(main).findByRole('button', { name: 'Ouvrir Hôtel X' });
  });

  it('opens on the canvas by default, dives into a case and keeps the path after editing a case (lot 5)', async () => {
    localStorage.removeItem('humcase-view-mode');
    notesEnBase.current = [
      makeNote('voyage', 'Voyage', 'open', null),
      { ...makeNote('hotels', 'Hôtels', 'open', null), parent_id: 'voyage' },
      { ...makeNote('hotel-x', 'Hôtel X', 'open', null), parent_id: 'hotels' },
    ];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Toutes les cases' }));
    const board = await screen.findByRole('region', { name: 'Plan de travail' });
    expect(within(board).getByRole('button', { name: 'Ouvrir Voyage' })).toBeTruthy();
    expect(within(board).queryByRole('button', { name: 'Ouvrir Hôtels' })).toBeNull();

    fireEvent.click(within(board).getByRole('button', { name: '1 sous-case' }));
    await within(board).findByRole('button', { name: 'Ouvrir Hôtels' });
    expect(within(board).getByRole('navigation', { name: 'Chemin dans le plan de travail' }).textContent).toContain('Voyage');

    fireEvent.click(within(board).getByRole('button', { name: 'Ouvrir Hôtels' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Retour : Toutes les cases' }));
    const back = await screen.findByRole('region', { name: 'Plan de travail' });
    expect(within(back).getByRole('button', { name: 'Ouvrir Hôtels' })).toBeTruthy();

    fireEvent.click(screen.getByRole('radio', { name: 'Cartes' }));
    expect(localStorage.getItem('humcase-view-mode')).toBe('cards');
  });

  it('shows the same filters above every view: on the canvas they dim cases instead of hiding them', async () => {
    render(<App />);
    await openProjectList();
    const filters = () => within(screen.getByRole('search', { name: 'Filtres des cases' }));
    fireEvent.click(filters().getByRole('radio', { name: 'Résolu' }));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Ouvrir la case Urgente' })).toBeNull());

    fireEvent.click(screen.getByRole('radio', { name: 'Plan de travail' }));
    const board = await screen.findByRole('region', { name: 'Plan de travail' });
    expect(filters().getByRole('radio', { name: 'Résolu' }).getAttribute('aria-checked')).toBe('true');
    const cardOf = (title: string) => within(board).getByRole('button', { name: `Ouvrir ${title}` }).closest('article') as HTMLElement;
    expect(cardOf('Urgente').className).toContain('is-dimmed');
    expect(cardOf('Ordinaire').className).not.toContain('is-dimmed');

    fireEvent.click(filters().getByRole('radio', { name: 'Toutes' }));
    expect(cardOf('Urgente').className).not.toContain('is-dimmed');
    fireEvent.change(filters().getByRole('searchbox', { name: 'Rechercher une case' }), { target: { value: 'Urgente' } });
    expect(cardOf('Ordinaire').className).toContain('is-dimmed');
    expect(within(board).getByText('1 trouvée')).toBeTruthy();

    fireEvent.click(screen.getByRole('radio', { name: 'Liste' }));
    expect((filters().getByRole('searchbox', { name: 'Rechercher une case' }) as HTMLInputElement).value).toBe('Urgente');
    fireEvent.click(filters().getByRole('button', { name: 'Effacer les filtres' }));
    expect(await screen.findByRole('button', { name: 'Ouvrir la case Ordinaire' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Ouvrir la case Urgente' })).toBeTruthy();
  });

  it('filters by nature, and keeps the sub-cases choice whatever the section', async () => {
    notesEnBase.current = [
      { ...makeNote('voyage', 'Voyage', 'open', null), note_type: 'idea' },
      { ...makeNote('billets', 'Réserver les billets', 'in_progress', null), note_type: 'todo', parent_id: 'voyage' },
      { ...makeNote('budget', 'Budget', 'open', null), note_type: 'note' },
    ];
    render(<App />);
    const projects = await screen.findByRole('navigation', { name: 'Projets' });
    fireEvent.click(within(projects).getByRole('button', { name: 'Projet' }));
    fireEvent.click(await screen.findByRole('radio', { name: 'Cartes' }));
    const main = await screen.findByRole('main');
    expect(within(main).queryByRole('button', { name: 'Ouvrir Réserver les billets' })).toBeNull();
    // Natures count the level shown: the only task is a sub-case, offered once the box is ticked.
    const box = () => screen.getByRole('checkbox', { name: /Inclure les sous-cases/ }) as HTMLInputElement;
    const natures = () => within(screen.getByRole('group', { name: 'Filtrer par nature' }));
    expect(box().checked).toBe(false);
    expect(natures().queryByRole('button', { name: /^Tâche/ })).toBeNull();
    fireEvent.click(box());
    // One click on the nature, like on the canvas before; the box keeps the person's choice.
    fireEvent.click(natures().getByRole('button', { name: /^Tâche/ }));
    expect(box().checked).toBe(true);
    expect(await within(screen.getByRole('main')).findByRole('button', { name: 'Ouvrir Réserver les billets' })).toBeTruthy();
    expect(within(screen.getByRole('main')).queryByRole('button', { name: 'Ouvrir Budget' })).toBeNull();
    // Changing section or clearing the filters leaves the choice as it is.
    fireEvent.click(screen.getAllByRole('button', { name: 'Effacer les filtres' })[0]);
    expect(box().checked).toBe(true);
    fireEvent.click(screen.getByRole('radio', { name: /^En cours/ }));
    expect(box().checked).toBe(true);
    expect(await within(screen.getByRole('main')).findByRole('button', { name: 'Ouvrir Réserver les billets' })).toBeTruthy();
    fireEvent.click(box());
    fireEvent.click(screen.getByRole('radio', { name: /^Toutes/ }));
    expect(box().checked).toBe(false);
    expect(localStorage.getItem('humcase-include-sub-cases')).toBe('false');
  });

  it('blocks creation and palette behind an editor modal', async () => {
    render(<App />);
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    document.body.appendChild(dialog);
    try {
      fireEvent.keyDown(window, { key: 'n', ctrlKey: true });
      document.dispatchEvent(new CustomEvent('open-command-palette'));
      expect(mockApi.createNote).not.toHaveBeenCalled();
      expect(mockApi.getOrCreateInbox).not.toHaveBeenCalled();
      expect(screen.queryByTestId('palette')).toBeNull();
    } finally { dialog.remove(); }
  });
  it('keeps the draft after a failure then opens the project actually created', async () => {
    mockApi.createProject.mockRejectedValueOnce(new Error('write refused')).mockImplementationOnce(async name => {
      storedProjects.current = [...storedProjects.current, { id: 'p2', name, color: '#5273a8', space: 'work', created_at: '2026-09-24T10:00:00.000Z', updated_at: '2026-09-24T10:00:00.000Z', revision: 0 }];
      return 'p2';
    });
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Nouveau projet' }));
    const name = screen.getByRole('textbox', { name: 'Nom du projet' }) as HTMLInputElement;
    fireEvent.change(name, { target: { value: 'Projet ajouté' } });
    fireEvent.click(screen.getByRole('button', { name: 'Créer le projet' }));
    await screen.findByText(/Création impossible/);
    expect(name.value).toBe('Projet ajouté');
    expect(screen.getByRole('dialog', { name: 'Nouveau projet' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Créer le projet' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Nouveau projet' })).toBeNull());
    expect(mockApi.createProject).toHaveBeenCalledTimes(2);
    expect(mockApi.createProject).toHaveBeenCalledWith('Projet ajouté', undefined, 'work');
    expect(await screen.findByRole('heading', { level: 1, name: 'Projet ajouté' })).toBeTruthy();
  });

  it('opens a recent note from the home in its actual project', async () => {
    storedProjects.current = [...storedProjects.current, { id: 'p2', name: 'Deuxième projet', color: '#5273a8', space: 'work', created_at: '2026-09-24T10:00:00.000Z', updated_at: '2026-09-24T10:00:00.000Z', revision: 0 }];
    notesEnBase.current = [...notesEnBase.current, { ...makeNote('ailleurs', 'Note distante', 'open', null), project_id: 'p2', updated_at: '2026-09-24T11:00:00.000Z' }];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Note distante' }));
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('ailleurs'));
    expect(screen.queryByRole('dialog', { name: 'Note distante' })).toBeNull();
    // Opened from Home, the way back leads to Home, not to the case's project.
    fireEvent.click(screen.getByRole('button', { name: 'Retour : Accueil' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Reprendre le fil' })).toBeTruthy();
  });

  it('goes back from a sub-case to its case, then to the page the case was opened from', async () => {
    notesEnBase.current = [makeNote('parent', 'Audit', 'open', null), { ...makeNote('child', 'Bouton retour', 'open', null), parent_id: 'parent' }];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Audit' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Sous-case Bouton retour' }));
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('child'));
    fireEvent.click(screen.getByRole('button', { name: 'Retour : Audit' }));
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('parent'));
    fireEvent.click(screen.getByRole('button', { name: 'Retour : Accueil' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Reprendre le fil' })).toBeTruthy();
  });

  it('opens a new note directly on its page with a way back to the notes', async () => {
    mockApi.createNote.mockImplementation(async () => {
      notesEnBase.current = [...notesEnBase.current, makeNote('nouvelle', 'Nouvelle case', 'open', null)];
      return 'nouvelle';
    });
    render(<App />);
    const projects = await screen.findByRole('navigation', { name: 'Projets' });
    fireEvent.click(within(projects).getByRole('button', { name: 'Projet' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Nouvelle case' }));
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('nouvelle'));
    expect(screen.getByRole('button', { name: 'Retour : Projet' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Retour à la note' })).toBeNull();
  });

  it('goes back to the notes from the main note page', async () => {
    notesEnBase.current = [{ ...makeNote('source', 'Pensée', 'open', null), content: 'Texte structuré', raw_content: 'Brouillon dicté' }];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Pensée' }));
    expect(screen.getByTestId('editor-note').textContent).toBe('source');
    fireEvent.click(screen.getByRole('button', { name: 'Retour : Accueil' }));
    expect(screen.getByTestId('editor-note').textContent).toBe('');
    expect(await screen.findByRole('button', { name: 'Ouvrir Pensée' })).toBeTruthy();
  });

  it('announces the open note to quick capture, then none on return (0.9.83)', async () => {
    const { emit } = await import('@tauri-apps/api/event');
    const emitMock = vi.mocked(emit);
    notesEnBase.current = [makeNote('source', 'Pensée', 'open', null)];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Pensée' }));
    await waitFor(() => expect(emitMock).toHaveBeenCalledWith('humcase:open-note', { id: 'source', title: 'Pensée' }));
    emitMock.mockClear();
    fireEvent.click(screen.getByRole('button', { name: 'Retour : Accueil' }));
    await waitFor(() => expect(emitMock).toHaveBeenCalledWith('humcase:open-note', null));
  });

  it('never offers an open drawing to quick capture: a text cannot be added to it', async () => {
    const { emit } = await import('@tauri-apps/api/event');
    const emitMock = vi.mocked(emit);
    notesEnBase.current = [{ ...makeNote('plan', 'Plan', 'open', null), content_kind: 'sketch', content: '{"type":"excalidraw","version":2,"elements":[]}' }];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Plan' }));
    await waitFor(() => expect(emitMock).toHaveBeenCalledWith('humcase:open-note', null));
    expect(emitMock).not.toHaveBeenCalledWith('humcase:open-note', expect.objectContaining({ id: 'plan' }));
  });

  it('opens the trash from the sidebar and rereads the notes after a restore (0.9.82)', async () => {
    notesEnBase.current = [makeNote('vivante', 'Vivante', 'open', null)];
    const discarded = { ...makeNote('jetee', 'Jetée', 'open', null), deleted_at: '2026-09-28T10:00:00.000Z' };
    mockApi.getTrashedNotes.mockResolvedValueOnce([discarded]).mockResolvedValueOnce([]);
    mockApi.restoreFromTrash.mockImplementation(async () => { notesEnBase.current = [...notesEnBase.current, { ...discarded, deleted_at: null }]; });
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Corbeille' }));
    expect(await screen.findByRole('heading', { name: 'Corbeille' })).toBeTruthy();
    fireEvent.click(await screen.findByRole('button', { name: /Restaurer/ }));
    await waitFor(() => expect(mockApi.restoreFromTrash).toHaveBeenCalledWith('jetee'));
    fireEvent.click(screen.getByRole('button', { name: 'Toutes les cases' }));
    expect(await screen.findByRole('button', { name: 'Ouvrir Jetée' })).toBeTruthy();
  });
});

describe('App — search by meaning', () => {
  it('says no case holds the words when only meaning found cases, in the cards and the list', async () => {
    vi.mocked(api.searchByMeaning).mockResolvedValue([{ note_id: 'archive', score: 0.8, lead: 3.2, strong: true, very_close: false }]);
    render(<App />);
    await openProjectList();
    const filters = within(screen.getByRole('search', { name: 'Filtres des cases' }));
    fireEvent.change(filters.getByRole('searchbox', { name: 'Rechercher une case' }), { target: { value: 'vieux dossiers rangés' } });
    const close = await screen.findByRole('list', { name: 'Proches par le sens' });
    expect(within(close).getByText('Ordinaire')).toBeTruthy();
    // The list (opened by openProjectList), then the cards.
    expect(await screen.findByText('Aucune case ne contient ces mots.')).toBeTruthy();
    expect(screen.queryByText('Aucune case pour ces filtres.')).toBeNull();
    fireEvent.click(screen.getByRole('radio', { name: 'Cartes' }));
    expect(await screen.findByText('Aucune case ne contient ces mots.')).toBeTruthy();
    expect(screen.queryByText('Aucune case trouvée')).toBeNull();
    vi.mocked(api.searchByMeaning).mockResolvedValue([]);
  });
});

describe('App — persisted activities', () => {
  it('creates an activity in the database and creates no project or note', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Personnaliser' }));
    fireEvent.click(screen.getByRole('button', { name: '+ Développement' }));
    await waitFor(() => expect(mockApi.createActivity).toHaveBeenCalledWith('Développement', 'work'));
    // Settings are a modal window: the sidebar behind it is inert (aria-hidden),
    // but its filter must already offer the created activity.
    expect(await screen.findByRole('option', { name: 'Développement', hidden: true })).toBeTruthy();
    expect(mockApi.createProject).not.toHaveBeenCalled();
    expect(mockApi.createNote).not.toHaveBeenCalled();
  });

  it('filters projects through their links without hiding the notes of All cases', async () => {
    storedActivities.current = [{ id: 'a1', name: 'Développement', space: 'work', created_at: '', updated_at: '', revision: 0, project_ids: ['p1'] }];
    storedProjects.current.push({ id: 'p2', name: 'Projet marketing', color: '#123456', space: 'work', created_at: '', updated_at: '', revision: 0 });
    notesEnBase.current.push({ ...makeNote('m2', 'Idée marketing', 'open', null), project_id: 'p2' });
    render(<App />);
    fireEvent.change(await screen.findByRole('combobox', { name: 'Filtrer les projets par activité' }), { target: { value: 'a1' } });
    const projects = screen.getByRole('navigation', { name: 'Projets' });
    expect(within(projects).queryByRole('button', { name: /Projet marketing/ })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Toutes les cases' }));
    expect(await screen.findByRole('button', { name: 'Ouvrir Idée marketing' })).toBeTruthy();
  });

  it('keeps the link choice when saving fails and passes the baseline to the backend', async () => {
    storedActivities.current = [{ id: 'a1', name: 'Développement', space: 'work', created_at: '', updated_at: '', revision: 0, project_ids: [] }];
    mockApi.setProjectActivities.mockRejectedValueOnce(new Error('conflit'));
    render(<App />);
    const projects = await screen.findByRole('navigation', { name: 'Projets' });
    fireEvent.click(within(projects).getByRole('button', { name: 'Réglages de Projet' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Développement' }));
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    expect(await screen.findByText(/conflit/)).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: 'Développement' })).toHaveProperty('checked', true);
    expect(mockApi.setProjectActivities).toHaveBeenCalledWith('p1', ['a1'], []);
  });

  it('reports an impossible reread after creating an activity (review 0.9.69)', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Personnaliser' }));
    await waitFor(() => expect(mockApi.getActivities).toHaveBeenCalled());
    mockApi.getActivities.mockRejectedValueOnce(new Error('database busy'));
    fireEvent.click(screen.getByRole('button', { name: '+ Développement' }));
    await waitFor(() => expect(mockApi.createActivity).toHaveBeenCalledWith('Développement', 'work'));
    expect(await screen.findByText(/n’ont pas pu être relues/)).toBeTruthy();
  });
});

describe('App — Today view', () => {
  const todayButton = () => screen.getByTitle("Cases en cours, en retard, dues aujourd’hui ou prioritaires (P0/P1), sur tout l’espace");

  it('covers the whole space, narrows every view like a filter and is left from its chip', async () => {
    storedProjects.current = [...storedProjects.current, { id: 'p2', name: 'Autre projet', color: '#5273a8', space: 'work', created_at: '2026-09-24T10:00:00.000Z', updated_at: '2026-09-24T10:00:00.000Z', revision: 0 }];
    notesEnBase.current = [...notesEnBase.current, { ...makeNote('ailleurs', 'Ailleurs en cours', 'in_progress', null), project_id: 'p2' }];
    render(<App />);
    await openProjectList();
    const filters = () => within(screen.getByRole('search', { name: 'Filtres des cases' }));
    fireEvent.change(filters().getByRole('searchbox', { name: 'Rechercher une case' }), { target: { value: 'Ordinaire' } });
    fireEvent.click(filters().getByRole('radio', { name: 'Résolu' }));
    await screen.findByRole('button', { name: 'Ouvrir la case Ordinaire' });

    // The same count from any project: the urgent case here and the case in progress elsewhere.
    expect(todayButton().textContent).toContain('2');
    fireEvent.click(todayButton());
    await screen.findByRole('button', { name: 'Ouvrir la case Urgente' });
    expect(screen.getByRole('button', { name: 'Ouvrir la case Ailleurs en cours' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Ouvrir la case Ordinaire' })).toBeNull();
    // "Today" starts from its own rule: the earlier search and status are cleared, and it shows.
    expect((filters().getByRole('searchbox', { name: 'Rechercher une case' }) as HTMLInputElement).value).toBe('');
    expect(filters().getByRole('radio', { name: 'Toutes' }).getAttribute('aria-checked')).toBe('true');
    // A second click keeps the view.
    fireEvent.click(todayButton());
    expect(filters().getByRole('button', { name: /Aujourd’hui/ })).toBeTruthy();

    fireEvent.click(screen.getByRole('radio', { name: 'Tableau' }));
    expect(within(screen.getByTestId('kanban')).getByText('Urgente')).toBeTruthy();
    expect(within(screen.getByTestId('kanban')).queryByText('Ordinaire')).toBeNull();
    fireEvent.click(filters().getByRole('button', { name: /Aujourd’hui/ }));
    expect(await within(screen.getByTestId('kanban')).findByText('Ordinaire')).toBeTruthy();
  });

  it('adds a tag search to Today', async () => {
    render(<App />);
    await openProjectList();
    fireEvent.click(todayButton());
    fireEvent.click(await screen.findByRole('button', { name: 'Rechercher les cases avec le tag bug' }));
    await screen.findByRole('button', { name: 'Ouvrir la case Urgente' });
    expect((screen.getByRole('searchbox', { name: 'Rechercher une case' }) as HTMLInputElement).value).toBe('#bug');
  });

  it('clearly announces the absence of urgency', async () => {
    notesEnBase.current = [makeNote('archive', 'Ordinaire', 'resolved', null)];
    render(<App />);
    await screen.findByRole('button', { name: 'Ouvrir Ordinaire' });
    fireEvent.click(todayButton());
    fireEvent.click(await screen.findByRole('radio', { name: 'Cartes' }));
    expect(screen.getByText('Rien d’urgent aujourd’hui')).toBeTruthy();
    expect(screen.getByText(/Ici s’affichent les cases en cours, en retard/)).toBeTruthy();
    // The board says it too, instead of three empty columns.
    fireEvent.click(screen.getByRole('radio', { name: 'Tableau' }));
    expect(screen.getByText('Rien d’urgent aujourd’hui')).toBeTruthy();
    expect(screen.queryByTestId('kanban')).toBeNull();
  });

  it('Ctrl+F works with Caps Lock on', async () => {
    render(<App />);
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    fireEvent.keyDown(window, { key: 'F', ctrlKey: true });
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('searchbox', { name: 'Rechercher une case' })));
  });

  it('Ctrl+J and the Ask button open the assistant window; closed, it stays as a key in the corner', async () => {
    render(<App />);
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    expect(screen.queryByRole('region', { name: 'Assistant' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Assistant (Ctrl+J)' })).toBeNull();
    fireEvent.keyDown(window, { key: 'j', ctrlKey: true });
    expect(await screen.findByRole('region', { name: 'Assistant' })).toBeTruthy();
    expect(await screen.findByText('Claude Haiku 5.5 · prêt')).toBeTruthy();
    fireEvent.keyDown(window, { key: 'j', ctrlKey: true });
    expect(screen.queryByRole('region', { name: 'Assistant' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Assistant (Ctrl+J)' }));
    expect(screen.getByRole('region', { name: 'Assistant' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Assistant' }));
    expect(screen.queryByRole('region', { name: 'Assistant' })).toBeNull();
  });

  it('Ctrl+F searches the whole space, leaving Today and the open project', async () => {
    render(<App />);
    await openProjectList();
    fireEvent.click(todayButton());
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('searchbox', { name: 'Rechercher une case' })));
    // The "Today" chip of the filters is gone (the sidebar entry stays).
    expect(within(screen.getByRole('search')).queryByRole('button', { name: /Aujourd’hui/ })).toBeNull();
    expect(screen.getByRole('button', { name: 'Toutes les cases' }).getAttribute('aria-current')).toBe('page');
  });
});

describe('App — review 0.9.69: writing and Inbox', () => {
  it('Ctrl+F in a note does not leave writing and does not steal the focus', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Urgente' }));
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('urgence'));

    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    await new Promise(resolve => setTimeout(resolve, 50));
    // Without the fix, the page goes back to the notes and the search takes the focus.
    expect(screen.queryByRole('searchbox', { name: 'Rechercher une case' })).toBeNull();
    expect(screen.getByTestId('editor-note').textContent).toBe('urgence');
  });

  it('the note breadcrumb names its project, not "My notes"', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Urgente' }));
    const topbar = document.querySelector('.workspace-topbar') as HTMLElement;
    await waitFor(() => expect(within(topbar).getByRole('button', { name: 'Projet' })).toBeTruthy());
    expect(within(topbar).queryByRole('button', { name: 'Mes notes' })).toBeNull();
  });

  it('the visible note page is the single main region', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Urgente' }));
    await waitFor(() => expect(screen.getByRole('main', { name: 'Case' })).toBeTruthy());
    expect(within(screen.getByRole('main', { name: 'Case' })).getByTestId('editor-note').textContent).toBe('urgence');
    expect(screen.getAllByRole('main')).toHaveLength(1);
  });

  it('a project chosen without being applied shows neither on another note nor on reopening', async () => {
    storedProjects.current = [
      { id: 'p1', name: 'Projet', color: '#123456', space: 'work', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
      { id: 'inbox', name: 'Réception', color: '#777777', space: 'work', is_inbox: true, created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
    ];
    notesEnBase.current = [
      { ...makeNote('a', 'Idée A', 'open', 'P0'), project_id: 'inbox' },
      { ...makeNote('b', 'Idée B', 'open', 'P0'), project_id: 'inbox' },
    ];
    render(<App />);
    const openFromHome = async (title: string) => {
      fireEvent.click(await screen.findByRole('button', { name: 'Accueil' }));
      // The home shows a case once: to pick up, to follow or among the recent cases.
      fireEvent.click((await screen.findAllByRole('button', { name: new RegExp(`^(Ouvrir|Poursuivre) ${title}$`) }))[0]);
    };
    const target = () => screen.getByLabelText('Ranger dans un projet') as HTMLSelectElement;

    await openFromHome('Idée A');
    fireEvent.change(await waitFor(() => target()), { target: { value: 'p1' } });
    expect(target().value).toBe('p1');

    await openFromHome('Idée B');
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('b'));
    expect(target().value).toBe('');

    fireEvent.change(target(), { target: { value: 'p1' } });
    await openFromHome('Idée B');
    expect(target().value).toBe('');
    expect(mockApi.moveNoteToProject).not.toHaveBeenCalled();
  });

  it('offers filing only for a top-level case of the Inbox: a sub-case follows its parent', async () => {
    storedProjects.current = [
      { id: 'p1', name: 'Projet', color: '#123456', space: 'work', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
      { id: 'inbox', name: 'Réception', color: '#777777', space: 'work', is_inbox: true, created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
    ];
    notesEnBase.current = [
      { ...makeNote('idee', 'Idée captée', 'open', 'P0'), project_id: 'inbox' },
      { ...makeNote('detail', 'Détail capté', 'open', 'P0'), project_id: 'inbox', parent_id: 'idee' },
    ];
    render(<App />);
    fireEvent.click((await screen.findAllByRole('button', { name: /^(Ouvrir|Poursuivre) Détail capté$/ }))[0]);
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('detail'));
    expect(screen.queryByLabelText('Ranger dans un projet')).toBeNull();
  });

  it('files an Inbox case straight into a case of a project, in one call', async () => {
    storedProjects.current = [
      { id: 'p1', name: 'Projet', color: '#123456', space: 'work', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
      { id: 'inbox', name: 'Réception', color: '#777777', space: 'work', is_inbox: true, created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
    ];
    notesEnBase.current = [
      makeNote('voyage', 'Voyage', 'open', null),
      { ...makeNote('hotels', 'Hôtels', 'open', null), parent_id: 'voyage' },
      { ...makeNote('idee', 'Idée captée', 'open', 'P0'), project_id: 'inbox' },
    ];
    render(<App />);
    fireEvent.click((await screen.findAllByRole('button', { name: /^(Ouvrir|Poursuivre) Idée captée$/ }))[0]);
    fireEvent.change(await screen.findByLabelText('Ranger dans un projet'), { target: { value: 'p1' } });
    const parent = screen.getByLabelText('Dans la case') as HTMLSelectElement;
    expect([...parent.options].map(option => option.textContent)).toEqual(['Au premier niveau du projet', 'Voyage', 'Voyage › Hôtels']);
    fireEvent.change(parent, { target: { value: 'hotels' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ranger la case' }));
    await waitFor(() => expect(mockApi.moveNoteToProject).toHaveBeenCalledWith('idee', 'p1', 'hotels'));
    expect(mockApi.moveCase).not.toHaveBeenCalled();
  });

  it('files an Inbox case although a draft of another case waits for recovery', async () => {
    storedProjects.current = [
      { id: 'p1', name: 'Projet', color: '#123456', space: 'work', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
      { id: 'inbox', name: 'Réception', color: '#777777', space: 'work', is_inbox: true, created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
    ];
    notesEnBase.current = [{ ...makeNote('idee', 'Idée captée', 'open', 'P0'), project_id: 'inbox' }];
    addPendingWrite({ kind: 'update', noteId: 'other', payload: { content: 'Brouillon ailleurs' }, space: 'private' });
    render(<App />);
    fireEvent.click((await screen.findAllByRole('button', { name: /^(Ouvrir|Poursuivre) Idée captée$/ }))[0]);
    fireEvent.change(await screen.findByLabelText('Ranger dans un projet'), { target: { value: 'p1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ranger la case' }));
    await waitFor(() => expect(mockApi.moveNoteToProject).toHaveBeenCalledWith('idee', 'p1', null));
  });

  it('does not file an Inbox case while its own write waits for recovery', async () => {
    storedProjects.current = [
      { id: 'p1', name: 'Projet', color: '#123456', space: 'work', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
      { id: 'inbox', name: 'Réception', color: '#777777', space: 'work', is_inbox: true, created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 },
    ];
    notesEnBase.current = [{ ...makeNote('idee', 'Idée captée', 'open', 'P0'), project_id: 'inbox' }];
    addPendingWrite({ kind: 'update', noteId: 'idee', payload: { content: 'À reprendre' } });
    render(<App />);
    fireEvent.click((await screen.findAllByRole('button', { name: /^(Ouvrir|Poursuivre) Idée captée$/ }))[0]);
    fireEvent.change(await screen.findByLabelText('Ranger dans un projet'), { target: { value: 'p1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ranger la case' }));
    await screen.findByText(/non enregistr/);
    expect(mockApi.moveNoteToProject).not.toHaveBeenCalled();
  });

  it('the resume brief shows its waiting state and does not launch twice', async () => {
    const brief = deferred<string>();
    mockApi.getHandoff.mockImplementation(() => brief.promise);
    render(<App />);
    await openProjectList();
    const button = screen.getByRole('button', { name: 'Brief de session' });
    fireEvent.click(button);
    await waitFor(() => expect(button.getAttribute('aria-busy')).toBe('true'));
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
    expect(mockApi.getHandoff).toHaveBeenCalledTimes(1);

    brief.resolve('Brief du projet');
    await screen.findByText(/Brief du projet/);
    expect((button as HTMLButtonElement).disabled).toBe(false);
  });

  it('during the first read, the Notes view announces loading and not an empty space', async () => {
    const read = deferred<Note[]>();
    mockApi.getNotes.mockImplementation(() => read.promise);
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Toutes les cases' }));
    expect(screen.getByRole('status').textContent).toContain('Chargement des cases');
    expect(screen.queryByText('Encore aucune case')).toBeNull();
    expect(screen.queryByText('Ce projet attend sa première case')).toBeNull();

    read.resolve(notesEnBase.current);
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    expect(screen.queryByText('Chargement des cases…')).toBeNull();
  });
});

describe('App — notes loading', () => {
  it('ignores a late error after the success of a more recent reload', async () => {
    const oldLoad = deferred<Note[]>();
    let call = 0;
    mockApi.getNotes.mockImplementation(() => (call++ === 0 ? oldLoad.promise : Promise.resolve(notesEnBase.current)));
    let fingerprint = 'v1';
    mockApi.notesChangeMarker.mockImplementation(async () => fingerprint);

    render(<App />);
    await waitFor(() => expect(mockApi.getNotes).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(focusListener.current).not.toBeNull());
    await waitFor(() => expect(mockApi.notesChangeMarker).toHaveBeenCalled());

    // Back to the foreground after an agent wrote: the fingerprint changed, a more recent load succeeds.
    fingerprint = 'v2';
    focusListener.current?.({ payload: true });
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    expect(mockApi.getNotes).toHaveBeenCalledTimes(2);

    // The initial request then fails: its error must not cancel the recent result.
    oldLoad.reject(new Error('stale failure'));
    await Promise.resolve();
    await Promise.resolve();

    expect(screen.queryByRole('alert')).toBeNull();
    mockApi.notesChangeMarker.mockImplementation(async () => 'stable');
  });

  it('Back after the first capture leads to the Inbox it created, not to an empty one', async () => {
    const inbox: Project = { id: 'inbox', name: 'Réception', color: '#777777', space: 'work', is_inbox: true, created_at: '2026-10-03T10:00:00.000Z', updated_at: '2026-10-03T10:00:00.000Z', revision: 0 };
    mockApi.getOrCreateInbox.mockImplementation(async () => { storedProjects.current = [...storedProjects.current, inbox]; return 'inbox'; });
    mockApi.createNote.mockImplementation(async input => { notesEnBase.current = [...notesEnBase.current, { ...makeNote('capture', 'Idée captée', 'open', null), project_id: input.project_id }]; return 'capture'; });
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Réception' }));
    const page = (await screen.findByText('La réception est vide')).closest('.workspace-page') as HTMLElement;
    fireEvent.click(within(page).getByRole('button', { name: 'Nouvelle case' }));
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('capture'));
    fireEvent.click(screen.getByRole('button', { name: /^Retour/ }));
    expect(await screen.findByRole('button', { name: 'Ouvrir Idée captée' })).toBeTruthy();
    expect(screen.queryByText('La réception est vide')).toBeNull();
  });

  it('Back drops an activity deleted meanwhile instead of hiding every project', async () => {
    storedActivities.current = [{ id: 'a1', name: 'Développement', space: 'work', created_at: '', updated_at: '', revision: 0, project_ids: ['p1'] }];
    let fingerprint = 'v1';
    mockApi.notesChangeMarker.mockImplementation(async () => fingerprint);
    render(<App />);
    fireEvent.change(await screen.findByRole('combobox', { name: 'Filtrer les projets par activité' }), { target: { value: 'a1' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Ouvrir Urgente' }));
    await waitFor(() => expect(screen.getByTestId('editor-note').textContent).toBe('urgence'));
    // The activity is deleted elsewhere while the case is open.
    storedActivities.current = [];
    fingerprint = 'v2';
    await waitFor(() => expect(focusListener.current).not.toBeNull());
    focusListener.current?.({ payload: true });
    await waitFor(() => expect(screen.queryByRole('option', { name: 'Développement', hidden: true })).toBeNull());
    fireEvent.click(screen.getByRole('button', { name: /^Retour/ }));
    expect(await within(screen.getByRole('navigation', { name: 'Projets' })).findByRole('button', { name: /^Projet/ })).toBeTruthy();
  });

  it('the Inbox shown before it exists leaves Today and is the one highlighted', async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: /^Aujourd/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Réception' }));
    expect(await screen.findByText('La réception est vide')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Réception' }).getAttribute('data-active')).toBe('true');
    expect(screen.getByRole('button', { name: /^Aujourd/ }).getAttribute('data-active')).not.toBe('true');
    expect(screen.getByRole('button', { name: 'Toutes les cases' }).getAttribute('data-active')).not.toBe('true');
  });

  it('has one header in every view: the title, its count and "New case" at its right, once', async () => {
    render(<App />);
    await openProjectList();
    for (const view of ['Plan de travail', 'Cartes', 'Liste', 'Tableau']) {
      fireEvent.click(screen.getByRole('radio', { name: view }));
      const header = await waitFor(() => document.querySelector('.workspace-cases-page > .workspace-page__header') as HTMLElement);
      expect(within(header).getByRole('heading', { level: 1, name: 'Projet' })).toBeTruthy();
      expect(within(header).getByRole('button', { name: /Nouvelle case|Ajouter une sous-case/ })).toBeTruthy();
      expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
      expect(screen.getAllByRole('button', { name: 'Nouvelle case' })).toHaveLength(1);
    }
  });

  it('creates a case of the nature the page shows: the nature filter, or a shortcut', async () => {
    mockApi.createNote.mockResolvedValue('nouvelle');
    render(<App />);
    await openProjectList();
    fireEvent.click(within(screen.getByRole('group', { name: 'Filtrer par nature' })).getByRole('button', { name: /^Pensée/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle case' }));
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith(expect.objectContaining({ project_id: 'p1', note_type: 'note' })));
    mockApi.createNote.mockClear();
    mockApi.getOrCreateInbox.mockResolvedValue('inbox');
    fireEvent.click(screen.getByRole('button', { name: 'Décisions' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Nouvelle case' }));
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith(expect.objectContaining({ project_id: 'inbox', note_type: 'decision' })));
  });

  it('counts the unresolved cases of a project in the sidebar, and says the rule', async () => {
    render(<App />);
    const projects = await screen.findByRole('navigation', { name: 'Projets' });
    // "Urgente" is open, "Ordinaire" resolved.
    const count = await waitFor(() => within(projects).getByTitle('1 case non résolue, sous-cases comprises'));
    expect(count.textContent).toBe('1');
  });

  it('offers the project briefs only on the project, not on Home', async () => {
    render(<App />);
    await openProjectList();
    expect(screen.getByRole('button', { name: 'Brief de session' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Accueil' }));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Brief de session' })).toBeNull());
  });

  it('a return to the window that changed nothing rereads nothing', async () => {
    render(<App />);
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    await waitFor(() => expect(focusListener.current).not.toBeNull());
    const reads = mockApi.getNotes.mock.calls.length;
    focusListener.current?.({ payload: true });
    await waitFor(() => expect(mockApi.notesChangeMarker.mock.calls.length).toBeGreaterThan(1));
    expect(mockApi.getNotes.mock.calls.length).toBe(reads);
  });

  it('a project, a status or a nature narrows the list without asking the database again', async () => {
    render(<App />);
    await openProjectList();
    const reads = mockApi.getNotes.mock.calls.length;
    const projectsReads = mockApi.getProjects.mock.calls.length;
    fireEvent.click(screen.getByRole('radio', { name: /^Résolu/ }));
    expect(await screen.findByRole('button', { name: 'Ouvrir la case Ordinaire' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Ouvrir la case Urgente' })).toBeNull();
    fireEvent.click(screen.getByRole('radio', { name: /^Toutes/ }));
    fireEvent.click(screen.getAllByRole('button', { name: 'Toutes les cases' })[0]);
    await screen.findByRole('button', { name: 'Ouvrir la case Urgente' });
    expect(mockApi.getNotes.mock.calls.length).toBe(reads);
    expect(mockApi.getProjects.mock.calls.length).toBe(projectsReads);
  });
});

describe('App — espaces persistants', () => {
  it('isolates home, notes, urgencies and palette on space change', async () => {
    storedProjects.current.push({ id: 'p2', name: 'Projet privé', color: '#aabbcc', space: 'private', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 });
    notesEnBase.current.push({ ...makeNote('secret', 'Note privée', 'open', 'P0'), project_id: 'p2' });
    render(<App />);
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    expect(screen.queryByRole('button', { name: 'Ouvrir Note privée' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Toutes les cases' }));
    await screen.findByRole('button', { name: 'Ouvrir Urgente' });
    expect(screen.queryByRole('button', { name: 'Ouvrir Note privée' })).toBeNull();
    expect(screen.getByTitle("Cases en cours, en retard, dues aujourd’hui ou prioritaires (P0/P1), sur tout l’espace").textContent).toContain('1');

    fireEvent.click(screen.getByLabelText('Changer d’espace : Espace pro'));
    fireEvent.click(screen.getByRole('button', { name: 'Espace privé' }));
    await screen.findByRole('button', { name: 'Ouvrir Note privée' });
    expect(screen.queryByRole('button', { name: 'Ouvrir Urgente' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Ouvrir Urgente' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Toutes les cases' }));
    await screen.findByRole('button', { name: 'Ouvrir Note privée' });
    document.dispatchEvent(new CustomEvent('open-command-palette'));
    await screen.findByTestId('palette');
    expect(screen.getByTestId('palette').textContent).toContain('Note privée');
    expect(screen.getByTestId('palette').textContent).not.toContain('Urgente');
    expect(within(screen.getByTestId('palette')).queryByText('Projet')).toBeNull();
    expect(JSON.parse(localStorage.getItem('humcase-spaces-v1') ?? '{}').scope).toBe('private');
  });

  it('keeps the old projects in Unsorted then shows them only after assignment', async () => {
    storedProjects.current = [{ id: 'legacy', name: 'Ancien projet', color: '#123456', space: null, created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 }];
    notesEnBase.current = [{ ...makeNote('old', 'Note héritée', 'open', null), project_id: 'legacy' }];
    render(<App />);
    expect(await screen.findByRole('button', { name: 'Voir les projets à classer' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Ouvrir Note héritée' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Voir les projets à classer' }));
    await screen.findByRole('button', { name: 'Ouvrir Note héritée' });
    fireEvent.change(screen.getByRole('combobox', { name: 'Classer Ancien projet' }), { target: { value: 'private' } });
    await waitFor(() => expect(mockApi.updateProjectSpace).toHaveBeenCalledWith('legacy', 'private'));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Ouvrir Note héritée' })).toBeNull());
    fireEvent.click(screen.getByLabelText('Changer d’espace : À classer'));
    fireEvent.click(screen.getByRole('button', { name: 'Espace privé' }));
    await screen.findByRole('button', { name: 'Ouvrir Note héritée' });
  });

  it('preselects the only active space to create from Unsorted', async () => {
    localStorage.setItem('humcase-spaces-v1', JSON.stringify({ usage: 'work', combined: false, scope: 'unassigned' }));
    storedProjects.current = [{ id: 'legacy', name: 'Ancien projet', color: '#123456', space: null, created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0 }];
    notesEnBase.current = [];
    mockApi.createProject.mockImplementation(async name => { storedProjects.current.push({ id: 'new', name, color: '#123456', space: 'work', created_at: '2026-09-24T10:00:00.000Z', updated_at: '2026-09-24T10:00:00.000Z', revision: 0 }); return 'new'; });
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Nouveau projet' }));
    const space = screen.getByRole('combobox', { name: 'Espace' }) as HTMLSelectElement;
    expect(space.value).toBe('work');
    expect(space.options.length).toBe(1);
    fireEvent.change(screen.getByRole('textbox', { name: 'Nom du projet' }), { target: { value: 'Nouveau pro' } });
    fireEvent.click(screen.getByRole('button', { name: 'Créer le projet' }));
    await waitFor(() => expect(mockApi.createProject).toHaveBeenCalledWith('Nouveau pro', undefined, 'work'));
  });
});

describe('App — project management (0.9.76)', () => {
  beforeEach(() => {
    mockApi.renameProject.mockImplementation(async (id, name) => { storedProjects.current = storedProjects.current.map(project => project.id === id ? { ...project, name } : project); });
    mockApi.setProjectColor.mockImplementation(async (id, color) => { storedProjects.current = storedProjects.current.map(project => project.id === id ? { ...project, color } : project); });
    mockApi.setProjectArchived.mockImplementation(async (id, archived) => { storedProjects.current = storedProjects.current.map(project => project.id === id ? { ...project, archived_at: archived ? '2026-09-28T16:00:00.000Z' : null } : project); });
    mockApi.deleteProject.mockImplementation(async id => { const moved = notesEnBase.current.filter(note => note.project_id === id).length; storedProjects.current = storedProjects.current.filter(project => project.id !== id); return moved; });
  });

  const openSettings = async (name: string) => {
    const nav = await screen.findByRole('navigation', { name: 'Projets' });
    fireEvent.click(within(nav).getByRole('button', { name: `Réglages de ${name}` }));
  };

  it('renames a project and changes its color without touching the notes', async () => {
    render(<App />);
    await openSettings('Projet');
    fireEvent.change(screen.getByRole('textbox', { name: 'Nom du projet' }), { target: { value: '  Humcase  ' } });
    fireEvent.click(screen.getByRole('radio', { name: 'Couleur #8b5cf6' }));
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await waitFor(() => expect(mockApi.renameProject).toHaveBeenCalledWith('p1', 'Humcase'));
    expect(mockApi.setProjectColor).toHaveBeenCalledWith('p1', '#8b5cf6');
    expect(mockApi.setProjectActivities).not.toHaveBeenCalled();
    const nav = screen.getByRole('navigation', { name: 'Projets' });
    expect(await within(nav).findByRole('button', { name: 'Humcase' })).toBeTruthy();
  });

  it('keeps the window open and explains a name already taken', async () => {
    mockApi.renameProject.mockRejectedValueOnce('un autre projet porte déjà ce nom');
    render(<App />);
    await openSettings('Projet');
    fireEvent.change(screen.getByRole('textbox', { name: 'Nom du projet' }), { target: { value: 'Autre' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    expect(await screen.findByText(/un autre projet porte déjà ce nom/)).toBeTruthy();
    expect((screen.getByRole('textbox', { name: 'Nom du projet' }) as HTMLInputElement).value).toBe('Autre');
  });

  it('archives a project: it leaves the list, its notes leave All cases, and it stays openable', async () => {
    render(<App />);
    await openSettings('Projet');
    fireEvent.click(screen.getByRole('button', { name: 'Archiver' }));
    await waitFor(() => expect(mockApi.setProjectArchived).toHaveBeenCalledWith('p1', true));
    const archived = await screen.findByRole('navigation', { name: 'Projets archivés' });
    expect(within(screen.getByRole('navigation', { name: 'Projets' })).queryByRole('button', { name: 'Projet' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Toutes les cases' }));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Ouvrir Urgente' })).toBeNull());
    fireEvent.click(within(archived).getByRole('button', { name: 'Projet' }));
    expect(await screen.findByRole('button', { name: 'Ouvrir Urgente' })).toBeTruthy();
  });

  it('deletes a project after confirmation and announces the notes sent to the Inbox', async () => {
    render(<App />);
    await openSettings('Projet');
    fireEvent.click(screen.getByRole('button', { name: 'Supprimer…' }));
    expect(mockApi.deleteProject).not.toHaveBeenCalled();
    expect(screen.getByText(/ses cases rejoignent la Réception/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Supprimer le projet' }));
    await waitFor(() => expect(mockApi.deleteProject).toHaveBeenCalledWith('p1'));
    expect(await screen.findByText(/2 cases ont rejoint la Réception/)).toBeTruthy();
  });
});

describe('App — note archiving (0.9.77)', () => {
  it('hides an archived note, shows it with "Archived" and finds it through the search', async () => {
    notesEnBase.current = [
      makeNote('active', 'Active', 'open', null),
      { ...makeNote('rangee', 'Mise de côté', 'open', null), archived_at: '2026-09-28T16:00:00.000Z' },
    ];
    render(<App />);
    fireEvent.click(await screen.findByRole('button', { name: 'Toutes les cases' }));
    expect(await screen.findByRole('button', { name: 'Ouvrir Active' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Ouvrir Mise de côté' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: /Archivées · 1/ }));
    expect(await screen.findByRole('button', { name: 'Ouvrir Mise de côté' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Ouvrir Active' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: /Archivées · 1/ }));
    fireEvent.change(screen.getByRole('searchbox', { name: 'Rechercher une case' }), { target: { value: 'côté' } });
    expect(await screen.findByRole('button', { name: 'Ouvrir Mise de côté' })).toBeTruthy();
  });
});

describe('App — notes written by an agent', () => {
  it('shows a note resolved by an agent without the window losing focus', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      let fingerprint = 'v1';
      mockApi.notesChangeMarker.mockImplementation(async () => fingerprint);
      render(<App />);
      await openProjectList();
      const callsBefore = mockApi.getNotes.mock.calls.length;

      // An agent resolves the note through MCP: only the database changes, no focus event.
      notesEnBase.current = notesEnBase.current.map(note => note.id === 'urgence' ? { ...note, status: 'resolved' } : note);
      fingerprint = 'v2';
      await vi.advanceTimersByTimeAsync(4500);

      await waitFor(() => expect(mockApi.getNotes.mock.calls.length).toBeGreaterThan(callsBefore));
      expect(focusListener.current).not.toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('App — projects and activities written by an agent', () => {
  // Fully simulated clock: the periodic read of projects and activities (5 s) does not
  // run before 5000 ms, the fingerprint is checked at 4000 ms. At 4050 ms, only the
  // fingerprint watch could have reread, which isolates what these tests verify.
  const rereads = async (fingerprintAfter: string) => {
    vi.useFakeTimers();
    try {
      let fingerprint = 'v1';
      mockApi.notesChangeMarker.mockImplementation(async () => fingerprint);
      render(<App />);
      await vi.advanceTimersByTimeAsync(100);
      const before = { projets: mockApi.getProjects.mock.calls.length, activites: mockApi.getActivities.mock.calls.length };

      // An agent creates a project and an activity through MCP: only the database changes.
      storedProjects.current = [...storedProjects.current, { id: 'p2', name: 'Projet d’agent', color: '#5273a8', space: 'work', created_at: '2026-09-29T10:00:00.000Z', updated_at: '2026-09-29T10:00:00.000Z', revision: 0 }];
      storedActivities.current = [{ id: 'a1', name: 'Recherche', space: 'work', created_at: '2026-09-29T10:00:00.000Z', updated_at: '2026-09-29T10:00:00.000Z', revision: 0, project_ids: [] }];
      fingerprint = fingerprintAfter;
      await vi.advanceTimersByTimeAsync(3950);
      return {
        projets: mockApi.getProjects.mock.calls.length - before.projets,
        activites: mockApi.getActivities.mock.calls.length - before.activites,
      };
    } finally {
      vi.useRealTimers();
    }
  };

  it('rereads projects and activities when the fingerprint changes', async () => {
    const newOnes = await rereads('v2');
    expect(newOnes.projets).toBeGreaterThan(0);
    expect(newOnes.activites).toBeGreaterThan(0);
  });

  it('rereads nothing as long as the fingerprint stays the same (control: the periodic read has not happened yet)', async () => {
    const newOnes = await rereads('v1');
    expect(newOnes).toEqual({ projets: 0, activites: 0 });
  });
});

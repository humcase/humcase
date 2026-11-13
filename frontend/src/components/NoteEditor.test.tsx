import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import NoteEditor from './NoteEditor';
import type { Note, Project } from '../types';

// Simulated Tauri event bus: the transcription proxy (Rust) emits
// `transcription://message` and `transcription://closed`.
const bus = vi.hoisted(() => {
  const handlers = new Map<string, Set<(e: { payload: unknown }) => void>>();
  return {
    handlers,
    listen: async (event: string, handler: (e: { payload: unknown }) => void) => {
      if (!handlers.has(event)) handlers.set(event, new Set());
      handlers.get(event)!.add(handler);
      return () => {
        handlers.get(event)?.delete(handler);
      };
    },
    emit: (event: string, payload: unknown) => {
      handlers.get(event)?.forEach(h => h({ payload }));
    },
    reset: () => handlers.clear(),
  };
});

const emittedToTauri = vi.hoisted(() => [] as Array<{ event: string; payload: unknown }>);
vi.mock('@tauri-apps/api/event', () => ({
  listen: bus.listen,
  emit: async (event: string, payload: unknown) => { emittedToTauri.push({ event, payload }); },
}));

// Simulated Tauri window: `onCloseRequested` makes it possible to check the flush on close.
const winBus = vi.hoisted(() => {
  const handlers: Array<() => void> = [];
  return {
    handlers,
    onCloseRequested: async (cb: () => void) => {
      handlers.push(cb);
      return () => {
        const i = handlers.indexOf(cb);
        if (i >= 0) handlers.splice(i, 1);
      };
    },
    destroy: async () => {},
    reset: () => {
      handlers.length = 0;
    },
  };
});

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ onCloseRequested: winBus.onCloseRequested, destroy: winBus.destroy }),
}));

// The Tauri layer is replaced: we test the editor's safeguards, not the IPC.
vi.mock('../services/tauriApi', () => ({
  api: {
    setNoteArchived: vi.fn(),
    updateNote: vi.fn(),
    appendRawContent: vi.fn(),
    retryNoteUpdate: vi.fn(),
    appendNoteContent: vi.fn(),
    getNote: vi.fn(),
    deleteNote: vi.fn(),
    trashIfEmpty: vi.fn(async () => false),
    restoreFromTrash: vi.fn(),
    findSimilarNotes: vi.fn(),
    recognizeCase: vi.fn(),
    cleanTranscript: vi.fn(),
    integrateDictation: vi.fn(),
    getNoteRelations: vi.fn(),
    getCaseContext: vi.fn(),
    createSubCase: vi.fn(),
    moveCase: vi.fn(),
    createLinkedTask: vi.fn(),
    linkNoteTask: vi.fn(),
    unlinkNoteTask: vi.fn(),
    getNoteHistory: vi.fn(),
    structureNote: vi.fn(),
    applyStructuredNote: vi.fn(),
    listAiProviders: vi.fn(),
    startTranscription: vi.fn(),
    sendAudioChunk: vi.fn(),
    stopTranscription: vi.fn(),
    saveImageToDisk: vi.fn(),
    germinateCase: vi.fn(),
    listProposals: vi.fn(async () => []),
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
}));

// Tiptap/ProseMirror does not run in jsdom: a plain textarea is enough for the value/onChange contract.
vi.mock('./TiptapEditor', async () => {
  const { useEffect, useRef } = await import('react');
  return {
    default: function MockEditor({ value, onChange, noteId, autoFocus = false, focusSignal = 0 }: { value: string; onChange: (value: string) => void; noteId: string; autoFocus?: boolean; focusSignal?: number }) {
      const ref = useRef<HTMLTextAreaElement>(null);
      useEffect(() => { if (autoFocus || focusSignal > 0) ref.current?.focus(); }, [autoFocus, focusSignal]);
      return <textarea ref={ref} aria-label={noteId.startsWith('structure-') ? 'proposition de structure' : 'contenu'} value={value} onChange={e => onChange(e.target.value)} />;
    },
  };
});

import { api } from '../services/tauriApi';
import i18n from '../i18n';
import { addPendingWrite, readTaskDraft, readPendingWrites, clearPendingWrites } from '../utils/pendingWrites';
import { prepareSnapshot, type BackupPreparation } from '../utils/backupBarrier';
import { deriveTitle } from '../utils/title';

const mockApi = vi.mocked(api);

const makeNote = (over: Partial<Note> = {}): Note => ({
  id: 'n1',
  project_id: 'p1',
  title: 'Titre',
  content: 'Contenu',
  note_type: 'note',
  status: 'open',
  resolution_comment: null,
  raw_content: null,
  priority: null,
  git_branch: null,
  tags: null,
  created_at: '2026-09-22T10:00:00.000Z',
  updated_at: '2026-09-22T10:00:00.000Z',
  due_at: null,
  revision: 0,
  ...over,
});

/** Escrow key: target of the "a single write" assertions (spy on localStorage.setItem). */
const ESCROW_KEY = 'humcase-pending-writes';

const titleField = () => screen.getByPlaceholderText('Titre de la case…') as HTMLInputElement;
const contentField = () => screen.getByLabelText('contenu') as HTMLTextAreaElement;
const dictationButton = () => screen.getByRole('button', { name: 'Dicter' });
const structureButton = () => screen.getByRole('button', { name: 'Structurer' });
const applyButton = () => screen.getByRole('button', { name: /^Appliquer/ });
const selectPriority = () =>
  screen
    .getAllByRole('combobox')
    .find(s => [...(s as HTMLSelectElement).options].some(o => o.value === 'P0')) as HTMLSelectElement;

class FakeMediaRecorder {
  static last: FakeMediaRecorder | null = null;
  static deferFinal = false;
  state = 'inactive';
  stream: unknown;
  ondataavailable: ((event: { data: { size: number; arrayBuffer: () => Promise<ArrayBuffer> } }) => void) | null = null;
  private listeners = new Map<string, Set<() => void>>();
  addEventListener = vi.fn((type: string, cb: () => void) => {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(cb);
  });
  removeEventListener = vi.fn();
  private emit(type: string) {
    this.listeners.get(type)?.forEach(cb => cb());
  }
  start = vi.fn(() => {
    this.state = 'recording';
  });
  // Like the real MediaRecorder: `stop()` produces a **last** `dataavailable` then `stop`.
  deliverFinal = () => {
    this.ondataavailable?.({ data: { size: 3, arrayBuffer: async () => new ArrayBuffer(3) } });
    this.emit('stop');
  };
  stop = vi.fn(() => {
    this.state = 'inactive';
    if (!FakeMediaRecorder.deferFinal) this.deliverFinal();
  });

  constructor(stream: unknown) {
    this.stream = stream;
    FakeMediaRecorder.last = this;
  }
}

/** Replaces MediaRecorder and getUserMedia: the Deepgram session is simulated by the bus. */
function installDictationEnvironment() {
  bus.reset();
  FakeMediaRecorder.last = null;
  FakeMediaRecorder.deferFinal = false;
  vi.stubGlobal('MediaRecorder', FakeMediaRecorder);
  Object.defineProperty(navigator, 'mediaDevices', {
    configurable: true,
    value: { getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [{ stop: vi.fn() }] }) },
  });
}

/** Starts a dictation and waits for the event listeners to be in place. */
async function startDictation() {
  fireEvent.click(dictationButton());
  await waitFor(() => expect(mockApi.startTranscription).toHaveBeenCalled());
  await waitFor(() =>
    expect(bus.handlers.get('transcription://message')?.size ?? 0).toBeGreaterThan(0)
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  // Unknown AI state by default (list not provided): the AI actions are not locked.
  mockApi.listAiProviders.mockReset();
  bus.reset();
  winBus.reset();
  clearPendingWrites();
  localStorage.clear();
  // The historical dictation tests cover simple appending; smart dictation (0.9.80)
  // has its own tests, which re-enable this setting.
  localStorage.setItem('dictation_auto_structure', 'false');
  mockApi.updateNote.mockResolvedValue(undefined);
  mockApi.appendRawContent.mockResolvedValue(undefined);
  mockApi.retryNoteUpdate.mockResolvedValue('applied');
  mockApi.appendNoteContent.mockResolvedValue(undefined);
  mockApi.deleteNote.mockResolvedValue(undefined);
  mockApi.restoreFromTrash.mockResolvedValue(undefined);
  mockApi.findSimilarNotes.mockResolvedValue([]);
  mockApi.recognizeCase.mockResolvedValue({ nature: null, line_of_work: null });
  mockApi.cleanTranscript.mockResolvedValue('Texte nettoyé');
  mockApi.getNoteRelations.mockResolvedValue({ sources: [], tasks: [] });
  mockApi.getCaseContext.mockResolvedValue({ path: [], provenance: [] });
  mockApi.getNoteHistory.mockResolvedValue([]);
  mockApi.structureNote.mockResolvedValue('## Proposition');
  mockApi.applyStructuredNote.mockImplementation(async (_noteId, _source, result) => makeNote({ content: result, raw_content: 'Contenu' }));
  mockApi.startTranscription.mockResolvedValue(undefined);
  mockApi.sendAudioChunk.mockResolvedValue(undefined);
  mockApi.stopTranscription.mockResolvedValue(undefined);
});

// Templates are only offered for a note of the Work space (space read from the project, no longer from the label).
const workProject: Project = { id: 'p1', name: 'Projet', color: '#6353a5', space: 'work', created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 0 };

function chooseTemplate(label: string) {
  fireEvent.keyDown(screen.getByRole('button', { name: 'Modèle de case' }), { key: 'Enter' });
  fireEvent.click(screen.getByRole('menuitem', { name: new RegExp(label) }));
}

describe('NoteEditor — templates', () => {
  it('replaces an intact template instead of stacking them', async () => {
    render(<NoteEditor note={makeNote({ content: '' })} onNoteUpdated={() => {}} structuringContextLabel="Pro · Projet" projects={[workProject]} />);
    chooseTemplate('Rapport d’incident');
    expect(contentField().value).toContain('Comportement observé');
    chooseTemplate('Liste de vérification');
    expect(contentField().value).toContain('Vérifications');
    expect(contentField().value).not.toContain('Comportement observé');
  });

  it('asks for confirmation before replacing a modified text', () => {
    render(<NoteEditor note={makeNote({ content: 'Ma pensée' })} onNoteUpdated={() => {}} structuringContextLabel="Pro · Projet" projects={[workProject]} />);
    chooseTemplate('Note de décision');
    expect(contentField().value).toBe('Ma pensée');
    fireEvent.click(screen.getByRole('button', { name: 'Conserver ma case' }));
    expect(contentField().value).toBe('Ma pensée');
    chooseTemplate('Note de décision');
    fireEvent.click(screen.getByRole('button', { name: 'Remplacer le contenu' }));
    expect(contentField().value).toContain('Décision');
    expect(contentField().value).not.toContain('Ma pensée');
  });
});

describe('NoteEditor — proposition de structure', () => {
  it('waits for the saved draft, allows correcting and applies via CAS with an intact source', async () => {
    const note = makeNote();
    mockApi.getNote.mockResolvedValue({ ...note, content: 'Texte récent' });
    let finishSave: () => void = () => {};
    mockApi.updateNote.mockReturnValueOnce(new Promise<void>(resolve => { finishSave = resolve; }));
    const onNoteUpdated = vi.fn();
    render(<NoteEditor note={note} onNoteUpdated={onNoteUpdated} structuringContextLabel="Pro · Projet · Marketing" />);
    fireEvent.change(contentField(), { target: { value: 'Texte récent' } });
    fireEvent.click(structureButton());
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { content: 'Texte récent' }));
    expect(mockApi.structureNote).not.toHaveBeenCalled();
    await act(async () => { finishSave(); });
    await waitFor(() => expect(mockApi.structureNote).toHaveBeenCalledWith('n1', 'Texte récent', expect.anything(), undefined, 'auto'));
    expect(screen.getByText('Contexte : Pro · Projet · Marketing')).toBeTruthy();
    fireEvent.change(screen.getByLabelText('proposition de structure'), { target: { value: '## Version corrigée' } });
    fireEvent.click(applyButton());
    await waitFor(() => expect(mockApi.applyStructuredNote).toHaveBeenCalledWith('n1', 'Texte récent', '## Version corrigée'));
    expect(contentField().value).toBe('## Version corrigée');
    expect(onNoteUpdated).toHaveBeenCalled();
  });

  it('Undo after a structuring restores the previous version without touching the raw text (0.9.78)', async () => {
    const note = makeNote({ content: 'Texte source' });
    mockApi.getNote.mockResolvedValue(note);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(structureButton());
    await waitFor(() => expect(applyButton()).toBeTruthy());
    fireEvent.click(applyButton());
    await waitFor(() => expect(contentField().value).toBe('## Proposition'));
    expect(await screen.findByText(/La version précédente est dans l’historique/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(mockApi.applyStructuredNote).toHaveBeenLastCalledWith('n1', '## Proposition', 'Texte source'));
    await waitFor(() => expect(contentField().value).toBe('Texte source'));
  });

  it('relaunches the proposal with the chosen frame and remembers it (0.9.78)', async () => {
    const note = makeNote({ content: 'Conversation' });
    mockApi.getNote.mockResolvedValue(note);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(structureButton());
    await waitFor(() => expect(mockApi.structureNote).toHaveBeenCalledWith('n1', 'Conversation', expect.anything(), undefined, 'auto'));
    await waitFor(() => expect(applyButton()).toBeTruthy());
    fireEvent.change(screen.getByLabelText('Trame'), { target: { value: 'reflection' } });
    await waitFor(() => expect(mockApi.structureNote).toHaveBeenLastCalledWith('n1', 'Conversation', expect.anything(), undefined, 'reflection'));
    expect(localStorage.getItem('structure_frame')).toBe('reflection');
    localStorage.removeItem('structure_frame');
  });

  it('does not allow applying a slow response after a local input', async () => {
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    let finish: (text: string) => void = () => {};
    mockApi.structureNote.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(structureButton());
    await waitFor(() => expect(mockApi.structureNote).toHaveBeenCalled());
    fireEvent.change(contentField(), { target: { value: 'Contenu ajouté' } });
    await act(async () => { finish('## Proposition ancienne'); });
    expect(applyButton().hasAttribute('disabled')).toBe(true);
    expect(contentField().value).toBe('Contenu ajouté');
    expect(mockApi.applyStructuredNote).not.toHaveBeenCalled();
  });

  it('keeps an input that arrived during the slow application', async () => {
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    let finish: (saved: Note) => void = () => {};
    mockApi.applyStructuredNote.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(structureButton());
    await waitFor(() => expect(applyButton()).toBeTruthy());
    fireEvent.click(applyButton());
    await waitFor(() => expect(mockApi.applyStructuredNote).toHaveBeenCalled());
    fireEvent.change(contentField(), { target: { value: 'Texte saisi pendant l’application' } });
    await act(async () => { finish(makeNote({ content: '## Proposition' })); });
    expect(contentField().value).toBe('Texte saisi pendant l’application');
  });

  it('ignores the response after a note change and cancellation', async () => {
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    let finish: (text: string) => void = () => {};
    mockApi.structureNote.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    const { rerender } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(structureButton());
    await waitFor(() => expect(mockApi.structureNote).toHaveBeenCalled());
    rerender(<NoteEditor note={makeNote({ id: 'n2', content: 'Autre' })} onNoteUpdated={() => {}} />);
    await act(async () => { finish('## Réponse tardive'); });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(contentField().value).toBe('Autre');

    mockApi.getNote.mockResolvedValue(makeNote({ id: 'n2', content: 'Autre' }));
    fireEvent.click(structureButton());
    await waitFor(() => expect(screen.getByRole('dialog')).toBeTruthy());
    await waitFor(() => expect(applyButton()).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(mockApi.applyStructuredNote).not.toHaveBeenCalled();
  });

  it('shows the AI error or the CAS conflict without writing the content', async () => {
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    mockApi.structureNote.mockRejectedValueOnce(new Error('Cerebras HTTP 401'));
    const { unmount } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(structureButton());
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('401'));
    expect(contentField().value).toBe('Contenu');
    unmount();

    mockApi.structureNote.mockResolvedValue('## Proposition');
    mockApi.applyStructuredNote.mockRejectedValueOnce(new Error('Conflit : contenu modifié'));
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(structureButton());
    await waitFor(() => expect(applyButton()).toBeTruthy());
    fireEvent.click(applyButton());
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Conflit'));
    expect(contentField().value).toBe('Contenu');
    expect(screen.getByRole('dialog')).toBeTruthy();
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('NoteEditor — data non-regression', () => {
  it('flushes the draft before the debounce ends to prepare an archive', async () => {
    let prepare: (() => Promise<BackupPreparation>) | null = null;
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}}
      onRegisterPrepareBackup={fn => { prepare = fn; }} />);
    fireEvent.change(contentField(), { target: { value: 'Brouillon récent' } });
    expect(mockApi.updateNote).not.toHaveBeenCalled();

    const result = await prepareSnapshot(prepare);
    expect(result).toEqual({ ready: true });
    expect(mockApi.updateNote).toHaveBeenCalledWith('n1', expect.objectContaining({ content: 'Brouillon récent' }));
  });

  it("waits for the in-flight write of an unmounted instance", async () => {
    let release!: () => void;
    mockApi.updateNote.mockImplementationOnce(() => new Promise<void>(resolve => { release = resolve; }));
    let prepare: (() => Promise<BackupPreparation>) | null = null;
    const onRegisterPrepareBackup = (fn: (() => Promise<BackupPreparation>) | null) => { prepare = fn; };
    const previous = render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}}
      onRegisterPrepareBackup={onRegisterPrepareBackup} />);
    fireEvent.change(contentField(), { target: { value: 'Ancien brouillon' } });
    previous.unmount(); // Triggers the flush of the old editor.
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalled());
    const current = render(<NoteEditor note={makeNote({ id: 'n2' })} onNoteUpdated={() => {}}
      onRegisterPrepareBackup={onRegisterPrepareBackup} />);
    let finishedFlag = false;
    const pending = prepareSnapshot(prepare).then(result => { finishedFlag = true; return result; });
    await act(async () => { await Promise.resolve(); });
    expect(finishedFlag).toBe(false);
    release();
    expect(await pending).toEqual({ ready: true });
    current.unmount();
  });

  it("refuses the archive if the flush fails and puts the draft in the escrow", async () => {
    mockApi.updateNote.mockRejectedValueOnce(new Error('disque plein'));
    let prepare: (() => Promise<BackupPreparation>) | null = null;
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}}
      onRegisterPrepareBackup={fn => { prepare = fn; }} />);
    fireEvent.change(contentField(), { target: { value: 'À conserver' } });

    const result = await prepareSnapshot(prepare);
    expect(result.ready).toBe(false);
    expect(readPendingWrites()[0]).toMatchObject({ kind: 'update', noteId: 'n1', payload: { content: 'À conserver' } });
  });

  it("refuses the archive during an active dictation", async () => {
    installDictationEnvironment();
    let prepare: (() => Promise<BackupPreparation>) | null = null;
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}}
      onRegisterPrepareBackup={fn => { prepare = fn; }} />);
    await startDictation();

    const result = await prepareSnapshot(prepare);
    expect(result).toEqual({ ready: false, reason: expect.stringMatching(/dictée/) });
    expect(mockApi.updateNote).not.toHaveBeenCalled();
  });

  it("a backup preparation is not blocked by a linked-task draft", async () => {
    addPendingWrite({ kind: 'task-draft', noteId: 'n1', titre: 'Tâche en cours', contenu: 'Détails' });
    expect(await prepareSnapshot(null)).toEqual({ ready: true });
  });

  it('a pending DB write (update or raw) always blocks the backup', async () => {
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'À récupérer' } });
    expect((await prepareSnapshot(null)).ready).toBe(false);
    clearPendingWrites();
    addPendingWrite({ kind: 'brut', noteId: 'n1', texte: 'Dictée brute à récupérer' });
    expect((await prepareSnapshot(null)).ready).toBe(false);
  });

  it('shows at once a write another case kept, as a drawing does when its write fails', async () => {
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    expect(screen.queryByText(/écritures? non enregistrées?,/)).toBeNull();
    act(() => { addPendingWrite({ kind: 'update', noteId: 'sketch-1', payload: { content: 'Dessin à récupérer' } }); });
    expect(await screen.findByText(/1 écriture non enregistrée,/)).toBeTruthy();
  });

  it('gives an explicit label to the title field', () => {
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    expect(screen.getByRole('textbox', { name: 'Titre de la case' })).toBe(titleField());
  });

  it('1. keeps the local input when the same note is refreshed (slow save)', () => {
    const note = makeNote();
    const { rerender } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    fireEvent.change(titleField(), { target: { value: 'Frappe en cours' } });
    expect(titleField().value).toBe('Frappe en cours');

    // The save response (or an MCP refresh) returns the same note, older.
    rerender(<NoteEditor note={{ ...note, title: 'Titre', content: 'Contenu' }} onNoteUpdated={() => {}} />);

    expect(titleField().value).toBe('Frappe en cours');
  });

  it("2. a dictation is never injected into another note's editor", async () => {
    installDictationEnvironment();
    const noteA = makeNote({ id: 'n1', title: 'Note A', content: 'Contenu A' });
    const noteB = makeNote({ id: 'n2', title: 'Note B', content: 'Contenu B' });
    mockApi.getNote.mockImplementation(async (id: string) => (id === 'n1' ? noteA : noteB));

    const { rerender } = render(<NoteEditor note={noteA} onNoteUpdated={() => {}} />);
    await startDictation();

    // The user changes note during the dictation.
    rerender(<NoteEditor note={noteB} onNoteUpdated={() => {}} />);
    expect(contentField().value).toBe('Contenu B');

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'bonjour' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });

    // The dictation is written to the targeted note (n1), never to the displayed note (n2)…
    await waitFor(() => expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'bonjour'));
    // …and the content goes through an atomic append (no get-modify-write that would overwrite MCP).
    await waitFor(() =>
      expect(mockApi.appendNoteContent).toHaveBeenCalledWith('n1', 'Texte nettoyé', 'Texte nettoyé')
    );
    expect(mockApi.updateNote).not.toHaveBeenCalledWith('n1', expect.objectContaining({ content: expect.anything() }));
    expect(mockApi.updateNote).not.toHaveBeenCalledWith('n2', expect.anything());
    // …and the editor still displays note B (no cross injection).
    expect(contentField().value).toBe('Contenu B');
  });

  it('3. a Cerebras cleanup failure keeps the raw dictation and reports it', async () => {
    installDictationEnvironment();
    mockApi.cleanTranscript.mockRejectedValue(new Error('Cerebras HTTP 401'));
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({
          channel: { alternatives: [{ transcript: 'bonjour le monde' }] },
          is_final: true,
        })
      );
      bus.emit('transcription://closed', undefined);
    });

    // The raw text is persisted BEFORE the LLM call, hence despite the cleanup failure.
    await waitFor(() =>
      expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'bonjour le monde')
    );
    await screen.findByText(/Échec du nettoyage IA/);
  });

  // "Missing key" contract: the interface reads the stable code returned by Rust, never the translated
  // text. The last two cases fail if we go back to detection by the `XXX_API_KEY` regex.
  it.each([
    ['stable code with variable name', '[ai_key_missing] CEREBRAS_API_KEY manquante', /clé API manquante/],
    ['stable code, text without variable name', '[ai_key_missing] Missing key', /clé API manquante/],
    ['other error citing a variable, no code', 'Cerebras HTTP 401 (OPENAI_API_KEY refused)', /Échec du nettoyage IA/],
  ])('3b. detects the missing key by its code (%s)', async (_case, error, expectedToast) => {
    installDictationEnvironment();
    // Tauri rejects with the error string returned by the Rust command.
    mockApi.cleanTranscript.mockRejectedValue(error);
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'bonjour' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });

    await waitFor(() => expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'bonjour'));
    await screen.findByText(expectedToast);
  });

  it('archives then unarchives the note without deleting it (0.9.77)', async () => {
    mockApi.setNoteArchived.mockResolvedValue(undefined);
    const onNoteUpdated = vi.fn();
    const { rerender } = render(<NoteEditor note={makeNote()} onNoteUpdated={onNoteUpdated} />);
    fireEvent.click(screen.getByRole('button', { name: 'Archiver' }));
    await waitFor(() => expect(mockApi.setNoteArchived).toHaveBeenCalledWith('n1', true));
    expect(await screen.findByText(/Case archivée/)).toBeTruthy();
    expect(onNoteUpdated).toHaveBeenCalled();
    expect(mockApi.deleteNote).not.toHaveBeenCalled();
    rerender(<NoteEditor note={makeNote({ archived_at: '2026-09-28T16:00:00.000Z' })} onNoteUpdated={onNoteUpdated} />);
    fireEvent.click(screen.getByRole('button', { name: 'Désarchiver' }));
    await waitFor(() => expect(mockApi.setNoteArchived).toHaveBeenCalledWith('n1', false));
  });

  it("4. deleting moves the note to the trash; Undo takes it out", async () => {
    const onNoteUpdated = vi.fn();
    render(<NoteEditor note={makeNote()} onNoteUpdated={onNoteUpdated} />);

    fireEvent.click(screen.getByLabelText('Mettre la case à la corbeille'));
    fireEvent.click(screen.getByRole('button', { name: 'Mettre à la corbeille' }));
    await waitFor(() => expect(mockApi.deleteNote).toHaveBeenCalledWith('n1'));
    expect(await screen.findByText(/placée dans la corbeille/)).toBeTruthy();

    fireEvent.click(await screen.findByText('Annuler'));
    await waitFor(() => expect(mockApi.restoreFromTrash).toHaveBeenCalledWith('n1'));
    await waitFor(() => expect(screen.queryByText(/placée dans la corbeille/)).toBeNull());
    expect(onNoteUpdated).toHaveBeenCalledTimes(2);
  });

  it('quick capture to the open note: appends to the unsaved draft then confirms (0.9.83)', async () => {
    emittedToTauri.length = 0;
    render(<NoteEditor note={makeNote({ content: 'Début' })} onNoteUpdated={() => {}} />);
    await waitFor(() => expect(bus.handlers.get('humcase:qc-append')?.size ?? 0).toBeGreaterThan(0));
    // Local input not yet saved: it must not be overwritten by the append.
    fireEvent.change(screen.getByLabelText('contenu'), { target: { value: 'Début modifié' } });

    await act(async () => { bus.emit('humcase:qc-append', { requestId: 'autre', noteId: 'n2', text: 'Pas pour moi' }); });
    await act(async () => { bus.emit('humcase:qc-append', { requestId: 'r1', noteId: 'n1', text: '  Idée capturée  ' }); });

    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledWith('n1', expect.objectContaining({ content: 'Début modifié\n\nIdée capturée' })));
    expect(JSON.stringify(mockApi.updateNote.mock.calls)).not.toContain('Pas pour moi');
    expect(emittedToTauri).toEqual([{ event: 'humcase:qc-append-result', payload: { requestId: 'r1', ok: true } }]);
  });

  it("4 bis. a failure to take out of the trash keeps \"Undo\" available", async () => {
    mockApi.restoreFromTrash.mockRejectedValueOnce(new Error('verrou'));
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByLabelText('Mettre la case à la corbeille'));
    fireEvent.click(screen.getByRole('button', { name: 'Mettre à la corbeille' }));
    fireEvent.click(await screen.findByText('Annuler'));
    expect(await screen.findByText(/retrouvez la case dans la corbeille/)).toBeTruthy();
    expect(screen.getByText('Annuler')).toBeTruthy();
  });

  it('waits for the draft save before opening a linked task', async () => {
    const source = makeNote({ id: 'source', content: 'Idée' });
    const linked = makeNote({ id: 'task', title: 'Tâche liée', note_type: 'todo' });
    mockApi.getNoteRelations.mockResolvedValue({ sources: [], tasks: [linked] });
    let finishSave: () => void = () => {};
    mockApi.updateNote.mockReturnValueOnce(new Promise<void>(resolve => { finishSave = resolve; }));
    const open = vi.fn();
    render(<NoteEditor note={source} onNoteUpdated={() => {}} allNotes={[source, linked]} onOpenRelatedNote={open} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /Tâches liées/ }).textContent).toContain('1'));
    fireEvent.change(contentField(), { target: { value: 'Idée modifiée' } });
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Tâche liée' }));
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledWith('source', { content: 'Idée modifiée' }));
    expect(open).not.toHaveBeenCalled();
    await act(async () => { finishSave(); });
    await waitFor(() => expect(open).toHaveBeenCalledWith(linked));
  });

  it('duplicates neither the report nor the relations when the same note is refreshed', async () => {
    const note = makeNote({ id: 'n1', resolution_comment: null });
    const props = { onNoteUpdated: () => {}, allNotes: [note], onOpenRelatedNote: () => {} };
    const { rerender } = render(<NoteEditor note={note} {...props} />);
    for (let i = 0; i < 4; i += 1) rerender(<NoteEditor note={{ ...note }} {...props} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /Tâches liées/ }).textContent).toContain('0'));
    expect(screen.getAllByRole('button', { name: 'Ajouter un compte rendu' })).toHaveLength(1);
    expect(screen.getAllByRole('region', { name: 'Liens entre cases et tâches' })).toHaveLength(1);
  });

  it('blocks the link if the outgoing draft fails and keeps the text recoverable', async () => {
    const source = makeNote({ id: 'source', content: 'Idée' });
    const linked = makeNote({ id: 'task', title: 'Tâche liée', note_type: 'todo' });
    mockApi.getNoteRelations.mockResolvedValue({ sources: [], tasks: [linked] });
    mockApi.updateNote.mockRejectedValueOnce(new Error('disque plein'));
    const open = vi.fn();
    render(<NoteEditor note={source} onNoteUpdated={() => {}} allNotes={[source, linked]} onOpenRelatedNote={open} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /Tâches liées/ }).textContent).toContain('1'));
    fireEvent.change(contentField(), { target: { value: 'Texte non sauvegardé' } });
    fireEvent.click(screen.getByRole('button', { name: /Tâches liées/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Tâche liée' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('brouillon'));
    expect(open).not.toHaveBeenCalled();
    expect(readPendingWrites()).toEqual(expect.arrayContaining([expect.objectContaining({ noteId: 'source', payload: { content: 'Texte non sauvegardé' } })]));
  });

  it('5. removing the priority sends an empty string (removal), not null', async () => {
    const note = makeNote({ priority: 'P2' });
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    fireEvent.change(selectPriority(), { target: { value: '' } });

    await waitFor(() =>
      expect(mockApi.updateNote).toHaveBeenCalledWith('n1', expect.objectContaining({ priority: '' }))
    );
  });

  it("6. a write failure (nonexistent note) is reported without losing the input", async () => {
    const note = makeNote();
    const onNoteUpdated = vi.fn();
    mockApi.updateNote.mockRejectedValueOnce(new Error('Note introuvable'));

    render(<NoteEditor note={note} onNoteUpdated={onNoteUpdated} />);
    fireEvent.change(screen.getByRole('combobox', { name: 'Nature de la case' }), { target: { value: 'bug' } });

    await screen.findByText("Échec de l’enregistrement");
    // A failure must neither reread the database nor suggest that the note is up to date.
    expect(onNoteUpdated).not.toHaveBeenCalled();
    expect(titleField().value).toBe('Titre');
  });

  it('escrows a failed autosave with its base and only removes the fields confirmed afterwards', async () => {
    const note = makeNote({ priority: null });
    const reportDraft = addPendingWrite({ kind: 'update', noteId: 'n1', payload: { resolution_comment: 'Compte rendu en attente' }, base: { resolution_comment: null } });
    mockApi.updateNote.mockRejectedValueOnce(new Error('disque plein')).mockResolvedValue(undefined);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.change(selectPriority(), { target: { value: 'P1' } });
    await waitFor(() => expect(readPendingWrites()).toEqual(expect.arrayContaining([
      expect.objectContaining({ kind: 'update', noteId: 'n1', payload: { priority: 'P1' }, base: { priority: '' } }),
    ])));
    expect(screen.getByText(/écritures? non enregistrées?,/)).toBeTruthy();
    fireEvent.change(selectPriority(), { target: { value: 'P2' } });
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { priority: 'P2' }));
    // The report draft survives; the displayed note back-fills its project anchor.
    await waitFor(() => expect(readPendingWrites()).toHaveLength(1));
    expect(readPendingWrites()[0]).toMatchObject({
      id: reportDraft.id,
      kind: 'update',
      noteId: 'n1',
      payload: { resolution_comment: 'Compte rendu en attente' },
      projectId: 'p1',
    });
  });

  it('Ignore does not remove a new report draft absent from the displayed list', async () => {
    const old = addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'ancien échec' }, base: { content: 'Contenu' } });
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    expect(screen.getByText(/1 écriture non enregistrée,/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Ajouter un compte rendu' }));
    fireEvent.change(screen.getByLabelText('Résultat et limites'), { target: { value: 'Résultat à conserver' } });
    fireEvent.click(screen.getByRole('button', { name: 'Fermer' }));
    expect(readPendingWrites()).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'Ignorer ces écritures' }));
    expect(readPendingWrites()).toEqual([expect.objectContaining({ payload: { resolution_comment: 'Résultat à conserver' } })]);
    expect(readPendingWrites().some(entry => entry.id === old.id)).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'Reprendre le brouillon du compte rendu' }));
    expect((screen.getByLabelText('Résultat et limites') as HTMLTextAreaElement).value).toBe('Résultat à conserver');
  });

  it('Retry does not validate a report added after the list was rendered', async () => {
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'ancien échec' }, base: { content: 'Contenu' } });
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: 'Ajouter un compte rendu' }));
    fireEvent.change(screen.getByLabelText('Résultat et limites'), { target: { value: 'Brouillon nouveau' } });
    fireEvent.click(screen.getByRole('button', { name: 'Fermer' }));
    fireEvent.click(screen.getByRole('button', { name: 'Réessayer' }));
    await waitFor(() => expect(mockApi.retryNoteUpdate).toHaveBeenCalledTimes(1));
    expect(mockApi.retryNoteUpdate).toHaveBeenCalledWith('n1', { content: 'ancien échec' }, { content: 'Contenu' });
    expect(readPendingWrites()).toEqual([expect.objectContaining({ payload: { resolution_comment: 'Brouillon nouveau' } })]);
  });

  it('7. the chosen AI provider is passed to the Rust commands', async () => {
    installDictationEnvironment();
    localStorage.setItem('ai_clean_provider', 'ollama');
    localStorage.setItem('ai_stt_provider', 'deepgram');
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    // Transcription receives the voice provider and, by default, the interface language…
    expect(mockApi.startTranscription).toHaveBeenCalledWith('deepgram', i18n.resolvedLanguage ?? i18n.language);

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'bonjour' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });

    // …and the cleanup the LLM provider chosen in Settings (empty model → default).
    await waitFor(() => expect(mockApi.cleanTranscript).toHaveBeenCalledWith('bonjour', 'ollama', undefined, 'n1'));
    localStorage.removeItem('ai_clean_provider');
    localStorage.removeItem('ai_stt_provider');
  });

  const noAiConnected = () => mockApi.listAiProviders.mockResolvedValue({
    cleanup: [{
      id: 'cerebras', label: 'Cerebras', key_env: 'CEREBRAS_API_KEY', default_model: 'gpt-oss-120b',
      url_env: '', model_env: '', needs_key: true, key_defined: false, key_source: 'none',
      url_editable: false, url: 'https://api.cerebras.ai/v1/chat/completions', ready: false,
    }],
    transcription: [],
    config_alert: null,
  });

  it('8. without a connected AI, Structure is locked and "Connect an AI" opens Settings', async () => {
    noAiConnected();
    const opened = vi.fn();
    window.addEventListener('humcase:open-settings', opened);
    render(<NoteEditor note={makeNote({ content: 'Texte' })} onNoteUpdated={() => {}} />);
    await waitFor(() => expect((structureButton() as HTMLButtonElement).disabled).toBe(true));
    expect(structureButton().getAttribute('title')).toMatch(/Ajoutez une clé IA/);
    fireEvent.click(screen.getByRole('button', { name: 'Brancher une IA' }));
    expect(opened).toHaveBeenCalledTimes(1);
    window.removeEventListener('humcase:open-settings', opened);
  });

  it('9. without a connected AI, the dictation is appended as dictated, raw text kept, no AI call', async () => {
    installDictationEnvironment();
    noAiConnected();
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();
    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'bonjour' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });
    await waitFor(() => expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'bonjour'));
    await screen.findByText(/Dictée ajoutée sans nettoyage/);
    expect(contentField().value).toContain('bonjour');
    expect(mockApi.cleanTranscript).not.toHaveBeenCalled();
    expect(mockApi.structureNote).not.toHaveBeenCalled();
  });

  const dateField = () => document.querySelector('.due-date-field input') as HTMLInputElement;

  it("8. adopts an external modification (MCP) of a field NOT modified without overwriting it", () => {
    const note = makeNote({ due_at: null });
    const { rerender } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    expect(dateField().value).toBe('');

    // An agent adds a due date while the note is open, without the user touching it.
    rerender(<NoteEditor note={{ ...note, due_at: '2026-12-31' }} onNoteUpdated={() => {}} />);

    // The external value is adopted, and no write comes to erase it.
    expect(dateField().value).toBe('31/12/2026');
    expect(mockApi.updateNote).not.toHaveBeenCalled();
  });

  it("8b. a local modification never sends a field modified elsewhere", async () => {
    const note = makeNote({ due_at: null });
    const { rerender } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    fireEvent.change(titleField(), { target: { value: 'Titre local' } });
    // While the title is being edited, the due date changes on the agent (MCP) side.
    rerender(<NoteEditor note={{ ...note, due_at: '2026-12-31' }} onNoteUpdated={() => {}} />);
    expect(dateField().value).toBe('31/12/2026');

    // Autosave must only send the title: never the due date (which was not touched).
    await waitFor(
      () => expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { title: 'Titre local' }),
      { timeout: 3000 }
    );
    expect(mockApi.updateNote).not.toHaveBeenCalledWith('n1', expect.objectContaining({ due_at: expect.anything() }));
  });

  it("8c. adopts title, tags and priority modified by an agent without losing the local input (ab3ce49d)", () => {
    const note = makeNote({
      title: 'Titre initial',
      content: 'Contenu initial',
      tags: JSON.stringify(['alpha']),
      priority: null,
      revision: 1,
    });
    const { rerender } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    // The user edits the CONTENT, with no save yet passed through autosave.
    fireEvent.change(contentField(), { target: { value: 'Saisie locale non sauvegardée' } });
    expect(screen.getByText('#alpha')).toBeTruthy();

    // An MCP agent (higher revision) changes the title, tags and priority.
    rerender(<NoteEditor
      note={makeNote({
        title: 'Titre modifié par l’agent',
        content: 'Contenu initial',
        tags: JSON.stringify(['beta']),
        priority: 'P1',
        revision: 2,
      })}
      onNoteUpdated={() => {}}
    />);

    // Fields not edited locally adopt the agent's values…
    expect(titleField().value).toBe('Titre modifié par l’agent');
    expect(selectPriority().value).toBe('P1');
    expect(screen.getByText('#beta')).toBeTruthy();
    expect(screen.queryByText('#alpha')).toBeNull();
    // …and the unsaved local content input survives.
    expect(contentField().value).toBe('Saisie locale non sauvegardée');
  });

  it("9. the delete confirmation cannot target another note", () => {
    const noteA = makeNote({ id: 'n1', title: 'Note A' });
    const noteB = makeNote({ id: 'n2', title: 'Note B' });
    const { rerender } = render(<NoteEditor note={noteA} onNoteUpdated={() => {}} />);

    fireEvent.click(screen.getByLabelText('Mettre la case à la corbeille'));
    expect(screen.getByRole('button', { name: 'Mettre à la corbeille' })).toBeTruthy();

    // Note change: the confirmation must not follow.
    rerender(<NoteEditor note={noteB} onNoteUpdated={() => {}} />);
    expect(screen.queryByRole('button', { name: 'Mettre à la corbeille' })).toBeNull();
    expect(mockApi.deleteNote).not.toHaveBeenCalled();
  });

  it("9a. the title carries the class its layout is styled by, in every language", () => {
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    expect(titleField().classList.contains('note-editor-title')).toBe(true);
  });

  it("9b. typing not yet saved is saved before the note goes to the trash", async () => {
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.change(contentField(), { target: { value: 'Dernière phrase' } });
    fireEvent.click(screen.getByLabelText('Mettre la case à la corbeille'));
    fireEvent.click(screen.getByRole('button', { name: 'Mettre à la corbeille' }));
    await waitFor(() => expect(mockApi.deleteNote).toHaveBeenCalledWith('n1'));
    expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { content: 'Dernière phrase' });
    expect(mockApi.updateNote.mock.invocationCallOrder[0]).toBeLessThan(mockApi.deleteNote.mock.invocationCallOrder[0]);
  });

  it("10. keeps the raw dictation in the escrow if its write fails", async () => {
    installDictationEnvironment();
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    mockApi.appendRawContent.mockRejectedValueOnce(new Error('disque plein'));

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'texte a sauver' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });

    // The unsaved text joins the recoverable escrow (not a state that dies with the
    // component): it therefore survives a later dictation and an unmount.
    await waitFor(() => expect(readPendingWrites()).toHaveLength(1));
    expect(readPendingWrites()[0]).toMatchObject({
      kind: 'brut',
      noteId: 'n1',
      texte: 'texte a sauver',
    });

    // It stays visible and recoverable in the common banner.
    await screen.findByText(/écritures? non enregistrées?,/);
    expect(screen.getByText(/texte a sauver/)).toBeTruthy();
  });

  it("11. persists the accumulated text if the editor is unmounted during the dictation", async () => {
    installDictationEnvironment();
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);

    const { unmount } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'en cours' }] }, is_final: true })
      );
    });

    // Unmount before closing: the already accumulated text must be persisted.
    await act(async () => { unmount(); });

    await waitFor(() =>
      expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'en cours')
    );
  });

  it("11b. waits for the last fragment and the Deepgram end after a view change", async () => {
    installDictationEnvironment();
    FakeMediaRecorder.deferFinal = true;
    const noteA = makeNote({ id: 'n1', content: 'Origine' });
    const noteB = makeNote({ id: 'n2', content: 'Autre vue' });
    let finishPrefix: () => void = () => {};
    mockApi.appendRawContent.mockReturnValueOnce(
      new Promise<void>(resolve => { finishPrefix = resolve; })
    );
    let finishSend: () => void = () => {};
    mockApi.sendAudioChunk.mockReturnValue(new Promise<void>(resolve => { finishSend = resolve; }));
    mockApi.stopTranscription.mockImplementation(async () => {
      // Rust still emits a transcription after CloseStream, then `closed`.
      bus.emit('transcription://message', JSON.stringify({
        channel: { alternatives: [{ transcript: 'dernier mot' }] }, is_final: true,
      }));
      bus.emit('transcription://closed', undefined);
    });

    const { rerender } = render(<NoteEditor key="n1" note={noteA} onNoteUpdated={() => {}} />);
    await startDictation();
    await act(async () => {
      bus.emit('transcription://message', JSON.stringify({
        channel: { alternatives: [{ transcript: 'premier mot' }] }, is_final: true,
      }));
    });

    // The view change destroys the old editor before the recorder's final events.
    rerender(<NoteEditor key="n2" note={noteB} onNoteUpdated={() => {}} />);
    await waitFor(() => expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'premier mot'));
    expect(mockApi.stopTranscription).not.toHaveBeenCalled();

    await act(async () => { FakeMediaRecorder.last?.deliverFinal(); });
    await waitFor(() => expect(mockApi.sendAudioChunk).toHaveBeenCalledTimes(1));
    expect(mockApi.stopTranscription).not.toHaveBeenCalled();

    await act(async () => { finishSend(); });
    await waitFor(() => expect(mockApi.stopTranscription).toHaveBeenCalledTimes(1));
    expect(mockApi.cleanTranscript).not.toHaveBeenCalled();
    await act(async () => { finishPrefix(); });
    await waitFor(() => expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'dernier mot'));
    await waitFor(() => expect(mockApi.appendNoteContent).toHaveBeenCalledWith(
      'n1', 'Texte nettoyé', 'Texte nettoyé'
    ));
    expect(mockApi.cleanTranscript).toHaveBeenCalledWith(
      'premier mot dernier mot', expect.anything(), undefined, 'n1'
    );
    expect(mockApi.updateNote).not.toHaveBeenCalledWith('n2', expect.anything());
    expect(contentField().value).toBe('Autre vue');
  });

  it("12. does not write a note that was merely viewed (no field modified)", async () => {
    const note = makeNote();
    const { unmount } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    // Focus then leaving the title without modification, then unmount.
    fireEvent.blur(titleField());
    await act(async () => { unmount(); });

    expect(mockApi.updateNote).not.toHaveBeenCalled();
  });

  it("13. serializes writes of the same note (an older one does not double a more recent one)", async () => {
    const note = makeNote();
    let resolveFirst: () => void = () => {};
    const first = new Promise<void>(res => { resolveFirst = res; });
    mockApi.updateNote.mockReturnValueOnce(first).mockResolvedValue(undefined);

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    // First modification: the write goes out and stays "in flight".
    fireEvent.change(selectPriority(), { target: { value: 'P1' } });
    await act(async () => {});
    expect(mockApi.updateNote).toHaveBeenCalledTimes(1);

    // Second modification while the first is still in flight: it is queued,
    // not launched in parallel.
    fireEvent.change(screen.getByRole('combobox', { name: 'Nature de la case' }), { target: { value: 'bug' } });
    await act(async () => {});
    expect(mockApi.updateNote).toHaveBeenCalledTimes(1);

    resolveFirst();
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledTimes(2));
    // The most recent one does apply last.
    expect(mockApi.updateNote).toHaveBeenLastCalledWith(
      'n1',
      expect.objectContaining({ note_type: 'bug' })
    );
  });

  it("14. the return of the AI cleanup does not replace an unsaved local input", async () => {
    installDictationEnvironment();
    const note = makeNote({ content: 'Contenu' });
    mockApi.getNote.mockResolvedValue(note);

    // The LLM cleanup stays "in flight" while the user types.
    let resolveCleanup: (v: string) => void = () => {};
    mockApi.cleanTranscript.mockReturnValue(
      new Promise<string>(res => { resolveCleanup = res; })
    );

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'bonjour' }] }, is_final: true })
      );
    });
    // Local input during the AI processing.
    fireEvent.change(contentField(), { target: { value: 'saisie locale' } });

    await act(async () => {
      bus.emit('transcription://closed', undefined);
    });
    await act(async () => { resolveCleanup('Texte nettoyé'); });

    // The cleaned text is appended to the local draft, it does not replace it.
    await waitFor(() =>
      expect(mockApi.updateNote).toHaveBeenCalledWith('n1', {
        content: 'saisie locale\n\nTexte nettoyé',
      })
    );
    expect(contentField().value).toBe('saisie locale\n\nTexte nettoyé');
  });

  it("15. sends the last audio fragment before closing the session", async () => {
    installDictationEnvironment();
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    const stop = await screen.findByRole('button', { name: 'Arrêter' });
    fireEvent.click(stop);

    await waitFor(() => expect(mockApi.sendAudioChunk).toHaveBeenCalled());
    await waitFor(() => expect(mockApi.stopTranscription).toHaveBeenCalled());
    // The last fragment is sent **before** the session is closed.
    expect(mockApi.sendAudioChunk.mock.invocationCallOrder[0]).toBeLessThan(
      mockApi.stopTranscription.mock.invocationCallOrder[0]
    );
  });

  it("16. a double click starts only one dictation session", async () => {
    installDictationEnvironment();
    const note = makeNote();
    let resolve: () => void = () => {};
    mockApi.startTranscription.mockReturnValueOnce(new Promise<void>(r => { resolve = r; }));

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(dictationButton());
    fireEvent.click(dictationButton());

    expect(mockApi.startTranscription).toHaveBeenCalledTimes(1);
    await act(async () => { resolve(); });
  });

  it("17. assigns a title derived from the cleaned text if the note has no title", async () => {
    installDictationEnvironment();
    const note = makeNote({ title: 'Nouvelle case', content: '' });
    mockApi.getNote.mockResolvedValue(note);
    mockApi.cleanTranscript.mockResolvedValue('# Corriger le crash\n\nDétails du bug');

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'corriger le crash' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });

    await waitFor(() =>
      expect(mockApi.updateNote).toHaveBeenCalledWith('n1', {
        content: '# Corriger le crash\n\nDétails du bug',
        title: 'Corriger le crash',
      })
    );
    expect(titleField().value).toBe('Corriger le crash');
  });

  it("18. does not replace a custom title during a dictation", async () => {
    installDictationEnvironment();
    const note = makeNote({ title: 'Titre choisi', content: '' });
    mockApi.getNote.mockResolvedValue(note);
    mockApi.cleanTranscript.mockResolvedValue('# Autre titre\n\nDétails');

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'autre chose' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });

    await waitFor(() =>
      expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { content: '# Autre titre\n\nDétails' })
    );
    expect(titleField().value).toBe('Titre choisi');
    expect(mockApi.updateNote).not.toHaveBeenCalledWith('n1', expect.objectContaining({ title: expect.anything() }));
  });

  it("19. \"Compare sources\" is a kit segment, selectable and without a tint that overrides it", () => {
    const note = makeNote({ raw_content: 'texte brut de la dictée' });
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    // The Editor / Compare choice is a segmented control: the state comes from the kit, not from a
    // hand-placed color class (a `text-primary` tint made the label unreadable).
    const comparer = screen.getByRole('radio', { name: /Comparer/ });
    expect(comparer.getAttribute('aria-checked')).toBe('false');
    expect(comparer.className).not.toMatch(/text-primary/);

    fireEvent.click(comparer);

    const active = screen.getByRole('radio', { name: /Comparer/ });
    expect(active.getAttribute('aria-checked')).toBe('true');
    expect(active.className).not.toMatch(/text-primary/);
    expect(screen.getByRole('radio', { name: /Éditeur riche/ }).getAttribute('aria-checked')).toBe('false');
  });

  it("20. serializes the unmount flush behind a write still in flight", async () => {
    const note = makeNote();
    let resolveFirst: () => void = () => {};
    const first = new Promise<void>(res => { resolveFirst = res; });
    mockApi.updateNote.mockReturnValueOnce(first).mockResolvedValue(undefined);

    const { unmount } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    // First write (priority) left in flight.
    fireEvent.change(selectPriority(), { target: { value: 'P1' } });
    await act(async () => {});
    expect(mockApi.updateNote).toHaveBeenCalledTimes(1);

    // Content edit while the write is in flight, then note change (unmount).
    fireEvent.change(contentField(), { target: { value: 'contenu final' } });
    await act(async () => { unmount(); });

    // The unmount flush is chained: it is not launched before the in-flight write ends.
    expect(mockApi.updateNote).toHaveBeenCalledTimes(1);

    await act(async () => { resolveFirst(); });
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledTimes(2));
    // The most recent value does apply last.
    expect(mockApi.updateNote).toHaveBeenLastCalledWith(
      'n1',
      expect.objectContaining({ content: 'contenu final' })
    );
  });

  it("21. keeps the input if the unmount write fails (recoverable escrow)", async () => {
    mockApi.updateNote.mockRejectedValue(new Error('DB verrouillée'));
    const note = makeNote();
    const { unmount } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.change(contentField(), { target: { value: 'à ne pas perdre' } });
    await act(async () => { unmount(); });

    // The failure is no longer swallowed: the write is kept outside the component, not lost without a trace.
    await waitFor(() => expect(readPendingWrites()).toHaveLength(1));
    expect(readPendingWrites()[0]).toMatchObject({
      kind: 'update',
      noteId: 'n1',
      payload: { content: 'à ne pas perdre' },
      base: { content: 'Contenu' },
    });

    // On the next display, a recoverable banner offers to replay the write.
    mockApi.updateNote.mockResolvedValue(undefined);
    mockApi.getNote.mockResolvedValue(note);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await screen.findByText(/écritures? non enregistrées?,/);
    fireEvent.click(screen.getByText('Réessayer'));
    await waitFor(() => expect(readPendingWrites()).toHaveLength(0));
  });

  it('21b. Retry preserves the draft if an agent modified the same field', async () => {
    const note = makeNote();
    mockApi.updateNote.mockRejectedValue(new Error('DB verrouillée'));
    const { unmount } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.change(contentField(), { target: { value: 'brouillon local précieux' } });
    await act(async () => { unmount(); });
    await waitFor(() => expect(readPendingWrites()).toHaveLength(1));

    mockApi.updateNote.mockClear().mockResolvedValue(undefined);
    mockApi.retryNoteUpdate.mockResolvedValue('conflict');
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByText('Réessayer'));

    await screen.findByText(/La case a changé depuis l’échec/);
    expect(mockApi.updateNote).not.toHaveBeenCalled();
    expect(mockApi.retryNoteUpdate).toHaveBeenCalledWith(
      'n1', { content: 'brouillon local précieux' }, { content: 'Contenu' }
    );
    expect(readPendingWrites()).toHaveLength(1);
    fireEvent.click(screen.getByText('Voir le texte à récupérer'));
    expect(screen.getByText(/brouillon local précieux/)).toBeTruthy();
  });

  it('21c-bis. A vanished note is reported by the engine result, without comparing text', async () => {
    addPendingWrite({
      kind: 'update', noteId: 'n1', payload: { content: 'brouillon orphelin' },
      base: { content: 'Contenu' },
    });
    mockApi.retryNoteUpdate.mockResolvedValue('not_found');
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByText('Réessayer'));

    await screen.findByText(/Case introuvable. Copiez le brouillon/);
    // The draft stays to be recovered: it is never removed from the escrow.
    expect(readPendingWrites()).toHaveLength(1);
  });

  it('21d. A conflict that appeared after an earlier read keeps the draft', async () => {
    addPendingWrite({
      kind: 'update', noteId: 'n1', payload: { content: 'brouillon local' },
      base: { content: 'Contenu' },
    });
    // The decision only comes from the atomic command; no prior getNote can
    // wrongly declare the draft already applied then remove it after an MCP write.
    mockApi.retryNoteUpdate.mockResolvedValue('conflict');
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByText('Réessayer'));

    await screen.findByText(/La case a changé depuis l’échec/);
    expect(mockApi.retryNoteUpdate).toHaveBeenCalledWith(
      'n1', { content: 'brouillon local' }, { content: 'Contenu' }
    );
    expect(mockApi.updateNote).not.toHaveBeenCalled();
    expect(mockApi.getNote).not.toHaveBeenCalled();
    expect(readPendingWrites()).toHaveLength(1);
  });

  it('21e. An already applied draft is removed on atomic decision', async () => {
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'déjà écrit' } });
    mockApi.retryNoteUpdate.mockResolvedValue('already_applied');
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByText('Réessayer'));

    await waitFor(() => expect(readPendingWrites()).toHaveLength(0));
    expect(mockApi.retryNoteUpdate).toHaveBeenCalledWith('n1', { content: 'déjà écrit' }, {});
  });

  it('21f. A raw replay failure keeps the text then removes it after a successful append', async () => {
    addPendingWrite({ kind: 'brut', noteId: 'n1', texte: 'dictée à récupérer' });
    mockApi.appendRawContent.mockRejectedValueOnce(new Error('disque plein'));
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByText('Réessayer'));

    await screen.findByText(/Nouvel échec de l’enregistrement/);
    expect(readPendingWrites()).toHaveLength(1);
    expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'dictée à récupérer');

    fireEvent.click(screen.getByText('Réessayer'));
    await waitFor(() => expect(readPendingWrites()).toHaveLength(0));
    expect(mockApi.appendRawContent).toHaveBeenCalledTimes(2);
  });

  it('21c. An old entry without a base never rewrites the note', async () => {
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'ancien brouillon' } });
    mockApi.retryNoteUpdate.mockResolvedValue('conflict');
    render(<NoteEditor note={makeNote()} onNoteUpdated={() => {}} />);
    fireEvent.click(screen.getByText('Réessayer'));

    await screen.findByText(/Ancien brouillon sans version de référence/);
    expect(mockApi.updateNote).not.toHaveBeenCalled();
    expect(readPendingWrites()).toHaveLength(1);
    fireEvent.click(screen.getByText('Voir le texte à récupérer'));
    expect(screen.getByText(/ancien brouillon/)).toBeTruthy();
  });

  it("22. flushes the draft on window close request", async () => {
    const note = makeNote();
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await waitFor(() => expect(winBus.handlers.length).toBeGreaterThan(0));

    fireEvent.change(contentField(), { target: { value: 'avant fermeture' } });
    await act(async () => {
      winBus.handlers.forEach(h => h());
    });

    await waitFor(() =>
      expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { content: 'avant fermeture' })
    );
  });

  it("24. does not show similar notes of another note (race)", async () => {
    const noteA = makeNote({ id: 'n1', title: 'A', content: 'contenu bug A' });
    const noteB = makeNote({ id: 'n2', title: 'B', content: 'contenu B' });
    let resolve: (v: Note[]) => void = () => {};
    mockApi.findSimilarNotes.mockReturnValueOnce(
      new Promise<Note[]>(res => { resolve = res; })
    );

    const { rerender } = render(<NoteEditor note={noteA} onNoteUpdated={() => {}} />);
    // Autosave triggers the duplicate search after 1.5 s.
    await waitFor(() => expect(mockApi.findSimilarNotes).toHaveBeenCalled(), { timeout: 4000 });

    // The user changes note before the response.
    rerender(<NoteEditor note={noteB} onNoteUpdated={() => {}} />);
    await act(async () => {
      resolve([{ ...noteA, title: 'Similaire de A' }]);
    });

    // A's similar notes must not display under B.
    expect(screen.queryByText(/Notes similaires détectées/)).toBeNull();
    expect(screen.queryByText('Similaire de A')).toBeNull();
  });

  it("suggests the nature read from the wording, only for a plain thought", async () => {
    mockApi.recognizeCase.mockResolvedValue({ nature: { value: 'todo', confidence: 0.9 }, line_of_work: null });
    render(<NoteEditor note={makeNote({ content: "ne pas oublier d'appeler le plombier" })} onNoteUpdated={() => {}} />);

    await waitFor(() => expect(mockApi.recognizeCase).toHaveBeenCalledWith(expect.stringContaining('plombier')), { timeout: 4000 });
    expect(await screen.findByText(/Nature suggérée/)).toBeTruthy();
  });

  it("reads a title taken from the text only once", async () => {
    const content = 'y si hacemos un podcast mensual? podría funcionar';
    render(<NoteEditor note={makeNote({ title: deriveTitle(content) ?? '', content })} onNoteUpdated={() => {}} />);

    await waitFor(() => expect(mockApi.recognizeCase).toHaveBeenCalledWith(content), { timeout: 4000 });
  });

  it("never questions a nature the person chose", async () => {
    mockApi.recognizeCase.mockResolvedValue({ nature: { value: 'todo', confidence: 0.9 }, line_of_work: null });
    render(<NoteEditor note={makeNote({ note_type: 'idea', content: "il faut appeler le plombier" })} onNoteUpdated={() => {}} />);

    await waitFor(() => expect(mockApi.findSimilarNotes).toHaveBeenCalled(), { timeout: 4000 });
    expect(mockApi.recognizeCase).not.toHaveBeenCalled();
    expect(screen.queryByText(/Nature suggérée/)).toBeNull();
  });

  it("does not query similarities for an empty note with the default title", async () => {
    vi.useFakeTimers();
    render(<NoteEditor note={makeNote({ title: 'Nouvelle case', content: '' })} onNoteUpdated={() => {}} />);

    await act(async () => { await vi.advanceTimersByTimeAsync(1600); });

    expect(mockApi.findSimilarNotes).not.toHaveBeenCalled();
  });

  it("ignores a similarity response that became stale after an edit of the same note", async () => {
    const note = makeNote({ title: 'Note éditée', content: 'Un contenu initial suffisamment long.' });
    let resolve: (v: Note[]) => void = () => {};
    mockApi.findSimilarNotes.mockReturnValueOnce(new Promise<Note[]>(res => { resolve = res; }));
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    await waitFor(() => expect(mockApi.findSimilarNotes).toHaveBeenCalled(), { timeout: 4000 });
    fireEvent.change(contentField(), { target: { value: 'Un contenu entièrement différent et nouveau.' } });
    await act(async () => { resolve([{ ...note, id: 'n-similaire', title: 'Résultat périmé' }]); });

    expect(screen.queryByText('Résultat périmé')).toBeNull();
  });

  it("23. flushes the draft on beforeunload (safety net)", async () => {
    const note = makeNote();
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    fireEvent.change(contentField(), { target: { value: 'avant extinction' } });

    await act(async () => {
      window.dispatchEvent(new Event('beforeunload'));
    });

    await waitFor(() =>
      expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { content: 'avant extinction' })
    );
  });

  it("25. saves the outgoing draft on note change (without unmount)", async () => {
    const noteA = makeNote({ id: 'n1', title: 'Note A', content: 'Contenu A' });
    const noteB = makeNote({ id: 'n2', title: 'Note B', content: 'Contenu B' });
    const { rerender } = render(<NoteEditor note={noteA} onNoteUpdated={() => {}} />);

    // Input in A, not yet passed through autosave (1.5 s).
    fireEvent.change(titleField(), { target: { value: 'Titre A modifié' } });

    // App changes note (Ctrl+N): same component, new prop, no unmount.
    rerender(<NoteEditor note={noteB} onNoteUpdated={() => {}} />);

    // A's draft is written before its baseline is replaced.
    await waitFor(() =>
      expect(mockApi.updateNote).toHaveBeenCalledWith('n1', { title: 'Titre A modifié' })
    );
    // The editor does display B, and nothing is written to B.
    expect(titleField().value).toBe('Note B');
    expect(mockApi.updateNote).not.toHaveBeenCalledWith('n2', expect.anything());
  });

  it("26. a return to the initial value during a save is indeed rewritten", async () => {
    const note = makeNote({ priority: '' });
    let resolveFirst: () => void = () => {};
    const first = new Promise<void>(res => { resolveFirst = res; });
    mockApi.updateNote.mockReturnValueOnce(first).mockResolvedValue(undefined);

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    // Switch to P1: the write goes out and stays "in flight".
    fireEvent.change(selectPriority(), { target: { value: 'P1' } });
    await act(async () => {});
    expect(mockApi.updateNote).toHaveBeenCalledTimes(1);

    // Back to "no priority" while P1 is still in flight.
    fireEvent.change(selectPriority(), { target: { value: '' } });
    await act(async () => {});
    expect(mockApi.updateNote).toHaveBeenCalledTimes(1);

    resolveFirst();
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledTimes(2));
    // The return to the initial value is re-emitted: the database must not keep P1.
    expect(mockApi.updateNote).toHaveBeenLastCalledWith(
      'n1',
      expect.objectContaining({ priority: '' })
    );
  });

  it("27. a later successful dictation does not erase a raw text already failed", async () => {
    installDictationEnvironment();
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);
    // First dictation: the raw write fails.
    mockApi.appendRawContent.mockRejectedValueOnce(new Error('disque plein'));

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();
    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'premier echec' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });
    await waitFor(() => expect(readPendingWrites()).toHaveLength(1));

    // Second dictation: the write succeeds.
    await startDictation();
    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'deuxieme succes' }] }, is_final: true })
      );
      bus.emit('transcription://closed', undefined);
    });
    await waitFor(() =>
      expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'deuxieme succes')
    );

    // The first raw text stays in the escrow: it is only removed if actually recovered.
    expect(readPendingWrites()).toHaveLength(1);
    expect(readPendingWrites()[0]).toMatchObject({ kind: 'brut', texte: 'premier echec' });
  });

  it("28. closing during a dictation does not append the raw text twice at the end", async () => {
    installDictationEnvironment();
    const note = makeNote();
    mockApi.getNote.mockResolvedValue(note);

    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await waitFor(() => expect(winBus.handlers.length).toBeGreaterThan(0));
    await startDictation();

    await act(async () => {
      bus.emit(
        'transcription://message',
        JSON.stringify({ channel: { alternatives: [{ transcript: 'Bonjour le monde' }] }, is_final: true })
      );
    });

    // Window close (hiding): the received raw text is saved.
    await act(async () => { winBus.handlers.forEach(h => h()); });
    await waitFor(() =>
      expect(mockApi.appendRawContent).toHaveBeenCalledWith('n1', 'Bonjour le monde')
    );

    // The dictation then ends normally: the already persisted text is not appended again.
    await act(async () => { bus.emit('transcription://closed', undefined); });
    await waitFor(() => expect(mockApi.cleanTranscript).toHaveBeenCalled());

    expect(mockApi.appendRawContent).toHaveBeenCalledTimes(1);
  });

  it("29. recreates the rich editor on note change (isolated undo history)", () => {
    const noteA = makeNote({ id: 'n1', title: 'A', content: 'Contenu A' });
    const noteB = makeNote({ id: 'n2', title: 'B', content: 'Contenu B' });
    const { rerender } = render(<NoteEditor note={noteA} onNoteUpdated={() => {}} />);

    const firstEditor = contentField();
    // Same note (new object identity): no remount, undo is preserved.
    rerender(<NoteEditor note={{ ...noteA }} onNoteUpdated={() => {}} />);
    expect(contentField()).toBe(firstEditor);

    // Another note: the editor is recreated, the undo history is isolated per note.
    rerender(<NoteEditor note={noteB} onNoteUpdated={() => {}} />);
    expect(contentField()).not.toBe(firstEditor);
  });

  it("30. derives a title on manual input as long as the title is the default", async () => {
    const note = makeNote({ title: 'Nouvelle case', content: '' });
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    fireEvent.change(contentField(), { target: { value: 'Corriger le crash au démarrage' } });

    expect(titleField().value).toBe('Corriger le crash au démarrage');
    // The derived title is persisted by autosave (like the content).
    await waitFor(
      () =>
        expect(mockApi.updateNote).toHaveBeenCalledWith(
          'n1',
          expect.objectContaining({ title: 'Corriger le crash au démarrage' })
        ),
      { timeout: 3000 }
    );
  });

  it("31. never replaces a custom title on manual input", () => {
    const note = makeNote({ title: 'Mon titre', content: '' });
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    fireEvent.change(contentField(), { target: { value: 'Un contenu quelconque' } });

    expect(titleField().value).toBe('Mon titre');
  });

  it("32. freezes the automatic title as soon as the user edits the title", () => {
    const note = makeNote({ title: 'Nouvelle case', content: '' });
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    fireEvent.change(contentField(), { target: { value: 'Premier contenu' } });
    expect(titleField().value).toBe('Premier contenu');

    // The user corrects the title…
    fireEvent.change(titleField(), { target: { value: 'Titre choisi' } });
    // …then keeps writing: the title is no longer derived.
    fireEvent.change(contentField(), { target: { value: 'Autre contenu plus tard' } });

    expect(titleField().value).toBe('Titre choisi');
  });

  it("33. does not stay on \"Saving…\" when there is nothing to write", async () => {
    const note = makeNote();
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    // Focus then blur of the title WITHOUT modification: the payload is empty. The indicator must not
    // stay stuck (counter of writes in progress incremented without a write).
    fireEvent.blur(titleField());
    await act(async () => {});

    expect(mockApi.updateNote).not.toHaveBeenCalled();
    expect(screen.queryByText(/Enregistrement/)).toBeNull();
  });

  it("34. turns off \"Saving…\" after a real write", async () => {
    const note = makeNote();
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);

    fireEvent.change(selectPriority(), { target: { value: 'P1' } });
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalled());
    // After success: "Saved" (or nothing), but no longer "Saving…".
    await waitFor(() => expect(screen.queryByText(/Enregistrement/)).toBeNull());
  });

  const projectSpaces = (): Project[] => [
    { id: 'p1', name: 'Pro', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: 'work' },
    { id: 'p2', name: 'Privé', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: 'private' },
    { id: 'p3', name: 'À classer', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: null },
  ];

  it("35. never offers a similar note from the other space (in both directions)", async () => {
    const projects = projectSpaces();
    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Note pro', content: 'un contenu assez long pour la recherche' });
    const privateNote = makeNote({ id: 'n2', project_id: 'p2', title: 'Note privée', content: 'un contenu assez long pour la recherche' });
    const similarWork = makeNote({ id: 'n3', project_id: 'p1', title: 'Similaire pro', content: 'x' });
    const similarPrivate = makeNote({ id: 'n4', project_id: 'p2', title: 'Similaire privée', content: 'x' });
    mockApi.findSimilarNotes.mockResolvedValue([similarWork, similarPrivate]);

    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projects} />);
    // Work → Private direction: the other space's similarity is hidden.
    await waitFor(() => expect(screen.getByText('Similaire pro')).toBeTruthy(), { timeout: 4000 });
    expect(screen.queryByText('Similaire privée')).toBeNull();

    // Private → Work direction: a private note never sees the Work similarity.
    rerender(<NoteEditor note={privateNote} onNoteUpdated={() => {}} projects={projects} />);
    await waitFor(() => expect(screen.getByText('Similaire privée')).toBeTruthy(), { timeout: 4000 });
    expect(screen.queryByText('Similaire pro')).toBeNull();
  }, 15000);

  it("36. does not offer another space's draft, but finds it again on return", () => {
    const projects = projectSpaces();
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'Brouillon pro' }, base: { content: '' }, space: 'work' });
    addPendingWrite({ kind: 'brut', noteId: 'n2', texte: 'Dictée privée', space: 'private' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const privateNote = makeNote({ id: 'n2', project_id: 'p2', title: 'Privé' });
    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projects} />);

    // Only the Work space draft is offered.
    expect(screen.getByText(/Brouillon · case n1/)).toBeTruthy();
    expect(screen.queryByText(/Dictée brute · case n2/)).toBeNull();

    // On return to the private note, that space's draft reappears…
    rerender(<NoteEditor note={privateNote} onNoteUpdated={() => {}} projects={projects} />);
    expect(screen.getByText(/Dictée brute · case n2/)).toBeTruthy();
    expect(screen.queryByText(/Brouillon · case n1/)).toBeNull();

    // …and the other draft was never deleted: no data loss.
    expect(readPendingWrites()).toHaveLength(2);
  });

  it("37. a note without a space keeps the current behavior (no limit)", () => {
    const projects = projectSpaces();
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'a' }, space: 'work' });
    addPendingWrite({ kind: 'brut', noteId: 'n2', texte: 'b', space: 'private' });

    render(<NoteEditor note={makeNote({ id: 'n1', project_id: 'p3' })} onNoteUpdated={() => {}} projects={projects} />);

    expect(screen.getByText(/Brouillon · case n1/)).toBeTruthy();
    expect(screen.getByText(/Dictée brute · case n2/)).toBeTruthy();
  });

  it("38. the report draft of another space stays hidden then reappears", () => {
    const projects = projectSpaces();
    // Entry escrowed as NoteReport would on a private note.
    addPendingWrite({ kind: 'update', noteId: 'n2', payload: { resolution_comment: 'Compte rendu privé' }, base: { resolution_comment: null }, space: 'private' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const privateNote = makeNote({ id: 'n2', project_id: 'p2', title: 'Privé' });
    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projects} />);

    // The private draft is not offered from the Work note.
    expect(screen.queryByText(/Brouillon · case n2/)).toBeNull();

    // NoteReport does attach the displayed note's space (Work).
    fireEvent.click(screen.getByRole('button', { name: 'Ajouter un compte rendu' }));
    fireEvent.change(screen.getByLabelText('Résultat et limites'), { target: { value: 'Compte rendu pro' } });
    fireEvent.click(screen.getByRole('button', { name: 'Fermer' }));
    const pro = readPendingWrites().find(e => e.kind === 'update' && e.payload.resolution_comment === 'Compte rendu pro');
    expect(pro?.space).toBe('work');

    // On return to the private note, the private draft reappears; no loss.
    rerender(<NoteEditor note={privateNote} onNoteUpdated={() => {}} projects={projects} />);
    expect(screen.getByText(/Brouillon · case n2/)).toBeTruthy();
    expect(readPendingWrites()).toHaveLength(2);
  });

  it("39. repairs a draft's space when the project changed space", () => {
    const projects = projectSpaces();
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'Brouillon' }, base: { content: '' }, space: 'work' });

    // Project p1 is now in the Private space: the old 'work' space would be invisible everywhere.
    const updatedProjects = projects.map(p => p.id === 'p1' ? { ...p, space: 'private' as const } : p);
    render(<NoteEditor note={makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' })} onNoteUpdated={() => {}} projects={updatedProjects} />);

    expect(screen.getByText(/Brouillon · case n1/)).toBeTruthy();
    expect(readPendingWrites()).toHaveLength(1);
    expect(readPendingWrites()[0].space).toBe('private');
  });

  it("40. repairs through its projectId the draft of a note from ANOTHER project (allNotes = active project only)", async () => {
    // Like App.tsx: `allNotes` only contains the active project's notes. Note n2 belongs
    // to project p2, which went from "work" to "private" while its draft was
    // escrowed in the old space, without ever reopening n2.
    const projectsBefore = projectSpaces().map(p => p.id === 'p2' ? { ...p, space: 'work' as const } : p);
    const projectsAfter = projectSpaces(); // p2 → private
    addPendingWrite({ kind: 'update', noteId: 'n2', payload: { content: 'Brouillon privé' }, base: { content: '' }, space: 'work', projectId: 'p2' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const { rerender } = render(
      <NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsBefore} allNotes={[notePro]} />
    );
    // Project still in "work": nothing to repair.
    expect(readPendingWrites()[0].space).toBe('work');

    // The global projects polling delivers the new value: the repair happens without allNotes.
    rerender(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsAfter} allNotes={[notePro]} />);
    await waitFor(() => expect(readPendingWrites()[0].space).toBe('private'));
    // No longer offered in the old space (Work)…
    expect(screen.queryByText(/Brouillon · case n2/)).toBeNull();

    // …and recoverable from the targeted note, even if it was never in `allNotes`.
    const privateNote = makeNote({ id: 'n2', project_id: 'p2', title: 'Privé' });
    rerender(<NoteEditor note={privateNote} onNoteUpdated={() => {}} projects={projectsAfter} allNotes={[privateNote]} />);
    expect(screen.getByText(/Brouillon · case n2/)).toBeTruthy();

    // No loss: still a single entry.
    expect(readPendingWrites()).toHaveLength(1);
  });

  it("41. leaves intact a legacy entry without projectId of a note not displayed", () => {
    const projects = projectSpaces();
    // Entry from an old version (no projectId) for a note that is not on screen:
    // no way to know its actual space, the limit is accepted.
    addPendingWrite({ kind: 'update', noteId: 'nx', payload: { content: 'Brouillon' }, space: 'work' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projects} allNotes={[notePro]} />);

    expect(readPendingWrites()).toHaveLength(1);
    expect(readPendingWrites()[0].space).toBe('work');
  });

  it("42. leaves intact an entry whose project is not loaded yet (unknown space)", () => {
    const projects = projectSpaces();
    addPendingWrite({ kind: 'update', noteId: 'n2', payload: { content: 'Brouillon' }, space: 'work', projectId: 'p-inconnu' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projects} allNotes={[notePro]} />);

    // Project unknown to `projects`: actual space null → no repair.
    expect(readPendingWrites()).toHaveLength(1);
    expect(readPendingWrites()[0].space).toBe('work');
  });

  it("43. never pins an entry without a space: it stays visible everywhere", async () => {
    const projects = projectSpaces();
    // Legacy entry without a space, but with a known project: it must NOT be pinned.
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'Ancienne version' }, base: { content: '' }, space: null, projectId: 'p1' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projects} allNotes={[notePro]} />);
    expect(screen.getByText(/Brouillon · case n1/)).toBeTruthy();

    await act(async () => {});
    expect(readPendingWrites()[0].space ?? null).toBeNull();

    // Also visible from the other space: never made unrecoverable.
    const privateNote = makeNote({ id: 'n2', project_id: 'p2', title: 'Privé' });
    rerender(<NoteEditor note={privateNote} onNoteUpdated={() => {}} projects={projects} allNotes={[privateNote]} />);
    expect(screen.getByText(/Brouillon · case n1/)).toBeTruthy();
    expect(readPendingWrites()).toHaveLength(1);
  });

  it('44. repairs several entries in a single write, without deleting any', async () => {
    const projectsBefore: Project[] = [
      { id: 'p1', name: 'Pro', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: 'work' },
      { id: 'p2', name: 'Deux', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: 'work' },
      { id: 'p3', name: 'Trois', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: 'work' },
    ];
    const projectsAfter = projectsBefore.map(p => p.id === 'p1' ? p : { ...p, space: 'private' as const });
    addPendingWrite({ kind: 'update', noteId: 'n2', payload: { content: 'B2' }, space: 'work', projectId: 'p2' });
    addPendingWrite({ kind: 'brut', noteId: 'n3', texte: 'B3', space: 'work', projectId: 'p3' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsBefore} allNotes={[notePro]} />);
    expect(readPendingWrites().map(e => e.space)).toEqual(['work', 'work']);

    const setItem = vi.spyOn(Storage.prototype, 'setItem');
    setItem.mockClear();
    rerender(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsAfter} allNotes={[notePro]} />);
    await waitFor(() => expect(readPendingWrites().map(e => e.space)).toEqual(['private', 'private']));

    // No deletion, and a single storage write for the whole pass.
    expect(readPendingWrites()).toHaveLength(2);
    expect(setItem.mock.calls.filter(([key]) => key === ESCROW_KEY)).toHaveLength(1);
    // The old space (Work, displayed) no longer offers anything repaired.
    expect(screen.queryByText(/Brouillon · case n2/)).toBeNull();
    expect(screen.queryByText(/Dictée brute · case n3/)).toBeNull();
    setItem.mockRestore();
  });

  it('45. repairs on the next pass an entry created with an already stale space', async () => {
    const projectsBefore = projectSpaces().map(p => p.id === 'p2' ? { ...p, space: 'work' as const } : p);
    const projectsAfter = projectSpaces(); // p2 → private
    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsBefore} allNotes={[notePro]} />);

    // The failure happens after the first pass: the remembered space is that of the project at that moment.
    addPendingWrite({ kind: 'update', noteId: 'n2', payload: { content: 'B' }, space: 'work', projectId: 'p2' });
    expect(readPendingWrites()[0].space).toBe('work');

    // The projects polling delivers the new value: the next pass repairs the in-flight entry.
    rerender(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsAfter} allNotes={[notePro]} />);
    await waitFor(() => expect(readPendingWrites()[0].space).toBe('private'));
    expect(readPendingWrites()).toHaveLength(1);
  });

  it("46. space ping-pong: converges and no longer writes if nothing changes", async () => {
    const projetsWork: Project[] = [
      { id: 'p1', name: 'Pro', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: 'work' },
      { id: 'p2', name: 'Deux', color: '#000000', created_at: '2026-01-01T00:00:00.000Z', updated_at: '2026-01-01T00:00:00.000Z', revision: 0, space: 'work' },
    ];
    const privateProjects = projetsWork.map(p => p.id === 'p2' ? { ...p, space: 'private' as const } : p);
    addPendingWrite({ kind: 'update', noteId: 'n2', payload: { content: 'B' }, space: 'private', projectId: 'p2' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projetsWork} allNotes={[notePro]} />);
    // The project goes back to "work": the entry follows.
    await waitFor(() => expect(readPendingWrites()[0].space).toBe('work'));

    const setItem = vi.spyOn(Storage.prototype, 'setItem');
    setItem.mockClear();
    // …then goes back to "private": symmetric repair, one write.
    rerender(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={privateProjects} allNotes={[notePro]} />);
    await waitFor(() => expect(readPendingWrites()[0].space).toBe('private'));
    expect(setItem.mock.calls.filter(([key]) => key === ESCROW_KEY)).toHaveLength(1);

    // New render with the same space value (new array, like the polling): no
    // write; the pass is idempotent and does not loop.
    setItem.mockClear();
    rerender(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={[...privateProjects]} allNotes={[notePro]} />);
    await act(async () => {});
    expect(setItem.mock.calls.filter(([key]) => key === ESCROW_KEY)).toHaveLength(0);
    expect(readPendingWrites()).toHaveLength(1);
    setItem.mockRestore();
  });

  it("47. a note filed in another project repairs from the displayed note and back-fills the projectId", async () => {
    const projects = projectSpaces(); // p1 → work, p2 → private
    // Entry escrowed when note n1 belonged to p1 ("work"); the note has since been filed
    // in p2. The entry's anchor still points to the old project.
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'B' }, base: { content: '' }, space: 'work', projectId: 'p1' });

    const movedNote = makeNote({ id: 'n1', project_id: 'p2', title: 'Rangée' });
    const { rerender } = render(<NoteEditor note={movedNote} onNoteUpdated={() => {}} projects={projects} allNotes={[movedNote]} />);

    // The displayed note is authoritative: space and projectId follow its current project.
    await waitFor(() => expect(readPendingWrites()[0].projectId).toBe('p2'));
    expect(readPendingWrites()[0].space).toBe('private');
    // Offered in the note's new space…
    expect(screen.getByText(/Brouillon · case n1/)).toBeTruthy();

    // …and never again exposed from a note of the old space (unbounded leak fixed).
    const notePro = makeNote({ id: 'n9', project_id: 'p1', title: 'Pro' });
    rerender(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projects} allNotes={[notePro]} />);
    expect(screen.queryByText(/Brouillon · case n1/)).toBeNull();
    expect(readPendingWrites()).toHaveLength(1);
  });

  it("48. repairs the displayed note's entry even if its old project was deleted", async () => {
    const projects = projectSpaces();
    addPendingWrite({ kind: 'brut', noteId: 'n1', texte: 'Dictée à récupérer', space: 'work', projectId: 'p-supprime' });

    const note = makeNote({ id: 'n1', project_id: 'p2', title: 'Privé' });
    render(<NoteEditor note={note} onNoteUpdated={() => {}} projects={projects} allNotes={[note]} />);

    await waitFor(() => expect(readPendingWrites()[0].projectId).toBe('p2'));
    expect(readPendingWrites()[0].space).toBe('private');
    expect(screen.getByText(/Dictée brute · case n1/)).toBeTruthy();
  });

  it('49. back-fills the projectId of a legacy entry of the displayed note (it survives closing)', async () => {
    const projects = projectSpaces();
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'Ancien brouillon' }, base: { content: '' }, space: 'work' });

    const note = makeNote({ id: 'n1', project_id: 'p2', title: 'Privé' });
    const { rerender } = render(<NoteEditor note={note} onNoteUpdated={() => {}} projects={projects} allNotes={[note]} />);

    await waitFor(() => expect(readPendingWrites()[0].projectId).toBe('p2'));
    expect(readPendingWrites()[0].space).toBe('private');

    // Close / reopen on another note: the entry is no longer orphaned, the next repair
    // no longer depends on the displayed note.
    const other = makeNote({ id: 'n2', project_id: 'p1', title: 'Pro' });
    rerender(<NoteEditor note={other} onNoteUpdated={() => {}} projects={projects} allNotes={[other]} />);
    await act(async () => {});
    expect(readPendingWrites()[0].projectId).toBe('p2');

    // Project p2 then changes space: the repair follows without reopening case n1.
    const updatedProjects = projects.map(p => p.id === 'p2' ? { ...p, space: 'work' as const } : p);
    rerender(<NoteEditor note={other} onNoteUpdated={() => {}} projects={updatedProjects} allNotes={[other]} />);
    await waitFor(() => expect(readPendingWrites()[0].space).toBe('work'));
  });

  it('50. repairs a task-draft through its projectId like the other entries', async () => {
    const projectsBefore = projectSpaces().map(p => p.id === 'p2' ? { ...p, space: 'work' as const } : p);
    const projectsAfter = projectSpaces(); // p2 → private
    addPendingWrite({ kind: 'task-draft', noteId: 'n2', titre: 'Tâche', contenu: 'Détails', space: 'work', projectId: 'p2' });

    const notePro = makeNote({ id: 'n1', project_id: 'p1', title: 'Pro' });
    const { rerender } = render(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsBefore} allNotes={[notePro]} />);
    expect(readPendingWrites()[0].space).toBe('work');

    rerender(<NoteEditor note={notePro} onNoteUpdated={() => {}} projects={projectsAfter} allNotes={[notePro]} />);
    await waitFor(() => expect(readPendingWrites()[0].space).toBe('private'));

    // Retrievable from the source note's form in the new space, nowhere else.
    expect(readTaskDraft('n2', 'private')?.titre).toBe('Tâche');
    expect(readTaskDraft('n2', 'work')).toBeUndefined();
    expect(readPendingWrites()).toHaveLength(1);
  });

  it("51. a second failure of the same payload refreshes the project anchor without creating a duplicate", () => {
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'Même charge' }, base: { content: '' }, space: 'work', projectId: 'p1' });
    // The note was filed in p2 between the two failures: the freshest capture is authoritative.
    addPendingWrite({ kind: 'update', noteId: 'n1', payload: { content: 'Même charge' }, base: { content: '' }, space: 'work', projectId: 'p2' });

    expect(readPendingWrites()).toHaveLength(1);
    expect(readPendingWrites()[0].projectId).toBe('p2');
    const entry = readPendingWrites()[0];
    expect(entry.kind === 'update' && entry.payload.content).toBe('Même charge');
  });
});

describe('NoteEditor — smart dictation (0.9.80)', () => {
  const INTEGRATED = '## Plan\n\nNote existante\n\nIdée dictée.';
  const dictate = async (text: string) => {
    await act(async () => {
      bus.emit('transcription://message', JSON.stringify({ channel: { alternatives: [{ transcript: text }] }, is_final: true }));
      bus.emit('transcription://closed', undefined);
    });
  };

  beforeEach(() => {
    installDictationEnvironment();
    localStorage.removeItem('dictation_auto_structure');
  });

  it('integrates the dictation into the whole note after saving the raw text, and Undo restores the previous version', async () => {
    const note = makeNote({ content: 'Note existante' });
    mockApi.getNote.mockResolvedValue(note);
    mockApi.integrateDictation.mockResolvedValue(INTEGRATED);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();
    await dictate('idée dictée');

    await waitFor(() => expect(mockApi.applyStructuredNote).toHaveBeenCalledWith('n1', 'Note existante', INTEGRATED));
    expect(mockApi.integrateDictation).toHaveBeenCalledWith('n1', 'Note existante', 'idée dictée', '', undefined, 'auto');
    // Dictation safeguard: the raw text is written before the AI call.
    expect(mockApi.appendRawContent.mock.invocationCallOrder[0]).toBeLessThan(mockApi.integrateDictation.mock.invocationCallOrder[0]);
    expect(mockApi.cleanTranscript).not.toHaveBeenCalled();
    expect(mockApi.appendNoteContent).not.toHaveBeenCalled();

    fireEvent.click(await screen.findByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(mockApi.applyStructuredNote).toHaveBeenLastCalledWith('n1', INTEGRATED, 'Note existante'));
  });

  it('goes back to appending the cleaned dictation if the integration is refused', async () => {
    const note = makeNote({ content: 'Note existante' });
    mockApi.getNote.mockResolvedValue(note);
    mockApi.integrateDictation.mockRejectedValue(new Error('Intégration refusée : note raccourcie'));
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();
    await dictate('idée dictée');

    await waitFor(() => expect(mockApi.cleanTranscript).toHaveBeenCalledWith('idée dictée', '', undefined, 'n1'));
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledWith('n1', expect.objectContaining({ content: 'Note existante\n\nTexte nettoyé' })));
    expect(mockApi.applyStructuredNote).not.toHaveBeenCalled();
  });

  it('does not apply the integration if the text changed during the AI call', async () => {
    const note = makeNote({ content: 'Note existante' });
    mockApi.getNote.mockResolvedValue(note);
    let renderHelper: (text: string) => void = () => {};
    mockApi.integrateDictation.mockReturnValue(new Promise<string>(resolve => { renderHelper = resolve; }));
    const { rerender } = render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();
    await dictate('idée dictée');
    await waitFor(() => expect(mockApi.integrateDictation).toHaveBeenCalled());

    // An agent (or an input) modifies the text during the call: it is adopted by the editor.
    rerender(<NoteEditor note={{ ...note, content: 'Note existante modifiée', revision: 1 }} onNoteUpdated={() => {}} />);
    await act(async () => { renderHelper('## Intégrée'); });

    await waitFor(() => expect(mockApi.cleanTranscript).toHaveBeenCalled());
    expect(mockApi.applyStructuredNote).not.toHaveBeenCalled();
  });

  it('disabled in the settings: the cleaned dictation is appended at the end, without integration', async () => {
    localStorage.setItem('dictation_auto_structure', 'false');
    const note = makeNote({ content: 'Note existante' });
    mockApi.getNote.mockResolvedValue(note);
    render(<NoteEditor note={note} onNoteUpdated={() => {}} />);
    await startDictation();
    await dictate('idée dictée');

    await waitFor(() => expect(mockApi.cleanTranscript).toHaveBeenCalled());
    expect(mockApi.integrateDictation).not.toHaveBeenCalled();
  });
});

/**
 * A nudge's "Sprout" (Home) opens the case and sprouts it at once. The editor is already mounted
 * with no case (or another one): sprouting must wait until this case is loaded, or the flush
 * before sprouting would write the previous fields (empty) into it. Fails if the wait is removed.
 */
describe('Sprout on open', () => {
  it('sprouts the case once it is loaded, without writing anything into it', async () => {
    mockApi.germinateCase.mockResolvedValue({ dropped: 0, checked: false } as never);
    const onStarted = vi.fn();
    const note = makeNote({ id: 'q1', title: 'Émail céladon ou tenmoku ?', content: 'Le céladon plaît en boutique, le tenmoku se vend mieux en ligne.' });
    const { rerender } = render(<NoteEditor note={null} onNoteUpdated={() => {}} germinateOnOpen={null} onGerminateStarted={onStarted} />);
    rerender(<NoteEditor note={note} onNoteUpdated={() => {}} germinateOnOpen="q1" onGerminateStarted={onStarted} />);
    await waitFor(() => expect(mockApi.germinateCase).toHaveBeenCalledWith('q1', expect.anything(), undefined));
    expect(onStarted).toHaveBeenCalledTimes(1);
    expect(mockApi.updateNote).not.toHaveBeenCalled();
    expect(titleField().value).toBe('Émail céladon ou tenmoku ?');
    // Asked once: a later render does not sprout again.
    rerender(<NoteEditor note={{ ...note, revision: 1 }} onNoteUpdated={() => {}} germinateOnOpen="q1" onGerminateStarted={onStarted} />);
    await act(async () => { await Promise.resolve(); });
    expect(mockApi.germinateCase).toHaveBeenCalledTimes(1);
  });
});

describe('a new case left empty', () => {
  const contentBox = () => screen.getByLabelText('contenu') as HTMLTextAreaElement;

  it('asks the engine to trash the case left, only once its last write has landed', async () => {
    const onNoteUpdated = vi.fn();
    mockApi.trashIfEmpty.mockResolvedValue(true);
    const fresh = makeNote({ id: 'n1', title: 'Nouvelle case', content: '' });
    const { rerender } = render(<NoteEditor note={fresh} onNoteUpdated={onNoteUpdated} />);
    // Typing then leaving at once: the engine is asked only after the write.
    let landed: () => void = () => {};
    mockApi.updateNote.mockReturnValueOnce(new Promise<void>(resolve => { landed = resolve; }));
    fireEvent.change(contentBox(), { target: { value: 'Une idée' } });
    rerender(<NoteEditor note={makeNote({ id: 'n2' })} onNoteUpdated={onNoteUpdated} />);
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledWith('n1', expect.objectContaining({ content: 'Une idée' })));
    expect(mockApi.trashIfEmpty).not.toHaveBeenCalled();
    await act(async () => { landed(); });
    await waitFor(() => expect(mockApi.trashIfEmpty).toHaveBeenCalledWith('n1'));
    // The engine said it went: the lists reload.
    await waitFor(() => expect(onNoteUpdated).toHaveBeenCalled());
  });

  it('leaves alone a case whose write failed, kept in escrow', async () => {
    const fresh = makeNote({ id: 'n1', title: 'Nouvelle case', content: '' });
    const { rerender } = render(<NoteEditor note={fresh} onNoteUpdated={() => {}} />);
    mockApi.updateNote.mockRejectedValueOnce(new Error('database locked'));
    fireEvent.change(contentBox(), { target: { value: 'Texte à garder' } });
    rerender(<NoteEditor note={makeNote({ id: 'n2' })} onNoteUpdated={() => {}} />);
    await waitFor(() => expect(readPendingWrites().some(entry => entry.noteId === 'n1')).toBe(true));
    await act(async () => { await Promise.resolve(); });
    expect(mockApi.trashIfEmpty).not.toHaveBeenCalled();
    clearPendingWrites();
  });
});

describe('NoteEditor — focus when a case opens', () => {
  it('a new case opens on its title, selected, and Enter goes on in the text', () => {
    render(<NoteEditor note={makeNote({ title: 'Nouvelle case', content: '' })} onNoteUpdated={() => {}} focusOnOpen />);
    const title = titleField();
    expect(document.activeElement).toBe(title);
    expect([title.selectionStart, title.selectionEnd]).toEqual([0, 'Nouvelle case'.length]);
    fireEvent.change(title, { target: { value: 'Appeler le plombier' } });
    fireEvent.keyDown(title, { key: 'Enter' });
    expect(document.activeElement).toBe(contentField());
    expect(contentField().value).toBe('');
  });

  it('an existing case keeps the cursor in its text', () => {
    render(<NoteEditor note={makeNote({ title: 'Appeler le plombier', content: 'Avant vendredi.' })} onNoteUpdated={() => {}} focusOnOpen />);
    expect(document.activeElement).toBe(contentField());
  });
});

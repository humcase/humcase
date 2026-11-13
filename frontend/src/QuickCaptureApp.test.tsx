import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act, waitFor, cleanup } from '@testing-library/react';

// Simulated Tauri window: quick capture hides after creation.
const winBus = vi.hoisted(() => ({
  hide: vi.fn().mockResolvedValue(undefined),
  onFocusChanged: vi.fn().mockResolvedValue(() => {}),
  setTitle: vi.fn().mockResolvedValue(undefined),
}));
const eventBus = vi.hoisted(() => ({
  // Listeners by event name (open note, append confirmation…).
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation(async (name: string, callback: (event: { payload: unknown }) => void) => {
    eventBus.handlers.set(name, callback);
    return () => {
      if (eventBus.handlers.get(name) === callback) eventBus.handlers.delete(name);
    };
  }),
  emit: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ hide: winBus.hide, onFocusChanged: winBus.onFocusChanged, setTitle: winBus.setTitle }),
}));

vi.mock('./services/tauriApi', () => ({
  api: {
    getProjects: vi.fn(),
    createNote: vi.fn(),
    getOrCreateInbox: vi.fn(),
  },
}));

import QuickCaptureApp from './QuickCaptureApp';
import { api } from './services/tauriApi';
import { emit } from '@tauri-apps/api/event';
import { setQuickCapturePersistenceRefused, prepareQuickCaptureForRestore, quickCapturePersistenceUnavailable, QUICK_CAPTURE_STATE_KEY, readQuickCaptureState, writeQuickCaptureState } from './utils/quickCaptureBarrier';

const mockApi = vi.mocked(api);
const mockEmit = vi.mocked(emit);

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  // The persistence flag is module state shared between tests: we start from a healthy state.
  setQuickCapturePersistenceRefused(false);
  eventBus.handlers.clear();
  winBus.hide.mockResolvedValue(undefined);
  winBus.setTitle.mockResolvedValue(undefined);
  winBus.onFocusChanged.mockResolvedValue(() => {});
  mockApi.getProjects.mockResolvedValue([
    { id: 'p1', name: 'Projet', color: '#3b82f6', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0, space: null },
  ]);
  mockApi.createNote.mockResolvedValue('n1');
  mockApi.getOrCreateInbox.mockResolvedValue('inbox-work');
});

describe('Quick Capture — restore preparation', () => {
  it('follows the main window theme without replacing the draft in progress', async () => {
    localStorage.setItem('humcase-workspace-theme', 'light');
    const view = render(<QuickCaptureApp />);
    await waitFor(() => expect(mockApi.getProjects).toHaveBeenCalled());
    const input = screen.getByRole('textbox', { name: 'Texte de la capture' }) as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'Une idée conservée' } });
    expect(view.container.querySelector('.workspace-app')?.getAttribute('data-theme')).toBe('light');
    act(() => {
      localStorage.setItem('humcase-workspace-theme', 'dark');
      window.dispatchEvent(new StorageEvent('storage', { key: 'humcase-workspace-theme', newValue: 'dark' }));
    });
    expect(view.container.querySelector('.workspace-app')?.getAttribute('data-theme')).toBe('dark');
    expect(input.value).toBe('Une idée conservée');
    expect(screen.getByRole('combobox', { name: 'Nature de la case' })).toBeTruthy();
    expect(mockApi.createNote).not.toHaveBeenCalled();
  });
  it('respects a private-only usage and does not offer the professional projects', async () => {
    localStorage.setItem('humcase-spaces-v1', JSON.stringify({ usage: 'private', scope: 'private', combined: false }));
    mockApi.getProjects.mockResolvedValue([{ id: 'work-project', name: 'Projet pro', color: '#000000', space: 'work', created_at: '', updated_at: '', revision: 0 }]);
    render(<QuickCaptureApp />);
    const destination = screen.getByRole('combobox', { name: 'Destination de la case' }) as HTMLSelectElement;
    await waitFor(() => expect(destination.value).toBe('inbox:private'));
    expect(screen.queryByRole('option', { name: 'Projet pro' })).toBeNull();
    expect(screen.queryByRole('option', { name: 'Réception · Pro' })).toBeNull();
    const input = screen.getByPlaceholderText("J’ai besoin de…");
    fireEvent.change(input, { target: { value: 'Une sortie à préparer' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(mockApi.getOrCreateInbox).toHaveBeenCalledWith('private'));
  });
  it('captures without a project and only creates the inbox on confirmation', async () => {
    mockApi.getProjects.mockResolvedValue([]);
    render(<QuickCaptureApp />);
    await waitFor(() => expect((screen.getByRole('combobox', { name: 'Destination de la case' }) as HTMLSelectElement).value).toBe('inbox:work'));
    expect(mockApi.getOrCreateInbox).not.toHaveBeenCalled();
    const input = screen.getByPlaceholderText("J’ai besoin de…");
    fireEvent.change(input, { target: { value: 'Une pensée à ranger plus tard' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith(expect.objectContaining({ project_id: 'inbox-work', title: 'Une pensée à ranger plus tard' })));
    expect(mockApi.getOrCreateInbox).toHaveBeenCalledWith('work');
  });

  it('keeps the draft if the inbox is unavailable', async () => {
    mockApi.getOrCreateInbox.mockRejectedValue(new Error('indisponible'));
    render(<QuickCaptureApp />);
    await waitFor(() => expect((screen.getByRole('combobox', { name: 'Destination de la case' }) as HTMLSelectElement).value).toBe('inbox:work'));
    const input = screen.getByPlaceholderText("J’ai besoin de…");
    fireEvent.change(input, { target: { value: 'À conserver' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await screen.findByText(/Création impossible/);
    expect((input as HTMLInputElement).value).toBe('À conserver');
    expect(mockApi.createNote).not.toHaveBeenCalled();
    expect(winBus.hide).not.toHaveBeenCalled();
    expect(prepareQuickCaptureForRestore().ready).toBe(false);
  });
  it('refuses a submission even with an empty title and an unreadable state', () => {
    writeQuickCaptureState({ title: '', noteType: 'todo', projectId: 'p1', submitting: true });
    expect(prepareQuickCaptureForRestore().ready).toBe(false);
    localStorage.setItem(QUICK_CAPTURE_STATE_KEY, '{');
    expect(prepareQuickCaptureForRestore().ready).toBe(false);
  });

  it('blocks a draft even after hiding and unmounting, then rehydrates it', async () => {
    const view = render(<QuickCaptureApp />);
    const field = await screen.findByPlaceholderText("J’ai besoin de…");
    fireEvent.change(field, { target: { value: 'À garder' } });
    expect(prepareQuickCaptureForRestore().ready).toBe(false);
    fireEvent.keyDown(field, { key: 'Escape' });
    await waitFor(() => expect(winBus.hide).toHaveBeenCalled());
    view.unmount();
    expect(prepareQuickCaptureForRestore().ready).toBe(false);
    render(<QuickCaptureApp />);
    expect((await screen.findByPlaceholderText("J’ai besoin de…") as HTMLInputElement).value).toBe('À garder');
  });

  it('blocks the restore during a submission, even after unmounting', async () => {
    let resolve: (id: string) => void = () => {};
    mockApi.createNote.mockReturnValue(new Promise<string>(res => { resolve = res; }));
    const view = render(<QuickCaptureApp />);
    await waitFor(() => expect(mockApi.getProjects).toHaveBeenCalled());
    const field = await screen.findByPlaceholderText("J’ai besoin de…");
    fireEvent.change(field, { target: { value: 'En cours' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(prepareQuickCaptureForRestore().ready).toBe(false);
    expect(JSON.parse(localStorage.getItem(QUICK_CAPTURE_STATE_KEY)!).submitting).toBe(true);
    view.unmount();
    expect(prepareQuickCaptureForRestore().ready).toBe(false);
    await act(async () => { resolve('n1'); });
    expect(prepareQuickCaptureForRestore()).toEqual({ ready: true });
  });

  it('after a restore, falls back to the Inbox when the remembered project no longer exists', async () => {
    // A successful restore restarts the whole application: the Quick Capture window starts
    // again and must not offer a project that the archive does not contain.
    localStorage.setItem('last_project_id', 'gone');
    render(<QuickCaptureApp />);
    await waitFor(() => expect(mockApi.getProjects).toHaveBeenCalled());
    const input = await screen.findByPlaceholderText("J’ai besoin de…");
    fireEvent.change(input, { target: { value: 'After the restore' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith(expect.objectContaining({ project_id: 'inbox-work', title: 'After the restore' })));
    cleanup();
  });

  it('keeps the remembered project when it still exists', async () => {
    localStorage.setItem('last_project_id', 'p1');
    render(<QuickCaptureApp />);
    await waitFor(() => expect(mockApi.getProjects).toHaveBeenCalled());
    const input = await screen.findByPlaceholderText("J’ai besoin de…");
    fireEvent.change(input, { target: { value: 'Same project' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith(expect.objectContaining({ project_id: 'p1', title: 'Same project' })));
    cleanup();
  });
});

describe("Quick Capture — input during saving", () => {
  it("disables the field during the write and clears it on success", async () => {
    let resolve: (id: string) => void = () => {};
    mockApi.createNote.mockReturnValue(new Promise<string>(res => { resolve = res; }));

    render(<QuickCaptureApp />);
    await waitFor(() => expect(mockApi.getProjects).toHaveBeenCalled());
    const field = (await screen.findByPlaceholderText("J’ai besoin de…")) as HTMLInputElement;

    fireEvent.change(field, { target: { value: 'Nouvelle idée' } });
    fireEvent.keyDown(field, { key: 'Enter' });

    // During the write, input is no longer editable: nothing can be erased by mistake.
    await waitFor(() => expect(field.disabled).toBe(true));
    expect(mockApi.createNote).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Nouvelle idée' })
    );

    await act(async () => { resolve('n1'); });
    // Success: the (unchanged) field is cleared and the window hidden.
    await waitFor(() =>
      expect((screen.getByPlaceholderText("J’ai besoin de…") as HTMLInputElement).value).toBe('')
    );
    expect(winBus.hide).toHaveBeenCalled();
  });

  it("does not clear the field if the value changed during the write", async () => {
    let resolve: (id: string) => void = () => {};
    mockApi.createNote.mockReturnValue(new Promise<string>(res => { resolve = res; }));

    render(<QuickCaptureApp />);
    await waitFor(() => expect(mockApi.getProjects).toHaveBeenCalled());
    const field = (await screen.findByPlaceholderText("J’ai besoin de…")) as HTMLInputElement;

    fireEvent.change(field, { target: { value: 'Note A' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    await waitFor(() => expect(field.disabled).toBe(true));

    // Simulates a programmatic change of the value during the write (path covered by the
    // functional guard, independently of `disabled`).
    fireEvent.change(field, { target: { value: 'Note B' } });
    await act(async () => { resolve('n1'); });

    await waitFor(() =>
      expect((screen.getByPlaceholderText("J’ai besoin de…") as HTMLInputElement).value).toBe('Note B')
    );
  });
});

describe('Quick Capture — in-memory fallback when storage refuses', () => {
  it('writing without storage does not throw, rereads the in-memory fallback, then recovers', () => {
    const refus = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('quota atteint', 'QuotaExceededError');
    });
    try {
      expect(() => writeQuickCaptureState({ title: 'En mémoire', noteType: 'note', projectId: 'p1', submitting: false })).not.toThrow();
      expect(quickCapturePersistenceUnavailable()).toBe(true);
      expect(readQuickCaptureState()).toEqual(expect.objectContaining({ title: 'En mémoire' }));
    } finally {
      refus.mockRestore();
    }
    // Recovery: a successful write goes back to normal persistence.
    writeQuickCaptureState({ title: '', noteType: 'note', projectId: 'p1', submitting: false });
    expect(quickCapturePersistenceUnavailable()).toBe(false);
    expect(readQuickCaptureState()).toEqual(expect.objectContaining({ title: '' }));
  });

  it('keeps input possible and shows a warning when setItem fails', async () => {
    const refus = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('quota atteint', 'QuotaExceededError');
    });
    const view = render(<QuickCaptureApp />);
    try {
      const field = (await screen.findByPlaceholderText("J’ai besoin de…")) as HTMLInputElement;
      // Without the in-memory fallback, writeQuickCaptureState would throw in onChange: the field
      // would no longer update at all.
      fireEvent.change(field, { target: { value: 'Brouillon sans stockage' } });
      expect(field.value).toBe('Brouillon sans stockage');
      expect(screen.getByRole('status').textContent).toMatch(/Stockage local indisponible/);
      // The last typed state stays readable even without persistence.
      expect(readQuickCaptureState()).toEqual(expect.objectContaining({ title: 'Brouillon sans stockage' }));
    } finally {
      refus.mockRestore();
    }
    // Recovery: storage becomes usable again, the warning disappears and the state
    // becomes persistent again.
    const field = screen.getByPlaceholderText("J’ai besoin de…") as HTMLInputElement;
    fireEvent.change(field, { target: { value: 'Brouillon sans stockage 2' } });
    await waitFor(() => expect(screen.queryByRole('status')).toBeNull());
    expect(JSON.parse(localStorage.getItem(QUICK_CAPTURE_STATE_KEY)!).title).toBe('Brouillon sans stockage 2');
    view.unmount();
  });

  it("reports to the main window every change of persistence state", async () => {
    const refus = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('quota atteint', 'QuotaExceededError');
    });
    const view = render(<QuickCaptureApp />);
    try {
      const field = (await screen.findByPlaceholderText("J’ai besoin de…")) as HTMLInputElement;
      fireEvent.change(field, { target: { value: 'Brouillon sans stockage' } });
      // The main window must learn about the refusal (restore fail-closed).
      await waitFor(() =>
        expect(mockEmit).toHaveBeenCalledWith('humcase:qc-persistence', { refused: true })
      );
    } finally {
      refus.mockRestore();
    }

    // Recovery: a successful write must report again that the state is persisted again.
    const field = screen.getByPlaceholderText("J’ai besoin de…") as HTMLInputElement;
    fireEvent.change(field, { target: { value: 'Brouillon sans stockage 2' } });
    await waitFor(() =>
      expect(mockEmit).toHaveBeenCalledWith('humcase:qc-persistence', { refused: false })
    );
    view.unmount();
  });
});

describe('Quick Capture — append to the open note (0.9.83)', () => {
  const openAction = async () => {
    render(<QuickCaptureApp />);
    await waitFor(() => expect(eventBus.handlers.has('humcase:open-note')).toBe(true));
    expect(mockEmit).toHaveBeenCalledWith('humcase:open-note-query');
    act(() => eventBus.handlers.get('humcase:open-note')!({ payload: { id: 'n-ouverte', title: 'Plan de lancement' } }));
    const destination = await screen.findByRole('combobox', { name: 'Destination de la case' }) as HTMLSelectElement;
    fireEvent.change(destination, { target: { value: 'open-note' } });
    expect(destination.value).toBe('open-note');
    expect((screen.getByRole('combobox', { name: 'Nature de la case' }) as HTMLSelectElement).disabled).toBe(true);
    const field = screen.getByRole('textbox', { name: 'Texte de la capture' }) as HTMLInputElement;
    fireEvent.change(field, { target: { value: 'Relancer le fournisseur' } });
    return field;
  };

  it('offers the open note and only clears the text after its confirmation', async () => {
    const field = await openAction();
    await act(async () => { fireEvent.keyDown(field, { key: 'Enter' }); });
    const send = mockEmit.mock.calls.find(([fileName]) => fileName === 'humcase:qc-append');
    expect(send?.[1]).toMatchObject({ noteId: 'n-ouverte', text: 'Relancer le fournisseur' });
    expect(field.value).toBe('Relancer le fournisseur');
    const { requestId } = send![1] as { requestId: string };
    await act(async () => { eventBus.handlers.get('humcase:qc-append-result')!({ payload: { requestId, ok: true } }); });
    await waitFor(() => expect(field.value).toBe(''));
    expect(winBus.hide).toHaveBeenCalled();
    expect(mockApi.createNote).not.toHaveBeenCalled();
    expect(localStorage.getItem('last_project_id')).not.toBe('open-note');
  });

  it('keeps the text if the open note does not respond', async () => {
    const field = await openAction();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      await act(async () => { fireEvent.keyDown(field, { key: 'Enter' }); });
      await act(async () => { await vi.advanceTimersByTimeAsync(4100); });
    } finally {
      vi.useRealTimers();
    }
    expect(await screen.findByText(/Ajout impossible.*Le texte est conservé/)).toBeTruthy();
    expect(field.value).toBe('Relancer le fournisseur');
    expect(winBus.hide).not.toHaveBeenCalled();
    expect(mockApi.createNote).not.toHaveBeenCalled();
  });
});

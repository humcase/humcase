import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import type { Note } from '../../types';

vi.mock('../../services/tauriApi', () => ({ api: { updateNote: vi.fn(), openLink: vi.fn() } }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ onCloseRequested: () => Promise.resolve(() => {}) }) }));
vi.mock('../../utils/pendingWrites', () => ({ addPendingWrite: vi.fn(() => ({ id: 'kept-1' })), removePendingWrite: vi.fn(), pendingWritesPersistent: vi.fn(() => true) }));

type Element = { id: string; version: number; isDeleted?: boolean };
type Props = {
  onChange: (elements: Element[], appState: Record<string, unknown>, files: Record<string, unknown>) => void;
  initialData: { elements: Element[] };
  excalidrawAPI: (api: unknown) => void;
  children?: ReactNode;
};
let props: Props | null = null;
const excalidrawApi = { updateScene: vi.fn(), addFiles: vi.fn() };
const Pass = ({ children }: { children?: ReactNode }) => <>{children}</>;
const Nothing = () => null;
// Excalidraw itself is out of these tests: a stand-in keeps its props and serializes like it.
vi.mock('./loadExcalidraw', () => ({
  loadExcalidraw: () => Promise.resolve({
    Excalidraw: (received: Props) => { props = received; received.excalidrawAPI(excalidrawApi); return <div data-testid="excalidraw">{received.children}</div>; },
    MainMenu: Object.assign(Pass, { Separator: Nothing, DefaultItems: { Export: Nothing, SaveAsImage: Nothing, Help: Nothing, ClearCanvas: Nothing, ChangeCanvasBackground: Nothing } }),
    WelcomeScreen: Object.assign(Nothing, { Hints: { ToolbarHint: Nothing, MenuHint: Nothing, HelpHint: Nothing }, Center: Object.assign(Pass, { Heading: Pass }) }),
    serializeAsJSON: (elements: Element[]) => JSON.stringify({ type: 'excalidraw', version: 2, elements, appState: {}, files: {} }),
    getSceneVersion: (elements: Element[]) => elements.reduce((sum, element) => sum + element.version, 0),
    restoreElements: (elements: Element[]) => elements,
  }),
}));

import { api } from '../../services/tauriApi';
import { addPendingWrite, pendingWritesPersistent, removePendingWrite } from '../../utils/pendingWrites';
import SketchEditor, { SKETCH_RETRY_DELAY } from './SketchEditor';

const mockApi = vi.mocked(api);
const scene = (elements: Element[]) => JSON.stringify({ type: 'excalidraw', version: 2, elements, appState: {}, files: {} });
const sketch = (content: string, revision = 1): Note => ({
  id: 's1', project_id: 'p1', title: 'Plan', content, note_type: 'note', status: 'open',
  created_at: '2026-10-02T10:00:00.000Z', updated_at: '2026-10-02T10:00:00.000Z', revision, parent_id: 'c1', content_kind: 'sketch',
});
const box = { id: 'box', version: 1 };
const draw = (elements: Element[]) => act(() => { props?.onChange(elements, { viewBackgroundColor: '#fff' }, {}); });

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.clearAllMocks();
  props = null;
  mockApi.updateNote.mockResolvedValue(undefined);
});
afterEach(() => { vi.useRealTimers(); });

async function open(note: Note, onNoteUpdated = vi.fn()) {
  const view = render(<SketchEditor note={note} theme="dark" onNoteUpdated={onNoteUpdated} />);
  await screen.findByTestId('excalidraw');
  return view;
}

describe('SketchEditor', () => {
  it('writes the drawing a moment after a stroke, not when the scene loads', async () => {
    await open(sketch(scene([box])));
    draw([box]);
    act(() => { vi.advanceTimersByTime(2000); });
    expect(mockApi.updateNote).not.toHaveBeenCalled();
    draw([box, { id: 'arrow', version: 1 }]);
    expect(screen.getByRole('status').textContent).toBe('Modifié');
    act(() => { vi.advanceTimersByTime(800); });
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledWith('s1', { content: scene([box, { id: 'arrow', version: 1 }]) }));
    await waitFor(() => expect(screen.getByRole('status').textContent).toBe('Enregistré'));
  });

  it('writes a stroke at once when the case is left', async () => {
    const view = await open(sketch(scene([])));
    draw([]);
    draw([box]);
    view.unmount();
    // Without waiting for the pause after the stroke.
    await act(async () => { await Promise.resolve(); });
    expect(mockApi.updateNote).toHaveBeenCalledWith('s1', { content: scene([box]) });
  });

  it('keeps a drawing that could not be written, with what the database held', async () => {
    mockApi.updateNote.mockRejectedValue('disk full');
    await open(sketch(scene([])));
    draw([]);
    draw([box]);
    act(() => { vi.advanceTimersByTime(800); });
    expect((await screen.findByRole('alert')).textContent).toContain('disk full');
    expect(addPendingWrite).toHaveBeenCalledWith(expect.objectContaining({
      kind: 'update', noteId: 's1', payload: { content: scene([box]) }, base: { content: scene([]) },
    }));
  });

  it('tries a drawing that could not be written again, then drops the copy kept for recovery', async () => {
    mockApi.updateNote.mockRejectedValueOnce('disk full');
    await open(sketch(scene([])));
    draw([]);
    draw([box]);
    act(() => { vi.advanceTimersByTime(800); });
    await screen.findByRole('alert');
    expect(removePendingWrite).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(SKETCH_RETRY_DELAY); });
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledTimes(2));
    expect(mockApi.updateNote).toHaveBeenLastCalledWith('s1', { content: scene([box]) });
    await waitFor(() => expect(removePendingWrite).toHaveBeenCalledWith('kept-1'));
    expect(screen.getByRole('status').textContent).toBe('Enregistré');
  });

  it('writes a drawing that could not be written again when the case is left', async () => {
    mockApi.updateNote.mockRejectedValueOnce('disk full');
    const view = await open(sketch(scene([])));
    draw([]);
    draw([box]);
    act(() => { vi.advanceTimersByTime(800); });
    await screen.findByRole('alert');
    view.unmount();
    await waitFor(() => expect(mockApi.updateNote).toHaveBeenCalledTimes(2));
    expect(mockApi.updateNote).toHaveBeenLastCalledWith('s1', { content: scene([box]) });
  });

  it('says when the kept drawing will not survive closing Humcase', async () => {
    mockApi.updateNote.mockRejectedValue('disk full');
    vi.mocked(pendingWritesPersistent).mockReturnValueOnce(false);
    await open(sketch(scene([])));
    draw([]);
    draw([box]);
    act(() => { vi.advanceTimersByTime(800); });
    expect((await screen.findByRole('alert')).textContent).toContain('ne fermez pas Humcase');
  });

  it('shows a change made elsewhere, but never an older read arriving late', async () => {
    const view = await open(sketch(scene([box]), 3));
    draw([box]);
    view.rerender(<SketchEditor note={sketch(scene([]), 2)} theme="dark" onNoteUpdated={vi.fn()} />);
    expect(excalidrawApi.updateScene).not.toHaveBeenCalled();
    const agent = [box, { id: 'note', version: 1 }];
    view.rerender(<SketchEditor note={sketch(scene(agent), 4)} theme="dark" onNoteUpdated={vi.fn()} />);
    expect(excalidrawApi.updateScene).toHaveBeenCalledWith({ elements: agent });
  });

  it('shows a content that is not a drawing as it is, and never writes over it', async () => {
    const view = render(<SketchEditor note={sketch('# Plan\n\nA header and a map')} theme="light" onNoteUpdated={vi.fn()} />);
    expect(screen.getByRole('alert').textContent).toContain('rien de lisible');
    expect(screen.getByText(/A header and a map/)).toBeTruthy();
    expect(screen.queryByTestId('excalidraw')).toBeNull();
    view.unmount();
    act(() => { vi.advanceTimersByTime(1000); });
    expect(mockApi.updateNote).not.toHaveBeenCalled();
  });
});

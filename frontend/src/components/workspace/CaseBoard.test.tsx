import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useState } from 'react';
import type { Note, NoteStatus } from '../../types';
import CaseBoard from './CaseBoard';
import { columnsFor, layoutCases } from '../../utils/caseLayout';
import { canMoveInto } from '../../utils/caseBoard';

vi.mock('../../services/tauriApi', () => ({ api: {
  getCasePositions: vi.fn(),
  setCasePosition: vi.fn(),
  setCasePositions: vi.fn(),
  createSubCase: vi.fn(),
  createNote: vi.fn(),
  moveCase: vi.fn(),
  deleteNote: vi.fn(),
  restoreFromTrash: vi.fn(),
  getNote: vi.fn(),
  openLink: vi.fn(),
  saveImageToDisk: vi.fn(),
  importJsonCanvas: vi.fn(),
  exportJsonCanvas: vi.fn(),
} }));
// The picture of a sketch is drawn by Excalidraw, out of these tests.
vi.mock('../sketch/SketchThumbnail', () => ({ default: ({ label }: { label: string }) => <span role="img" aria-label={label} /> }));

import { api } from '../../services/tauriApi';
const mockApi = vi.mocked(api);

const note = (id: string, title: string, parent_id: string | null = null, content = '', created_at = '2026-10-01T10:00:00.000Z'): Note => ({
  id, project_id: 'p1', title, content, note_type: 'note', status: 'open',
  created_at, updated_at: created_at, revision: 0, parent_id, content_kind: 'text',
});

const voyage = note('voyage', 'Voyage', null, 'Une semaine à Séville.');
const budget = note('budget', 'Budget', null, '', '2026-10-01T10:01:00.000Z');
const hotels = { ...note('hotels', 'Hôtels', 'voyage', 'Trois hôtels retenus.'), content_kind: 'research' as const };
const hotelX = note('hotel-x', 'Hôtel X', 'hotels', '140 € la nuit.');
const all = [voyage, budget, hotels, hotelX];

function Harness(props: Partial<Parameters<typeof CaseBoard>[0]> & { initialPath?: string[] }) {
  const [path, setPath] = useState<string[]>(props.initialPath ?? []);
  return <CaseBoard rootCases={[voyage, budget]} allNotes={all} projectId="p1" creationProjectId={props.creationProjectId === undefined ? 'p1' : props.creationProjectId} path={path} onPathChange={setPath}
    onOpenNote={props.onOpenNote ?? vi.fn()} onCreateRootCase={props.onCreateRootCase ?? vi.fn()} onChanged={props.onChanged ?? vi.fn()} />;
}

const cardOf = (title: string) => screen.getByRole('button', { name: `Ouvrir ${title}` }).closest('article') as HTMLElement;
/** A sub-case shown inside its group on the board. */
const chipOf = (title: string) => screen.getByRole('button', { name: title }).closest('li') as HTMLElement;

beforeEach(() => {
  vi.clearAllMocks();
  mockApi.getCasePositions.mockResolvedValue([]);
  mockApi.setCasePosition.mockResolvedValue(undefined);
  mockApi.moveCase.mockResolvedValue(undefined);
  mockApi.setCasePositions.mockResolvedValue(undefined);
  mockApi.deleteNote.mockResolvedValue(undefined);
  mockApi.restoreFromTrash.mockResolvedValue(undefined);
});

describe('layoutCases', () => {
  it('keeps stored positions and puts the other cases in free grid slots', () => {
    const layout = layoutCases([voyage, budget, hotels], new Map([['budget', { x: 24, y: 24 }]]));
    expect(layout.get('budget')).toEqual({ x: 24, y: 24 });
    // The first slot is taken by budget; the others follow, oldest first (then by id).
    expect(layout.get('hotels')).toEqual({ x: 296, y: 24 });
    expect(layout.get('voyage')).toEqual({ x: 568, y: 24 });
  });

  it('wraps to the width of the board', () => {
    expect(columnsFor(600)).toBe(2);
    expect(columnsFor(100)).toBe(1);
    const layout = layoutCases([voyage, budget, hotels], new Map(), 2);
    expect(layout.get('budget')).toEqual({ x: 24, y: 244 });
  });
});

describe('CaseBoard', () => {
  it('takes the database answer to the search, like the other views', async () => {
    // Alone, the board finds "seville" in Voyage ("Séville": accents ignored, like the database);
    // once the database has answered, its answer is the one shown.
    const view = (searchIds: ReadonlySet<string> | null) => <CaseBoard rootCases={[voyage, budget]} allNotes={all} projectId="p1" creationProjectId="p1" path={[]} onPathChange={vi.fn()}
      onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} query="seville" searchIds={searchIds} />;
    const { rerender } = render(view(null));
    expect((await screen.findByText(/trouvées?$/)).textContent).toBe('1 trouvée');
    expect(cardOf('Budget').className).toContain('is-dimmed');
    rerender(view(new Set(['budget'])));
    expect(cardOf('Budget').className).not.toContain('is-dimmed');
    expect(cardOf('Voyage').className).toContain('is-dimmed');
  });

  it('shows a case with sub-cases as a group holding them, and opens a case on its title', async () => {
    const onOpenNote = vi.fn();
    render(<Harness onOpenNote={onOpenNote} />);
    await waitFor(() => expect(mockApi.getCasePositions).toHaveBeenCalledWith('p1'));
    expect(within(cardOf('Voyage')).getByRole('button', { name: '1 sous-case' })).toBeTruthy();
    expect(within(cardOf('Voyage')).getByRole('button', { name: 'Hôtels' })).toBeTruthy();
    expect(within(cardOf('Budget')).getByRole('button', { name: 'Ouvrir le plan' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir Voyage' }));
    expect(onOpenNote).toHaveBeenCalledWith(voyage);
    fireEvent.click(screen.getByRole('button', { name: 'Hôtels' }));
    expect(onOpenNote).toHaveBeenCalledWith(hotels);
  });

  it('dives into a case by its button or a double-click, and climbs back through the path', async () => {
    render(<Harness />);
    fireEvent.click(within(cardOf('Voyage')).getByRole('button', { name: '1 sous-case' }));
    expect(within(cardOf('Hôtels')).getByRole('button', { name: 'Hôtel X' })).toBeTruthy();
    fireEvent.doubleClick(cardOf('Hôtels'));
    expect(screen.getByRole('button', { name: 'Ouvrir Hôtel X' })).toBeTruthy();
    const path = screen.getByRole('navigation', { name: 'Chemin dans le plan de travail' });
    expect(path.textContent).toContain('Voyage');
    expect(path.textContent).toContain('Hôtels');
    fireEvent.click(within(path).getByRole('button', { name: 'Plan de travail' }));
    expect(screen.getByRole('button', { name: 'Ouvrir Voyage' })).toBeTruthy();
  });

  it('places stored cases where they were left', async () => {
    mockApi.getCasePositions.mockResolvedValue([{ id: 'voyage', x: 400, y: 300 }]);
    render(<Harness />);
    await waitFor(() => expect(cardOf('Voyage').style.left).toBe('400px'));
    expect(cardOf('Voyage').style.top).toBe('300px');
  });

  it('saves a dragged case where it is dropped, and a drag never opens the case', async () => {
    const onOpenNote = vi.fn();
    const onChanged = vi.fn();
    render(<Harness onOpenNote={onOpenNote} onChanged={onChanged} />);
    await waitFor(() => expect(mockApi.getCasePositions).toHaveBeenCalled());
    const title = screen.getByRole('button', { name: 'Ouvrir Voyage' });
    const start = { x: Number.parseFloat(cardOf('Voyage').style.left), y: Number.parseFloat(cardOf('Voyage').style.top) };
    fireEvent.pointerDown(title, { button: 0, clientX: 10, clientY: 10 });
    act(() => {
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 70, clientY: 50 }));
      window.dispatchEvent(new MouseEvent('pointerup', { clientX: 70, clientY: 50 }));
    });
    fireEvent.click(title);
    expect(onOpenNote).not.toHaveBeenCalled();
    await waitFor(() => expect(mockApi.setCasePosition).toHaveBeenCalledWith('voyage', start.x + 60, start.y + 40));
    expect(cardOf('Voyage').style.left).toBe(`${start.x + 60}px`);
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    // After the drag, a click without a press (keyboard) opens the case again.
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)); });
    fireEvent.click(title);
    expect(onOpenNote).toHaveBeenCalledWith(voyage);
  });

  it('a position read still in flight never puts a moved case back', async () => {
    let answer: (positions: { id: string; x: number; y: number }[]) => void = () => {};
    mockApi.getCasePositions.mockReturnValueOnce(new Promise(resolve => { answer = resolve; }));
    render(<Harness />);
    await waitFor(() => expect(mockApi.getCasePositions).toHaveBeenCalled());
    fireEvent.pointerDown(cardOf('Budget'), { button: 0, clientX: 10, clientY: 10 });
    act(() => {
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 110, clientY: 210 }));
      window.dispatchEvent(new MouseEvent('pointerup', { clientX: 110, clientY: 210 }));
    });
    await waitFor(() => expect(mockApi.setCasePosition).toHaveBeenCalled());
    const [, x, y] = mockApi.setCasePosition.mock.calls[0];
    // The read started before the move answers now, without the move.
    await act(async () => { answer([]); });
    expect(cardOf('Budget').style.left).toBe(`${x}px`);
    expect(cardOf('Budget').style.top).toBe(`${y}px`);
  });

  it('a drag cancelled by the system writes nothing and lets go of the case', () => {
    render(<Harness />);
    fireEvent.pointerDown(cardOf('Budget'), { button: 0, clientX: 10, clientY: 10 });
    act(() => {
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 110, clientY: 210 }));
      window.dispatchEvent(new MouseEvent('pointercancel', { clientX: 110, clientY: 210 }));
      window.dispatchEvent(new MouseEvent('pointerup', { clientX: 110, clientY: 210 }));
    });
    expect(mockApi.setCasePosition).not.toHaveBeenCalled();
    expect(cardOf('Budget').className).not.toContain('is-dragging');
  });

  it('renders only the cases near the screen of a large board', () => {
    const original = globalThis.ResizeObserver;
    globalThis.ResizeObserver = class {
      callback: ResizeObserverCallback;
      constructor(callback: ResizeObserverCallback) { this.callback = callback; }
      observe() { this.callback([{ contentRect: { width: 800, height: 600 } } as ResizeObserverEntry], this as unknown as ResizeObserver); }
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
    try {
      const many = Array.from({ length: 300 }, (_, index) => note(`m${index}`, `Case ${index}`, null, '', `2026-10-01T10:${String(index % 60).padStart(2, '0')}:${String(Math.floor(index / 60)).padStart(2, '0')}.000Z`));
      render(<CaseBoard rootCases={many} allNotes={many} projectId="p1" creationProjectId="p1" path={[]}
        onPathChange={vi.fn()} onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} />);
      const shown = document.querySelectorAll('.case-board__layer > article').length;
      expect(shown).toBeGreaterThan(0);
      expect(shown).toBeLessThan(300);
    } finally {
      globalThis.ResizeObserver = original;
    }
  });

  it('a short press is a click, not a move', async () => {
    const onOpenNote = vi.fn();
    render(<Harness onOpenNote={onOpenNote} />);
    const title = screen.getByRole('button', { name: 'Ouvrir Voyage' });
    fireEvent.pointerDown(title, { button: 0, clientX: 10, clientY: 10 });
    act(() => {
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 12, clientY: 11 }));
      window.dispatchEvent(new MouseEvent('pointerup', { clientX: 12, clientY: 11 }));
    });
    fireEvent.click(title);
    expect(onOpenNote).toHaveBeenCalledWith(voyage);
    expect(mockApi.setCasePosition).not.toHaveBeenCalled();
  });

  it('reports a move that was not saved and rereads the positions', async () => {
    mockApi.setCasePosition.mockRejectedValue('database locked');
    render(<Harness />);
    await waitFor(() => expect(mockApi.getCasePositions).toHaveBeenCalledTimes(1));
    fireEvent.pointerDown(cardOf('Voyage'), { button: 0, clientX: 10, clientY: 10 });
    act(() => {
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 90, clientY: 10 }));
      window.dispatchEvent(new MouseEvent('pointerup', { clientX: 90, clientY: 10 }));
    });
    expect((await screen.findByRole('alert')).textContent).toContain('database locked');
    await waitFor(() => expect(mockApi.getCasePositions).toHaveBeenCalledTimes(2));
  });

  it('names a new top-level case right on the board and places it there', async () => {
    mockApi.createNote.mockResolvedValue('new');
    const onChanged = vi.fn();
    render(<Harness onChanged={onChanged} />);
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle case' }));
    const input = screen.getByLabelText('Nouvelle case');
    fireEvent.change(input, { target: { value: ' Idée de départ ' } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith({ project_id: 'p1', title: 'Idée de départ', note_type: 'note' }));
    // Voyage and Budget hold the first two slots (a group keeps the size of a card): the new case takes the third.
    await waitFor(() => expect(mockApi.setCasePosition).toHaveBeenCalledWith('new', 568, 24));
    expect(onChanged).toHaveBeenCalled();
  });

  it('leaves the usual flow create a top-level case when the board shows several projects', () => {
    const onCreateRootCase = vi.fn();
    render(<Harness creationProjectId={null} onCreateRootCase={onCreateRootCase} />);
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle case' }));
    expect(onCreateRootCase).toHaveBeenCalled();
    expect(screen.queryByLabelText('Nouvelle case')).toBeNull();
  });

  it('cancels an empty or escaped draft without writing', () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle case' }));
    fireEvent.keyDown(screen.getByLabelText('Nouvelle case'), { key: 'Escape' });
    expect(screen.queryByLabelText('Nouvelle case')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle case' }));
    fireEvent.submit(screen.getByLabelText('Nouvelle case').closest('form') as HTMLFormElement);
    expect(mockApi.createNote).not.toHaveBeenCalled();
  });

  it('inside a case, shows the case itself with a way to write in it, and adds a sub-case in place', async () => {
    const onOpenNote = vi.fn();
    mockApi.createSubCase.mockResolvedValue('resto');
    render(<Harness initialPath={['voyage']} onOpenNote={onOpenNote} />);
    const header = screen.getByRole('banner', { name: 'Case ouverte dans le plan de travail' });
    expect(within(header).getByRole('heading', { name: 'Voyage' })).toBeTruthy();
    expect(within(header).getByText('Une semaine à Séville.')).toBeTruthy();
    fireEvent.click(within(header).getByRole('button', { name: 'Écrire dans la case' }));
    expect(onOpenNote).toHaveBeenCalledWith(voyage);

    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle sous-case' }));
    const input = screen.getByLabelText('Titre de la sous-case');
    fireEvent.change(input, { target: { value: 'Restaurants' } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);
    await waitFor(() => expect(mockApi.createSubCase).toHaveBeenCalledWith('voyage', 'Restaurants', 'note', undefined, undefined));
    await waitFor(() => expect(mockApi.setCasePosition).toHaveBeenCalledWith('resto', 296, 24));
  });

  it('an empty case offers to add its first sub-case on the board', () => {
    render(<Harness initialPath={['budget']} />);
    expect(screen.getByText('Cette case n’a pas encore de texte.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Nouvelle sous-case.*double-cliquez/ }));
    expect(screen.getByLabelText('Titre de la sous-case')).toBeTruthy();
  });

  it('keeps the place of the cases when a newer case arrives', () => {
    const before = layoutCases([voyage, budget], new Map());
    const newer = note('newer', 'Plus récente', null, '', '2026-10-02T09:00:00.000Z');
    const after = layoutCases([newer, voyage, budget], new Map());
    expect(after.get('voyage')).toEqual(before.get('voyage'));
    expect(after.get('budget')).toEqual(before.get('budget'));
  });

  it('gathers a case and all its sub-cases in the document overview, each opening its original', async () => {
    const onOpenNote = vi.fn();
    render(<Harness onOpenNote={onOpenNote} initialPath={['voyage']} />);
    fireEvent.click(screen.getByRole('button', { name: 'Aperçu document' }));
    const document = screen.getByRole('complementary', { name: 'Aperçu document' });
    expect(within(document).getByRole('heading', { level: 2, name: 'Voyage' })).toBeTruthy();
    expect(within(document).getByRole('heading', { level: 3, name: 'Hôtels' })).toBeTruthy();
    expect(within(document).getByRole('heading', { level: 4, name: 'Hôtel X' })).toBeTruthy();
    expect(within(document).getByText('140 € la nuit.')).toBeTruthy();
    fireEvent.click(within(document).getByRole('button', { name: 'Ouvrir la case Hôtel X pour la modifier' }));
    expect(onOpenNote).toHaveBeenCalledWith(hotelX);
  });

  it('creates a case of the nature chosen on its draft', async () => {
    mockApi.createNote.mockResolvedValue('q');
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle case' }));
    const input = screen.getByLabelText('Nouvelle case');
    expect(screen.getByRole('radio', { name: 'Pensée' }).getAttribute('aria-checked')).toBe('true');
    fireEvent.click(screen.getByRole('radio', { name: 'Question' }));
    fireEvent.change(input, { target: { value: 'Hôtel ou appartement ?' } });
    fireEvent.submit(input.closest('form') as HTMLFormElement);
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith({ project_id: 'p1', title: 'Hôtel ou appartement ?', note_type: 'question' }));
  });

  it("shows what the page's search finds: matching cases stand out, the others fade", () => {
    render(<CaseBoard rootCases={[voyage, budget]} allNotes={all} projectId="p1" creationProjectId="p1" path={[]} onPathChange={vi.fn()} onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} query="hotels" />);
    expect(screen.getByText('1 trouvée')).toBeTruthy();
    expect(cardOf('Voyage').className).not.toContain('is-dimmed');
    expect(chipOf('Hôtels').className).toContain('is-hit');
    expect(cardOf('Budget').className).toContain('is-dimmed');
  });

  it("dims the cases outside the page's nature filter", () => {
    const idea = { ...note('idea', 'Partir en juin', null, '', '2026-10-01T10:02:00.000Z'), note_type: 'idea' as const };
    render(<CaseBoard rootCases={[voyage, budget, idea]} allNotes={[...all, idea]} projectId="p1" creationProjectId="p1" path={[]} onPathChange={vi.fn()} onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} nature="idea" />);
    expect(cardOf('Partir en juin').className).not.toContain('is-dimmed');
    expect(cardOf('Budget').className).toContain('is-dimmed');
  });

  it("dims the cases outside the page's status filter, and a group stands out when one of its sub-cases matches", () => {
    const done = { ...note('done', 'Billets réservés', null, '', '2026-10-01T10:02:00.000Z'), status: 'resolved' as const };
    const doing = { ...hotels, status: 'in_progress' as const };
    const view = (status: NoteStatus | null) => <CaseBoard rootCases={[voyage, budget, done]} allNotes={[voyage, budget, doing, hotelX, done]} projectId="p1" creationProjectId="p1" path={[]} onPathChange={vi.fn()} onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} status={status} />;
    const { rerender } = render(view('resolved'));
    expect(cardOf('Billets réservés').className).not.toContain('is-dimmed');
    expect(cardOf('Budget').className).toContain('is-dimmed');
    rerender(view('in_progress'));
    expect(cardOf('Billets réservés').className).toContain('is-dimmed');
    expect(cardOf('Voyage').className).not.toContain('is-dimmed');
    rerender(view(null));
    expect(cardOf('Budget').className).not.toContain('is-dimmed');
  });

  it('zoomed out, a group becomes one tile with its number of cases', () => {
    render(<Harness />);
    const zoomOut = screen.getByRole('button', { name: 'Dézoomer' });
    fireEvent.click(zoomOut);
    fireEvent.click(zoomOut);
    fireEvent.click(zoomOut);
    expect(screen.getByText('51 %')).toBeTruthy();
    const tile = screen.getByRole('article', { name: 'Voyage, 1 sous-case' });
    expect(within(tile).getByText('1 sous-case')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Hôtels' })).toBeNull();
    // The whole board fits at 100 %: it is not enlarged beyond.
    fireEvent.click(screen.getByRole('button', { name: 'Tout voir' }));
    expect(screen.getByText('100 %')).toBeTruthy();
  });

  it('groups the selected cases in a new case named by the user, with a way to undo', async () => {
    const idea = note('idea', 'Partir en juin', null, '', '2026-10-01T10:02:00.000Z');
    mockApi.createNote.mockResolvedValue('group');
    render(<CaseBoard rootCases={[voyage, budget, idea]} allNotes={[...all, idea]} projectId="p1" creationProjectId="p1" path={[]} onPathChange={vi.fn()} onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} />);
    for (const title of ['Budget', 'Partir en juin']) {
      fireEvent.pointerDown(cardOf(title), { button: 0, clientX: 5, clientY: 5, shiftKey: true });
      act(() => { window.dispatchEvent(new MouseEvent('pointerup', { clientX: 5, clientY: 5 })); });
    }
    expect(screen.getByText('2 cases sélectionnées')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Regrouper' }));
    expect(screen.getByRole('alert').textContent).toBe('Donnez un nom au groupe.');
    expect(mockApi.createNote).not.toHaveBeenCalled();
    fireEvent.change(screen.getByRole('textbox', { name: 'Nom du groupe' }), { target: { value: 'Budget du voyage' } });
    fireEvent.click(screen.getByRole('button', { name: 'Regrouper' }));
    await waitFor(() => expect(mockApi.createNote).toHaveBeenCalledWith({ project_id: 'p1', title: 'Budget du voyage', note_type: 'note' }));
    await waitFor(() => expect(mockApi.moveCase).toHaveBeenCalledTimes(2));
    expect(mockApi.moveCase).toHaveBeenCalledWith('budget', 'group');
    expect(mockApi.moveCase).toHaveBeenCalledWith('idea', 'group');
    // Budget held the second slot: the group takes its place.
    await waitFor(() => expect(mockApi.setCasePosition).toHaveBeenCalledWith('group', 296, 24));

    fireEvent.click(await screen.findByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(mockApi.deleteNote).toHaveBeenCalledWith('group'));
    expect(mockApi.moveCase).toHaveBeenCalledWith('budget', null);
    expect(mockApi.moveCase).toHaveBeenCalledWith('idea', null);
  });

  it('undoing a grouping keeps the group when another case went into it since', async () => {
    const idea = note('idea', 'Partir en juin', null, '', '2026-10-01T10:02:00.000Z');
    mockApi.createNote.mockResolvedValue('group');
    const props = { projectId: 'p1', creationProjectId: 'p1', path: [], onPathChange: vi.fn(), onOpenNote: vi.fn(), onCreateRootCase: vi.fn(), onChanged: vi.fn() };
    const { rerender } = render(<CaseBoard rootCases={[voyage, budget, idea]} allNotes={[...all, idea]} {...props} />);
    for (const title of ['Budget', 'Partir en juin']) {
      fireEvent.pointerDown(cardOf(title), { button: 0, clientX: 5, clientY: 5, shiftKey: true });
      act(() => { window.dispatchEvent(new MouseEvent('pointerup', { clientX: 5, clientY: 5 })); });
    }
    fireEvent.change(screen.getByRole('textbox', { name: 'Nom du groupe' }), { target: { value: 'Juin' } });
    fireEvent.click(screen.getByRole('button', { name: 'Regrouper' }));
    await waitFor(() => expect(mockApi.moveCase).toHaveBeenCalledTimes(2));
    // Voyage was dropped into the new group in the meantime.
    const group = note('group', 'Juin', null, '', '2026-10-01T10:05:00.000Z');
    rerender(<CaseBoard rootCases={[group]} allNotes={[group, { ...voyage, parent_id: 'group' }, { ...budget, parent_id: 'group' }, { ...idea, parent_id: 'group' }, hotels, hotelX]} {...props} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Annuler' }));
    await waitFor(() => expect(mockApi.moveCase).toHaveBeenCalledWith('idea', null));
    expect(mockApi.deleteNote).not.toHaveBeenCalled();
  });

  describe('moving cases to the trash from the board', () => {
    const click = (title: string, shiftKey = false) => {
      fireEvent.pointerDown(cardOf(title), { button: 0, clientX: 5, clientY: 5, shiftKey });
      act(() => { window.dispatchEvent(new MouseEvent('pointerup', { clientX: 5, clientY: 5 })); });
    };

    it('a click selects a case; Delete asks first, naming its sub-cases, and Enter confirms, with a way to undo', async () => {
      const onChanged = vi.fn();
      render(<Harness onChanged={onChanged} />);
      click('Voyage');
      expect(screen.getByText('1 case sélectionnée')).toBeTruthy();
      fireEvent.keyDown(window, { key: 'Delete' });
      expect(screen.getByRole('alertdialog').textContent).toContain('Mettre « Voyage » et 2 sous-cases à la corbeille ?');
      expect(mockApi.deleteNote).not.toHaveBeenCalled();
      fireEvent.keyDown(window, { key: 'Enter' });
      await waitFor(() => expect(mockApi.deleteNote).toHaveBeenCalledWith('voyage'));
      expect(mockApi.deleteNote).toHaveBeenCalledTimes(1);
      expect(onChanged).toHaveBeenCalled();
      fireEvent.click(await screen.findByRole('button', { name: 'Annuler' }));
      await waitFor(() => expect(mockApi.restoreFromTrash).toHaveBeenCalledWith('voyage'));
    });

    it('the confirmation holds the cases it was asked for, and Escape or Keep cancels it', async () => {
      render(<Harness />);
      click('Budget');
      fireEvent.click(screen.getByRole('button', { name: 'Mettre à la corbeille' }));
      expect(screen.getByRole('alertdialog').textContent).toContain('Mettre « Budget » à la corbeille ?');
      fireEvent.keyDown(window, { key: 'Escape' });
      expect(screen.queryByRole('alertdialog')).toBeNull();
      fireEvent.keyDown(window, { key: 'Backspace' });
      fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
      expect(screen.queryByRole('alertdialog')).toBeNull();
      // Choosing another case drops a pending confirmation: Enter then trashes nothing.
      fireEvent.keyDown(window, { key: 'Delete' });
      click('Voyage');
      fireEvent.keyDown(window, { key: 'Enter' });
      expect(mockApi.deleteNote).not.toHaveBeenCalled();
      click('Budget', true);
      fireEvent.keyDown(window, { key: 'Delete' });
      expect(screen.getByRole('alertdialog').textContent).toContain('Mettre 2 cases et 2 sous-cases à la corbeille ?');
      fireEvent.click(screen.getByRole('button', { name: 'Mettre à la corbeille' }));
      await waitFor(() => expect(mockApi.deleteNote).toHaveBeenCalledTimes(2));
      expect(mockApi.deleteNote).toHaveBeenCalledWith('voyage');
      expect(mockApi.deleteNote).toHaveBeenCalledWith('budget');
    });

    it('Enter on Keep keeps the cases, and leaving the level drops the confirmation', () => {
      render(<Harness />);
      click('Budget');
      fireEvent.keyDown(window, { key: 'Delete' });
      const keep = screen.getByRole('button', { name: 'Annuler' });
      keep.focus();
      fireEvent.keyDown(keep, { key: 'Enter' });
      expect(mockApi.deleteNote).not.toHaveBeenCalled();
      fireEvent.doubleClick(cardOf('Voyage'));
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });

    it('the offer to undo waits while a selection hides it', async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        render(<Harness />);
        click('Budget');
        fireEvent.keyDown(window, { key: 'Delete' });
        fireEvent.keyDown(window, { key: 'Enter' });
        expect(await screen.findByRole('button', { name: 'Annuler' })).toBeTruthy();
        click('Voyage');
        expect(screen.queryByRole('button', { name: 'Annuler' })).toBeNull();
        act(() => { vi.advanceTimersByTime(15_000); });
        fireEvent.keyDown(window, { key: 'Escape' });
        expect(screen.getByRole('button', { name: 'Annuler' })).toBeTruthy();
        act(() => { vi.advanceTimersByTime(11_000); });
        expect(screen.queryByRole('button', { name: 'Annuler' })).toBeNull();
      } finally {
        vi.useRealTimers();
      }
    });

    it('Delete typed in a field stays in the field', () => {
      render(<Harness />);
      for (const title of ['Voyage', 'Budget']) {
        fireEvent.pointerDown(cardOf(title), { button: 0, clientX: 5, clientY: 5, shiftKey: true });
        act(() => { window.dispatchEvent(new MouseEvent('pointerup', { clientX: 5, clientY: 5 })); });
      }
      const name = screen.getByRole('textbox', { name: 'Nom du groupe' });
      fireEvent.keyDown(name, { key: 'Backspace' });
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
  });

  it('grouping cases never lets the cases after them slide into their room', async () => {
    const idea = note('idea', 'Partir en juin', null, '', '2026-10-01T10:02:00.000Z');
    const last = note('last', 'Dernière', null, '', '2026-10-01T10:03:00.000Z');
    mockApi.createNote.mockResolvedValue('group');
    render(<CaseBoard rootCases={[voyage, budget, idea, last]} allNotes={[...all, idea, last]} projectId="p1" creationProjectId="p1" path={[]} onPathChange={vi.fn()} onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} />);
    for (const title of ['Voyage', 'Budget']) {
      fireEvent.pointerDown(cardOf(title), { button: 0, clientX: 5, clientY: 5, shiftKey: true });
      act(() => { window.dispatchEvent(new MouseEvent('pointerup', { clientX: 5, clientY: 5 })); });
    }
    fireEvent.change(screen.getByRole('textbox', { name: 'Nom du groupe' }), { target: { value: 'Préparatifs' } });
    fireEvent.click(screen.getByRole('button', { name: 'Regrouper' }));
    // The group takes the first slot; the third and fourth cases stay where they were, pinned
    // before the first two leave.
    await waitFor(() => expect(mockApi.setCasePositions).toHaveBeenCalledWith([
      { id: 'idea', x: 568, y: 24 },
      { id: 'last', x: 840, y: 24 },
    ]));
    expect(mockApi.setCasePositions.mock.invocationCallOrder[0]).toBeLessThan(mockApi.moveCase.mock.invocationCallOrder[0]);
  });

  describe('a group with many sub-cases', () => {
    const at = (minute: number) => `2026-10-01T11:${String(minute).padStart(2, '0')}:00.000Z`;
    const big = note('big', 'Gros groupe', null, '', at(0));
    const kids = (count: number) => Array.from({ length: count }, (_, index) => note(`k${index}`, `Sous-case ${index + 1}`, 'big', '', at(10 + index)));
    const loose = ['a', 'b', 'c', 'd', 'e'].map((id, index) => note(id, `Case ${id}`, null, '', at(1 + index)));
    const board = (children: Note[]) => (
      <CaseBoard rootCases={[big, ...loose]} allNotes={[big, ...loose, ...children]} projectId="p1" creationProjectId="p1" path={[]}
        onPathChange={vi.fn()} onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={vi.fn()} />
    );

    it('lists its first ten sub-cases, two rows high', () => {
      render(board(kids(12)));
      const group = cardOf('Gros groupe');
      expect(within(group).getAllByRole('listitem')).toHaveLength(10);
      expect(within(group).getByRole('button', { name: '12 sous-cases' })).toBeTruthy();
      // Two rows is a ceiling: the box ends right after its count.
      expect(group.style.maxHeight).toBe('424px');
      expect(group.style.height).toBe('');
    });

    it('stays a card high when a case placed by hand sits right below, and unfolds over it when pointed at', async () => {
      mockApi.getCasePositions.mockResolvedValue([{ id: 'big', x: 24, y: 24 }, { id: 'd', x: 24, y: 244 }]);
      render(board(kids(7)));
      await waitFor(() => expect(cardOf('Case d').style.top).toBe('244px'));
      const group = cardOf('Gros groupe');
      expect(group.style.maxHeight).toBe('204px');
      expect(group.className).toContain('is-folded');
      expect(group.style.getPropertyValue('--group-unfolded')).toBe('424px');
      expect(within(group).getAllByRole('listitem')).toHaveLength(7);
      expect(cardOf('Case d').style.top, 'the case below stays put').toBe('244px');
    });

    it('growing two rows high never lets the cases around it slide', async () => {
      render(board(kids(4)));
      await waitFor(() => expect(mockApi.getCasePositions).toHaveBeenCalled());
      const target = cardOf('Gros groupe');
      Object.defineProperty(document, 'elementsFromPoint', { value: vi.fn(() => [target]), configurable: true });
      try {
        fireEvent.pointerDown(cardOf('Case a'), { button: 0, clientX: 10, clientY: 10 });
        act(() => {
          window.dispatchEvent(new MouseEvent('pointermove', { clientX: 60, clientY: 10 }));
          window.dispatchEvent(new MouseEvent('pointerup', { clientX: 60, clientY: 10 }));
        });
        // Case a leaves and the group grows to its fifth sub-case: b, c and d would slide.
        await waitFor(() => expect(mockApi.setCasePositions).toHaveBeenCalledWith([
          { id: 'b', x: 568, y: 24 },
          { id: 'c', x: 840, y: 24 },
          { id: 'd', x: 24, y: 244 },
        ]));
        expect(mockApi.setCasePositions.mock.invocationCallOrder[0]).toBeLessThan(mockApi.moveCase.mock.invocationCallOrder[0]);
      } finally {
        Object.defineProperty(document, 'elementsFromPoint', { value: undefined, configurable: true });
      }
    });
  });

  it('puts a case dropped on another one inside it, like a file into a folder, with a way to undo', async () => {
    render(<Harness />);
    const target = cardOf('Voyage');
    Object.defineProperty(document, 'elementsFromPoint', { value: vi.fn(() => [target]), configurable: true });
    try {
      fireEvent.pointerDown(cardOf('Budget'), { button: 0, clientX: 10, clientY: 10 });
      act(() => { window.dispatchEvent(new MouseEvent('pointermove', { clientX: 200, clientY: 40 })); });
      expect(target.className).toContain('is-drop-target');
      act(() => { window.dispatchEvent(new MouseEvent('pointerup', { clientX: 200, clientY: 40 })); });
      await waitFor(() => expect(mockApi.moveCase).toHaveBeenCalledWith('budget', 'voyage'));
      expect((await screen.findByRole('status')).textContent).toContain('Case rangée dans « Voyage »');
      fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
      await waitFor(() => expect(mockApi.moveCase).toHaveBeenCalledWith('budget', null));
    } finally {
      Object.defineProperty(document, 'elementsFromPoint', { value: undefined, configurable: true });
    }
  });

  it('refuses a drop that would put a case into itself, its own sub-cases or another project', () => {
    const byId = new Map(all.map(item => [item.id, item]));
    expect(canMoveInto(['budget'], 'voyage', byId)).toBe(true);
    expect(canMoveInto(['voyage'], 'voyage', byId)).toBe(false);
    expect(canMoveInto(['voyage'], 'hotel-x', byId)).toBe(false);
    expect(canMoveInto(['hotels'], 'voyage', byId), 'already there').toBe(false);
    expect(canMoveInto(['hotels'], null, byId)).toBe(true);
    byId.set('elsewhere', { ...note('elsewhere', 'Ailleurs'), project_id: 'p2' });
    expect(canMoveInto(['budget'], 'elsewhere', byId)).toBe(false);
  });

  it('drops from the path a case that left the scope', () => {
    render(<Harness initialPath={['gone', 'voyage']} />);
    expect(screen.getByRole('button', { name: 'Ouvrir Hôtels' })).toBeTruthy();
  });
});

describe('CaseBoard rich contents', () => {
  const sketch = { ...note('plan', 'Plan du quartier', 'voyage', '{"type":"excalidraw","version":2,"elements":[{"id":"r","type":"rectangle"}],"appState":{},"files":{}}'), content_kind: 'sketch' as const };
  const site = { ...note('site', 'Office du tourisme', 'voyage', '<https://www.visitasevilla.es/fr>\n'), content_kind: 'link' as const };
  const rich = [voyage, budget, hotels, hotelX, sketch, site];
  function RichHarness({ initialPath = ['voyage'], onOpenNote = vi.fn() }: { initialPath?: string[]; onOpenNote?: (note: Note) => void }) {
    const [path, setPath] = useState<string[]>(initialPath);
    return <CaseBoard rootCases={[voyage, budget]} allNotes={rich} projectId="p1" creationProjectId="p1" path={path} onPathChange={setPath}
      onOpenNote={onOpenNote} onCreateRootCase={vi.fn()} onChanged={vi.fn()} />;
  }
  const newSubCase = () => {
    fireEvent.click(screen.getByRole('button', { name: 'Nouvelle sous-case' }));
    return screen.getByLabelText('Titre de la sous-case').closest('form') as HTMLFormElement;
  };

  it('shows a sketch as its picture and a link as its site, which opens in the browser', async () => {
    mockApi.openLink.mockResolvedValue(undefined);
    render(<RichHarness />);
    expect(within(cardOf('Plan du quartier')).getByRole('img', { name: 'Dessin Plan du quartier' })).toBeTruthy();
    // The scene of the drawing is never shown as text.
    expect(cardOf('Plan du quartier').textContent).not.toContain('excalidraw');
    fireEvent.click(within(cardOf('Office du tourisme')).getByRole('button', { name: 'visitasevilla.es' }));
    await waitFor(() => expect(mockApi.openLink).toHaveBeenCalledWith('https://www.visitasevilla.es/fr'));
  });

  it('creates a sketch inside a case and opens it to draw', async () => {
    mockApi.createSubCase.mockResolvedValue('new-sketch');
    const created = { ...note('new-sketch', 'Dessin', 'voyage'), content_kind: 'sketch' as const };
    mockApi.getNote.mockResolvedValue(created);
    const onOpenNote = vi.fn();
    render(<RichHarness onOpenNote={onOpenNote} />);
    const form = newSubCase();
    fireEvent.click(within(form).getByRole('radio', { name: 'Dessin' }));
    fireEvent.click(within(form).getByRole('button', { name: 'Créer' }));
    await waitFor(() => expect(mockApi.createSubCase).toHaveBeenCalledWith('voyage', 'Dessin', 'note', 'sketch', undefined));
    await waitFor(() => expect(onOpenNote).toHaveBeenCalledWith(created));
  });

  it('creates a link only from a web address, named after its site when untitled', async () => {
    mockApi.createSubCase.mockResolvedValue('new-link');
    render(<RichHarness />);
    const form = newSubCase();
    fireEvent.click(within(form).getByRole('radio', { name: 'Lien' }));
    const address = within(form).getByLabelText('Adresse du lien');
    fireEvent.change(address, { target: { value: 'javascript:alert(1)' } });
    fireEvent.click(within(form).getByRole('button', { name: 'Créer' }));
    expect((await screen.findByRole('alert')).textContent).toContain('http://');
    expect(mockApi.createSubCase).not.toHaveBeenCalled();
    fireEvent.change(address, { target: { value: 'https://www.example.com/a' } });
    fireEvent.click(within(form).getByRole('button', { name: 'Créer' }));
    await waitFor(() => expect(mockApi.createSubCase).toHaveBeenCalledWith('voyage', 'example.com', 'note', 'link', '<https://www.example.com/a>'));
  });

  it('keeps an image draft open while the file is being chosen, and saves the image first', async () => {
    mockApi.createSubCase.mockResolvedValue('new-image');
    mockApi.saveImageToDisk.mockResolvedValue('humcase-image://abc.png');
    render(<RichHarness />);
    const form = newSubCase();
    fireEvent.click(within(form).getByRole('radio', { name: 'Image' }));
    // The file dialog takes the focus away from the draft: it stays.
    fireEvent.blur(screen.getByLabelText('Titre de la sous-case'), { relatedTarget: null });
    const picker = form.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], 'facade.png', { type: 'image/png' });
    Object.defineProperty(file, 'arrayBuffer', { value: async () => new Uint8Array([0x89, 0x50, 0x4e, 0x47]).buffer });
    fireEvent.change(picker, { target: { files: [file] } });
    fireEvent.click(within(form).getByRole('button', { name: 'Créer' }));
    await waitFor(() => expect(mockApi.createSubCase).toHaveBeenCalledWith('voyage', 'facade', 'note', 'image', '![facade](humcase-image://abc.png)\n'));
  });

  it('turns a pasted web address into a link case of the open case, never at the top level', async () => {
    mockApi.createSubCase.mockResolvedValue('pasted');
    const paste = (text: string) => {
      const event = new Event('paste', { bubbles: true, cancelable: true }) as ClipboardEvent;
      Object.defineProperty(event, 'clipboardData', { value: { files: [], getData: () => text } });
      act(() => { document.body.dispatchEvent(event); });
    };
    const { unmount } = render(<RichHarness initialPath={[]} />);
    paste('https://example.org/page');
    expect((await screen.findByRole('alert')).textContent).toContain('Ouvrez une case');
    expect(mockApi.createSubCase).not.toHaveBeenCalled();
    unmount();
    render(<RichHarness />);
    paste('just words');
    paste('https://example.org/page');
    await waitFor(() => expect(mockApi.createSubCase).toHaveBeenCalledTimes(1));
    expect(mockApi.createSubCase).toHaveBeenCalledWith('voyage', 'example.org', 'note', 'link', '<https://example.org/page>');
  });

  it('imports a JSON Canvas file inside the open case and says how many cases came in', async () => {
    mockApi.importJsonCanvas.mockResolvedValue({ cases: ['a', 'b'], ignored_edges: 0 });
    const onChanged = vi.fn();
    render(<CaseBoard rootCases={[voyage, budget]} allNotes={rich} projectId="p1" creationProjectId="p1" path={['voyage']} onPathChange={vi.fn()}
      onOpenNote={vi.fn()} onCreateRootCase={vi.fn()} onChanged={onChanged} />);
    const input = screen.getByLabelText(/Ajouter à ce plan les cases d’un fichier JSON Canvas/) as HTMLInputElement;
    const file = new File(['{"nodes":[]}'], 'idees.canvas', { type: 'application/json' });
    Object.defineProperty(file, 'text', { value: async () => '{"nodes":[]}' });
    fireEvent.change(input, { target: { files: [file] } });
    await waitFor(() => expect(mockApi.importJsonCanvas).toHaveBeenCalledWith('{"nodes":[]}', undefined, 'voyage'));
    expect((await screen.findByRole('status', { name: '' })).textContent).toBe('2 cases importées.');
    expect(onChanged).toHaveBeenCalled();
  });
});

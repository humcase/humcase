import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type DragEvent as ReactDragEvent, type FocusEvent, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowCounterClockwise, ArrowSquareOut, ArrowsOut, CaretRight, DownloadSimple, UploadSimple, FileText, FolderPlus, Image as ImageIcon, LinkSimple, MagnifyingGlass, Minus, Paperclip, PencilSimple, PencilSimpleLine, Plus, SquaresFour, TextAlignLeft, Trash, X } from '@phosphor-icons/react';
import { NOTE_TYPES, type CasePosition, type ContentKind, type Note, type NoteStatus, type NoteType } from '../../types';
import { api } from '../../services/tauriApi';
import { BOARD_CARD_WIDTH, SLOT_HEIGHT, boardBounds, columnsFor, gridColumns, layoutCases, nextFreeSlot, spanSize, type Point, type Span } from '../../utils/caseLayout';
import { GROUP_ROWS_SMALL, GROUP_ROWS_TALL, canMoveInto, caseMatches, caseSpan, natureCounts, subCaseCount } from '../../utils/caseBoard';
import { TYPE_ICONS, TYPE_LABELS, notePreview } from '../../utils/notePresentation';
import { firstLocalNoteImage } from '../../utils/noteImage';
import { Button } from '../ui/Button';
import { Hint } from '../ui/Hint';
import { Card } from '../ui/Card';
import { StatusBadge, TypeBadge } from '../ui/Badge';
import { NoteThumbnail } from './WorkspaceCards';
import CaseDocument from './CaseDocument';
import SketchThumbnail from '../sketch/SketchThumbnail';
import { imageContent, linkContent, linkHost, linkOf, webAddress } from '../../utils/caseContent';
import './case-board.css';

/** Below this distance a press is a click, above it a drag. */
const DRAG_THRESHOLD = 4;
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 1.5;
/** Below this zoom a group shows as one tile and a card as its nature and title only. */
const FAR_ZOOM = 0.6;
/** How long the board offers to undo a move or a grouping. */
const UNDO_DELAY = 10_000;
/** Pixels a pan may move the board before the cards are rendered for the new place. */
const RENDER_DRIFT = 400;

const KIND_ICONS: Record<Exclude<ContentKind, 'text'>, typeof FileText> = {
  image: ImageIcon,
  link: LinkSimple,
  file: Paperclip,
  sketch: PencilSimpleLine,
  research: MagnifyingGlass,
};

type View = { x: number; y: number; zoom: number };
const NO_CASES: Note[] = [];
const previews = new WeakMap<Note, { text: string; image: ReturnType<typeof firstLocalNoteImage> }>();
/** Preview text and first image of a case, computed once per version of the note. */
function cardContent(note: Note) {
  let content = previews.get(note);
  if (!content) {
    content = { text: notePreview(note), image: firstLocalNoteImage(note.content) };
    previews.set(note, content);
  }
  return content;
}
/** What a new sub-case holds: text, a drawing, an image or a link. */
type DraftKind = 'text' | 'sketch' | 'image' | 'link';
const DRAFT_KINDS: { kind: DraftKind; icon: typeof FileText }[] = [
  { kind: 'text', icon: TextAlignLeft },
  { kind: 'sketch', icon: PencilSimpleLine },
  { kind: 'image', icon: ImageIcon },
  { kind: 'link', icon: LinkSimple },
];
type Draft = { point: Point; nature: NoteType; kind: DraftKind; url: string; file: File | null };
type NewCase = { title: string; nature: NoteType; kind: DraftKind; content?: string; point: Point };

/** PNG and JPEG images among dropped or pasted files: the formats an image case keeps. */
const imagesOf = (files: FileList | null | undefined) => Array.from(files ?? []).filter(file => file.type === 'image/png' || file.type === 'image/jpeg');
const fileStem = (name: string) => name.replace(/\.[^.]+$/, '').trim();
type DropTarget = { key: string; parentId: string | null; valid: boolean };
type Dragging = { id: string; ids: string[]; point: Point; onBoard: boolean };
type Undo = { message: string; revert: () => Promise<void> };

const clampZoom = (zoom: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));

interface CaseBoardProps {
  /** Cases of the top-level board, filtered by the page only for archives: search and status dim, they never remove. */
  rootCases: Note[];
  /** Every loaded case of the scope, sub-cases included. */
  allNotes: Note[];
  /** Project shown, or null for every project of the space. */
  projectId: string | null;
  /** Project where a top-level case can be created right on the board; null otherwise. */
  creationProjectId: string | null;
  /** Cases dived into, from the top level down; empty on the top-level board. */
  path: string[];
  onPathChange: (path: string[]) => void;
  onOpenNote: (note: Note) => void;
  /** Creates a top-level case through the usual flow, when the board cannot (no single project). */
  onCreateRootCase: () => void;
  /** Rereads the notes after a write. */
  onChanged: () => void;
  /** Search and filters of the page, shared with the other views: the board only says what they find. */
  query?: string;
  status?: NoteStatus | null;
  nature?: NoteType | null;
  /** Only these cases stand out ("Today"); null: no such limit. */
  focusIds?: ReadonlySet<string> | null;
  /** Cases the database found for the search (the other views' answer); null: the board matches the words itself. */
  searchIds?: ReadonlySet<string> | null;
  /**
   * The page's "New case" button creates on the board (a draft in the next free place, inside the
   * case dived into); the board hands it the action and its label, or null when it leaves.
   */
  onRegisterCreate?: (entry: { create: () => void; label: string } | null) => void;
}

/**
 * Board of cases: cases placed freely and grouped, zoom and overview, search, drag a case into
 * another one like a file into a folder, dive into a case, document overview beside.
 */
export default function CaseBoard({ rootCases, allNotes, projectId, creationProjectId, path, onPathChange, onOpenNote, onCreateRootCase, onChanged, onRegisterCreate, ...shared }: CaseBoardProps) {
  const { t } = useTranslation();
  const [stored, setStored] = useState<Map<string, Point>>(new Map());
  const [dragging, setDragging] = useState<Dragging | null>(null);
  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);
  // Panning and zooming move the board directly; React renders the cards again only when the
  // gesture ends (or when the zoom crosses the far threshold), so 200 cases stay fluid.
  const [view, setViewState] = useState<View>({ x: 0, y: 0, zoom: 1 });
  const viewRef = useRef(view);
  const sectionRef = useRef<HTMLElement>(null);
  const layerRef = useRef<HTMLDivElement>(null);
  const zoomLabelRef = useRef<HTMLButtonElement>(null);
  const ghostRef = useRef<HTMLDivElement>(null);
  const commitTimer = useRef<number | undefined>(undefined);
  // View React last rendered: a pan that moves far from it renders again on the way, so the
  // cards it brings into sight are there (only the cards near the screen are rendered).
  const renderedViewRef = useRef(view);
  const setView = useCallback((next: View | ((previous: View) => View), commit: boolean | 'later' = true) => {
    const value = typeof next === 'function' ? next(viewRef.current) : next;
    viewRef.current = value;
    const layer = layerRef.current, surface = surfaceRef.current;
    if (layer) layer.style.transform = `translate(${value.x}px, ${value.y}px) scale(${value.zoom})`;
    if (surface) {
      surface.style.backgroundPosition = `${value.x}px ${value.y}px`;
      surface.style.backgroundSize = `${22 * value.zoom}px ${22 * value.zoom}px`;
    }
    sectionRef.current?.style.setProperty('--board-zoom', String(value.zoom));
    if (zoomLabelRef.current) zoomLabelRef.current.textContent = `${Math.round(value.zoom * 100)} %`;
    window.clearTimeout(commitTimer.current);
    const rendered = renderedViewRef.current;
    const drifted = Math.abs(value.x - rendered.x) + Math.abs(value.y - rendered.y) > RENDER_DRIFT;
    if (commit === true || (commit === false && drifted)) {
      renderedViewRef.current = value;
      setViewState(value);
    } else if (commit === 'later') {
      commitTimer.current = window.setTimeout(() => { renderedViewRef.current = viewRef.current; setViewState(viewRef.current); }, 150);
    }
  }, []);
  useEffect(() => () => window.clearTimeout(commitTimer.current), []);
  const [showDocument, setShowDocument] = useState(false);
  // New case being named, at its place on the board.
  const [draft, setDraft] = useState<Draft | null>(null);
  const [title, setTitle] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const importRef = useRef<HTMLInputElement>(null);
  const [pending, setPending] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [groupName, setGroupName] = useState('');
  const [groupError, setGroupError] = useState<string | null>(null);
  const query = shared.query ?? '';
  const nature = shared.nature ?? null;
  const status = shared.status ?? null;
  const [undo, setUndo] = useState<Undo | null>(null);
  // Cases waiting for the confirmation of their move to the trash (bound to their ids).
  const [confirmTrash, setConfirmTrash] = useState<string[] | null>(null);
  const draggedRef = useRef(false);
  // The notes as last read, for an undo that runs after other changes.
  const latestNotes = useRef(allNotes);
  useEffect(() => { latestNotes.current = allNotes; }, [allNotes]);
  // Cases never placed fill the visible width of the board (4 columns until it is measured).
  const surfaceRef = useRef<HTMLDivElement>(null);
  const [visibleColumns, setColumns] = useState(4);
  const [measured, setMeasured] = useState<{ width: number; height: number } | null>(null);
  useEffect(() => {
    const surface = surfaceRef.current;
    if (!surface || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(([entry]) => {
      setColumns(columnsFor(entry.contentRect.width));
      setMeasured({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(surface);
    return () => observer.disconnect();
  }, []);
  const epochRef = useRef(0);

  // A position written here makes any read still in flight stale: it would put the case back.
  const storeLocally = (update: (previous: Map<string, Point>) => Map<string, Point>) => {
    epochRef.current++;
    setStored(update);
  };
  // A drag or a pan in progress ends with the board (unmount) or when the system cancels it.
  const gestureEndRef = useRef<(() => void) | null>(null);
  useEffect(() => () => gestureEndRef.current?.(), []);

  // Positions follow the notes list: an agent or another window can place or add a case.
  const loadPositions = useCallback(async () => {
    const epoch = ++epochRef.current;
    try {
      const positions: CasePosition[] = await api.getCasePositions(projectId ?? undefined) ?? [];
      if (epoch === epochRef.current) setStored(new Map(positions.map(p => [p.id, { x: p.x, y: p.y }])));
    } catch (cause) {
      if (epoch === epochRef.current) setError(t('workspace.board.loadError', { error: String(cause) }));
    }
  }, [projectId, t, setError]);
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => { if (!cancelled) void loadPositions(); });
    return () => { cancelled = true; };
  }, [allNotes, loadPositions]);

  // The offer to undo goes away by itself, counting only while it shows: a selection or a
  // confirmation takes its place in the bar, and the offer waits for them.
  const undoShown = !!undo && selected.size === 0 && !confirmTrash;
  useEffect(() => {
    if (!undoShown) return;
    const timer = window.setTimeout(() => setUndo(null), UNDO_DELAY);
    return () => window.clearTimeout(timer);
  }, [undo, undoShown]);

  // The path keeps only cases still live in the scope (a case trashed or moved elsewhere).
  const byId = useMemo(() => new Map(allNotes.map(note => [note.id, note])), [allNotes]);
  const validPath = path.filter(id => byId.has(id));
  const current = validPath.length ? byId.get(validPath[validPath.length - 1]) ?? null : null;
  // Sub-cases of every case, read once per list of notes.
  const childrenMap = useMemo(() => {
    const map = new Map<string, Note[]>();
    for (const note of allNotes) {
      if (!note.parent_id || note.deleted_at) continue;
      const list = map.get(note.parent_id);
      if (list) list.push(note); else map.set(note.parent_id, [note]);
    }
    for (const list of map.values()) list.sort((a, b) => a.created_at.localeCompare(b.created_at) || a.id.localeCompare(b.id));
    return map;
  }, [allNotes]);
  const cases = useMemo(() => current ? childrenMap.get(current.id) ?? NO_CASES : rootCases, [current, childrenMap, rootCases]);
  const columns = gridColumns(cases.length, visibleColumns);
  const childrenOf = (id: string) => childrenMap.get(id) ?? NO_CASES;
  const parentOf = (id: string) => byId.get(id)?.parent_id ?? null;
  /** Cases of the canvas of `parent` (null: the top level). */
  const boardOf = (parent: string | null) => parent === (current?.id ?? null) ? cases : parent ? childrenOf(parent) : rootCases;
  const spanOf = (note: Note) => caseSpan(childrenOf(note.id).length);
  const layout = useMemo(
    () => layoutCases(cases, stored, columns, note => caseSpan(childrenMap.get(note.id)?.length ?? 0)),
    [cases, stored, columns, childrenMap],
  );
  const placed = () => cases.map(note => ({ point: layout.get(note.id) ?? { x: 0, y: 0 }, span: spanOf(note) }));
  // Inside a case or in a single project, a new case is named right on the board.
  const createsInPlace = Boolean(current || creationProjectId);
  const far = view.zoom < FAR_ZOOM;
  const focusIds = shared.focusIds ?? null;
  const filtering = Boolean(query.trim() || nature || status || focusIds);
  // The search of the page is answered by the database, like in the other views: same words, same cases.
  const searchIds = query.trim() ? shared.searchIds ?? null : null;
  const matches = (note: Note) => (searchIds ? searchIds.has(note.id) && caseMatches(note, '', nature, status) : caseMatches(note, query, nature, status)) && (!focusIds || focusIds.has(note.id));
  const itemMatches = (note: Note) => matches(note) || childrenOf(note.id).some(matches);
  const hits = filtering ? cases.filter(itemMatches) : [];

  // Leaving or entering a case: back to the origin of the board and nothing in progress.
  const goTo = (next: string[]) => {
    onPathChange(next);
    setView({ x: 0, y: 0, zoom: 1 });
    setDraft(null);
    setTitle('');
    setError(null);
    setSelected(new Set());
    setConfirmTrash(null);
  };

  const surfaceSize = () => {
    const rect = surfaceRef.current?.getBoundingClientRect();
    return { width: rect?.width || 800, height: rect?.height || 600 };
  };
  /** Board coordinates of a point of the screen. */
  const toBoard = (clientX: number, clientY: number): Point => {
    const rect = surfaceRef.current?.getBoundingClientRect();
    const { x, y, zoom } = viewRef.current;
    return { x: (clientX - (rect?.left ?? 0) - x) / zoom, y: (clientY - (rect?.top ?? 0) - y) / zoom };
  };

  // Zoom keeps the point under the cursor (or the center of the board) where it is.
  const zoomAt = useCallback((factor: number, anchor?: Point, commit: boolean | 'later' = true) => {
    const rect = surfaceRef.current?.getBoundingClientRect();
    const focus = anchor ?? { x: (rect?.width ?? 800) / 2, y: (rect?.height ?? 600) / 2 };
    setView(previous => {
      const zoom = clampZoom(previous.zoom * factor);
      const ratio = zoom / previous.zoom;
      return { zoom, x: focus.x - (focus.x - previous.x) * ratio, y: focus.y - (focus.y - previous.y) * ratio };
    }, commit);
  }, [setView]);
  useEffect(() => {
    const surface = surfaceRef.current;
    if (!surface) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = surface.getBoundingClientRect();
      zoomAt(event.deltaY < 0 ? 1.1 : 1 / 1.1, { x: event.clientX - rect.left, y: event.clientY - rect.top }, 'later');
    };
    surface.addEventListener('wheel', onWheel, { passive: false });
    return () => surface.removeEventListener('wheel', onWheel);
  }, [zoomAt]);

  /** The whole board at once: every case fits in the surface. */
  const showAll = () => {
    const bounds = boardBounds(placed());
    if (!bounds) { setView({ x: 0, y: 0, zoom: 1 }); return; }
    const { width, height } = surfaceSize();
    const zoom = clampZoom(Math.min(1, (width - 48) / bounds.width, (height - 48) / bounds.height));
    // Centered when it all fits; otherwise the board starts at its beginning, top left.
    const room = width - bounds.width * zoom;
    setView({ zoom, x: (room >= 0 ? room / 2 : 24) - bounds.x * zoom, y: 24 - bounds.y * zoom });
  };
  /** Brings a case of the board to the middle of the surface. */
  const centerOn = (note: Note) => {
    const point = layout.get(note.id);
    if (!point) return;
    const { width: w, height: h } = spanSize(spanOf(note));
    const { width, height } = surfaceSize();
    setView(previous => {
      const zoom = Math.max(previous.zoom, FAR_ZOOM);
      return { zoom, x: width / 2 - (point.x + w / 2) * zoom, y: height / 2 - (point.y + h / 2) * zoom };
    });
  };

  const startDraft = (point: Point) => {
    if (!createsInPlace) { onCreateRootCase(); return; }
    // A nature chosen in the filters is the one the new case takes.
    setDraft({ point, nature: nature ?? 'note', kind: 'text', url: '', file: null });
    setTitle('');
    setError(null);
  };
  const startDraftInNextSlot = () => {
    const slot = nextFreeSlot(placed(), columns);
    startDraft(slot);
    // The new case is where the eye is.
    const { width, height } = surfaceSize();
    setView(previous => {
      const zoom = Math.max(previous.zoom, FAR_ZOOM);
      const visible = slot.x * zoom + previous.x >= 0 && (slot.x + BOARD_CARD_WIDTH) * zoom + previous.x <= width
        && slot.y * zoom + previous.y >= 0 && (slot.y + 160) * zoom + previous.y <= height;
      return visible && zoom === previous.zoom ? previous : { zoom, x: width / 2 - (slot.x + BOARD_CARD_WIDTH / 2) * zoom, y: height / 3 - slot.y * zoom };
    });
  };

  const cancelDraft = () => { setDraft(null); setTitle(''); };
  // Leaving the draft empty cancels it; moving to one of its nature buttons does not.
  const leaveDraft = (event: FocusEvent<HTMLFormElement>) => {
    if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
    // An image being chosen (the file dialog takes the focus) or a link being typed stays.
    if (!title.trim() && !pending && draft?.kind === 'text') cancelDraft();
  };

  /** Creates a case on this board at `point`, the open case growing by one on its own board. */
  const placeNewCase = async ({ title: name, nature: type, kind, content, point }: NewCase): Promise<string> => {
    if (current) await pinBoards([], new Map([[current.id, 1]]));
    const id = current
      ? await api.createSubCase(current.id, name, type, kind === 'text' ? undefined : kind, content)
      : await api.createNote({ project_id: creationProjectId as string, title: name, note_type: type });
    const at = { x: Math.round(point.x), y: Math.round(point.y) };
    storeLocally(previous => new Map(previous).set(id, at));
    await api.setCasePosition(id, at.x, at.y);
    onChanged();
    return id;
  };
  const saveImage = async (file: File) => api.saveImageToDisk(new Uint8Array(await file.arrayBuffer()));

  /**
   * Images and a link pasted or dropped on the board become cases of the open case: images
   * first at `at` (or in the next free places), then the link.
   */
  const addContents = async (files: File[], url: string | null, at: Point | null, pasted: boolean) => {
    if (!current) { setError(t('workspace.board.richNeedsCase')); return; }
    if (pending) return;
    setPending(true);
    setError(null);
    try {
      const count = files.length + (url ? 1 : 0);
      const places = placesIn(current.id, Array.from({ length: count }, (_, index) => `new:${index}`), at ?? undefined);
      for (const [index, file] of files.entries()) {
        const name = (!pasted && fileStem(file.name)) || t('workspace.board.pastedImage');
        await placeNewCase({ title: name, nature: 'note', kind: 'image', content: imageContent(name, await saveImage(file)), point: places[index] });
      }
      if (url) await placeNewCase({ title: linkHost(url), nature: 'note', kind: 'link', content: linkContent(url), point: places[count - 1] });
    } catch (cause) {
      setError(t('workspace.board.createError', { error: String(cause) }));
      onChanged();
    } finally {
      setPending(false);
    }
  };

  const createCase = async () => {
    if (!draft || pending) return;
    // Rich contents live inside a case; the top level holds text cases.
    const kind = current ? draft.kind : 'text';
    let cleanTitle = title.trim();
    if (kind === 'text' && !cleanTitle) { cancelDraft(); return; }
    const url = kind === 'link' ? webAddress(draft.url) : null;
    if (kind === 'link' && !url) { setError(t('workspace.board.linkInvalid')); return; }
    if (kind === 'image' && !draft.file) { setError(t('workspace.board.imageRequired')); return; }
    setPending(true);
    setError(null);
    try {
      let content: string | undefined;
      if (url) {
        cleanTitle ||= linkHost(url);
        content = linkContent(url);
      } else if (kind === 'image' && draft.file) {
        cleanTitle ||= fileStem(draft.file.name) || t('workspace.board.kind.image');
        content = imageContent(cleanTitle, await saveImage(draft.file));
      } else if (kind === 'sketch') {
        cleanTitle ||= t('workspace.board.kind.sketch');
      }
      const id = await placeNewCase({ title: cleanTitle, nature: draft.nature, kind, content, point: draft.point });
      cancelDraft();
      // A new drawing opens at once, ready to draw.
      if (kind === 'sketch') onOpenNote(await api.getNote(id));
    } catch (cause) {
      setError(t('workspace.board.createError', { error: String(cause) }));
      onChanged();
    } finally {
      setPending(false);
    }
  };

  /** Free places for cases arriving on the board of `parentId` (null: the top level). */
  const placesIn = (parentId: string | null, ids: readonly string[], first?: Point): Point[] => {
    const siblings = boardOf(parentId);
    const siblingColumns = gridColumns(siblings.length, visibleColumns);
    const siblingLayout = siblings === cases ? layout : layoutCases(siblings, stored, siblingColumns, spanOf);
    const taken = siblings.map(note => ({ point: siblingLayout.get(note.id) ?? { x: 0, y: 0 }, span: spanOf(note) }));
    const places: Point[] = [];
    for (const [index, id] of ids.entries()) {
      const note = byId.get(id);
      const span = note ? spanOf(note) : { w: 1, h: 1 };
      const place = index === 0 && first ? first : nextFreeSlot(taken, siblingColumns, span);
      places.push(place);
      taken.push({ point: place, span });
    }
    return places;
  };

  /**
   * A change of tree reshapes canvases in two ways: cases leave one, and groups grow or shrink as
   * their number of sub-cases changes. Every case never placed that would then slide is pinned
   * where it is, in one write, before the change. `moves` are cases going from one parent to
   * another, `grown` other changes of sub-case counts, `arrival` a new case placed on a canvas.
   */
  const pinBoards = async (
    moves: readonly { ids: readonly string[]; from: string | null; to: string | null }[],
    grown: ReadonlyMap<string, number> = new Map(),
    arrival?: { board: string | null; id: string; point: Point; subCases: number },
  ) => {
    const delta = new Map(grown);
    const boards = new Set<string | null>([...grown.keys()].map(parentOf));
    for (const { ids, from, to } of moves) {
      boards.add(from);
      if (from) { delta.set(from, (delta.get(from) ?? 0) - ids.length); boards.add(parentOf(from)); }
      if (to) { delta.set(to, (delta.get(to) ?? 0) + ids.length); boards.add(parentOf(to)); }
    }
    if (arrival) { boards.add(arrival.board); delta.set(arrival.id, arrival.subCases); }
    const spanAfter = (note: Note) => caseSpan(childrenOf(note.id).length + (delta.get(note.id) ?? 0));
    const pins = new Map<string, Point>();
    for (const board of boards) {
      const siblings = boardOf(board);
      const leaving = new Set(moves.filter(move => move.from === board).flatMap(move => move.ids));
      const arriving = arrival?.board === board ? arrival : undefined;
      const remaining = siblings.filter(note => !leaving.has(note.id));
      const after = layoutCases(
        arriving ? [...remaining, { id: arriving.id } as Note] : remaining,
        arriving ? new Map([...stored, [arriving.id, arriving.point]]) : stored,
        gridColumns(remaining.length + (arriving ? 1 : 0), visibleColumns),
        spanAfter,
      );
      const now = layoutCases(siblings, stored, gridColumns(siblings.length, visibleColumns), spanOf);
      for (const note of remaining) {
        const before = now.get(note.id), then = after.get(note.id);
        if (!stored.has(note.id) && before && then && (before.x !== then.x || before.y !== then.y)) pins.set(note.id, before);
      }
    }
    if (pins.size === 0) return;
    await api.setCasePositions([...pins].map(([id, point]) => ({ id, ...point })));
    storeLocally(previous => new Map([...previous, ...pins]));
  };

  /** Puts cases inside another case (or at a level of the path), with an offer to undo. */
  const moveCases = async (ids: string[], parentId: string | null, first?: Point) => {
    const before = ids.map(id => ({ id, parent: byId.get(id)?.parent_id ?? null, point: stored.get(id) ?? null }));
    const places = placesIn(parentId, ids, first);
    setSelected(new Set());
    setError(null);
    try {
      await pinBoards([{ ids, from: parentOf(ids[0]), to: parentId }]);
      for (const [index, id] of ids.entries()) {
        await api.moveCase(id, parentId);
        const place = { x: Math.round(places[index].x), y: Math.round(places[index].y) };
        storeLocally(previous => new Map(previous).set(id, place));
        await api.setCasePosition(id, place.x, place.y);
      }
      const target = parentId ? byId.get(parentId)?.title || t('common.untitled') : t('workspace.board.top');
      setUndo({
        message: t('workspace.board.moved', { count: ids.length, target }),
        revert: async () => {
          for (const { id, parent, point } of before) {
            await api.moveCase(id, parent);
            if (point) await api.setCasePosition(id, point.x, point.y);
          }
        },
      });
    } catch (cause) {
      setError(t('workspace.board.moveError', { error: String(cause) }));
      void loadPositions();
    }
    onChanged();
  };

  const runUndo = async () => {
    if (!undo) return;
    const { revert } = undo;
    setUndo(null);
    try {
      await revert();
    } catch (cause) {
      setError(t('workspace.board.undoError', { error: String(cause) }));
    }
    void loadPositions();
    onChanged();
  };

  /** The case or the level of the path under the pointer, if the dragged cases may go there. */
  const findDropTarget = (clientX: number, clientY: number, ids: string[]): DropTarget | null => {
    const elements = document.elementsFromPoint?.(clientX, clientY) ?? [];
    for (const element of elements) {
      const zone = (element as HTMLElement).closest?.('[data-drop-case], [data-drop-level]') as HTMLElement | null;
      if (!zone) continue;
      const caseId = zone.dataset.dropCase;
      if (caseId && ids.includes(caseId)) continue;
      const parentId = caseId ?? (zone.dataset.dropLevel || null);
      const key = caseId ? `case:${caseId}` : `level:${zone.dataset.dropLevel ?? ''}`;
      return { key, parentId, valid: canMoveInto(ids, parentId, byId) };
    }
    return null;
  };

  const toggleSelected = (id: string) => setSelected(previous => {
    const next = new Set(previous);
    if (next.has(id)) next.delete(id); else next.add(id);
    return next;
  });

  /**
   * Press on a case: a click selects it (with Shift or Ctrl), a drag moves it on the board, into
   * another case or to a level of the path. A sub-case shown in a group can be dragged out of it.
   */
  const startCaseDrag = (event: ReactPointerEvent<HTMLElement>, note: Note, onBoard: boolean) => {
    if (event.button !== 0) return;
    event.stopPropagation();
    const pointer = toBoard(event.clientX, event.clientY);
    const origin = onBoard ? layout.get(note.id) ?? { x: 0, y: 0 } : { x: pointer.x - 20, y: pointer.y - 16 };
    const start = { x: event.clientX, y: event.clientY };
    const ids = onBoard && selected.has(note.id) && selected.size > 1 ? [...selected] : [note.id];
    const element = event.currentTarget;
    const additive = event.shiftKey || event.ctrlKey || event.metaKey;
    draggedRef.current = false;
    let last = origin;
    let target: DropTarget | null = null;
    const move = (e: PointerEvent) => {
      const zoom = viewRef.current.zoom;
      const dx = e.clientX - start.x, dy = e.clientY - start.y;
      if (!draggedRef.current && Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
      last = { x: Math.round(origin.x + dx / zoom), y: Math.round(origin.y + dy / zoom) };
      if (!draggedRef.current) {
        element.setPointerCapture?.(e.pointerId);
        draggedRef.current = true;
        setDragging({ id: note.id, ids, point: last, onBoard });
      }
      const moved = onBoard ? element : ghostRef.current;
      if (moved) { moved.style.left = `${last.x}px`; moved.style.top = `${last.y}px`; }
      const next = findDropTarget(e.clientX, e.clientY, ids);
      if (next?.key !== target?.key || next?.valid !== target?.valid) setDropTarget(next);
      target = next;
    };
    const end = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel);
      gestureEndRef.current = null;
      setDragging(null);
      setDropTarget(null);
      // The click that ends a drag is ignored; the next one (mouse or keyboard) is not.
      window.setTimeout(() => { draggedRef.current = false; }, 0);
    };
    // Cancelled by the system (touch, pen, gesture): the case goes back, nothing is written.
    const cancel = () => end();
    const up = () => {
      end();
      if (!draggedRef.current) {
        if (additive && onBoard) toggleSelected(note.id);
        else if (onBoard) setSelected(new Set([note.id]));
        setConfirmTrash(null);
        return;
      }
      if (target?.valid) { void moveCases(ids, target.parentId); return; }
      if (target) return; // A refused target: the case stays where it was.
      if (!onBoard) { void moveCases(ids, current?.id ?? null, last); return; }
      storeLocally(previous => new Map(previous).set(note.id, last));
      api.setCasePosition(note.id, last.x, last.y)
        .then(onChanged)
        .catch(cause => { setError(t('workspace.board.moveError', { error: String(cause) })); void loadPositions(); });
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    gestureEndRef.current = end;
  };

  const startPan = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || event.target !== event.currentTarget) return;
    const start = { x: event.clientX - viewRef.current.x, y: event.clientY - viewRef.current.y };
    let moved = false;
    const move = (e: PointerEvent) => {
      moved = true;
      setView(previous => ({ ...previous, x: e.clientX - start.x, y: e.clientY - start.y }), false);
    };
    const end = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', end);
      gestureEndRef.current = null;
      if (moved) setView(viewRef.current);
    };
    const up = () => {
      end();
      if (!moved) { setSelected(new Set()); setConfirmTrash(null); }
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', end);
    gestureEndRef.current = end;
  };

  // Double-click on an empty spot of the board: a new case right there.
  const createAtPoint = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    const point = toBoard(event.clientX, event.clientY);
    startDraft({ x: point.x - BOARD_CARD_WIDTH / 2, y: point.y - 20 });
  };

  /** Puts the selected cases inside a new case named by the user, where the first one was. */
  const groupSelection = async () => {
    const name = groupName.trim();
    if (!name) { setGroupError(t('workspace.board.groupNameRequired')); return; }
    const ids = cases.filter(note => selected.has(note.id)).map(note => note.id);
    const projects = new Set(ids.map(id => byId.get(id)?.project_id));
    if (!current && projects.size > 1) { setGroupError(t('workspace.board.groupProjects')); return; }
    const points = ids.map(id => layout.get(id) ?? { x: 0, y: 0 });
    const corner = { x: Math.round(Math.min(...points.map(p => p.x))), y: Math.round(Math.min(...points.map(p => p.y))) };
    const before = ids.map(id => ({ id, point: stored.get(id) ?? null }));
    setPending(true);
    setGroupError(null);
    try {
      const groupId = current
        ? await api.createSubCase(current.id, name, 'note')
        : await api.createNote({ project_id: creationProjectId ?? (byId.get(ids[0])?.project_id as string), title: name, note_type: 'note' });
      // The cases leave the board first, so the group takes the place of the first one; the cases
      // around keep their places.
      const board = current?.id ?? null;
      await pinBoards([{ ids, from: board, to: null }], new Map(current ? [[current.id, 1]] : []), { board, id: groupId, point: corner, subCases: ids.length });
      for (const id of ids) await api.moveCase(id, groupId);
      storeLocally(previous => new Map(previous).set(groupId, corner));
      await api.setCasePosition(groupId, corner.x, corner.y);
      setSelected(new Set());
      setGroupName('');
      setUndo({
        message: t('workspace.board.grouped', { count: ids.length, name }),
        revert: async () => {
          for (const { id, point } of before) {
            await api.moveCase(id, current?.id ?? null);
            if (point) await api.setCasePosition(id, point.x, point.y);
          }
          // A case put in the group since then would go to the trash with it: the group stays.
          const grouped = new Set(ids);
          if (!latestNotes.current.some(note => note.parent_id === groupId && !note.deleted_at && !grouped.has(note.id))) await api.deleteNote(groupId);
        },
      });
    } catch (cause) {
      setGroupError(t('workspace.board.groupError', { error: String(cause) }));
    } finally {
      setPending(false);
      void loadPositions();
      onChanged();
    }
  };

  /**
   * Moves cases to the trash with their sub-cases, after confirmation, with an offer to undo. The
   * cases around keep their places.
   */
  const trashCases = async (ids: readonly string[]) => {
    setConfirmTrash(null);
    setSelected(new Set());
    setError(null);
    const gone: string[] = [];
    try {
      await pinBoards([{ ids, from: parentOf(ids[0]), to: null }]);
      for (const id of ids) { await api.deleteNote(id); gone.push(id); }
    } catch (cause) {
      setError(t('workspace.board.trashError', { error: String(cause) }));
    }
    if (gone.length) {
      const first = byId.get(gone[0]);
      setUndo({
        message: gone.length === 1 && first ? t('workspace.board.trashedOne', { title: first.title || t('common.untitled') }) : t('workspace.board.trashed', { count: gone.length }),
        revert: async () => { for (const id of [...gone].reverse()) await api.restoreFromTrash(id); },
      });
    }
    void loadPositions();
    onChanged();
  };

  // An image or a web address pasted on the board (not in a field) becomes a case of the open case.
  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      const section = sectionRef.current;
      // A board out of sight (another page shown) takes nothing.
      if (!section || section.checkVisibility?.() === false || draft) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest?.('input, textarea, select, [contenteditable="true"], .excalidraw')) return;
      const files = imagesOf(event.clipboardData?.files);
      const url = files.length ? null : webAddress(event.clipboardData?.getData('text/plain') ?? '');
      if (!files.length && !url) return;
      event.preventDefault();
      void addContents(files, url, null, true);
    };
    document.addEventListener('paste', onPaste);
    return () => document.removeEventListener('paste', onPaste);
  });

  const carriesContent = (event: ReactDragEvent) => ['Files', 'text/uri-list'].some(type => event.dataTransfer.types.includes(type));
  const dropContents = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!carriesContent(event)) return;
    event.preventDefault();
    const files = imagesOf(event.dataTransfer.files);
    const dropped = (event.dataTransfer.getData('text/uri-list') || event.dataTransfer.getData('text/plain')).split(/\r?\n/)[0] ?? '';
    const url = files.length ? null : webAddress(dropped);
    if (!files.length && !url) {
      if (event.dataTransfer.files.length) setError(t('workspace.board.dropOnlyImages'));
      return;
    }
    const point = toBoard(event.clientX, event.clientY);
    void addContents(files, url, { x: point.x - BOARD_CARD_WIDTH / 2, y: point.y - 20 }, false);
  };

  // Escape lets go of the selection; Delete or Backspace asks to move it to the trash, Enter
  // confirms. Keys typed in a field stay with the field.
  useEffect(() => {
    if (selected.size === 0 && !confirmTrash) return;
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
      if (event.key === 'Escape') {
        if (confirmTrash) setConfirmTrash(null); else setSelected(new Set());
      } else if (confirmTrash && event.key === 'Enter') {
        // Enter on a button (Keep, a card, the confirmation itself) does what that button says.
        if (target?.closest?.('button, a, [role="button"]')) return;
        event.preventDefault();
        void trashCases(confirmTrash);
      } else if (!confirmTrash && (event.key === 'Delete' || event.key === 'Backspace')) {
        const ids = cases.filter(note => selected.has(note.id)).map(note => note.id);
        if (ids.length === 0) return;
        event.preventDefault();
        setConfirmTrash(ids);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const crumbs = validPath.map(id => byId.get(id)).filter((note): note is Note => Boolean(note));
  const currentPreview = current ? notePreview(current) : '';
  const addLabel = current ? t('notes.cases.create') : t('workspace.board.newCase');
  const canExport = Boolean(current?.project_id ?? projectId);
  const createRef = useRef(startDraftInNextSlot);
  useEffect(() => { createRef.current = startDraftInNextSlot; });
  useEffect(() => { onRegisterCreate?.({ create: () => createRef.current(), label: addLabel }); }, [onRegisterCreate, addLabel]);
  useEffect(() => () => onRegisterCreate?.(null), [onRegisterCreate]);
  const emptySlot = nextFreeSlot([], columns);
  const dropClass = (key: string) => dropTarget?.key === key ? (dropTarget.valid ? ' is-drop-target' : ' is-drop-refused') : '';
  const titleOf = (note: Note) => note.title || t('common.untitled');
  const levelCrumb = (key: string, label: string, next: string[]) => (
    <button type="button" className={`case-board__crumb${dropClass(`level:${key}`)}`} data-drop-level={key} onClick={() => goTo(next)}>{label}</button>
  );
  const dimmed = (note: Note) => filtering && !itemMatches(note) ? ' is-dimmed' : '';
  const natureIcon = (type: NoteType, size = 13) => { const Icon = TYPE_ICONS[type] ?? TYPE_ICONS.note; return <Icon size={size} aria-hidden="true" />; };

  /** Whether another case sits where a tall group would reach, below its first row. */
  const blockedBelow = (note: Note, point: Point) => {
    const below = { x: point.x, y: point.y + SLOT_HEIGHT, ...spanSize({ w: 1, h: 1 }) };
    return cases.some(other => {
      if (other.id === note.id) return false;
      const at = layout.get(other.id);
      if (!at) return false;
      const size = spanSize(spanOf(other));
      return at.x < below.x + below.width && below.x < at.x + size.width && at.y < below.y + below.height && below.y < at.y + size.height;
    });
  };

  const renderGroup = (note: Note, point: Point, span: Span) => {
    const children = childrenOf(note.id);
    // Two rows high to list ten sub-cases. When a case placed by hand sits right below, the group
    // keeps a card's height and unfolds over it while pointed at or focused, without moving it.
    const tall = span.h > 1;
    const folded = tall && blockedBelow(note, point);
    const size = spanSize(folded ? { w: 1, h: 1 } : span);
    const isSelected = selected.has(note.id);
    const common = {
      'data-case-id': note.id,
      'data-drop-case': note.id,
      'data-type': note.note_type,
      style: { left: point.x, top: point.y, width: size.width, height: size.height, ...(folded ? { '--group-unfolded': `${spanSize(span).height}px` } : {}) } as CSSProperties,
      onPointerDown: (event: ReactPointerEvent<HTMLElement>) => startCaseDrag(event, note, true),
      onDoubleClick: (event: ReactMouseEvent<HTMLElement>) => { if (!(event.target as HTMLElement).closest('button')) goTo([...validPath, note.id]); },
    };
    const state = `${isSelected ? ' is-selected' : ''}${dragging?.id === note.id ? ' is-dragging' : ''}${dropClass(`case:${note.id}`)}${dimmed(note)}`;
    if (far) {
      return (
        <article key={note.id} className={`case-board__group is-tile${state}`} {...common}
          aria-label={t('workspace.board.groupLabel', { title: titleOf(note), count: children.length })}>
          <button type="button" className="case-board__tile-title" onClick={() => { if (!draggedRef.current) centerOn(note); }}>{titleOf(note)}</button>
          <p className="case-board__tile-count">{t('workspace.board.subCases', { count: children.length })}</p>
          <ul className="case-board__tile-natures">
            {natureCounts(children).map(([type, count]) => <li key={type} data-type={type} title={TYPE_LABELS[type]}>{natureIcon(type, 22)}{count}</li>)}
          </ul>
        </article>
      );
    }
    const shown = children.slice(0, tall ? GROUP_ROWS_TALL : GROUP_ROWS_SMALL);
    // An open group ends right after its count: its place on the grid is only a ceiling.
    const fitted = { ...common.style, height: undefined, maxHeight: size.height } as CSSProperties;
    return (
      <article key={note.id} className={`case-board__group${folded ? ' is-folded' : ''}${state}`} {...common} style={fitted}>
        <header className="case-board__group-head">
          <span className="case-board__group-nature" title={TYPE_LABELS[note.note_type]}>{natureIcon(note.note_type, 15)}</span>
          <h3 className="case-board__title">
            <button type="button" onClick={() => { if (!draggedRef.current) onOpenNote(note); }} aria-label={t('workspace.cards.open', { title: note.title || t('workspace.cards.untitledLower') })}>{titleOf(note)}</button>
          </h3>
          <StatusBadge status={note.status} iconOnly />
        </header>
        <ul className="case-board__group-items">
          {shown.map(child => (
            <li key={child.id} data-type={child.note_type}
              className={`case-board__chip${filtering && !matches(child) ? ' is-dimmed' : ''}${filtering && matches(child) ? ' is-hit' : ''}${dragging?.ids.includes(child.id) ? ' is-dragging' : ''}`}
              onPointerDown={event => startCaseDrag(event, child, false)}>
              {natureIcon(child.note_type)}
              <button type="button" onClick={() => { if (!draggedRef.current) onOpenNote(child); }} title={TYPE_LABELS[child.note_type]}>{titleOf(child)}</button>
            </li>
          ))}
        </ul>
        <button type="button" className="case-board__dive" onClick={() => { if (!draggedRef.current) goTo([...validPath, note.id]); }}>
          <SquaresFour size={13} aria-hidden="true" />
          {t('workspace.board.subCases', { count: children.length })}
        </button>
      </article>
    );
  };

  const renderCard = (note: Note, point: Point) => {
    const kind = note.content_kind && note.content_kind !== 'text' ? note.content_kind : null;
    const KindIcon = kind ? KIND_ICONS[kind] : null;
    const content = far ? null : cardContent(note);
    const image = content?.image ?? null;
    const preview = content?.text ?? '';
    const hit = filtering && matches(note) ? ' is-hit' : '';
    const link = kind === 'link' && !far ? linkOf(note) : null;
    return (
      <Card key={note.id} as="article" interactive padding="sm"
        className={`case-board__card${selected.has(note.id) ? ' is-selected' : ''}${dragging?.id === note.id ? ' is-dragging' : ''}${dropClass(`case:${note.id}`)}${dimmed(note)}${hit}`}
        style={{ left: point.x, top: point.y, width: BOARD_CARD_WIDTH }}
        onPointerDown={event => startCaseDrag(event, note, true)}
        onDoubleClick={event => { if (!(event.target as HTMLElement).closest('button')) goTo([...validPath, note.id]); }}
        data-case-id={note.id} data-drop-case={note.id} data-type={note.note_type}>
        <div className="case-board__card-top">
          {KindIcon && kind ? <span className="case-board__kind"><KindIcon size={13} aria-hidden="true" />{t(`workspace.board.kind.${kind}`)}</span> : <TypeBadge type={note.note_type} />}
          {!far && <StatusBadge status={note.status} iconOnly />}
        </div>
        <h3 className="case-board__title">
          <button type="button" onClick={() => { if (!draggedRef.current) onOpenNote(note); }} aria-label={t('workspace.cards.open', { title: note.title || t('workspace.cards.untitledLower') })}>{far && natureIcon(note.note_type, 18)}{titleOf(note)}</button>
        </h3>
        {kind === 'sketch' && !far && <SketchThumbnail content={note.content} label={t('workspace.board.sketchPicture', { title: titleOf(note) })} />}
        {image && <NoteThumbnail key={`${note.id}:${image.src}`} image={image} />}
        {link && (
          <button type="button" className="case-board__link" title={link} onClick={() => { if (!draggedRef.current) openLink(link); }}>
            <ArrowSquareOut size={13} aria-hidden="true" /><span>{linkHost(link)}</span>
          </button>
        )}
        {preview && <p className="case-board__preview">{preview}</p>}
        {!far && (
          <button type="button" className="case-board__dive is-empty" onClick={() => { if (!draggedRef.current) goTo([...validPath, note.id]); }}>
            <SquaresFour size={13} aria-hidden="true" />{t('workspace.board.diveEmpty')}
          </button>
        )}
      </Card>
    );
  };

  /** Saves the board shown as a `.canvas` file, where the user chooses (a download otherwise). */
  const exportCanvas = async () => {
    const project = current?.project_id ?? projectId;
    if (!project) return;
    setError(null);
    try {
      const text = await api.exportJsonCanvas(project, current?.id);
      const name = `${(current ? titleOf(current) : t('workspace.board.top')).replace(/[\\/:*?"<>|]/g, '-')}.canvas`;
      const picker = (window as unknown as { showSaveFilePicker?: (options: unknown) => Promise<FileSystemFileHandle> }).showSaveFilePicker;
      if (picker) {
        const handle = await picker({ suggestedName: name, types: [{ description: 'JSON Canvas', accept: { 'application/json': ['.canvas'] } }] });
        const writable = await handle.createWritable();
        await writable.write(text);
        await writable.close();
      } else {
        const link = document.createElement('a');
        link.href = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
        link.download = name;
        link.click();
        URL.revokeObjectURL(link.href);
      }
      setNotice(t('workspace.board.exported'));
    } catch (cause) {
      // Closing the file dialog is not a failure.
      if ((cause as { name?: string })?.name === 'AbortError') return;
      setError(t('workspace.board.exportError', { error: String(cause) }));
    }
  };
  const importCanvas = async (file: File | null | undefined) => {
    if (!file || pending) return;
    setPending(true);
    setError(null);
    try {
      const imported = await api.importJsonCanvas(await file.text(), current ? undefined : creationProjectId ?? undefined, current?.id);
      setNotice(t('workspace.board.imported', { count: imported.cases.length }));
      void loadPositions();
      onChanged();
    } catch (cause) {
      setError(t('workspace.board.importError', { error: String(cause) }));
    } finally {
      setPending(false);
    }
  };
  const openLink = (url: string) => { api.openLink(url).catch(cause => setError(t('workspace.board.linkOpenError', { error: String(cause) }))); };
  const draggedNote = dragging && !dragging.onBoard ? byId.get(dragging.id) : null;
  // Only the cases within one screen of the visible area are rendered: a board of 1,000 cases
  // stays fluid. Before the board is measured (and in tests), every case is.
  const reach = measured && {
    left: (-view.x - measured.width) / view.zoom,
    top: (-view.y - measured.height) / view.zoom,
    right: (-view.x + 2 * measured.width) / view.zoom,
    bottom: (-view.y + 2 * measured.height) / view.zoom,
  };
  const nearScreen = (point: Point, span: Span) => {
    if (!reach) return true;
    const size = spanSize(span);
    return point.x + size.width >= reach.left && point.x <= reach.right && point.y + size.height >= reach.top && point.y <= reach.bottom;
  };
  const trashQuestion = (ids: readonly string[]) => {
    const subCases = subCaseCount(ids, allNotes);
    const first = byId.get(ids[0]);
    const what = ids.length === 1 && first ? t('workspace.board.trashWhatOne', { title: titleOf(first) }) : t('workspace.board.trashWhat', { count: ids.length });
    return subCases > 0 ? t('workspace.board.trashQuestionWith', { what, count: subCases }) : t('workspace.board.trashQuestion', { what });
  };
  const selectedOnBoard = cases.filter(note => selected.has(note.id)).length;

  return (
    <section ref={sectionRef} className={`case-board${current ? ' has-case' : ''}${far ? ' is-far' : ''}`} aria-label={t('workspace.board.label')}
      style={{ '--board-zoom': view.zoom } as CSSProperties}>
      {/* The path only once a case is dived into: at the top, the view switch already names the canvas. */}
      {crumbs.length > 0 && <div className="case-board__toolbar">
        <nav aria-label={t('workspace.board.pathLabel')} className="case-board__path">
          <ol>
            <li>{crumbs.length ? levelCrumb('', t('workspace.board.top'), []) : <span aria-current="page">{t('workspace.board.top')}</span>}</li>
            {crumbs.map((note, index) => <li key={note.id}>
              <CaretRight size={12} aria-hidden="true" />
              {index < crumbs.length - 1
                ? levelCrumb(note.id, titleOf(note), validPath.slice(0, index + 1))
                : <span aria-current="page">{titleOf(note)}</span>}
            </li>)}
          </ol>
        </nav>
      </div>}
      {current && (
        <Card as="header" padding="sm" className="case-board__case" aria-label={t('workspace.board.currentCase')}>
          <div className="case-board__case-text">
            <div className="case-board__case-meta"><TypeBadge type={current.note_type} /><StatusBadge status={current.status} /></div>
            <h2>{titleOf(current)}</h2>
            {currentPreview ? <p>{currentPreview}</p> : <p className="is-empty">{t('workspace.board.caseNoText')}</p>}
          </div>
          <Button type="button" variant="secondary" size="sm" onClick={() => onOpenNote(current)}><PencilSimple size={14} aria-hidden="true" />{t('workspace.board.writeInCase')}</Button>
        </Card>
      )}
      <div className="case-board__controls">
        <div className="case-board__zoom" role="group" aria-label={t('workspace.board.zoomLabel')}>
          <Button type="button" variant="ghost" size="sm" onClick={() => zoomAt(1 / 1.25)} aria-label={t('workspace.board.zoomOut')}><Minus size={14} aria-hidden="true" /></Button>
          <button ref={zoomLabelRef} type="button" className="case-board__zoom-level" onClick={() => setView(previous => ({ ...previous, zoom: 1 }))} title={t('workspace.board.zoomReset')}>{Math.round(view.zoom * 100)} %</button>
          <Button type="button" variant="ghost" size="sm" onClick={() => zoomAt(1.25)} aria-label={t('workspace.board.zoomIn')}><Plus size={14} aria-hidden="true" /></Button>
          <Button type="button" variant="ghost" size="sm" onClick={showAll}><ArrowsOut size={14} aria-hidden="true" />{t('workspace.board.showAll')}</Button>
        </div>
        {filtering && (
          <span className="case-board__found" aria-live="polite">
            <span className="case-board__hits">{t('workspace.board.hits', { count: hits.length })}</span>
            {hits[0] && <Button type="button" variant="ghost" size="sm" onClick={() => centerOn(hits[0])}>{t('workspace.board.showFirstHit')}</Button>}
          </span>
        )}
        <div className="case-board__actions">
          {/* On the cases page, "New case" is the page's own button, above the views. */}
          {!onRegisterCreate && <Button type="button" variant="secondary" size="sm" onClick={startDraftInNextSlot}>
            <Plus size={14} aria-hidden="true" />{addLabel}
          </Button>}
          {/* A greyed action says why: a disabled button shows no tooltip, so the hint sits on a focusable wrapper. */}
          <Hint label={canExport ? t('workspace.board.exportHint') : t('workspace.board.exportNeedsProject')}>
            <span className="case-board__action" tabIndex={canExport ? -1 : 0}>
              <Button type="button" variant="ghost" size="sm" disabled={!canExport} onClick={() => void exportCanvas()}>
                <DownloadSimple size={14} aria-hidden="true" />{t('workspace.board.export')}
              </Button>
            </span>
          </Hint>
          <Hint label={createsInPlace ? t('workspace.board.importHint') : t('workspace.board.importNeedsProject')}>
            <span className="case-board__action" tabIndex={createsInPlace ? -1 : 0}>
              <Button type="button" variant="ghost" size="sm" disabled={!createsInPlace || pending} onClick={() => importRef.current?.click()}>
                <UploadSimple size={14} aria-hidden="true" />{t('workspace.board.import')}
              </Button>
            </span>
          </Hint>
          <input ref={importRef} type="file" accept=".canvas,application/json" hidden aria-label={t('workspace.board.importHint')}
            onChange={event => { const file = event.target.files?.[0]; event.target.value = ''; void importCanvas(file); }} />
          <Button type="button" variant={showDocument ? 'secondary' : 'ghost'} size="sm" aria-pressed={showDocument} onClick={() => setShowDocument(value => !value)}>
            <FileText size={14} aria-hidden="true" />{t('workspace.board.document')}
          </Button>
        </div>
      </div>
      {error && <p role="alert" className="case-board__error">{error}</p>}
      {notice && !error && <p role="status" className="case-board__notice">{notice}</p>}
      <div className={showDocument ? 'case-board__body has-document' : 'case-board__body'}>
        <div ref={surfaceRef} className="case-board__surface" onPointerDown={startPan} onDoubleClick={createAtPoint}
          onDragOver={event => { if (carriesContent(event)) { event.preventDefault(); event.dataTransfer.dropEffect = 'copy'; } }} onDrop={dropContents}
          style={{ backgroundPosition: `${view.x}px ${view.y}px`, backgroundSize: `${22 * view.zoom}px ${22 * view.zoom}px` }}>
          <div ref={layerRef} className="case-board__layer" style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.zoom})` }}>
            {cases.map(note => {
              const point = dragging?.id === note.id && dragging.onBoard ? dragging.point : layout.get(note.id) ?? { x: 0, y: 0 };
              const span = spanOf(note);
              if (!nearScreen(point, span) && dragging?.id !== note.id && !selected.has(note.id)) return null;
              return childrenOf(note.id).length ? renderGroup(note, point, span) : renderCard(note, point);
            })}
            {draggedNote && dragging && (
              <div ref={ghostRef} className="case-board__ghost" data-type={draggedNote.note_type} style={{ left: dragging.point.x, top: dragging.point.y }}>
                {natureIcon(draggedNote.note_type)}{titleOf(draggedNote)}
              </div>
            )}
            {draft && (
              <Card as="form" padding="sm" className="case-board__card case-board__draft" style={{ left: draft.point.x, top: draft.point.y }}
                onSubmit={event => { event.preventDefault(); void createCase(); }} onBlur={leaveDraft}>
                <label className="case-board__draft-label" htmlFor="case-board-draft">{current ? t('notes.cases.newTitle') : t('workspace.board.newCaseTitle')}</label>
                <input id="case-board-draft" className="ui-field" autoFocus value={title} disabled={pending} maxLength={240}
                  placeholder={t('workspace.board.draftPlaceholder')}
                  onChange={event => setTitle(event.target.value)}
                  onKeyDown={event => { if (event.key === 'Escape') cancelDraft(); }} />
                {current && (
                  <div className="case-board__draft-kinds" role="radiogroup" aria-label={t('workspace.board.contentLabel')}>
                    {DRAFT_KINDS.map(({ kind, icon: Icon }) => (
                      <button key={kind} type="button" role="radio" aria-checked={draft.kind === kind}
                        className="case-board__nature" onMouseDown={event => event.preventDefault()}
                        onClick={() => { setError(null); setDraft(previous => previous && { ...previous, kind }); }}>
                        <Icon size={13} aria-hidden="true" />{t(`workspace.board.kind.${kind}`)}
                      </button>
                    ))}
                  </div>
                )}
                {current && draft.kind === 'link' && (
                  <input className="ui-field" type="url" value={draft.url} disabled={pending} aria-label={t('workspace.board.linkLabel')}
                    placeholder={t('workspace.board.linkPlaceholder')}
                    onChange={event => { const url = event.target.value; setError(null); setDraft(previous => previous && { ...previous, url }); }}
                    onKeyDown={event => { if (event.key === 'Escape') cancelDraft(); }} />
                )}
                {current && draft.kind === 'image' && (
                  <label className="case-board__file">
                    <ImageIcon size={14} aria-hidden="true" />
                    <span>{draft.file ? draft.file.name : t('workspace.board.imagePick')}</span>
                    <input type="file" accept="image/png,image/jpeg" disabled={pending}
                      onChange={event => { const file = event.target.files?.[0] ?? null; setError(null); setDraft(previous => previous && { ...previous, file }); }} />
                  </label>
                )}
                <div className="case-board__draft-natures" role="radiogroup" aria-label={t('workspace.board.natureLabel')}>
                  {NOTE_TYPES.map(type => (
                    <button key={type} type="button" role="radio" aria-checked={draft.nature === type} data-type={type}
                      className="case-board__nature" onMouseDown={event => event.preventDefault()}
                      onClick={() => setDraft(previous => previous && { ...previous, nature: type })}>
                      {natureIcon(type)}{TYPE_LABELS[type]}
                    </button>
                  ))}
                </div>
                {current && draft.kind !== 'text' ? (
                  <div className="case-board__draft-end">
                    <p className="case-board__draft-hint">{t(`workspace.board.draftHint_${draft.kind}`)}</p>
                    <Button type="submit" variant="secondary" size="sm" disabled={pending}>{t('workspace.board.createRich')}</Button>
                  </div>
                ) : <p className="case-board__draft-hint">{t('workspace.board.draftHint')}</p>}
              </Card>
            )}
            {cases.length === 0 && !draft && (
              <button type="button" className="case-board__add-slot" style={{ left: emptySlot.x, top: emptySlot.y, width: BOARD_CARD_WIDTH }} onClick={startDraftInNextSlot}>
                <Plus size={18} aria-hidden="true" />
                <span>{addLabel}</span>
                <small>{t('workspace.board.addHint')}</small>
              </button>
            )}
          </div>
          {confirmTrash ? (
            <div className="case-board__float is-danger" role="alertdialog" aria-label={t('workspace.board.trashConfirmLabel')}>
              <span>{trashQuestion(confirmTrash)}</span>
              <Button type="button" variant="destructive" size="sm" autoFocus onClick={() => void trashCases(confirmTrash)}><Trash size={14} aria-hidden="true" />{t('workspace.board.trashConfirm')}</Button>
              <Button type="button" variant="ghost" size="sm" onClick={() => setConfirmTrash(null)}>{t('workspace.board.trashCancel')}</Button>
            </div>
          ) : selectedOnBoard > 0 ? (
            <form className="case-board__float" onSubmit={event => { event.preventDefault(); void groupSelection(); }}>
              <span>{t('workspace.board.selected', { count: selectedOnBoard })}</span>
              <input className="ui-field" value={groupName} maxLength={240} disabled={pending} placeholder={t('workspace.board.groupPlaceholder')} aria-label={t('workspace.board.groupPlaceholder')}
                onChange={event => { setGroupName(event.target.value); setGroupError(null); }} />
              <Button type="submit" variant="secondary" size="sm" disabled={pending}><FolderPlus size={14} aria-hidden="true" />{t('workspace.board.group')}</Button>
              <Button type="button" variant="ghost" size="sm" data-tone="danger" title={t('workspace.board.trashHint')} aria-label={t('workspace.board.trashSelection')}
                onClick={() => setConfirmTrash(cases.filter(note => selected.has(note.id)).map(note => note.id))}><Trash size={14} aria-hidden="true" /></Button>
              <Button type="button" variant="ghost" size="sm" onClick={() => setSelected(new Set())} aria-label={t('workspace.board.clearSelection')}><X size={14} aria-hidden="true" /></Button>
              {groupError && <p role="alert" className="case-board__float-error">{groupError}</p>}
            </form>
          ) : undo ? (
            <div className="case-board__float" role="status">
              <span>{undo.message}</span>
              <Button type="button" variant="secondary" size="sm" onClick={() => void runUndo()}><ArrowCounterClockwise size={14} aria-hidden="true" />{t('workspace.board.undo')}</Button>
            </div>
          ) : !far && cases.length > 1 && !dragging ? (
            <p className="case-board__tip">{t('workspace.board.tip')}</p>
          ) : null}
        </div>
        {showDocument && <CaseDocument roots={current ? [current] : cases} allNotes={allNotes} onOpenNote={onOpenNote} />}
      </div>
    </section>
  );
}

import { columnsWithIntents, createStatusQueue, writeTrackedStatus, resolveColumnFromPoint, resolveColumnFromRect, resolveDropStatus } from '../utils/kanban';
import type { ColumnRect } from '../utils/kanban';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { DragEndEvent } from '@dnd-kit/core';
import { DndContext, DragOverlay, closestCorners, KeyboardSensor, PointerSensor, useDroppable, useSensor, useSensors } from '@dnd-kit/core';
import { SortableContext, sortableKeyboardCoordinates, verticalListSortingStrategy, useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { Note, NoteStatus } from '../types';
import { api } from '../services/tauriApi';
import { notePreview } from '../utils/notePresentation';
import { formatShortDate, parseTimestamp } from '../utils/datetime';
import { statusLabel } from '../utils/statuses';
import { Count, PriorityBadge, StatusBadge, TypeBadge } from './ui/Badge';
import { Card } from './ui/Card';
import { Notice } from './ui/Toast';
import { EmptyState } from './ui/EmptyState';
import './workspace/note-views.css';

interface KanbanBoardProps {
  notes: Note[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  onNotesUpdate: () => void;
}

export default function KanbanBoard({ notes, activeNoteId, onSelectNote, onNotesUpdate }: KanbanBoardProps) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  // Card being dragged: rendered by a `DragOverlay` (above everything). Without it, the card
  // stayed in the DOM paint order and went **under** the following cards.
  const [draggedNote, setDraggedNote] = useState<Note | null>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );

  // Status writes serialized per note: the last gesture wins, even if the response of the
  // first write arrives after the second.
  const statusQueue = useMemo(() => createStatusQueue(), []);

  // Incremented when an in-flight intent changes: forces the columns to render (the intents Map
  // is not React state).
  const [intentsVersion, setIntentsVersion] = useState(0);

  // As soon as props have caught up with an intent, it is forgotten: the real status takes over again.
  useEffect(() => {
    for (const n of notes) statusQueue.reconcile(n.id, n.status);
  }, [notes, statusQueue]);

  // Rendered with the in-flight intent: the dragged card joins the target column right away.
  const columnsData = useMemo(() => {
    // `intentsVersion` takes part in the recomputation: the intents Map is not React state.
    void intentsVersion;
    return columnsWithIntents(notes, statusQueue);
  }, [notes, statusQueue, intentsVersion]);

  // DOM elements of the columns: used to determine the column actually targeted on drop.
  const columnElements = useRef(new Map<NoteStatus, HTMLElement>());
  const registerColumn = useCallback((id: NoteStatus, element: HTMLElement | null) => {
    if (element) columnElements.current.set(id, element);
    else columnElements.current.delete(id);
  }, []);

  const columnRects = (): ColumnRect[] => {
    const rects: ColumnRect[] = [];
    columnElements.current.forEach((element, id) => {
      const r = element.getBoundingClientRect();
      rects.push({ id, x1: r.left, y1: r.top, x2: r.right, y2: r.bottom });
    });
    return rects;
  };

  // Cursor position during the drag: on its own, it determines the targeted column, including
  // when the card is dropped in the empty area of a column (where dnd-kit returns no hover).
  const pointerPosition = useRef<{ x: number; y: number } | null>(null);
  useEffect(() => {
    const trackPointer = (event: PointerEvent) => {
      pointerPosition.current = { x: event.clientX, y: event.clientY };
    };
    window.addEventListener('pointermove', trackPointer);
    window.addEventListener('pointerdown', trackPointer);
    return () => {
      window.removeEventListener('pointermove', trackPointer);
      window.removeEventListener('pointerdown', trackPointer);
    };
  }, []);

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;

    const draggedNoteId = active.id as string;
    const draggedNote = notes.find(n => n.id === draggedNoteId);
    if (!draggedNote) return;

    const rects = columnRects();

    // The cursor is only the reference for a **mouse** interaction. A keyboard move
    // emits no `pointermove`: `pointerPosition` then remembers the last mouse position and,
    // consulted first, made the card land under the old cursor instead of
    // the targeted column (bug #12). The sensor actually used is carried by `activatorEvent`.
    const isKeyboardDrag = event.activatorEvent instanceof KeyboardEvent;
    const pointer = isKeyboardDrag ? null : pointerPosition.current;
    const translated = active.rect.current.translated;
    // For a mouse, the cursor is what the user sees on release, which matters because during
    // the drag the content may scroll (dnd-kit scrolls the container when the card nears
    // an edge) and the id returned by dnd-kit may designate a card of another column. A drop
    // outside any column must change nothing, rather than apply an arbitrary status.
    const newStatus = pointer
      ? resolveColumnFromPoint(pointer, rects)
      : (translated ? resolveColumnFromRect(translated, rects) : null)
        ?? (over ? resolveDropStatus(String(over.id), notes) : null);

    if (!newStatus) return;

    // Writes of the same note are serialized and the last intent is remembered: two quick
    // moves can no longer replace each other out of order (the slowest response
    // no longer overwrites the last gesture). The write also goes through the chain tracked by the editor,
    // which export/restore waits for before freezing a snapshot.
    const scheduled = statusQueue.schedule(draggedNoteId, draggedNote.status, newStatus, async target => {
      try {
        await writeTrackedStatus(() => api.updateNote(draggedNoteId, { status: target }));
        setError(null);
        onNotesUpdate();
      } catch (e) {
        statusQueue.abandon(draggedNoteId, target);
        setIntentsVersion(v => v + 1);
        console.error('Failed to move the card', e);
        setError(t('notes.kanban.moveFailed'));
      }
    });
    if (scheduled) setIntentsVersion(v => v + 1);
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCorners}
      onDragStart={({ active }) => setDraggedNote(notes.find(n => n.id === active.id) ?? null)}
      onDragEnd={event => {
        setDraggedNote(null);
        void handleDragEnd(event);
      }}
      onDragCancel={() => setDraggedNote(null)}
    >
      <div className="workspace-kanban flex-1 flex flex-col bg-background overflow-hidden">
        {error && (
          <Notice tone="danger" className="workspace-kanban__error">{error}</Notice>
        )}
        <div className="workspace-kanban__columns">
          {columnsData.map(col => (
            <KanbanColumn
              key={col.id}
              id={col.id}
              title={statusLabel(col.id)}
              items={col.items}
              activeNoteId={activeNoteId}
              onSelectNote={onSelectNote}
              onRegister={registerColumn}
            />
          ))}
        </div>
      </div>

      {/* Rendered in a portal, above all the columns (including outside the original column). */}
      <DragOverlay dropAnimation={null}>
        {draggedNote ? (
          <Card active className="workspace-kanban__card workspace-kanban__card--dragging">
            <KanbanCardContent note={draggedNote} />
          </Card>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}

interface KanbanColumnProps {
  id: NoteStatus;
  title: string;
  items: Note[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  onRegister: (id: NoteStatus, element: HTMLElement | null) => void;
}

/**
 * Droppable column: without `useDroppable`, `over.id` could never be a column id
 * and dropping a card in the empty area of a column did nothing (B11).
 */
function KanbanColumn({ id, title, items, activeNoteId, onSelectNote, onRegister }: KanbanColumnProps) {
  const { t } = useTranslation();
  const { setNodeRef, isOver } = useDroppable({ id });
  // The element is also exposed to the parent card (column resolution on drop).
  const attachRef = useCallback((node: HTMLElement | null) => {
    setNodeRef(node);
    onRegister(id, node);
  }, [setNodeRef, onRegister, id]);

  return (
    <Card tone="sunken" padding="none" className="workspace-kanban__column">
      <h3 className="workspace-kanban__heading">
        {title}
        <Count value={items.length} />
      </h3>

      <SortableContext id={id} items={items.map(n => n.id)} strategy={verticalListSortingStrategy}>
        <div
          ref={attachRef}
          data-column={id}
          className={`flex flex-col gap-2 flex-1 min-h-[100px] overflow-y-auto pr-1 rounded-md transition-colors ${
            isOver ? 'bg-primary/5 ring-1 ring-primary/40' : ''
          }`}
        >
          {items.length === 0 && (
            <EmptyState size="sm" description={t('notes.kanban.dropHere')} />
          )}
          {items.map(note => (
            <SortableNote
              key={note.id}
              note={note}
              isActive={activeNoteId === note.id}
              onClick={() => onSelectNote(note.id)}
            />
          ))}
        </div>
      </SortableContext>
    </Card>
  );
}

interface SortableNoteProps {
  note: Note;
  isActive: boolean;
  onClick: () => void;
}

function SortableNote({ note, isActive, onClick }: SortableNoteProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: note.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : 1,
  };

  return (
    <Card
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      onClick={onClick}
      interactive
      active={isActive}
      className="workspace-kanban__card"
    >
      <KanbanCardContent note={note} onOpen={onClick} />
    </Card>
  );
}

/** Visual content of a card, shared by the sortable card and the drag preview (`DragOverlay`). */
function KanbanCardContent({ note, onOpen }: { note: Note; onOpen?: () => void }) {
  const { t } = useTranslation();
  const preview = notePreview(note);
  const title = note.title || t('common.untitled');
  return (
    <>
      <div className="workspace-kanban__card-top">
        <StatusBadge status={note.status} size="md" />
        <TypeBadge type={note.note_type} size="md" />
      </div>
      <h4>
        {onOpen ? (
          <button
            type="button"
            className="workspace-kanban__card-title"
            aria-label={t('notes.kanban.openNote', { title: title })}
            onClick={event => { event.stopPropagation(); onOpen(); }}
            onKeyDown={event => {
              // Enter/Space open the note and must not start dnd-kit's keyboard sensor
              // (opening stays distinct from dragging).
              if (event.key === 'Enter' || event.key === ' ') {
                event.stopPropagation();
                if (event.key === 'Enter') {
                  event.preventDefault();
                  onOpen();
                }
              }
            }}
          >
            {title}
          </button>
        ) : title}
      </h4>
      {preview && <p className="workspace-kanban__preview">{preview}</p>}
      <div className="workspace-kanban__date">
        {note.priority && (
          <PriorityBadge priority={note.priority} status={note.status} />
        )}
        <time dateTime={parseTimestamp(note.updated_at)?.toISOString()}>{formatShortDate(note.updated_at)}</time>
      </div>
    </>
  );
}

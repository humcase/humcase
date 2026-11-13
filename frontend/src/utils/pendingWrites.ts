import type { ProjectSpace, UpdateNoteInput } from '../types';

/**
 * Escrow of writes that failed **on editor unmount / close**.
 *
 * At that moment the React component no longer exists: its state (`rawPending`) is gone. To
 * never lose input without a trace, the failed write is kept here (memory + `localStorage`)
 * and can be replayed on the next display ("Retry").
 *
 * `space` records the space (Work/Private) of the target note **at the time of the failure**. It makes it possible to
 * never offer, from a note in one space, the content of a draft from the other space.
 * `projectId` captures the note's project at the time of the failure; it allows repairing the space
 * without reopening the note, from the globally loaded projects. An entry without `space` (old
 * version, or note without a space) stays visible everywhere: it is never pinned, deleted,
 * or made unrecoverable.
 */
export type PendingWrite =
  | { id: string; kind: 'update'; noteId: string; payload: UpdateNoteInput; base?: Partial<UpdateNoteInput>; space?: ProjectSpace | null; projectId?: string | null; at: string }
  | { id: string; kind: 'brut'; noteId: string; texte: string; space?: ProjectSpace | null; projectId?: string | null; at: string }
  // Draft of the "Create a linked task" form: `noteId` is the source note the draft is
  // attached to. It is not a write to replay: it is restored into the form.
  | { id: string; kind: 'task-draft'; noteId: string; titre: string; contenu: string; space?: ProjectSpace | null; projectId?: string | null; at: string };

export type TaskDraft = Extract<PendingWrite, { kind: 'task-draft' }>;

const STORAGE_KEY = 'humcase-pending-writes';

function newId(): string {
  const c = globalThis.crypto as Crypto | undefined;
  if (c && typeof c.randomUUID === 'function') return c.randomUUID();
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/**
 * In-memory copy of the escrow, **source of truth during the session**.
 *
 * It guarantees that a write refused by `localStorage` (quota exceeded, storage disabled)
 * remains available for recovery until the window is closed, instead of being lost.
 * `null` = not yet read from local storage.
 */
let memory: PendingWrite[] | null = null;
/** False as soon as a local write has been refused: retention is then no longer persistent. */
let persistent = true;

/** Defensive read: unreadable content or content of an unexpected shape is ignored, never propagated. */
function isValidEntry(e: unknown): e is PendingWrite {
  if (!e || typeof e !== 'object') return false;
  const o = e as Record<string, unknown>;
  if (typeof o.id !== 'string' || typeof o.noteId !== 'string' || typeof o.at !== 'string') {
    return false;
  }
  if (o.kind === 'update') return typeof o.payload === 'object' && o.payload !== null &&
    (o.base === undefined || (typeof o.base === 'object' && o.base !== null));
  if (o.kind === 'brut') return typeof o.texte === 'string';
  if (o.kind === 'task-draft') return typeof o.titre === 'string' && typeof o.contenu === 'string';
  return false;
}

/**
 * An unknown `space` value (future version, corrupted data) must **never** invalidate
 * the entry: it is normalized to `undefined` (= visible everywhere), otherwise rewriting
 * the list would purge it for good. A valid write always survives. Same defensive
 * treatment for `projectId`: a non-string **or empty** identifier is treated as absent,
 * never the whole entry.
 */
function normalizeEntry(e: PendingWrite): PendingWrite {
  const space = e.space;
  const project = e.projectId;
  const invalidSpace = space !== undefined && space !== null && space !== 'private' && space !== 'work';
  const invalidProject = project !== undefined && project !== null && (typeof project !== 'string' || project === '');
  if (!invalidSpace && !invalidProject) return e;
  const copy = { ...e };
  if (invalidSpace) copy.space = undefined;
  if (invalidProject) copy.projectId = undefined;
  return copy;
}

export function readPendingWrites(): PendingWrite[] {
  // Once initialized, the in-memory copy is authoritative: it contains at least everything local
  // storage accepted, plus the writes it refused.
  if (memory !== null) return memory.slice();
  try {
    const raw = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (!raw) {
      memory = [];
      return [];
    }
    const parsed = JSON.parse(raw) as unknown;
    memory = (Array.isArray(parsed) ? parsed.filter(isValidEntry) : []).map(normalizeEntry);
  } catch {
    // Unreadable storage: start from an empty list, but do not pretend that retention
    // is persistent.
    persistent = false;
    memory = [];
  }
  return memory.slice();
}

/** Readers that show the escrow (the editor's recovery panel) and must see an entry added elsewhere. */
const listeners = new Set<() => void>();

/**
 * Calls `listener` after every change of the escrow, whoever made it: a drawing's failed write
 * reaches the editor's recovery panel, which stays mounted while the drawing is open. Returns the
 * unsubscribe function.
 */
export function subscribePendingWrites(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

function write(list: PendingWrite[]): void {
  writeStorage(list);
  for (const listener of [...listeners]) listener();
}

function writeStorage(list: PendingWrite[]): void {
  memory = list.slice();
  try {
    if (list.length === 0) globalThis.localStorage?.removeItem(STORAGE_KEY);
    else globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(list));
    persistent = true;
  } catch {
    // Storage refused the write: the in-memory copy is the only recourse; never break
    // the caller because of it.
    persistent = false;
  }
}

/**
 * True as long as the escrow could be written to local storage. False if the write was
 * refused: recovery remains possible in this session (memory) but **will not survive**
 * closing. The interface must say so instead of announcing durable retention.
 */
export function pendingWritesPersistent(): boolean {
  return persistent;
}

function signature(e: { kind: string; noteId: string; payload?: unknown; base?: unknown; texte?: unknown; titre?: unknown; contenu?: unknown; space?: unknown }): string {
  // `?? null`: a legacy/normalized entry (space absent) and an explicit `space: null` entry
  // must produce the same signature, otherwise the same failed draft is displayed twice.
  // `projectId` is not part of the signature: it does not change the draft content, and two
  // successive failures of the same payload must not produce two entries on screen.
  return JSON.stringify({ kind: e.kind, noteId: e.noteId, payload: e.payload, base: e.base, texte: e.texte, titre: e.titre, contenu: e.contenu, space: e.space ?? null });
}

/**
 * Keeps a write that could not be saved, for later recovery.
 * Strict duplicates (same note, same payload) are not stacked: a close followed by an
 * unmount that both fail create only one entry.
 */
export function addPendingWrite(
  entry:
    | { kind: 'update'; noteId: string; payload: UpdateNoteInput; base?: Partial<UpdateNoteInput>; space?: ProjectSpace | null; projectId?: string | null }
    | { kind: 'brut'; noteId: string; texte: string; space?: ProjectSpace | null; projectId?: string | null }
    | { kind: 'task-draft'; noteId: string; titre: string; contenu: string; space?: ProjectSpace | null; projectId?: string | null }
): PendingWrite {
  const list = readPendingWrites();
  const sig = signature(entry);
  const duplicate = list.find(e => signature(e) === sig);
  if (duplicate) {
    // A second failure of the same payload knows the current anchor (note's project): it refreshes
    // the existing entry instead of leaving it frozen on the first failure. The content is never
    // merged; the space is already guaranteed equivalent by the signature (`space ?? null`).
    const projectId = entry.projectId || null;
    if ((duplicate.projectId ?? null) !== projectId) {
      updatePendingWriteEntries([{ id: duplicate.id, projectId }]);
      return readPendingWrites().find(e => e.id === duplicate.id) ?? duplicate;
    }
    return duplicate;
  }
  const newEntry = { ...entry, projectId: entry.projectId || null, id: newId(), at: new Date().toISOString() } as PendingWrite;
  write([...list, newEntry]);
  return newEntry;
}

export function removePendingWrite(id: string): void {
  write(readPendingWrites().filter(e => e.id !== id));
}

/**
 * Draft of the "Create a linked task" form attached to its source note, if any.
 * `space` filters like the rest of the escrow: a draft from one space is never offered
 * from another; `space == null` (unsorted note, unknown space) restricts nothing.
 */
export function readTaskDraft(noteId: string, space?: ProjectSpace | null): TaskDraft | undefined {
  return readPendingWrites().find((e): e is TaskDraft =>
    e.kind === 'task-draft' && e.noteId === noteId &&
    (space == null || e.space == null || e.space === space));
}

/**
 * Replaces the linked-task draft of a source note with the current input.
 * An empty draft removes the existing entry (input displayed then cleared); a non-empty input
 * replaces the old entry of the **same note visible in the same space**, never touching
 * an entry from another space, which would otherwise become unrecoverable. `projectId` captures the project of
 * the source note, for repairing the space without reopening the note.
 */
export function saveTaskDraft(noteId: string, title: string, content: string, space?: ProjectSpace | null, projectId?: string | null): void {
  const list = readPendingWrites();
  const replaces = (e: PendingWrite) => e.kind === 'task-draft' && e.noteId === noteId &&
    (space == null || e.space == null || e.space === space);
  const remainingEntries = list.filter(e => !replaces(e));
  if (title.trim() !== '' || content.trim() !== '') {
    remainingEntries.push({ kind: 'task-draft', noteId, titre: title, contenu: content, space: space ?? null, projectId: projectId || null, id: newId(), at: new Date().toISOString() });
  }
  write(remainingEntries);
}

/**
 * Removes the linked-task draft of a note. `space` filters like the readers: an entry
 * from another space (never displayed here) is never deleted. Called **only** after a
 * successful creation or an explicit abandon: never for an entry that is not displayed.
 */
export function removeTaskDraft(noteId: string, space?: ProjectSpace | null): void {
  const list = readPendingWrites();
  const remainingEntries = list.filter(e => !(e.kind === 'task-draft' && e.noteId === noteId &&
    (space == null || e.space == null || e.space === space)));
  if (remainingEntries.length !== list.length) write(remainingEntries);
}

/**
 * Rewrites the anchor (`space`, `projectId`) of several entries in **a single** rewrite of
 * the storage, without changing the content or the identifiers and without ever deleting an entry.
 *
 * Self-repair: if a note's project changes space after a draft was escrowed,
 * the entry carrying the old space would become invisible in **both** spaces. Attaching it to
 * the note's actual space makes it recoverable again; `projectId` makes it possible to infer that
 * space from the globally loaded projects, without reopening the note. The batch avoids an intermediate write and
 * render per entry when several drafts need repair.
 *
 * Defense in depth: an entry with `space: null` stays visible everywhere and is **never**
 * pinned: its space is only changed if it already had one. Same persistence as the rest
 * (memory + `localStorage`, in-memory fallback if storage refuses). Returns true if the list changed.
 */
export function updatePendingWriteEntries(
  updates: Array<{ id: string; space?: ProjectSpace; projectId?: string | null }>
): boolean {
  if (updates.length === 0) return false;
  const byId = new Map<string, { id: string; space?: ProjectSpace; projectId?: string | null }>();
  for (const update of updates) byId.set(update.id, update);
  let change = false;
  const list = readPendingWrites().map(e => {
    const update = byId.get(e.id);
    if (!update) return e;
    let copy = e;
    // Never pin: the space is only applied to an entry that already had one.
    if (update.space !== undefined && e.space != null && e.space !== update.space) {
      copy = { ...copy, space: update.space };
    }
    if (update.projectId !== undefined && (copy.projectId ?? null) !== (update.projectId ?? null)) {
      copy = { ...copy, projectId: update.projectId };
    }
    if (copy !== e) change = true;
    return copy;
  });
  if (change) write(list);
  return change;
}

export function clearPendingWrites(): void {
  write([]);
}

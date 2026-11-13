import { useTranslation } from 'react-i18next';
import { useState, useEffect, useLayoutEffect, useMemo, useRef, useCallback } from 'react';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Note, NoteType, Project, ProjectSpace } from '../types';
import { isIsoDay } from '../utils/datetime';
import { deriveTitle, isDefaultTitle } from '../utils/title';
import { subCaseCount } from '../utils/caseBoard';
import { readTags } from '../utils/tags';
import { trackEditorWrite, type BackupPreparation } from '../utils/backupBarrier';
import { pendingWritesPersistent, readPendingWrites, updatePendingWriteEntries } from '../utils/pendingWrites';
import { useAiReady } from '../hooks/useAiReady';
import { api } from '../services/tauriApi';
import TiptapEditor from './TiptapEditor';
import { NoteReport } from './NoteFollowUp';
import NoteRelations from './NoteRelations';
import NoteSubCases from './NoteSubCases';
import NoteHistory from './NoteHistory';
import { CaseActivity } from './review/CaseActivity';
import { useCaseJournal } from './review/useCaseJournal';
import { Dialog, DialogFooter, DialogHeader } from './ui/Dialog';
import { EmptyState } from './ui/EmptyState';
import { Button } from './ui/Button';
import './note-structure.css';
import './note-editor-layout.css';
import {
  advanceBaseline,
  baseDuPayload,
  EDITABLE_FIELDS,
  buildPayload,
  equal,
  spaceOfNote,
  stateFromNote,
  type EditableState,
} from './noteEditor/editableState';
import { PendingWritesPanel } from './noteEditor/PendingWritesPanel';
import { DictationOverlay } from './noteEditor/DictationOverlay';
import { useEditorToast } from './noteEditor/useEditorToast';
import { useDictation } from './noteEditor/useDictation';
import { useStructure } from './noteEditor/useStructure';
import { pendingWriteRepairs, usePendingWrites } from './noteEditor/usePendingWrites';
import { NoteProperties } from './noteEditor/NoteProperties';
import { NoteEditorHeader } from './noteEditor/NoteEditorHeader';
import { NoteToolbar, type EditorView } from './noteEditor/NoteToolbar';
import { CompareView } from './noteEditor/CompareView';
import { NoteActions } from './noteEditor/NoteActions';
import { StructureDialog } from './noteEditor/StructureDialog';
import { GerminatePanel } from './noteEditor/GerminatePanel';
import { DecisionHistory } from './noteEditor/DecisionHistory';
import { useGermination } from './noteEditor/useGermination';

interface NoteEditorProps {
  note: Note | null;
  onNoteUpdated: () => void;
  isMiniMode?: boolean;
  onBackToList?: () => void;
  /** Topbar place for the parent cases; without it they show above the title. */
  pathSlot?: HTMLElement | null;
  allNotes?: Note[];
  onOpenRelatedNote?: (target: Note) => Promise<void> | void;
  structuringContextLabel?: string;
  onRegisterPrepareBackup?: (prepare: (() => Promise<BackupPreparation>) | null) => void;
  /**
   * Known projects (id → space). Used to never expose the content of another space:
   * a note in the Work space must offer neither a similar note nor a draft from the Private space.
   * Absent → no limit (historical behavior, unit tests).
   */
  projects?: Project[];
  /** Id of a case to sprout as soon as it opens (a nudge's "Sprout" on Home). */
  germinateOnOpen?: string | null;
  /** The sprouting asked by `germinateOnOpen` started. */
  onGerminateStarted?: () => void;
  /** The case was just created by the person: the cursor goes in its text. */
  focusOnOpen?: boolean;
}

export default function NoteEditor({ note, onNoteUpdated, isMiniMode, onBackToList, pathSlot = null, allNotes = [], onOpenRelatedNote, structuringContextLabel, onRegisterPrepareBackup, projects, germinateOnOpen = null, onGerminateStarted, focusOnOpen = false }: NoteEditorProps) {
  const { t } = useTranslation();
  const [title, setTitle] = useState(note?.title || '');
  const [content, setContent] = useState(note?.content || '');
  const [noteType, setNoteType] = useState<NoteType>(note?.note_type as NoteType || 'note');
  const [tags, setTags] = useState<string[]>(() => readTags(note?.tags));
  const [priority, setPriority] = useState<string>(note?.priority || '');
  const [dueAt, setDueAt] = useState<string>(note?.due_at || '');
  const [viewMode, setViewMode] = useState<EditorView>('edit');
  const [pendingTemplate, setPendingTemplate] = useState<{ name: string; text: string } | null>(null);
  const supportRef = useRef<HTMLElement>(null);
  // The editor stays mounted from case to case (and its root element is reused for the empty state):
  // a case opens at its top, not where the previous one was scrolled.
  const rootRef = useRef<HTMLDivElement>(null);
  // A new case opens on its title (still the default one); an existing case on its text.
  const focusTitleOnOpen = focusOnOpen && isDefaultTitle(note?.title);
  const [bodyFocusSignal, setBodyFocusSignal] = useState(0);
  const openNoteId = note?.id ?? null;
  useLayoutEffect(() => {
    if (rootRef.current) rootRef.current.scrollTop = 0;
  }, [openNoteId]);
  // Who did what on the case and the decision on its result (v24).
  const caseJournal = useCaseJournal(note?.id ?? '', note?.revision ?? 0);
  // Opening the case (or seeing it change while open) takes it out of "Since your last visit".
  const seenNoteId = note?.id;
  const seenRevision = note?.revision;
  useEffect(() => {
    if (!seenNoteId) return;
    Promise.resolve(api.markCaseSeen?.(seenNoteId)).catch(() => { /* the brief simply keeps listing it */ });
  }, [seenNoteId, seenRevision]);
  const appliedTemplateRef = useRef<{ noteId: string; text: string } | null>(null);
  const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);
  const [similarNotes, setSimilarNotes] = useState<Note[]>([]);
  const [suggestedType, setSuggestedType] = useState<NoteType | null>(null);
  const [dismissedType, setDismissedType] = useState<NoteType | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [hasSaved, setHasSaved] = useState(false);
  const { showToast, toast } = useEditorToast(onNoteUpdated);
  // AI connected in Settings? `false` locks the AI actions (visible, explained); `null` = unknown.
  const aiReady = useAiReady();
  const savedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Last note whose editable fields were loaded: the same note refreshed from the database
  // must never overwrite a local input in progress (data loss).
  const loadedNoteIdRef = useRef<string | null>(null);
  // Identifier actually displayed, kept up to date on every render. Unlike a dictation's
  // closure, it reflects the note the user is looking at when the response arrives.
  const currentNoteIdRef = useRef<string | null>(note?.id ?? null);
  // Dictation in progress, read when a case is left (the hook is declared further down).
  const voiceBusyRef = useRef<() => boolean>(() => false);
  // The list reload after an empty case went to the trash (App passes a new closure on each render).
  const onNoteUpdatedRef = useRef(onNoteUpdated);
  const similarityRequestRef = useRef(0);
  const similarityQueryRef = useRef<string | null>(null);
  // Last known database version for the editable fields: the "unmodified" reference.
  const baselineRef = useRef<EditableState>(stateFromNote(note));
  // Last local state, kept up to date on every render: asynchronous closures (dictation, cleanup)
  // read the value actually displayed, never the one captured at mount.
  const localStateRef = useRef<EditableState>(stateFromNote(note));
  // Serializes writes of the same note: a more recent save cannot be
  // doubled by an older one still in flight (write order guaranteed).
  const saveChainRef = useRef<Promise<void>>(Promise.resolve());
  const savesInProgressRef = useRef(0);
  // Identifier targeted by the delete confirmation (never the note displayed by mistake).
  const confirmDeleteIdRef = useRef<string | null>(null);
  // True as soon as the user has edited the Title field: the automatic title (derived from the content on
  // manual input) must then never replace it again, even if the field is later emptied.
  const customTitleRef = useRef(false);
  // Space of the displayed note (Work/Private): filters similar notes and escrow drafts
  // so as to never offer the content of another space. `null` (unsorted note,
  // unknown projects) = no limit, historical behavior kept.
  const noteSpace = spaceOfNote(note, projects);
  // Space of the note currently **loaded**, remembered for asynchronous flushes: when switching
  // to another note, the outgoing draft must keep the space of its old note.
  const loadedNoteSpaceRef = useRef<ProjectSpace | null>(null);
  // Failed writes (draft or raw dictation), kept in the escrow and offered for recovery.
  const pending = usePendingWrites({ noteSpace, currentNoteIdRef, showToast, onNoteUpdated });
  const { recordDraftFailure, removeCoveredFailures, refresh: refreshPendingWrites } = pending;
  // Project of the note currently **loaded**, remembered for asynchronous flushes: it allows
  // the escrow space repair without the notes list (see effect below).
  const loadedNoteProjectRef = useRef<string | null>(null);

  // Space filtering. The displayed note imposes its space: never a title or a draft from another
  // space. `noteSpace == null` (unsorted note, unknown projects) = no limit.
  const displayedSimilarNotes = useMemo(() => {
    if (noteSpace == null) return similarNotes;
    return similarNotes.filter(sn => spaceOfNote(sn, projects) === noteSpace);
  }, [similarNotes, noteSpace, projects]);

  // Component mounted: after an `await`, never open a stream or a session if the editor has left.
  const mountedRef = useRef(true);

  /**
   * Persists the remaining draft (modified fields not yet written) **going through the write
   * chain**: an older write still in flight cannot apply after it
   * (safeguard no. 2). Used on note change, on unmount and on window close.
   *
   * The payload is recomputed **at execution time** against the last confirmed version (which may have
   * advanced behind a write in flight): a return to a previous value is not lost.
   * Only the fields actually written advance the reference.
   */
  const flushDraft = useCallback((): Promise<void> => {
    const base = baselineRef.current;
    if (!base.id) return Promise.resolve();
    const noteId = base.id;
    // Space of the note **still loaded** at flush time (captured before the new note
    // is installed by the synchronization effect).
    const space = loadedNoteSpaceRef.current;
    // Project of the same note: it attaches any failure to a repairable space without reopening the note.
    const projectId = loadedNoteProjectRef.current;
    // Snapshot of the draft at flush time: it stays valid even if the component state is
    // reloaded right after (note change).
    const local = localStateRef.current;
    const execute = async () => {
      const payload = buildPayload(base, local);
      if (Object.keys(payload).length === 0) return;
      // Capture the reference BEFORE the IPC: a note refresh may modify `base`
      // while the write waits for the locked database.
      const expectedValue = baseDuPayload(base, payload);
      try {
        await api.updateNote(noteId, payload);
        advanceBaseline(base, noteId, payload, local);
        removeCoveredFailures(noteId, payload);
      } catch (e) {
        // Never swallow the failure: the input is kept in a recoverable escrow, because
        // after unmount the component state no longer exists.
        console.error("Failed to save the draft", e);
        recordDraftFailure(noteId, payload, expectedValue, space, projectId);
      }
    };
    saveChainRef.current = trackEditorWrite(saveChainRef.current.then(execute, execute));
    return saveChainRef.current;
  }, [recordDraftFailure, removeCoveredFailures]);

  // Reflect the current render in the refs read by asynchronous closures (dictation, cleanup).
  // Written in an effect, never during render (React forbids mutating a ref during render).
  // Declared BEFORE the synchronization effect below, which reads `localStateRef`.
  useEffect(() => {
    currentNoteIdRef.current = note?.id ?? null;
    localStateRef.current = {
      id: note?.id ?? null,
      content,
      title,
      note_type: noteType,
      priority,
      due_at: dueAt,
      tags,
    };
  });

  /* eslint-disable react/set-state-in-effect -- deliberate prop→state synchronization: implements
     the "baseline + external adoption" safeguard (a field not modified locally adopts the value
     written by an MCP agent) and the resets on note change. Not expressible during render
     without mutating refs during render (forbidden by React). */
  useEffect(() => {
    // Another note (or none) is about to be displayed: the outgoing draft is saved BEFORE
    // its identifier and baseline are replaced. The previous autosave timer was just
    // cancelled by the cleanup: without this flush, the old note's input would be abandoned.
    const leftId = loadedNoteIdRef.current;
    if (leftId !== (note?.id ?? null)) {
      const flushed = flushDraft();
      // A new case left empty goes to the trash, once its last write has landed. The engine
      // checks it is still empty in the same transaction; a case being dictated into, one whose
      // write failed (escrow) or one opened again meanwhile is left alone.
      if (leftId) {
        void flushed.then(async () => {
          if (currentNoteIdRef.current === leftId || voiceBusyRef.current()) return;
          if (readPendingWrites().some(entry => entry.noteId === leftId)) return;
          try {
            if (await api.trashIfEmpty(leftId)) onNoteUpdatedRef.current();
          } catch (e) {
            console.error('Failed to tidy an empty case', e);
          }
        });
      }
    }

    if (!note) {
      loadedNoteIdRef.current = null;
      baselineRef.current = stateFromNote(null);
      setContent('');
      setTitle('');
      setTags([]);
      setPriority('');
      setDueAt('');
      setIsConfirmingDelete(false);
      setPendingTemplate(null);
      appliedTemplateRef.current = null;
      confirmDeleteIdRef.current = null;
      customTitleRef.current = false;
      setViewMode('edit');
      setSimilarNotes([]);
      setSuggestedType(null);
      setDismissedType(null);
      return;
    }

    const incoming = stateFromNote(note);

    // Another note: full load of the editable fields.
    if (loadedNoteIdRef.current !== note.id) {
      loadedNoteIdRef.current = note.id;
      baselineRef.current = incoming;
      setContent(incoming.content);
      setTitle(incoming.title);
      setNoteType(incoming.note_type);
      setPriority(incoming.priority);
      setDueAt(incoming.due_at);
      setTags(incoming.tags);
      setSuggestedType(null);
      setDismissedType(null);
      setIsConfirmingDelete(false);
      setPendingTemplate(null);
      appliedTemplateRef.current = null;
      confirmDeleteIdRef.current = null;
      customTitleRef.current = false;
      setViewMode('edit');
      setSimilarNotes([]);
      return;
    }

    // Same note refreshed from the database: only adopt the external value for fields not
    // modified locally. A field touched by the user is kept; a field modified
    // elsewhere (MCP agent, other instance) is adopted instead of being overwritten by autosave.
    const base = baselineRef.current;
    const local = localStateRef.current;
    if (equal(local.content, base.content) && !equal(base.content, incoming.content)) {
      base.content = incoming.content;
      setContent(incoming.content);
    }
    if (equal(local.title, base.title) && !equal(base.title, incoming.title)) {
      base.title = incoming.title;
      setTitle(incoming.title);
    }
    if (equal(local.note_type, base.note_type) && !equal(base.note_type, incoming.note_type)) {
      base.note_type = incoming.note_type;
      setNoteType(incoming.note_type);
    }
    if (equal(local.priority, base.priority) && !equal(base.priority, incoming.priority)) {
      base.priority = incoming.priority;
      setPriority(incoming.priority);
    }
    if (equal(local.due_at, base.due_at) && !equal(base.due_at, incoming.due_at)) {
      base.due_at = incoming.due_at;
      setDueAt(incoming.due_at);
    }
    if (equal(local.tags, base.tags) && !equal(base.tags, incoming.tags)) {
      base.tags = incoming.tags;
      setTags(incoming.tags);
    }
  }, [note, flushDraft]);
  /* eslint-enable react/set-state-in-effect */

  // Remembers the space of the loaded note for asynchronous closures (flush, escrow). Triggered
  // after the synchronization effect: on note change, the outgoing flush has already captured
  // the old space. Also covers the late arrival of projects (note displayed before its projects).
  /* eslint-disable react/set-state-in-effect -- self-repair of an escrowed draft: the
     reread of the escrow is an external system (memory + localStorage), setState only
     refreshes the display after this write. */
  useEffect(() => {
    if (loadedNoteIdRef.current === (note?.id ?? null)) {
      loadedNoteSpaceRef.current = noteSpace;
      loadedNoteProjectRef.current = note?.project_id || null;
    }
    // Self-repair of the escrow anchors (`pendingWriteRepairs`). Reading goes through the escrow
    // itself (memory + localStorage), the source of truth; `pending.pendingWrites` is only a trigger
    // (an entry created in flight is repaired on the next pass).
    const repairs = pendingWriteRepairs(readPendingWrites(), note?.id, note?.project_id, noteSpace, projects);
    // A single write for the whole pass; without change, no write (no loop:
    // after repair, the anchor matches the target).
    if (updatePendingWriteEntries(repairs)) refreshPendingWrites();
  }, [noteSpace, note?.id, note?.project_id, projects, pending.pendingWrites, refreshPendingWrites]);
  /* eslint-enable react/set-state-in-effect */

  const handleSave = async (newContent?: string, newTitle?: string, newType?: NoteType, newPriority?: string, newTags?: string[], newDue?: string) => {
    if (!note) return;
    const targetNoteId = note.id; // Locks the ID of the current note
    // Space of the targeted note, captured at request time: the escrow of a failure must
    // stay attached to the right space even if the displayed note changes before execution.
    const space = noteSpace;
    // Project of the targeted note: it allows the escrow space repair without reopening the note.
    const projectId = note.project_id || null;
    // Reference of the note at request time: the object keeps the last confirmed version
    // even if `baselineRef` is reassigned in the meantime (note change before execution).
    const base = baselineRef.current;
    // Fallback snapshot if the displayed note changed before execution.
    const snapshot = localStateRef.current;

    // Explicit overrides of the call (selector, dictation, template…): they take precedence over the local
    // state read at execution time, which may have advanced since.
    const overrides: Partial<EditableState> = {};
    if (newContent !== undefined) overrides.content = newContent;
    if (newTitle !== undefined) overrides.title = newTitle;
    if (newType !== undefined) overrides.note_type = newType;
    if (newPriority !== undefined) overrides.priority = newPriority;
    if (newTags !== undefined) overrides.tags = newTags;
    if (newDue !== undefined) overrides.due_at = newDue;

    const execute = async () => {
      // The payload is recomputed **at execution time**, against the last confirmed version: a
      // previous write still in flight may have advanced the reference, and a return to an
      // earlier value must then be re-emitted (otherwise the intermediate value would stay in the database).
      // The most recent local state is used so as not to rewrite a field adopted from an
      // external modification (MCP).
      const sameNote = baselineRef.current.id === targetNoteId;
      const local = sameNote ? localStateRef.current : snapshot;
      const eff: EditableState = {
        id: targetNoteId,
        content: overrides.content ?? local.content,
        title: overrides.title ?? local.title,
        note_type: overrides.note_type ?? local.note_type,
        priority: overrides.priority ?? local.priority,
        due_at: overrides.due_at ?? local.due_at,
        tags: overrides.tags ?? local.tags,
      };

      // Only send the fields actually modified: a view without changes writes
      // nothing (and does not inflate `updated_at`), and a field modified elsewhere is not overwritten.
      const payload = buildPayload(base, eff);
      if (Object.keys(payload).length === 0) return;
      const expectedValue = baseDuPayload(base, payload);

      // Only increment around a real write: a `return` without payload must never
      // leave the counter (and the "Saving…" indicator) stuck.
      savesInProgressRef.current += 1;
      setIsSaving(true);
      try {
        await api.updateNote(targetNoteId, payload);
        // The database now knows these values: only advance the fields actually written.
        advanceBaseline(base, targetNoteId, payload, eff);
        removeCoveredFailures(targetNoteId, payload);
        setHasSaved(true);
        if (savedTimer.current) clearTimeout(savedTimer.current);
        savedTimer.current = setTimeout(() => setHasSaved(false), 2000);
        // Reload the list only after a success: a failure must neither reread the database
        // (and thus risk overwriting the draft) nor suggest that the note is up to date.
        onNoteUpdated();
      } catch (e) {
        console.error("Failed to save the note", e);
        recordDraftFailure(targetNoteId, payload, expectedValue, space, projectId);
        showToast(t('editor.messages.saveFailed'), { duration: 6000 });
      } finally {
        savesInProgressRef.current -= 1;
        if (savesInProgressRef.current <= 0) setIsSaving(false);
      }
    };

    // Writes of the same note are queued one behind the other: the most recent applies
    // last, even if an older one was still in flight (write order guaranteed).
    saveChainRef.current = trackEditorWrite(saveChainRef.current.then(execute, execute));
    return saveChainRef.current;
  };

  // Stable reference to the latest `handleSave`: the autosave effect must not depend on
  // the function's identity (recreated on every render), otherwise the debounce would restart endlessly.
  // Updated in an effect (not during render) and without a dependency array: it follows
  // every render, well before the autosave timer (1.5 s) reads the reference.
  const handleSaveRef = useRef(handleSave);
  useEffect(() => {
    handleSaveRef.current = handleSave;
  });

  // Quick capture to the open note (0.9.83): the text joins the **local draft**, like
  // a dictation, then goes through the usual write chain (escrow on failure). An
  // input not yet saved is therefore never overwritten. The capture only clears its text after
  // this response; without a response (note closed in the meantime), it keeps it.
  const appendCaptureRef = useRef<(text: string) => Promise<void>>(async () => {});
  useEffect(() => {
    appendCaptureRef.current = async (text: string) => {
      const draft = localStateRef.current.content;
      const fresh = draft.trim() ? `${draft.replace(/\s+$/, '')}\n\n${text}` : text;
      setContent(fresh);
      const autoTitle = isDefaultTitle(localStateRef.current.title) ? deriveTitle(text) : null;
      if (autoTitle) setTitle(autoTitle);
      await handleSaveRef.current(fresh, autoTitle ?? undefined);
    };
  });
  useEffect(() => {
    let stopFn: UnlistenFn | null = null;
    let finished = false;
    void listen<{ requestId: string; noteId: string; text: string }>('humcase:qc-append', async event => {
      const { requestId, noteId, text } = event.payload ?? {};
      if (!requestId || !text?.trim() || !mountedRef.current || currentNoteIdRef.current !== noteId) return;
      await appendCaptureRef.current(text.trim());
      void emit('humcase:qc-append-result', { requestId, ok: true }).catch(() => {});
    }).then(stop => { if (finished) stop(); else stopFn = stop; }).catch(() => {
      // Outside Tauri (tests): no event listening.
    });
    return () => { finished = true; stopFn?.(); };
  }, []);

  const handleTagsChange = (newTags: string[]) => {
    setTags(newTags);
    handleSave(undefined, undefined, undefined, undefined, newTags);
  };

  /**
   * Manual content input: the title is derived from the first meaningful line **as long as**
   * the title has stayed the default and the user has not touched the Title field. Same derivation
   * as dictation (no AI call); editing the title freezes it for good.
   */
  const handleContentChange = (newMarkdown: string) => {
    setContent(newMarkdown);
    if (customTitleRef.current) return;
    if (!isDefaultTitle(localStateRef.current.title)) return;
    const derive = deriveTitle(newMarkdown);
    if (derive && derive !== localStateRef.current.title) setTitle(derive);
  };

  // Recomputed only when the notes list changes (before: JSON re-parse on every render).
  const availableTags = useMemo(
    () => Array.from(new Set(allNotes.flatMap(n => readTags(n.tags)))),
    [allNotes]
  );

  // Autosave & Duplicate Detection debounced
  useEffect(() => {
    if (!note) return;
    const searchableTitle = isDefaultTitle(title) ? '' : title;
    const fullText = [searchableTitle, content].filter(Boolean).join(' ').trim();
    // A title taken from the text would count its words twice for the nature.
    const wordingText = searchableTitle && deriveTitle(content) !== searchableTitle ? fullText : content.trim();
    const queryKey = fullText.length > 10 ? JSON.stringify([note.id, fullText]) : null;
    const requestId = ++similarityRequestRef.current;

    // A suggestion only stays displayed for the text that produced it. The title created by
    // default is not user content and must trigger no search.
    if (similarityQueryRef.current !== queryKey) {
      setSimilarNotes([]);
      setSuggestedType(null);
    }
    similarityQueryRef.current = queryKey;

    const timeoutId = setTimeout(async () => {
      // Autosave first: never make it wait for the duplicate search (a slow
      // search must neither delay nor reorder writes). The payload only contains the
      // fields actually modified since the last known database version.
      const local = localStateRef.current;
      const base = baselineRef.current;
      if (EDITABLE_FIELDS.some(field => !equal(local[field], base[field]))) {
        void handleSaveRef.current();
      }

      // Duplicate Detection. The search only receives the title written by the user and the
      // content; an empty note with the title "Nouvelle note" stays silent.
      if (queryKey) {
        const noteId = note.id;
        try {
           // The nature is read from the wording without AI (Rust `recognize`), only for a
           // plain thought: a nature chosen by the person is never questioned.
           const [similar, recognition] = await Promise.all([
             api.findSimilarNotes(fullText, noteId),
             noteType === 'note' ? api.recognizeCase(wordingText).catch(() => null) : Promise.resolve(null),
           ]);
           // A text change also invalidates the response, even if the note ID stays the same.
           const local = localStateRef.current;
           const latestText = [isDefaultTitle(local.title) ? '' : local.title, local.content].filter(Boolean).join(' ').trim();
           if (requestId !== similarityRequestRef.current || currentNoteIdRef.current !== noteId ||
               local.id !== noteId || latestText !== fullText) return;
           setSimilarNotes(similar);

           if (noteType === 'note') {
             const suggested = recognition?.nature?.value ?? null;
             setSuggestedType(suggested && suggested !== 'note' && suggested !== dismissedType ? suggested : null);
           }
        } catch (e) {
          if (requestId === similarityRequestRef.current && currentNoteIdRef.current === note.id) console.error(e);
        }
      }

    }, 1500);
    return () => {
      clearTimeout(timeoutId);
      if (similarityRequestRef.current === requestId) similarityRequestRef.current += 1;
    };
    // All reactive values read by the effect are listed. `handleSave` is read via
    // `handleSaveRef` (stable identity): the debounce does not restart on every render.
  }, [content, title, noteType, priority, dueAt, tags, note, dismissedType]);

  const confirmDelete = async () => {
    const target = confirmDeleteIdRef.current;
    setIsConfirmingDelete(false);
    confirmDeleteIdRef.current = null;
    // The target is the one that requested the confirmation, never the note displayed by mistake.
    if (!note || !target || target !== note.id) return;
    const deleted = note;
    try {
      // Typing not yet saved goes in first: a trashed note is frozen, so the autosave after the
      // deletion would fail and leave a pending write on a note in the trash.
      await flushDraft();
      await api.deleteNote(target);
      onNoteUpdated();
      showToast(t('editor.messages.trashed', { title: deleted.title || t('common.untitled') }), { undoNote: deleted, duration: 8000 });
    } catch (e) {
      console.error('Failed to delete the note', e);
      showToast(t('editor.messages.deleteFailed'));
    }
  };

  const handlePasteImage = async (file: File): Promise<string | null> => {
    try {
      const buffer = await file.arrayBuffer();
      // Portable reference `humcase-image://<name>`: Tiptap's Image node inserts it as is
      // in the note and only translates it into a local URL for display.
      return await api.saveImageToDisk(new Uint8Array(buffer));
    } catch (err) {
      // A silent failure suggested the paste had produced nothing: we report it.
      console.error("Error while saving the image:", err);
      showToast(t('editor.messages.imagePasteFailed'), { duration: 6000 });
      return null;
    }
  };

  /**
   * Restores a previous version of the content (undo of a structuring, history). Same
   * transactional path as apply: the backend refuses if the text changed in the meantime,
   * and the replaced version joins the history in turn. The raw text is never touched.
   */
  const restoreVersion = async (noteId: string, expected: string | null, version: string) => {
    const execute = async () => {
      if (currentNoteIdRef.current !== noteId || localStateRef.current.id !== noteId) {
        throw new Error(t('editor.messages.errorReopen'));
      }
      const current = expected ?? localStateRef.current.content;
      if (localStateRef.current.content !== current) {
        throw new Error(t('editor.messages.errorTextChanged'));
      }
      const saved = await api.applyStructuredNote(noteId, current, version);
      if (currentNoteIdRef.current === noteId) {
        baselineRef.current.content = saved.content;
        if (localStateRef.current.content === current) {
          localStateRef.current.content = saved.content;
          setContent(saved.content);
        }
      }
      onNoteUpdated();
    };
    try {
      await flushDraft();
      saveChainRef.current = trackEditorWrite(saveChainRef.current.then(execute, execute));
      await saveChainRef.current;
      if (currentNoteIdRef.current === noteId) showToast(t('editor.messages.versionRestored'));
    } catch (e) {
      if (currentNoteIdRef.current === noteId) showToast(t('editor.messages.versionRestoreFailed', { error: String(e) }), { duration: 6000 });
    }
  };

  const openRelatedNote = async (target: Note) => {
    const sourceId = note?.id;
    if (!sourceId || !onOpenRelatedNote) return;
    if (voice.isBusy()) {
      throw new Error(t('editor.messages.errorFinishDictation'));
    }
    await flushDraft();
    if (currentNoteIdRef.current !== sourceId) return;
    if (readPendingWrites().some(entry => entry.noteId === sourceId && entry.kind !== 'task-draft')) {
      throw new Error(t('editor.messages.errorDraftNotSaved'));
    }
    await onOpenRelatedNote(target);
  };

  const structure = useStructure({
    note, voiceBusy: () => voice.isBusy(), currentNoteIdRef, localStateRef, baselineRef, saveChainRef, flushDraft, restoreVersion, setContent, showToast, onNoteUpdated,
  });
  const { structureOpen, structureFrame, requestStructure } = structure;
  const germination = useGermination({ note, flushDraft, onNoteUpdated });
  // A nudge's "Sprout" opens the case and asks for its steps at once, once per request. Only
  // once the editor shows this case (its baseline and its loaded fields): before that, the
  // fields still hold the previous case's values and the flush before sprouting would write
  // them into this one.
  const germinatedOnOpenRef = useRef<string | null>(null);
  useEffect(() => {
    if (germinateOnOpen === null) { germinatedOnOpenRef.current = null; return; }
    if (!note || germinateOnOpen !== note.id || germinatedOnOpenRef.current === note.id) return;
    const base = baselineRef.current;
    const local = localStateRef.current;
    if (base.id !== note.id || local.id !== note.id) return;
    if (Object.keys(buildPayload(base, local)).length > 0) return;
    germinatedOnOpenRef.current = note.id;
    onGerminateStarted?.();
    void germination.germinate();
  }, [note, germinateOnOpen, germination, onGerminateStarted, title, content, noteType, priority, dueAt, tags]);
  const voice = useDictation({
    note, noteSpace, structureOpen, structureFrame, mountedRef, currentNoteIdRef, localStateRef, baselineRef, saveChainRef,
    flushDraft, handleSave, restoreVersion, setContent, setTitle, showToast, refreshPendingWrites, onNoteUpdated,
  });
  const { isRecording, isProcessingVoice, startRecording, stopRecording, reportStopFailure, persistRemainderForSession, releaseOnUnmount } = voice;
  const voiceActive = voice.isActive;
  useEffect(() => { voiceBusyRef.current = voice.isBusy; onNoteUpdatedRef.current = onNoteUpdated; });

  const prepareBackup = useCallback(async (): Promise<BackupPreparation> => {
    if (structureOpen) {
      return { ready: false, reason: t('editor.messages.backupStructureOpen') };
    }
    if (voiceActive()) {
      return { ready: false, reason: t('editor.messages.backupVoiceActive') };
    }
    // Captures the draft immediately, before the autosave debounce ends.
    await flushDraft();
    if (voiceActive()) {
      return { ready: false, reason: t('editor.messages.backupVoiceActive') };
    }
    return { ready: true };
  }, [flushDraft, voiceActive, structureOpen, t]);

  useEffect(() => {
    onRegisterPrepareBackup?.(prepareBackup);
    return () => onRegisterPrepareBackup?.(null);
  }, [onRegisterPrepareBackup, prepareBackup]);



  // Unmount cleanup: never leave the microphone, the Deepgram session or a timer behind.
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (savedTimer.current) clearTimeout(savedTimer.current);
      // Dictation session interrupted by an unmount (view change, close): the part
      // already received is saved right away; the Deepgram end will bring the last fragment.
      persistRemainderForSession();
      // Draft waiting for autosave: chained behind writes in flight, and never swallowed
      // (a failure is kept in the recoverable escrow, not lost without a trace).
      void flushDraft();
      releaseOnUnmount();
    };
  }, [flushDraft, persistRemainderForSession, releaseOnUnmount]);

  // Window close (cross / shutdown): the unmount flush is not enough, React may
  // not unmount (the window is hidden) or the application may be quit. So we flush the
  // draft and the accumulated dictation as soon as the close is requested, before the window disappears.
  useEffect(() => {
    const flushBeforeClose = () => {
      // The dictation is NOT stopped (the window is only hidden): we only persist the
      // part not yet written and remember `alreadyPersisted`, so that the normal end of the
      // dictation does not append the same text twice.
      persistRemainderForSession();
      void flushDraft();
    };

    window.addEventListener('beforeunload', flushBeforeClose);
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void getCurrentWindow()
      .onCloseRequested(() => {
        // The window is intercepted on the Rust side (hiding): we flush just before, without preventing
        // the close, so as not to break the "minimize to the notification area" behavior.
        flushBeforeClose();
      })
      .then(stop => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        /* Window API unavailable (outside Tauri): `beforeunload` remains the safety net. */
      });

    return () => {
      disposed = true;
      window.removeEventListener('beforeunload', flushBeforeClose);
      unlisten?.();
    };
  }, [flushDraft, persistRemainderForSession]);

  if (!note) {
    return (
      <div className="note-editor-empty">
        {toast}
        <EmptyState size="sm" description={t('editor.empty.select')} />
      </div>
    );
  }

  const applyTemplate = (type: string) => {
    let tpl = "";
    if (type === "bug") {
      tpl = t('editor.templates.bug.body');
    } else if (type === "pr") {
      tpl = t('editor.templates.pr.body');
    } else if (type === "adr") {
      tpl = t('editor.templates.adr.body');
    }
    if (!tpl) return;
    const name = type === 'bug' ? t('editor.templates.bug.name') : type === 'pr' ? t('editor.templates.pr.name') : t('editor.templates.adr.name');
    const sameTemplate = appliedTemplateRef.current?.noteId === note.id && appliedTemplateRef.current.text.trim() === content.trim();
    if (content.trim() && !sameTemplate) {
      setPendingTemplate({ name, text: tpl });
      return;
    }
    appliedTemplateRef.current = { noteId: note.id, text: tpl };
    setContent(tpl);
    handleSave(tpl);
  };

  return (
    <div ref={rootRef} className="note-editor flex-1 flex flex-col bg-background h-full overflow-hidden relative">
      {toast}
      <article className="note-editor-document" aria-label={t('editor.header.document')}>
      <NoteEditorHeader
        note={note}
        isMiniMode={isMiniMode}
        onBackToList={onBackToList}
        pathSlot={pathSlot}
        allNotes={allNotes}
        onOpenNote={onOpenRelatedNote ? openRelatedNote : undefined}
        title={title}
        onTitleChange={value => { customTitleRef.current = true; setTitle(value); }}
        onTitleBlur={value => handleSave(undefined, value)}
        isSaving={isSaving}
        hasSaved={hasSaved}
        journal={caseJournal.journal}
        onReviewed={() => { caseJournal.reload(); onNoteUpdated(); }}
        onJumpToDetails={() => { supportRef.current?.scrollIntoView({ block: 'start' }); supportRef.current?.focus({ preventScroll: true }); }}
        similarNotes={displayedSimilarNotes}
        focusTitle={focusTitleOnOpen}
        onTitleEnter={() => setBodyFocusSignal(signal => signal + 1)}
      />

      <DecisionHistory noteId={note.id} isDecision={note.note_type === 'decision'} revision={note.revision ?? 0} allNotes={allNotes}
        onOpen={onOpenRelatedNote ? target => { openRelatedNote(target).catch(error => showToast(String(error instanceof Error ? error.message : error))); } : undefined}
        onChanged={onNoteUpdated} />

      <div className="note-editor-writing">

      <NoteToolbar
        hasRaw={Boolean(note.raw_content)}
        viewMode={viewMode}
        onViewModeChange={setViewMode}
        aiReady={aiReady}
        canStructure={Boolean(content.trim()) && !isRecording && !isProcessingVoice && !structureOpen}
        onStructure={() => { void requestStructure(); }}
        canGerminate={Boolean(content.trim()) && !isRecording && !isProcessingVoice && !germination.busy}
        germinating={germination.stage === 'open' || germination.stage === 'reading'}
        onGerminate={() => { void germination.germinate(); }}
        onTemplate={noteSpace === 'work' ? applyTemplate : undefined}
        isRecording={isRecording}
        isProcessingVoice={isProcessingVoice}
        onStartDictation={startRecording}
        onStopDictation={() => { void stopRecording().catch(reportStopFailure); }}
      />

      <GerminatePanel germination={germination} />

      {pending.displayed.length > 0 && (
        <PendingWritesPanel
          writes={pending.displayed}
          persistentWrites={pendingWritesPersistent()}
          recoveryInProgress={pending.recoveryInProgress}
          recoveryProblems={pending.recoveryProblems}
          onRetry={() => { void pending.retry(); }}
          onIgnore={pending.ignore}
        />
      )}

      <div className="note-editor-body flex-1 p-6 flex flex-col overflow-hidden relative">
        {isRecording && voice.voiceTargetNoteId === note?.id && (
          <DictationOverlay liveTranscript={voice.liveTranscript} interimTranscript={voice.interimTranscript} />
        )}

        {viewMode === 'edit' && (
          <TiptapEditor
            // `key` per note: Tiptap's undo history is **isolated per note**. Without
            // this, Ctrl+Z in B could undo a modification of A (same instance reused,
            // `setContent(..., { emitUpdate: false })` does not clear the history). The key stays
            // stable when switching List ↔ Kanban for the **same** note: undo is preserved there.
            key={note.id}
            value={content}
            noteId={note.id}
            onChange={handleContentChange}
            onBlur={() => handleSave()}
            onPasteImage={handlePasteImage}
            passages={germination.marks}
            reveal={germination.reveal}
            autoFocus={focusOnOpen && !focusTitleOnOpen}
            focusSignal={bodyFocusSignal}
          />
        )}

        {viewMode === 'diff' && <CompareView raw={note.raw_content} content={content} />}
      </div>
      </div>
      </article>
      <aside ref={supportRef} tabIndex={-1} className="note-editor-support" aria-label={t('editor.support.label')}>
        <section className="note-editor-properties">
          <NoteProperties
            note={note}
            onNoteUpdated={onNoteUpdated}
            priority={priority}
            onPriorityChange={newP => {
              setPriority(newP);
              handleSave(undefined, undefined, undefined, newP);
            }}
            dueAt={dueAt}
            onDueChange={newDue => {
              if (newDue && !isIsoDay(newDue)) { showToast(t('editor.messages.invalidDate')); return; }
              setDueAt(newDue);
              handleSave(undefined, undefined, undefined, undefined, undefined, newDue);
            }}
            onClearDue={() => {
              setDueAt('');
              handleSave(undefined, undefined, undefined, undefined, undefined, '');
            }}
            noteType={noteType}
            onTypeChange={newT => {
              setNoteType(newT);
              handleSave(undefined, undefined, newT, undefined);
            }}
            suggestedType={suggestedType}
            onAcceptSuggestion={type => {
              setNoteType(type);
              setSuggestedType(null);
              handleSave(undefined, undefined, type);
            }}
            onDismissSuggestion={type => {
              setDismissedType(type);
              setSuggestedType(null);
            }}
            tags={tags}
            onTagsChange={handleTagsChange}
            availableTags={availableTags}
          />

        {/* Added mr-12 when isMiniMode is active to avoid overlapping with the pin */}
        <NoteActions
          isMiniMode={isMiniMode}
          archived={Boolean(note.archived_at)}
          onCopy={async () => {
            const textToCopy = `# ${title}\n\n${content}`;
            // Success is only announced after the clipboard write is confirmed.
            try {
              await navigator.clipboard.writeText(textToCopy);
              showToast(t('editor.messages.copied'));
            } catch {
              showToast(t('editor.messages.copyFailed'));
            }
          }}
          onToggleArchive={async () => {
            const archive = !note.archived_at;
            try {
              await api.setNoteArchived(note.id, archive);
              showToast(archive ? t('editor.messages.archived') : t('editor.messages.unarchived'));
              onNoteUpdated();
            } catch (cause) {
              showToast(t(archive ? 'editor.messages.archiveFailed' : 'editor.messages.unarchiveFailed', { error: String(cause) }));
            }
          }}
          isConfirmingDelete={isConfirmingDelete}
          onAskDelete={() => { confirmDeleteIdRef.current = note.id; setIsConfirmingDelete(true); }}
          onConfirmDelete={confirmDelete}
          onCancelDelete={() => setIsConfirmingDelete(false)}
          subCases={isConfirmingDelete ? subCaseCount([note.id], allNotes) : 0}
        />
        </section>
        <h2 className="note-editor-section-label">{t('editor.support.heading')}</h2>
        {onOpenRelatedNote && <NoteSubCases key={`sub-cases-${note.id}`} note={note} allNotes={allNotes} onOpenNote={openRelatedNote} onChanged={onNoteUpdated} onBeforeWrite={flushDraft} />}
        <NoteReport key={`report-${note.id}`} note={note} onUpdated={onNoteUpdated} space={noteSpace} />
        {onOpenRelatedNote && <NoteRelations key={`relations-${note.id}`} note={note} allNotes={allNotes} onOpenNote={openRelatedNote} onChanged={onNoteUpdated} space={noteSpace} />}
        {/* One history: who did what, then the versions that can be restored. */}
        <CaseActivity journal={caseJournal.journal} failed={caseJournal.failed}>
          <NoteHistory key={`history-${note.id}`} note={note} embedded journalEvents={caseJournal.journal?.events} onRestore={version => { void restoreVersion(note.id, null, version); }} />
        </CaseActivity>
      </aside>

      {pendingTemplate && (
        <Dialog onClose={() => setPendingTemplate(null)} size="sm" describedBy="note-template-desc">
          <DialogHeader title={t('editor.templates.replaceTitle', { name: pendingTemplate.name })} description={<p id="note-template-desc">{t('editor.templates.replaceDescription')}</p>} />
          <DialogFooter>
            <Button variant="secondary" onClick={() => setPendingTemplate(null)}>{t('editor.templates.keep')}</Button>
            <Button onClick={() => { const next = pendingTemplate.text; setPendingTemplate(null); appliedTemplateRef.current = { noteId: note.id, text: next }; setContent(next); handleSave(next); }}>{t('editor.templates.replace')}</Button>
          </DialogFooter>
        </Dialog>
      )}
      {structureOpen && (
        <StructureDialog
          onClose={structure.closeStructure}
          structureBusy={structure.structureBusy}
          structureProposal={structure.structureProposal}
          structureError={structure.structureError}
          structuringContextLabel={structuringContextLabel}
          structureFrame={structureFrame}
          onFrameChange={structure.changeFrame}
          content={content}
          onProposalChange={structure.editProposal}
          onApply={() => { void structure.applyStructure(); }}
        />
      )}
    </div>
  );
}

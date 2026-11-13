import { TextField, SelectField } from './components/ui/Controls';
import { useWorkspaceTheme } from './hooks/useWorkspaceTheme';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import './workspace-theme.css';
import './components/ui/kit.css';
import './quick-capture.css';
import './game-object.css';
import React, { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from './i18n';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen, emit } from '@tauri-apps/api/event';
import { api } from './services/tauriApi';
import { NOTE_TYPES, type Project, type NoteType } from './types';
import { TYPE_LABELS } from './utils/notePresentation';
import { getLastProjectId, setLastProjectId } from './utils/prefs';
import { loadSpacePreferences, type ProjectSpace } from './utils/spaces';
import { readQuickCaptureState, writeQuickCaptureState, quickCapturePersistenceUnavailable, type QuickCaptureState } from './utils/quickCaptureBarrier';

/** "Open note" destination: the text joins the note displayed in the main window. */
const OPEN_NOTE = 'open-note';
type OpenNote = { id: string; title: string } | null;

/**
 * Asks the main window to append `text` to the note `noteId` and waits for its confirmation.
 * Without an answer (note closed, main window unavailable), the promise is rejected and the
 * capture keeps its text.
 */
async function appendToOpenNote(noteId: string, text: string, delayMs = 4000): Promise<void> {
  const requestId = crypto.randomUUID();
  let stopFn: (() => void) | undefined;
  try {
    const confirmed = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(i18n.t('quickCapture.noteNoAnswer'))), delayMs);
      void listen<{ requestId: string; ok: boolean }>('humcase:qc-append-result', event => {
        if (event.payload?.requestId !== requestId) return;
        clearTimeout(timer);
        if (event.payload.ok) resolve();
        else reject(new Error(i18n.t('quickCapture.appendRefused')));
      }).then(stop => { stopFn = stop; }).catch(reject);
    });
    await emit('humcase:qc-append', { requestId, noteId, text });
    await confirmed;
  } finally {
    stopFn?.();
  }
}

function initialCapture(): QuickCaptureState {
  try {
    return readQuickCaptureState() ?? { title: '', noteType: 'note', projectId: '', submitting: false };
  } catch {
    return { title: '', noteType: 'note', projectId: '', submitting: false };
  }
}

export default function QuickCaptureApp() {
  const { t } = useTranslation();
  const theme = useWorkspaceTheme();
  const windowTitle = t('quickCapture.windowTitle');
  // The native window title is static English in the configuration; follow the interface language.
  useEffect(() => {
    void getCurrentWindow().setTitle(windowTitle).catch(() => {});
  }, [windowTitle]);
  const [initial] = useState(initialCapture);
  const captureRef = useRef<QuickCaptureState>(initial);
  const [title, setTitle] = useState(initial.title);
  const [noteType, setNoteType] = useState<NoteType>(initial.noteType);
  const [projectId, setProjectId] = useState<string>(initial.projectId);
  const [projects, setProjects] = useState<Project[]>([]);
  const [openNote, setOpenNote] = useState<OpenNote>(null);
  const openNoteRef = useRef<OpenNote>(null);
  const [captureSpaces, setCaptureSpaces] = useState<ProjectSpace[]>(['work', 'private']);
  const [message, setMessage] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  // Local storage may refuse the write: typing continues (in-memory fallback) but the
  // draft will not be persisted, which must be visible rather than discovered later.
  // The initial state comes from the module: a write may have failed before the first render.
  const [persistenceUnavailable, setPersistenceUnavailable] = useState(() => quickCapturePersistenceUnavailable());
  const inputRef = useRef<HTMLInputElement>(null);
  // Last persistence value published to the main window: we only emit on changes.
  const publishedPersistenceRef = useRef<boolean | null>(null);
  // Synchronous lock: two presses of Enter (or auto-repeat) must not create two notes.
  const submittingRef = useRef(false);

  // M2: reports to the main window (via a global Tauri event) whether the Quick Capture state
  // is persisted. The restore is fail-closed as long as this information is missing on one side or
  // the other (cross-window in-memory fallback).
  const publishPersistence = useCallback((refused: boolean) => {
    if (publishedPersistenceRef.current === refused) return;
    publishedPersistenceRef.current = refused;
    try {
      void emit('humcase:qc-persistence', { refused }).catch(() => {
        // Best effort: the absence of a recipient (main window not mounted) is not an error.
      });
    } catch {
      // Events API unavailable (outside Tauri): the barrier is left as is.
    }
  }, []);

  useEffect(() => {
    publishPersistence(quickCapturePersistenceUnavailable());
  }, [publishPersistence]);

  const updateCapture = (change: Partial<QuickCaptureState>) => {
    captureRef.current = { ...captureRef.current, ...change };
    writeQuickCaptureState(captureRef.current);
    const refused = quickCapturePersistenceUnavailable();
    setPersistenceUnavailable(refused);
    publishPersistence(refused);
  };

  // Reading projects. Replayed on every focus return: a project created in the main
  // window since the last opening must be offered without restarting Humcase.
  const loadProjects = useCallback(async () => {
    try {
      const projs = await api.getProjects();
      const preferences = loadSpacePreferences();
      const spaces: ProjectSpace[] = preferences.usage === 'both' ? ['work', 'private'] : [preferences.usage];
      setCaptureSpaces(spaces);
      const visible = projs.filter(p => !p.is_inbox && (p.space == null || spaces.includes(p.space)));
      setProjects(visible);
      // Keep the current choice if it still exists, otherwise the last used project, otherwise the first.
      setProjectId(prev => {
        const preferred = prev;
        const last = getLastProjectId();
        const isAvailable = (id: string | null) => Boolean(id && ((id === OPEN_NOTE && openNoteRef.current) || visible.some(p => p.id === id) || spaces.some(space => id === `inbox:${space}`)));
        const fallbackSpace = preferences.scope === 'private' && spaces.includes('private') ? 'private' : spaces[0];
        const selected = isAvailable(preferred) ? preferred : isAvailable(last) ? last! : `inbox:${fallbackSpace}`;
        captureRef.current = { ...captureRef.current, projectId: selected };
        return selected;
      });
      setMessage(null);
    } catch (e) {
      console.error("Could not load projects", e);
      setMessage(t('quickCapture.loadError'));
    }
  }, [t]);

  useEffect(() => {
    // Initial load triggered outside the synchronous body of the effect (asynchronous read).
    void Promise.resolve().then(() => loadProjects());

    // On focus return: refocus the input **and** reread the projects.
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused) return;
      inputRef.current?.focus();
      void loadProjects();
    });
    // Note displayed in the main window: announced on every change, requested again on
    // opening and on focus return.
    const unlistenOpenNote = listen<OpenNote>('humcase:open-note', event => {
      const note = event.payload ?? null;
      openNoteRef.current = note;
      setOpenNote(note);
      // The open note was closed: back to the chosen destination rather than an empty choice.
      if (!note && captureRef.current.projectId === OPEN_NOTE) void loadProjects();
    });
    const requestOpenNote = () => { void emit('humcase:open-note-query').catch(() => {}); };
    void unlistenOpenNote.then(requestOpenNote).catch(() => {});
    const unlistenFocusNote = getCurrentWindow().onFocusChanged(({ payload: focused }) => { if (focused) requestOpenNote(); });

    return () => {
      unlisten.then(f => f());
      unlistenOpenNote.then(f => f()).catch(() => {});
      unlistenFocusNote.then(f => f()).catch(() => {});
    };
  }, [loadProjects]);

  // Remember the current project as soon as it is defined (including the loading default value).
  useEffect(() => {
    // "Open note" depends on the main window: it is not a choice to remember.
    if (projectId && projectId !== OPEN_NOTE) setLastProjectId(projectId);
  }, [projectId]);


  const handleKeyDown = async (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      // The window is hidden, not destroyed: the draft must stay recoverable.
      updateCapture({ title: captureRef.current.title, submitting: submittingRef.current });
      await getCurrentWindow().hide();
      return;
    }
    if (e.key !== 'Enter' || !title.trim()) return;

    // A validation already in progress ignores the second Enter key: before, auto-repeat created
    // an identical note per press.
    if (submittingRef.current) return;
    if (!projectId) {
      setMessage(t('quickCapture.chooseDestination'));
      return;
    }

    submittingRef.current = true;
    updateCapture({ submitting: true });
    setIsSubmitting(true);
    const submittedTitle = title.trim();
    try {
      if (projectId === OPEN_NOTE) {
        if (!openNote) throw new Error(t('quickCapture.noOpenNote'));
        await appendToOpenNote(openNote.id, submittedTitle);
        if (captureRef.current.title.trim() === submittedTitle) {
          updateCapture({ title: '' });
          setTitle('');
        }
        setMessage(null);
        await getCurrentWindow().hide();
        return;
      }
      const destination = projectId === 'inbox:work' || projectId === 'inbox:private'
        ? await api.getOrCreateInbox(projectId.slice(6) as ProjectSpace)
        : projectId;
      await api.createNote({
        project_id: destination,
        title: submittedTitle,
        content: '',
        note_type: noteType,
      });
      // Only clear the field if it still matches the sent version: input started
      // during the write must not be erased. (Input is disabled anyway
      // during sending; the guard covers any other modification path.)
      if (captureRef.current.title.trim() === submittedTitle) {
        updateCapture({ title: '' });
        setTitle('');
      }
      setMessage(null);
      await getCurrentWindow().hide();
    } catch (err) {
      console.error("Could not create the note", err);
      setMessage(projectId === OPEN_NOTE
        ? t('quickCapture.appendFailed', { message: err instanceof Error ? err.message : String(err) })
        : t('quickCapture.createFailed', { error: String(err) }));
    } finally {
      submittingRef.current = false;
      updateCapture({ submitting: false });
      setIsSubmitting(false);
    }
  };

  return (
    <div className="workspace-app quick-capture" data-theme={theme}>
      <div className="quick-capture__panel">
        <div className="quick-capture__heading">
          <span>{t('quickCapture.heading')}</span>
          <span className="quick-capture__hint">{t('quickCapture.closeHint')}</span>
        </div>

        <TextField
          aria-label={t('quickCapture.textLabel')}
          ref={inputRef}
          type="text"
          value={title}
          onChange={(e) => { updateCapture({ title: e.target.value }); setTitle(e.target.value); }}
          onKeyDown={handleKeyDown}
          disabled={isSubmitting}
          placeholder={isSubmitting ? t('quickCapture.saving') : t('quickCapture.placeholder')}
          className="quick-capture__input"
          autoFocus
        />

        {persistenceUnavailable && (
          <p role="status" className="quick-capture__message">
            {t('quickCapture.storageUnavailable')}
          </p>
        )}

        {message && (
          <p role="status" className="quick-capture__message">{message}</p>
        )}

        <div className="quick-capture__destinations">
          <SelectField
            aria-label={t('quickCapture.destinationLabel')}
            value={projectId}
            onChange={(e) => { updateCapture({ projectId: e.target.value }); setProjectId(e.target.value); if (e.target.value !== OPEN_NOTE) setLastProjectId(e.target.value); }}
            disabled={isSubmitting}

          >
            {openNote && <option value={OPEN_NOTE}>{t('quickCapture.openNoteOption', { title: openNote.title || t('common.untitled') })}</option>}
            {captureSpaces.map(space => <option key={space} value={`inbox:${space}`}>{t('common.inboxOf', { space: t(space === 'work' ? 'common.space.work' : 'common.space.private') })}</option>)}
            {projects.map(p => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </SelectField>

          <SelectField
            aria-label={t('quickCapture.typeLabel')}
            value={noteType}
            onChange={(e) => { updateCapture({ noteType: e.target.value as NoteType }); setNoteType(e.target.value as NoteType); }}
            disabled={isSubmitting || projectId === OPEN_NOTE}
            title={projectId === OPEN_NOTE ? t('quickCapture.typeLockedTitle') : undefined}

          >
            {NOTE_TYPES.map(type => <option key={type} value={type}>{TYPE_LABELS[type]}</option>)}
          </SelectField>
        </div>
      </div>
    </div>
  );
}

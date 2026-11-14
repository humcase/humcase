import { useCallback, useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PencilSimpleLine } from '@phosphor-icons/react';
import type { ExcalidrawImperativeAPI, AppState, BinaryFiles } from '@excalidraw/excalidraw/types';
import type { Note, Project, UpdateNoteInput } from '../../types';
import { api } from '../../services/tauriApi';
import { trackEditorWrite, type BackupPreparation } from '../../utils/backupBarrier';
import { addPendingWrite, pendingWritesPersistent, removePendingWrite } from '../../utils/pendingWrites';
import { MAX_SKETCH_BYTES, excalidrawLanguage, sketchScene, webAddress } from '../../utils/caseContent';
import { spaceOfNote } from '../noteEditor/editableState';
import { loadExcalidraw } from './loadExcalidraw';
import { onBeforeExit } from '../../utils/beforeExit';
import './sketch.css';

type ExcalidrawModule = Awaited<ReturnType<typeof loadExcalidraw>>;
type Elements = Parameters<ExcalidrawModule['getSceneVersion']>[0];
type Scene = { elements: Elements; appState: AppState; files: BinaryFiles };
type SaveState = 'saved' | 'waiting' | 'saving' | 'error';

/** Pause after the last stroke before the drawing is written. */
const SAVE_DELAY = 700;
/** A failed write is tried again after this pause, a few times, while the drawing stays open. */
export const SKETCH_RETRY_DELAY = 5000;
export const SKETCH_RETRY_LIMIT = 5;

interface SketchEditorProps {
  note: Note;
  theme: 'dark' | 'light';
  projects?: Project[];
  onNoteUpdated: () => void;
  onRegisterPrepareBackup?: (prepare: (() => Promise<BackupPreparation>) | null) => void;
}

/**
 * A sketch case: Excalidraw on the whole page, its title above. The drawing is written a moment
 * after each change, before leaving the case, before a backup and when the window closes; a
 * write that fails is kept for recovery like the editor's. The Rust side refuses any content
 * that is not a drawing, so nothing else can replace it.
 */
export default function SketchEditor({ note, theme, projects, onNoteUpdated, onRegisterPrepareBackup }: SketchEditorProps) {
  const { t, i18n } = useTranslation();
  const [module, setModule] = useState<ExcalidrawModule | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [title, setTitle] = useState(note.title);
  const [saveState, setSaveState] = useState<SaveState>('saved');
  const [error, setError] = useState<string | null>(null);
  // The scene read when the case opened: a content that is not a drawing is shown, never replaced.
  const [initial] = useState(() => sketchScene(note.content));

  const noteId = note.id;
  const space = spaceOfNote(note, projects);
  const anchor = useRef({ space, projectId: note.project_id });
  const apiRef = useRef<ExcalidrawImperativeAPI | null>(null);
  const moduleRef = useRef<ExcalidrawModule | null>(null);
  const sceneRef = useRef<Scene | null>(null);
  const versionRef = useRef<string | null>(null);
  const dirtyRef = useRef(false);
  const titleRef = useRef(note.title);
  const saved = useRef({ content: note.content, title: note.title });
  // Highest version of the case read so far: an older read arriving late is never shown.
  const seenRevision = useRef(note.revision);
  const timer = useRef<number | undefined>(undefined);
  // Copies of this drawing kept after a failed write: superseded once the drawing on screen is written.
  const keptEntries = useRef(new Set<string>());
  const retries = useRef(0);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  // The retry calls the current `flush` through this reference.
  const flushRef = useRef<() => Promise<unknown>>(() => Promise.resolve());
  const chain = useRef<Promise<unknown>>(Promise.resolve());
  const onUpdated = useRef(onNoteUpdated);
  useEffect(() => { onUpdated.current = onNoteUpdated; anchor.current = { space, projectId: note.project_id }; });

  useEffect(() => {
    let alive = true;
    loadExcalidraw().then(loaded => {
      if (!alive) return;
      moduleRef.current = loaded;
      setModule(loaded);
    }, cause => { if (alive) setLoadError(String(cause)); });
    return () => { alive = false; };
  }, []);

  /** Writes what changed since the last write, after the writes already started. */
  const flush = useCallback((): Promise<unknown> => {
    window.clearTimeout(timer.current);
    const payload: UpdateNoteInput = {};
    const loaded = moduleRef.current, scene = sceneRef.current;
    if (dirtyRef.current && loaded && scene) {
      const content = loaded.serializeAsJSON(scene.elements, scene.appState, scene.files, 'local');
      if (content.length > MAX_SKETCH_BYTES) {
        setSaveState('error');
        setError(t('workspace.sketch.tooLarge'));
        return chain.current;
      }
      dirtyRef.current = false;
      if (content !== saved.current.content) payload.content = content;
    }
    if (titleRef.current !== saved.current.title) payload.title = titleRef.current;
    if (payload.content === undefined && payload.title === undefined) {
      setSaveState(state => (state === 'waiting' ? 'saved' : state));
      return chain.current;
    }
    setSaveState('saving');
    const write = chain.current.then(async () => {
      // What the database holds before this write, for a recovery that must not overwrite a newer change.
      const base: Partial<UpdateNoteInput> = {};
      if (payload.content !== undefined) base.content = saved.current.content;
      if (payload.title !== undefined) base.title = saved.current.title;
      try {
        await api.updateNote(noteId, payload);
        saved.current = { ...saved.current, ...payload } as typeof saved.current;
        retries.current = 0;
        // Everything on screen is in the database now: the copies kept after a failure are superseded.
        if (!dirtyRef.current && titleRef.current === saved.current.title) {
          for (const id of keptEntries.current) removePendingWrite(id);
          keptEntries.current.clear();
        }
        setSaveState(dirtyRef.current ? 'waiting' : 'saved');
        setError(null);
        onUpdated.current();
      } catch (cause) {
        keptEntries.current.add(addPendingWrite({ kind: 'update', noteId, payload, base, space: anchor.current.space, projectId: anchor.current.projectId }).id);
        // The drawing still has to be written: the next stroke, leaving, closing or the retry sends it again.
        if (payload.content !== undefined) dirtyRef.current = true;
        setSaveState('error');
        setError(t(pendingWritesPersistent() ? 'workspace.sketch.saveError' : 'workspace.sketch.saveErrorMemory', { error: String(cause) }));
        if (mounted.current && retries.current < SKETCH_RETRY_LIMIT) {
          retries.current += 1;
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(() => { void flushRef.current(); }, SKETCH_RETRY_DELAY);
        }
      }
    });
    chain.current = trackEditorWrite(write);
    return write;
  }, [noteId, t]);
  useEffect(() => { flushRef.current = flush; });

  const schedule = useCallback(() => {
    setSaveState(state => (state === 'error' ? state : 'waiting'));
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => { void flush(); }, SAVE_DELAY);
  }, [flush]);

  // Leaving the case (another case, the board, a hidden editor) writes the drawing first.
  useEffect(() => () => { void flush(); }, [flush]);

  // Closing the window hides it: the drawing is written as the close is asked.
  useEffect(() => {
    const flushBeforeClose = () => { void flush(); };
    window.addEventListener('beforeunload', flushBeforeClose);
    // An update quits Humcase without closing the window: it waits for this write.
    const stopBeforeExit = onBeforeExit(flush);
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void getCurrentWindow()
      .onCloseRequested(flushBeforeClose)
      .then(stop => { if (disposed) stop(); else unlisten = stop; })
      .catch(() => { /* Outside Tauri: `beforeunload` remains the safety net. */ });
    return () => {
      disposed = true;
      window.removeEventListener('beforeunload', flushBeforeClose);
      stopBeforeExit();
      unlisten?.();
    };
  }, [flush]);

  // A backup waits for the drawing to be written.
  useEffect(() => {
    if (!onRegisterPrepareBackup) return;
    onRegisterPrepareBackup(async () => {
      await flush();
      return { ready: true };
    });
    return () => onRegisterPrepareBackup(null);
  }, [flush, onRegisterPrepareBackup]);

  // A change made elsewhere (an agent, another window) shows here unless a drawing is waiting
  // to be written: then the drawing on screen is written over it, as in the editor.
  useEffect(() => {
    if (note.revision <= seenRevision.current || !module) return;
    seenRevision.current = note.revision;
    if (note.content !== saved.current.content && !dirtyRef.current) {
      const scene = sketchScene(note.content);
      const excalidraw = apiRef.current;
      if (scene && excalidraw) {
        const files = Object.values(scene.files) as Parameters<ExcalidrawImperativeAPI['addFiles']>[0];
        if (files.length) excalidraw.addFiles(files);
        excalidraw.updateScene({ elements: module.restoreElements(scene.elements as never, null) });
        versionRef.current = null;
        saved.current = { ...saved.current, content: note.content };
      }
    }
    if (note.title !== saved.current.title && titleRef.current === saved.current.title) {
      saved.current = { ...saved.current, title: note.title };
      titleRef.current = note.title;
      setTitle(note.title);
    }
  }, [note.content, note.title, note.revision, module]);

  const onChange = useCallback((elements: Elements, appState: AppState, files: BinaryFiles) => {
    const loaded = moduleRef.current;
    if (!loaded) return;
    sceneRef.current = { elements, appState, files };
    const version = `${loaded.getSceneVersion(elements)}:${Object.keys(files).length}:${appState.viewBackgroundColor}`;
    const first = versionRef.current === null;
    if (version === versionRef.current) return;
    versionRef.current = version;
    // The first change is the scene being loaded, not a stroke.
    if (first) return;
    dirtyRef.current = true;
    schedule();
  }, [schedule]);

  // Links (of an element, of Excalidraw's help or libraries) open in the browser, never in the app.
  const openOutside = (url: string | null | undefined) => {
    const address = url ? webAddress(url) : null;
    if (address) void api.openLink(address).catch(cause => setError(String(cause)));
  };
  const catchLinks = (event: ReactMouseEvent<HTMLDivElement>) => {
    const link = (event.target as HTMLElement).closest('a[href]');
    if (!link) return;
    event.preventDefault();
    openOutside(link.getAttribute('href'));
  };

  const stateLabel = { saved: t('workspace.sketch.saved'), waiting: t('workspace.sketch.waiting'), saving: t('workspace.sketch.saving'), error: t('workspace.sketch.notSaved') }[saveState];

  return (
    <div className="sketch-case">
      <header className="sketch-case__bar">
        <span className="sketch-case__kind"><PencilSimpleLine size={14} aria-hidden="true" />{t('workspace.board.kind.sketch')}</span>
        <input className="ui-field sketch-case__title" value={title} maxLength={240} aria-label={t('workspace.sketch.titleLabel')} placeholder={t('common.untitled')}
          onChange={event => { setTitle(event.target.value); titleRef.current = event.target.value; schedule(); }}
          onBlur={() => { void flush(); }} />
        <span className={`sketch-case__state is-${saveState}`} role="status">{stateLabel}</span>
      </header>
      {error && <p role="alert" className="sketch-case__error">{error}</p>}
      {!initial ? (
        <div className="sketch-case__unreadable" role="alert">
          <p>{t('workspace.sketch.unreadable')}</p>
          <pre>{note.content}</pre>
        </div>
      ) : (
        <div className="sketch-case__canvas" onClickCapture={catchLinks}>
          {loadError ? <p role="alert" className="sketch-case__error">{t('workspace.sketch.loadError', { error: loadError })}</p>
            : !module ? <p className="sketch-case__loading" role="status">{t('workspace.sketch.loading')}</p>
              : (
                <module.Excalidraw
                  excalidrawAPI={excalidraw => { apiRef.current = excalidraw; }}
                  initialData={{ elements: initial.elements as never, appState: { ...initial.appState, theme } as never, files: initial.files as never, scrollToContent: true }}
                  onChange={onChange}
                  theme={theme}
                  langCode={excalidrawLanguage(i18n.language)}
                  name={title || t('common.untitled')}
                  aiEnabled={false}
                  autoFocus
                  onLinkOpen={(element, event) => { event.preventDefault(); openOutside(element.link); }}
                  UIOptions={{ canvasActions: { loadScene: false, saveToActiveFile: false, toggleTheme: null, export: { saveFileToDisk: true } } }}>
                  <module.MainMenu>
                    <module.MainMenu.DefaultItems.Export />
                    <module.MainMenu.DefaultItems.SaveAsImage />
                    <module.MainMenu.DefaultItems.Help />
                    <module.MainMenu.DefaultItems.ClearCanvas />
                    <module.MainMenu.Separator />
                    <module.MainMenu.DefaultItems.ChangeCanvasBackground />
                  </module.MainMenu>
                  <module.WelcomeScreen>
                    <module.WelcomeScreen.Hints.ToolbarHint />
                    <module.WelcomeScreen.Hints.MenuHint />
                    <module.WelcomeScreen.Hints.HelpHint />
                    <module.WelcomeScreen.Center>
                      <module.WelcomeScreen.Center.Heading>{t('workspace.sketch.welcome')}</module.WelcomeScreen.Center.Heading>
                    </module.WelcomeScreen.Center>
                  </module.WelcomeScreen>
                </module.Excalidraw>
              )}
        </div>
      )}
    </div>
  );
}

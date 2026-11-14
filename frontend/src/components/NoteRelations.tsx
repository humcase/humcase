import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CaretDown as ChevronDown, LinkSimple as Link2, CircleNotch as Loader2, Plus, X } from '@phosphor-icons/react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Note, NoteRelations as Relations, ProjectSpace } from '../types';
import { api } from '../services/tauriApi';
import { saveTaskDraft, readTaskDraft, removeTaskDraft } from '../utils/pendingWrites';
import { LinkedTaskPlan } from './LinkedTaskPlan';
import { Button } from './ui/Button';
import { Count } from './ui/Badge';
import { EmptyState } from './ui/EmptyState';
import './NoteRelations.css';
import { onBeforeExit } from '../utils/beforeExit';

interface NoteRelationsProps {
  note: Note;
  allNotes: Note[];
  onOpenNote: (target: Note) => Promise<void> | void;
  onChanged: () => void;
  /** Space of the note, to never offer another space's draft. */
  space?: ProjectSpace | null;
}

const EMPTY: Relations = { sources: [], tasks: [] };

/** Explicit relations of the note. No status claims to observe an execution. */
export default function NoteRelations({ note, allNotes, onOpenNote, onChanged, space }: NoteRelationsProps) {
  const { t } = useTranslation();
  // Resume of the linked-task draft attached to this note (shared escrow). Read only once,
  // at component creation (recreated for each note by its `key`): the initial form state
  // follows from it, and an input in progress is never overwritten if the space arrives later.
  const [initialDraft] = useState(() => {
    const draft = readTaskDraft(note.id, space);
    return draft && (draft.titre.trim() || draft.contenu.trim()) ? draft : null;
  });
  const [relations, setRelations] = useState<Relations>(EMPTY);
  const [expanded, setExpanded] = useState(note.note_type === 'todo');
  const [form, setForm] = useState<'create' | 'attach' | null>(initialDraft ? 'create' : null);
  const [title, setTitle] = useState(initialDraft?.titre ?? '');
  const [content, setContent] = useState(initialDraft?.contenu ?? '');
  const [targetId, setTargetId] = useState('');
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [restoredDraft, setRestoredDraft] = useState(initialDraft !== null);
  const aliveRef = useRef(false);
  const epochRef = useRef(0);
  const pendingRef = useRef(false);

  // The component is recreated on note change: on unmount, the creation form input
  // is kept in the escrow, attached to the source note and to its space.
  const draftRef = useRef({ title: '', content: '', open: false });
  useEffect(() => {
    draftRef.current = { title, content, open: form === 'create' };
  });
  useEffect(() => () => {
    const { title: draftTitle, content: draftContent, open } = draftRef.current;
    if (open) saveTaskDraft(note.id, draftTitle, draftContent, space, note.project_id);
  }, [note.id, space, note.project_id]);

  // Window close: React is not unmounted (the window is hidden). So the open input
  // is escrowed on the close request, before hiding. `saveTaskDraft`
  // replaces the entry of the same note/space: a possible unmount does not create a duplicate.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    const escrow = () => {
      const { title: draftTitle, content: draftContent, open } = draftRef.current;
      if (open) saveTaskDraft(note.id, draftTitle, draftContent, space, note.project_id);
    };
    // An update quits Humcase without closing the window nor unmounting: escrow there too.
    const stopBeforeExit = onBeforeExit(escrow);
    try {
      void getCurrentWindow()
        .onCloseRequested(escrow)
        .then(stop => { if (disposed) stop(); else unlisten = stop; })
        .catch(() => { /* Window API unavailable (outside Tauri): unmounting stays the safety net. */ });
    } catch {
      /* Window API unavailable (outside Tauri): unmounting stays the safety net. */
    }
    return () => { disposed = true; stopBeforeExit(); unlisten?.(); };
  }, [note.id, space, note.project_id]);

  const refresh = useCallback(async () => {
    const epoch = ++epochRef.current;
    if (!aliveRef.current || epoch !== epochRef.current) return;
    try {
      const result = await api.getNoteRelations(note.id);
      if (aliveRef.current && epoch === epochRef.current) {
        setRelations(result);
        setError(null);
      }
    } catch (cause) {
      if (aliveRef.current && epoch === epochRef.current) setError(t('notes.relations.loadError', { error: String(cause) }));
    } finally {
      if (aliveRef.current && epoch === epochRef.current) setLoading(false);
    }
  }, [note.id, t]);

  useEffect(() => {
    aliveRef.current = true;
    return () => { aliveRef.current = false; epochRef.current += 1; };
  }, [refresh]);

  // App reloads the list on return to the foreground: also reread the persisted links, without
  // closing the form or the panel. An MCP relation can change without the note changing.
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => { if (!cancelled) void refresh(); });
    return () => { cancelled = true; };
  }, [allNotes, refresh]);

  useEffect(() => {
    const onFocus = () => { void refresh(); };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [refresh]);

  const run = async (action: () => Promise<void>) => {
    if (pendingRef.current) return;
    pendingRef.current = true;
    setPending(true);
    setError(null);
    try {
      await action();
    } catch (cause) {
      if (aliveRef.current) setError(String(cause));
    } finally {
      pendingRef.current = false;
      if (aliveRef.current) setPending(false);
    }
  };

  const afterChange = async () => {
    onChanged();
    await refresh();
  };

  const create = () => run(async () => {
    const cleanTitle = title.trim();
    if (!cleanTitle) throw new Error(t('notes.relations.titleRequired'));
    await api.createLinkedTask(note.id, cleanTitle, content.trim() || undefined);
    // Successful creation: the escrowed draft (same space) is consumed.
    removeTaskDraft(note.id, space);
    if (!aliveRef.current) { onChanged(); return; }
    setForm(null);
    setTitle('');
    setContent('');
    setRestoredDraft(false);
    await afterChange();
  });

  const attach = () => run(async () => {
    if (!targetId) throw new Error(t('notes.relations.chooseSameProject'));
    await api.linkNoteTask(note.id, targetId);
    if (!aliveRef.current) { onChanged(); return; }
    setForm(null);
    setTargetId('');
    await afterChange();
  });

  const unlink = (sourceId: string, taskId: string) => run(async () => {
    await api.unlinkNoteTask(sourceId, taskId);
    await afterChange();
  });

  const open = (target: Note) => run(async () => { await onOpenNote(target); });

  const cancel = () => {
    if (pending) return;
    // Explicit abandon: the draft attached to this note (same space) no longer needs to be resumed.
    removeTaskDraft(note.id, space);
    setForm(null);
    setTitle('');
    setContent('');
    setTargetId('');
    setError(null);
    setRestoredDraft(false);
  };

  const currentTasks = relations.tasks.map(task => allNotes.find(candidate => candidate.id === task.id) ?? task);
  const currentSources = relations.sources.map(source => allNotes.find(candidate => candidate.id === source.id) ?? source);
  const candidates = allNotes.filter(candidate =>
    candidate.id !== note.id &&
    candidate.project_id === note.project_id &&
    candidate.note_type === 'todo' &&
    !relations.tasks.some(task => task.id === candidate.id)
  );
  const doneCount = currentTasks.filter(task => task.status === 'resolved').length;

  return (
    <section className="note-relations" aria-label={t('notes.relations.sectionLabel')}>
      <button type="button" className="note-relations-heading" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>
        <span className="note-relations-heading-title"><Link2 size={15} /> {t('notes.relations.heading')} <Count value={loading ? '…' : relations.tasks.length} /></span>
        <span className="note-relations-heading-meta">{!loading && relations.tasks.length > 0 && t('notes.relations.resolvedCount', { count: doneCount })} {relations.sources.length > 0 && t('notes.relations.sourceCount', { count: relations.sources.length })}</span>
        <ChevronDown size={15} className={expanded ? 'note-relations-chevron is-open' : 'note-relations-chevron'} />
      </button>
      {expanded && (
        <div className="note-relations-body">
          {error && <p role="alert" className="note-relations-error">{error}</p>}
          {loading ? <p className="note-relations-muted"><Loader2 size={14} className="animate-spin" /> {t('notes.relations.loading')}</p> : (
            <>
              {currentSources.length > 0 && (
                <div className="note-relations-group">
                  <h3>{t('notes.relations.sourcesHeading')}</h3>
                  <ul>{currentSources.map(source => <li key={source.id}>
                    <button type="button" className="note-relations-link" disabled={pending || form !== null} onClick={() => { void open(source); }}>{source.title || t('common.untitled')}</button>
                    <Button type="button" variant="ghost" size="icon-sm" data-tone="danger" className="note-relations-unlink" disabled={pending || form !== null} aria-label={t('notes.relations.unlinkNoteLabel', { title: source.title || t('common.untitled') })} title={t('notes.relations.unlinkTitle')} onClick={() => { void unlink(source.id, note.id); }}><X size={14} /></Button>
                  </li>)}</ul>
                </div>
              )}
              <div className="note-relations-group">
                <h3>{t('notes.relations.tasksHeading')}</h3>
                {currentTasks.length === 0 ? <EmptyState size="sm" className="note-relations-empty" description={t('notes.relations.noTasks')} /> : <LinkedTaskPlan tasks={currentTasks} disabled={pending || form !== null} onOpen={task => { void open(task); }} onUnlink={task => { void unlink(note.id, task.id); }} />}
              </div>
              {!form && <div className="note-relations-actions">
                <Button type="button" variant="secondary" size="sm" disabled={pending} onClick={() => { setForm('create'); setError(null); }}><Plus size={14} aria-hidden="true" />{t('notes.relations.createTask')}</Button>
                <Button type="button" variant="secondary" size="sm" disabled={pending || candidates.length === 0} onClick={() => { setForm('attach'); setError(null); }}>{t('notes.relations.attachTask')}</Button>
              </div>}
              {form === 'create' && <form className="note-relations-form" onSubmit={event => { event.preventDefault(); void create(); }}>
                {restoredDraft && <p className="note-relations-muted" role="status">{t('notes.relations.draftRestored')}</p>}
                <label>{t('notes.relations.taskTitle')}<input className="ui-field" autoFocus value={title} onChange={event => setTitle(event.target.value)} disabled={pending} maxLength={240} /></label>
                <label>{t('notes.relations.taskContent')}<textarea value={content} onChange={event => setContent(event.target.value)} disabled={pending} rows={2} /></label>
                <div className="note-relations-actions"><Button type="submit" size="sm" disabled={pending || !title.trim()}>{pending ? t('notes.relations.creating') : t('notes.relations.createAndLink')}</Button><Button type="button" variant="ghost" size="sm" onClick={cancel} disabled={pending}>{t('notes.relations.cancel')}</Button></div>
              </form>}
              {form === 'attach' && <form className="note-relations-form" onSubmit={event => { event.preventDefault(); void attach(); }}>
                <label>{t('notes.relations.existingTask')}<select className="ui-field" value={targetId} onChange={event => setTargetId(event.target.value)} disabled={pending}><option value="">{t('notes.relations.chooseTask')}</option>{candidates.map(candidate => <option key={candidate.id} value={candidate.id}>{candidate.title || t('common.untitled')}</option>)}</select></label>
                <div className="note-relations-actions"><Button type="submit" size="sm" disabled={pending || !targetId}>{pending ? t('notes.relations.attaching') : t('notes.relations.attach')}</Button><Button type="button" variant="ghost" size="sm" onClick={cancel} disabled={pending}>{t('notes.relations.cancel')}</Button></div>
              </form>}
            </>
          )}
        </div>
      )}
    </section>
  );
}

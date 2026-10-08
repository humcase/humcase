import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowLineUp, CaretDown as ChevronDown, CaretRight, CircleNotch as Loader2, Plus, SquaresFour } from '@phosphor-icons/react';
import type { CaseContext, Note } from '../types';
import { api } from '../services/tauriApi';
import { formatDateTime } from '../utils/datetime';
import { directSubCases } from '../utils/subCases';
import { Button } from './ui/Button';
import { Count } from './ui/Badge';
import { EmptyState } from './ui/EmptyState';
import './NoteRelations.css';
import './note-sub-cases.css';

const EMPTY: CaseContext = { path: [], provenance: [] };
/** Provenances Humcase writes itself, named in the interface's language; an agent's kind shows as it wrote it. */
const OWN_PROVENANCE_KINDS = ['ask', 'source', 'germinated'] as const;
const isOwnProvenanceKind = (kind: string): kind is typeof OWN_PROVENANCE_KINDS[number] => (OWN_PROVENANCE_KINDS as readonly string[]).includes(kind);

/**
 * Path and provenance of a case, reread when the notes list changes (an agent can move a case
 * or record a provenance without the open note changing on screen).
 */
function useCaseContext(noteId: string, allNotes: Note[]) {
  const [context, setContext] = useState<CaseContext>(EMPTY);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const epochRef = useRef(0);

  const refresh = useCallback(async () => {
    const epoch = ++epochRef.current;
    try {
      const result = await api.getCaseContext(noteId);
      if (epoch !== epochRef.current) return;
      setContext(result ?? EMPTY);
      setError(null);
    } catch (cause) {
      if (epoch === epochRef.current) setError(String(cause));
    } finally {
      if (epoch === epochRef.current) setLoading(false);
    }
  }, [noteId]);

  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => { if (!cancelled) void refresh(); });
    return () => { cancelled = true; epochRef.current += 1; };
  }, [allNotes, refresh]);

  return { context, loading, error, refresh };
}

/** Finds a case in the loaded notes, or reads it (an archived ancestor is not in the list). */
async function resolveCase(id: string, allNotes: Note[]): Promise<Note> {
  return allNotes.find(candidate => candidate.id === id) ?? await api.getNote(id);
}

interface CasePathProps {
  note: Note;
  allNotes: Note[];
  onOpenNote: (target: Note) => Promise<void> | void;
  /** In the topbar breadcrumb: each ancestor follows a slash, like the project before it. */
  crumbs?: boolean;
}

/** Where the open case sits: its ancestors, each one clickable. Nothing for a top-level case. */
export function CasePath({ note, allNotes, onOpenNote, crumbs = false }: CasePathProps) {
  const { t } = useTranslation();
  const { context } = useCaseContext(note.id, allNotes);
  const [error, setError] = useState<string | null>(null);
  if (!note.parent_id || context.path.length === 0) return null;
  const open = async (id: string) => {
    setError(null);
    try {
      await onOpenNote(await resolveCase(id, allNotes));
    } catch (cause) {
      setError(String(cause));
    }
  };
  return (
    <nav className="case-path" aria-label={t('notes.cases.pathLabel')}>
      <ol>
        {context.path.map(ancestor => (
          <li key={ancestor.id}>
            {crumbs && <span aria-hidden="true">/</span>}
            <button type="button" onClick={() => { void open(ancestor.id); }}>{ancestor.title || t('common.untitled')}</button>
            {!crumbs && <CaretRight size={12} aria-hidden="true" />}
          </li>
        ))}
      </ol>
      {error && <p role="alert" className="note-relations-error">{error}</p>}
    </nav>
  );
}

interface NoteSubCasesProps {
  note: Note;
  allNotes: Note[];
  onOpenNote: (target: Note) => Promise<void> | void;
  onChanged: () => void;
  /** Saves the editor's pending input before a write that advances the note's revision. */
  onBeforeWrite: () => Promise<void>;
}

/** Sub-cases of the open case and where it comes from. Creates a sub-case or moves the case up. */
export default function NoteSubCases({ note, allNotes, onOpenNote, onChanged, onBeforeWrite }: NoteSubCasesProps) {
  const { t } = useTranslation();
  const { context, loading, error: loadError, refresh } = useCaseContext(note.id, allNotes);
  const subCases = directSubCases(note.id, allNotes);
  const [expanded, setExpanded] = useState(true);
  const [creating, setCreating] = useState(false);
  const [title, setTitle] = useState('');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pendingRef = useRef(false);

  const run = async (action: () => Promise<void>) => {
    if (pendingRef.current) return;
    pendingRef.current = true;
    setPending(true);
    setError(null);
    try {
      await action();
    } catch (cause) {
      setError(String(cause));
    } finally {
      pendingRef.current = false;
      setPending(false);
    }
  };

  const create = () => run(async () => {
    const cleanTitle = title.trim();
    if (!cleanTitle) throw new Error(t('notes.cases.titleRequired'));
    const id = await api.createSubCase(note.id, cleanTitle);
    setCreating(false);
    setTitle('');
    onChanged();
    await onOpenNote(await api.getNote(id));
  });

  const open = (target: Note) => run(async () => { await onOpenNote(target); });

  // Up one level: into the grandparent, or back to top level for a direct sub-case.
  const parentPath = context.path;
  const moveUp = () => run(async () => {
    await onBeforeWrite();
    const grandparent = parentPath.length >= 2 ? parentPath[parentPath.length - 2].id : null;
    await api.moveCase(note.id, grandparent);
    onChanged();
    await refresh();
  });

  const nestedCount = (id: string) => allNotes.filter(candidate => candidate.parent_id === id && !candidate.deleted_at).length;

  return (
    <section className="note-relations note-sub-cases" aria-label={t('notes.cases.sectionLabel')}>
      <button type="button" className="note-relations-heading" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>
        <span className="note-relations-heading-title"><SquaresFour size={15} /> {t('notes.cases.heading')} <Count value={subCases.length} /></span>
        <span className="note-relations-heading-meta">{context.provenance.length > 0 && t('notes.cases.provenanceCount', { count: context.provenance.length })}</span>
        <ChevronDown size={15} className={expanded ? 'note-relations-chevron is-open' : 'note-relations-chevron'} />
      </button>
      {expanded && (
        <div className="note-relations-body">
          {(error || loadError) && <p role="alert" className="note-relations-error">{error ?? t('notes.cases.loadError', { error: loadError })}</p>}
          <div className="note-relations-group">
            {subCases.length === 0 ? <EmptyState size="sm" className="note-relations-empty" description={t('notes.cases.empty')} /> : (
              <ul>{subCases.map(subCase => {
                const nested = nestedCount(subCase.id);
                return (
                  <li key={subCase.id}>
                    <button type="button" className="note-relations-link" disabled={pending} onClick={() => { void open(subCase); }}>{subCase.title || t('common.untitled')}</button>
                    {nested > 0 && <span className="note-relations-status">{t('notes.cases.nestedCount', { count: nested })}</span>}
                  </li>
                );
              })}</ul>
            )}
          </div>
          {!creating && <div className="note-relations-actions">
            <Button type="button" variant="secondary" size="sm" disabled={pending} onClick={() => { setCreating(true); setError(null); }}><Plus size={14} aria-hidden="true" />{t('notes.cases.create')}</Button>
            {note.parent_id && <Button type="button" variant="ghost" size="sm" disabled={pending || loading} onClick={() => { void moveUp(); }}><ArrowLineUp size={14} aria-hidden="true" />{parentPath.length >= 2 ? t('notes.cases.moveUp', { title: parentPath[parentPath.length - 2].title || t('common.untitled') }) : t('notes.cases.moveToTop')}</Button>}
          </div>}
          {creating && <form className="note-relations-form" onSubmit={event => { event.preventDefault(); void create(); }}>
            <label>{t('notes.cases.newTitle')}<input className="ui-field" autoFocus value={title} onChange={event => setTitle(event.target.value)} disabled={pending} maxLength={240} /></label>
            <div className="note-relations-actions">
              <Button type="submit" size="sm" disabled={pending || !title.trim()}>{pending ? t('notes.cases.creating') : t('notes.cases.createAndOpen')}</Button>
              <Button type="button" variant="ghost" size="sm" disabled={pending} onClick={() => { setCreating(false); setTitle(''); setError(null); }}>{t('notes.cases.cancel')}</Button>
            </div>
          </form>}
          {loading ? <p className="note-relations-muted"><Loader2 size={14} className="animate-spin" /> {t('notes.cases.loading')}</p> : context.provenance.length > 0 && (
            <div className="note-relations-group case-provenance">
              <h3>{t('notes.cases.provenanceHeading')}</h3>
              <ul>{context.provenance.map(item => (
                <li key={item.id}>
                  <div>
                    <p><span className="case-provenance-kind">{isOwnProvenanceKind(item.kind) ? t(`notes.cases.provenanceKind.${item.kind}`) : item.kind}</span> {item.label ?? item.url}</p>
                    {item.label && item.url && <p className="case-provenance-url">{item.url}</p>}
                    {item.excerpt && <blockquote>{item.excerpt}</blockquote>}
                    <p className="case-provenance-meta">{item.agent ? t('notes.cases.recordedByAgent', { agent: item.agent, date: formatDateTime(item.created_at) }) : t('notes.cases.recordedByPerson', { date: formatDateTime(item.created_at) })}</p>
                  </div>
                </li>
              ))}</ul>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

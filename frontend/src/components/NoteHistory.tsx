import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from '../i18n';
import { Clock as Clock3 } from '@phosphor-icons/react';
import type { CaseEvent, Note, NoteRevision } from '../types';
import { parseTimestamp } from '../utils/datetime';
import { api } from '../services/tauriApi';
import { formatDateTime, formatRelative } from '../utils/datetime';
import { Notice } from './ui/Toast';
import { Button } from './ui/Button';
import './NoteHistory.css';

interface NoteHistoryProps {
  note: Note;
  /** Restores a previous content (versions saved by a structuring). */
  onRestore?: (content: string) => void;
  /**
   * Inside the case's history, under its journal: no heading of its own, and an agent edit the
   * journal already lists (recorded by both within two seconds) is not repeated. A version that can
   * be restored is always shown.
   */
  embedded?: boolean;
  journalEvents?: CaseEvent[];
}

/** Translation keys of the fields modified by an agent (keys of the Rust snapshot `fields`). */
const FIELD_KEYS = {
  title: 'notes.history.fieldTitle',
  content: 'notes.history.fieldContent',
  tags: 'notes.history.fieldTags',
  priority: 'notes.history.fieldPriority',
  due_at: 'notes.history.fieldDue',
  note_type: 'notes.history.fieldType',
  status: 'notes.history.fieldStatus',
} as const;

/**
 * Keys of the `fields` snapshot, tolerant of an unreadable future snapshot.
 *
 * The format is a JSON object produced by the backend: corrupted content or an unexpected type
 * (array, `null`, invalid JSON) must never crash the display of the note.
 */
function modifiedKeys(fields: string): string[] {
  try {
    const parsed: unknown = JSON.parse(fields);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return Object.keys(parsed);
    }
  } catch {
    /* Unreadable snapshot: no key can be derived, the row stays displayable. */
  }
  return [];
}

/** Label of a field in the current language; an unknown key is displayed as is (never translated by mistake). */
function fieldLabel(key: string): string {
  return Object.hasOwn(FIELD_KEYS, key) ? i18n.t(FIELD_KEYS[key as keyof typeof FIELD_KEYS]) : key;
}

/** Previous content of a structuring version, or `null` if the snapshot does not carry it. */
function previousContent(fields: string): string | null {
  try {
    const parsed: unknown = JSON.parse(fields);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      const content = (parsed as Record<string, unknown>).content;
      return typeof content === 'string' ? content : null;
    }
  } catch {
    /* Unreadable snapshot: no restore offered. */
  }
  return null;
}

/**
 * Compact history of modifications attributed to an agent on the note page.
 *
 * Loading is non-blocking: as long as no revision is known (loading, failure or note
 * never edited by an agent), the component renders nothing: never an empty section.
 */
export default function NoteHistory({ note, onRestore, embedded = false, journalEvents = [] }: NoteHistoryProps) {
  const { t } = useTranslation();
  // `state` carries the note the revisions relate to: a result for another note
  // (or a note that was left) is never displayed. No synchronous reset in the effect: the
  // `noteId` comparison is enough and avoids a cascading render.
  const [state, setState] = useState<{ noteId: string; revisions: NoteRevision[]; failed?: boolean }>({ noteId: note.id, revisions: [] });
  const aliveRef = useRef(false);
  const epochRef = useRef(0);

  useEffect(() => {
    aliveRef.current = true;
    return () => {
      aliveRef.current = false;
      epochRef.current += 1;
    };
  }, []);

  // Reloaded on note change **and on every revision**: an external modification (MCP agent
  // while the note is open, another window) increments `revision` and must be visible without
  // re-navigation. A delayed response from a previous note or revision is ignored by
  // the `epoch`: the history of one note must never display under another.
  useEffect(() => {
    const epoch = ++epochRef.current;
    api
      .getNoteHistory(note.id)
      .then(result => {
        if (aliveRef.current && epoch === epochRef.current) setState({ noteId: note.id, revisions: result });
      })
      .catch(cause => {
        // Non-blocking: the note page stays usable, but the failure is stated: it must not
        // pass for a note never modified by an agent.
        console.error('Note history unavailable', cause);
        if (aliveRef.current && epoch === epochRef.current) setState({ noteId: note.id, revisions: [], failed: true });
      });
    return () => {
      epochRef.current += 1;
    };
  }, [note.id, note.revision]);

  const revisions = state.noteId === note.id ? state.revisions : [];
  // The backend already returns the most recent first; we sort again for safety so that the
  // displayed order does not depend on the transport (increasing revision).
  const time = (value: string) => parseTimestamp(value)?.getTime() ?? 0;
  const inJournal = (revision: NoteRevision) => revision.source !== 'structure' && journalEvents.some(event =>
    time(revision.created_at) >= time(event.created_at) - 2000 && time(revision.created_at) <= time(event.last_at) + 2000);
  const orderedRevisions = [...revisions].filter(revision => !embedded || !inJournal(revision)).sort((a, b) => b.revision - a.revision);
  if (state.noteId === note.id && state.failed) {
    if (embedded) return <Notice tone="danger">{t('notes.history.agentError')}</Notice>;
    return (
      <section className="note-history" aria-label={t('notes.history.agentLabel')}>
        <h3 className="note-history-heading"><Clock3 size={15} aria-hidden="true" /> {t('notes.history.agentLabel')}</h3>
        <Notice tone="danger">{t('notes.history.agentError')}</Notice>
      </section>
    );
  }
  if (orderedRevisions.length === 0) return null;
  const list = (
      <ul className="note-history-list" aria-label={embedded ? t('notes.history.versions') : undefined}>
        {orderedRevisions.map(revision => {
          const labels = modifiedKeys(revision.fields).map(fieldLabel);
          return (
            <li key={revision.id} className="note-history-item">
              <span className="note-history-date" title={formatDateTime(revision.created_at)}>
                {formatRelative(revision.created_at)}
              </span>
              <span className="note-history-fields">
                {labels.length > 0
                  ? t('notes.history.fields', { fields: labels.join(', ') })
                  : t('notes.history.noDetail')}
              </span>
              <span className="note-history-source">
                {revision.source === 'agent' ? t('notes.history.byAgent') : revision.source === 'structure' ? t('notes.history.replaced') : revision.source}
              </span>
              {revision.source === 'structure' && onRestore && previousContent(revision.fields) !== null &&
                <Button variant="link" size="sm" className="note-history-restore" onClick={() => onRestore(previousContent(revision.fields)!)}>{t('notes.history.restore')}</Button>}
            </li>
          );
        })}
      </ul>
  );
  if (embedded) return list;

  return (
    <section className="note-history" aria-label={t('notes.history.label')}>
      <h3 className="note-history-heading"><Clock3 size={15} aria-hidden="true" /> {t('notes.history.heading')}</h3>
      {list}
    </section>
  );
}

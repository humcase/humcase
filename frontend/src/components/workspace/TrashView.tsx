import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowCounterClockwise, Trash } from '@phosphor-icons/react';
import { api } from '../../services/tauriApi';
import type { Note, Project } from '../../types';
import { Button } from '../ui/Button';
import { PageHeader } from './PageHeader';
import { Dialog, DialogFooter, DialogHeader } from '../ui/Dialog';
import { EmptyState } from '../ui/EmptyState';
import { List, ListRow } from '../ui/List';
import { TRASH_RETENTION_DAYS, daysBeforeErasure, trashRows } from '../../utils/trash';
import { currentDateLocale } from '../../i18n';

type Confirmation = { kind: 'one'; note: Note; subCases: number } | { kind: 'all' };


/**
 * Trash (0.9.82): deleted notes, restorable unchanged for 30 days. Permanently erasing
 * (one note or the whole trash) asks for a confirmation tied to the target.
 */
export function TrashView({ projects, onChanged, onMessage }: {
  projects: Project[];
  /** Rereads the application notes after a restore. */
  onChanged: () => void;
  onMessage: (message: string, tone?: 'danger' | 'success') => void;
}) {
  const { t } = useTranslation();
  const shortDate = new Intl.DateTimeFormat(currentDateLocale(), { day: 'numeric', month: 'short' });
  const [notes, setNotes] = useState<Note[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [inProgress, setInProgress] = useState(false);
  // The trash changed since "Empty the trash" was confirmed: the dialog shows the new count.
  const [trashChanged, setTrashChanged] = useState(false);

  const load = useCallback(async () => {
    try {
      setNotes(await api.getTrashedNotes());
      setError(null);
    } catch (cause) {
      setError(t('trash.loadError', { error: String(cause) }));
    }
  }, [t]);
  // First load: the state is only written when the response arrives, and not at all if the
  // view was left in the meantime.
  useEffect(() => {
    let active = true;
    api.getTrashedNotes().then(
      read => { if (active) { setNotes(read); setError(null); } },
      cause => { if (active) setError(t('trash.loadError', { error: String(cause) })); },
    );
    return () => { active = false; };
  }, [t]);

  const rows = trashRows(notes ?? []);

  const projectName = (id: string) => {
    const project = projects.find(p => p.id === id);
    if (!project) return t('trash.unknownProject');
    return project.is_inbox ? t('common.inboxOf', { space: t(project.space === 'private' ? 'common.space.private' : 'common.space.work') }) : project.name;
  };

  const restore = async (note: Note) => {
    setInProgress(true);
    try {
      await api.restoreFromTrash(note.id);
      const subCases = rows.find(row => row.note.id === note.id)?.subCases ?? 0;
      onMessage(subCases > 0
        ? t('trash.restoredWithSubCases', { title: note.title, project: projectName(note.project_id), count: subCases })
        : t('trash.restored', { title: note.title, project: projectName(note.project_id) }), 'success');
      onChanged();
      await load();
    } catch (cause) {
      onMessage(t('trash.restoreError', { error: String(cause) }));
    } finally {
      setInProgress(false);
    }
  };

  const erase = async () => {
    if (!confirmation) return;
    setInProgress(true);
    try {
      if (confirmation.kind === 'one') {
        await api.purgeNote(confirmation.note.id);
        onMessage(t('trash.purgedNote', { title: confirmation.note.title || t('common.untitled') }), 'success');
      } else {
        // The confirmation holds for the cases it counted: a case trashed since (an undo window
        // that lapsed, another window, an agent) is shown first, never erased unseen.
        const fresh = await api.getTrashedNotes();
        const shown = new Set((notes ?? []).map(note => note.id));
        if (fresh.length !== shown.size || fresh.some(note => !shown.has(note.id))) {
          setNotes(fresh);
          setTrashChanged(true);
          return;
        }
        // Only what was shown is erased, even if a case reaches the trash meanwhile.
        const n = await api.emptyTrash([...shown]);
        onMessage(t('trash.purgedCount', { count: n }), 'success');
      }
      setConfirmation(null);
      setTrashChanged(false);
      await load();
    } catch (cause) {
      onMessage(t('trash.purgeError', { error: String(cause) }));
    } finally {
      setInProgress(false);
    }
  };

  return (
    <div className="workspace-scroll"><div className="workspace-page workspace-home workspace-trash">
      <PageHeader eyebrow={t('trash.eyebrow')} title={t('trash.title')} count={notes ? rows.length : null} description={t('trash.intro', { days: TRASH_RETENTION_DAYS })}
        action={notes && notes.length > 0 && <Button type="button" variant="outline" data-tone="danger" disabled={inProgress} onClick={() => setConfirmation({ kind: 'all' })}>{t('trash.emptyAll')}</Button>} />
      {error && <div role="alert" className="ui-dialog__error">{error}</div>}
      {notes === null && !error && <div className="workspace-loading" role="status">{t('trash.loading')}</div>}
      {notes?.length === 0 && <EmptyState icon={<Trash />} title={t('trash.emptyTitle')} />}
      {notes && notes.length > 0 && <List label={t('trash.listLabel')}>
        {rows.map(({ note, subCases }) => {
          const days = note.deleted_at ? daysBeforeErasure(note.deleted_at) : TRASH_RETENTION_DAYS;
          return <ListRow key={note.id} title={note.title || t('common.untitled')}
            meta={<>{projectName(note.project_id)} · {note.deleted_at && <>{t('trash.deletedOn', { date: shortDate.format(new Date(note.deleted_at)) })} · </>}{days <= 1 ? t('trash.purgeNextLaunch') : t('trash.purgeIn', { count: days })}{subCases > 0 && <> · {t('trash.withSubCases', { count: subCases })}</>}</>}
            trailing={<span className="workspace-trash__actions">
              <Button type="button" variant="secondary" size="sm" disabled={inProgress} onClick={() => void restore(note)}><ArrowCounterClockwise aria-hidden="true" />{t('trash.restore')}</Button>
              <Button type="button" variant="ghost" size="sm" data-tone="danger" disabled={inProgress} onClick={() => setConfirmation({ kind: 'one', note, subCases })} aria-label={t('trash.purgeNoteLabel', { title: note.title || t('common.untitled') })} title={t('trash.purgeNoteTitle')}><Trash aria-hidden="true" /></Button>
            </span>} />;
        })}
      </List>}
      {confirmation && <Dialog onClose={() => { if (!inProgress) { setConfirmation(null); setTrashChanged(false); } }} size="sm" describedBy="trash-confirm-desc">
        <div className="ui-dialog__form">
          <DialogHeader
            title={confirmation.kind === 'one' ? t('trash.confirmNoteTitle', { title: confirmation.note.title || t('common.untitled') }) : t('trash.confirmAllTitle')}
            description={<p id="trash-confirm-desc">{confirmation.kind === 'one'
              ? <>{t('trash.confirmNoteBody')}{confirmation.subCases > 0 && <> {t('trash.subCasesStay', { count: confirmation.subCases })}</>}</>
              : t('trash.confirmAllBody', { count: notes?.length ?? 0 })}</p>} />
          {trashChanged && confirmation.kind === 'all' && <div role="alert" className="ui-dialog__error">{t('trash.changedSinceShown')}</div>}
          <DialogFooter>
            <Button type="button" variant="secondary" onClick={() => { setConfirmation(null); setTrashChanged(false); }} disabled={inProgress}>{t('common.cancel')}</Button>
            <Button type="button" variant="destructive" onClick={() => void erase()} disabled={inProgress}>{inProgress ? t('trash.purging') : confirmation.kind === 'one' ? t('trash.purgeNote') : t('trash.emptyAll')}</Button>
          </DialogFooter>
        </div>
      </Dialog>}
    </div></div>
  );
}

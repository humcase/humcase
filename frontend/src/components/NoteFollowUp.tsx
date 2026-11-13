import { Button } from './ui/Button';
import { Menu, MenuRadioGroup } from './ui/Menu';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle as CheckCircle2, Circle, CircleDashed, CaretDown as ChevronDown, ClipboardText as FileCheck2, Pencil } from '@phosphor-icons/react';
import ReactMarkdown from 'react-markdown';
import { transformerUrlMarkdown } from '../utils/imageRefs';
import remarkGfm from 'remark-gfm';
import type { Note, NoteStatus, ProjectSpace } from '../types';
import { api } from '../services/tauriApi';
import { Dialog, DialogBody, DialogFooter, DialogHeader } from './ui/Dialog';
import { addPendingWrite, removePendingWrite, pendingWritesPersistent, readPendingWrites } from '../utils/pendingWrites';
import { trackEditorWrite } from '../utils/backupBarrier';
import { statusLabel } from '../utils/statuses';
import './note-follow-up.css';

export function NoteStatusControl({ note, onUpdated }: { note: Note; onUpdated: () => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const writing = useRef(false);
  const Icon = note.status === 'resolved' ? CheckCircle2 : note.status === 'in_progress' ? CircleDashed : Circle;
  // Labels come from the single source `utils/statuses.ts`: the same state always carries the same word.
  async function change(status: NoteStatus) {
    if (writing.current || status === note.status) return;
    writing.current = true;
    setBusy(true);
    setError(false);
    try {
      // Partial payload: never overwrite the report or the editor's content.
      await api.updateNote(note.id, { status });
      onUpdated();
    } catch {
      setError(true);
    } finally {
      writing.current = false;
      setBusy(false);
    }
  }
  return <div className="note-status-control" data-status={note.status} aria-busy={busy}>
    <Menu label={t('notes.followUp.chooseStatus')} className="note-status-menu" trigger={
      <Button type="button" variant="field" className="note-status-field" aria-label={t('notes.followUp.statusLabel', { status: statusLabel(note.status) })} disabled={busy}>
        <Icon size={17} aria-hidden="true" /><span>{statusLabel(note.status)}</span><ChevronDown size={14} aria-hidden="true" />
      </Button>
    }>
      <MenuRadioGroup label={t('notes.followUp.statusGroup')} value={note.status} onValueChange={status => { void change(status); }} options={(['open', 'in_progress', 'resolved'] as const).map(status => ({ value: status, label: statusLabel(status) }))} />
    </Menu>
    {busy && <span role="status" className="note-status-feedback">{t('notes.followUp.saving')}</span>}
    {error && <span role="alert" className="note-status-feedback">{t('notes.followUp.statusError')}</span>}
  </div>;
}

export function NoteReport({ note, onUpdated, space }: { note: Note; onUpdated?: () => void; space?: ProjectSpace | null }) {
  const { t } = useTranslation();
  const [form, setForm] = useState<{ source: string | null; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const pendingId = useRef<string | null>(null);
  const writing = useRef(false);
  function storedDraft() {
    return readPendingWrites().find(e => e.noteId === note.id && e.kind === 'update' && typeof e.payload.resolution_comment === 'string');
  }
  // The escrow is shared with the editor. Deriving the badge from its current state prevents a
  // removal from the recovery panel from leaving a false "Resume" button.
  const hasDraft = !!storedDraft();
  function clearPending() {
    if (pendingId.current) removePendingWrite(pendingId.current);
    pendingId.current = null;
  }
  function close() {
    if (writing.current) return;
    // Closing keeps a changed draft in the common recovery store, never silently discards it.
    setForm(null);
  }
  function edit() {
    setError('');
    const stored = storedDraft();
    if (stored?.kind === 'update') {
      pendingId.current = stored.id;
      setForm({ source: stored.base?.resolution_comment ?? null, text: stored.payload.resolution_comment ?? '' });
      return;
    }
    setForm({ source: note.resolution_comment ?? null, text: note.resolution_comment ?? '' });
  }
  function change(text: string) {
    if (!form) return;
    const previousId = pendingId.current;
    // Store the new draft before removing its predecessor, including when storage refuses writes.
    // `space` attaches the draft to the right space: it must never be offered from another.
    pendingId.current = text !== (form.source ?? '') ? addPendingWrite({
      kind: 'update', noteId: note.id, payload: { resolution_comment: text },
      base: { resolution_comment: form.source }, space: space ?? null, projectId: note.project_id ?? null,
    }).id : null;
    if (previousId && previousId !== pendingId.current) removePendingWrite(previousId);
    setForm({ ...form, text });
  }
  async function save() {
    if (!form || writing.current) return;
    writing.current = true;
    setBusy(true);
    setError('');
    try {
      const result = await trackEditorWrite(api.retryNoteUpdate(note.id,
        { resolution_comment: form.text }, { resolution_comment: form.source }));
      if (result === 'conflict') {
        setError(t('notes.followUp.reportConflict'));
        return;
      }
      clearPending();
      setForm(null);
      onUpdated?.();
    } catch {
      setError(t('notes.followUp.saveError'));
    } finally {
      writing.current = false;
      setBusy(false);
    }
  }
  return <>
  {hasDraft && !form && <Button type="button" variant="secondary" className="note-report-add" onClick={edit}>{t('notes.followUp.resumeDraft')}</Button>}
  {note.resolution_comment?.trim() ? <details className="note-report">
    <summary>
      <FileCheck2 size={20} aria-hidden="true" />
      <span><strong>{t('notes.followUp.report')}</strong><small>{note.status === 'resolved' ? t('notes.followUp.reportResolved') : t('notes.followUp.reportOpen')}</small></span>
      <ChevronDown className="note-report-chevron" size={18} aria-hidden="true" />
    </summary>
    <div className="note-report-body prose max-w-none">
      <ReactMarkdown remarkPlugins={[remarkGfm]} urlTransform={transformerUrlMarkdown}>{note.resolution_comment}</ReactMarkdown>
      <Button type="button" variant="secondary" size="sm" className="note-report-edit" onClick={edit}><Pencil size={15} aria-hidden="true" />{t('notes.followUp.editReport')}</Button>
    </div>
  </details> : <Button type="button" variant="secondary" className="note-report-add" onClick={edit}><FileCheck2 size={17} className="text-primary" aria-hidden="true" />{t('notes.followUp.addReport')}</Button>}
  {form && <Dialog onClose={close} size="lg" className="note-report-dialog" describedBy="note-report-desc">
    <DialogHeader title={t('notes.followUp.dialogTitle')} description={<p id="note-report-desc">{t('notes.followUp.dialogDescription')}</p>} onClose={close} closeLabel={t('notes.followUp.closeReport')} closeDisabled={busy} />
    <DialogBody>
      <div className="ui-dialog__field"><label htmlFor="note-report-draft">{t('notes.followUp.resultLabel')}</label>
        <textarea id="note-report-draft" className="ui-field ui-field--prose" value={form.text} onChange={event => change(event.target.value)} disabled={busy} placeholder={t('notes.followUp.resultPlaceholder')} /></div>
      {error && <p role="alert" className="ui-dialog__error">{error}</p>}
      {!pendingWritesPersistent() && <p role="alert" className="ui-dialog__error">{t('notes.followUp.storageUnavailable')}</p>}
    </DialogBody>
    <DialogFooter note={t('notes.followUp.footerNote')}><Button variant="secondary" type="button" onClick={() => { clearPending(); setForm(null); }} disabled={busy}>{t('notes.followUp.discardDraft')}</Button><Button variant="secondary" type="button" onClick={close} disabled={busy}>{t('notes.followUp.close')}</Button><Button type="button" onClick={() => { void save(); }} disabled={busy || form.text === (form.source ?? '')}>{busy ? t('notes.followUp.saving') : t('notes.followUp.save')}</Button></DialogFooter>
  </Dialog>}
  </>;
}

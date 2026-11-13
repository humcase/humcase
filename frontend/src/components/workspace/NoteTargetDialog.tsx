import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Dialog, DialogBody, DialogFooter, DialogHeader } from '../ui/Dialog';
import type { Project } from '../../types';

interface NoteTargetDialogProps {
  projects: Project[];
  targetId: string;
  onTargetChange: (id: string) => void;
  pending: boolean;
  onClose: () => void;
  onSubmit: () => Promise<void>;
}

/** "Choose a project" window: destination of the next created note. */
export default function NoteTargetDialog({ projects, targetId, onTargetChange, pending, onClose, onSubmit }: NoteTargetDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog onClose={onClose} size="sm" describedBy="workspace-note-target-desc">
      <form className="ui-dialog__form" onSubmit={async event => { event.preventDefault(); if (!targetId || pending) return; await onSubmit(); }}>
        <DialogHeader title={t('workspace.noteTarget.title')} description={<p id="workspace-note-target-desc">{t('workspace.noteTarget.desc')}</p>} />
        <DialogBody>
          <div className="ui-dialog__field"><label htmlFor="workspace-note-target">{t('workspace.noteTarget.label')}</label>
            <select id="workspace-note-target" className="ui-field" value={targetId} onChange={event => onTargetChange(event.target.value)}>{projects.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}</select></div>
        </DialogBody>
        <DialogFooter><Button type="button" variant="secondary" onClick={onClose} disabled={pending}>{t('common.cancel')}</Button><Button type="submit" disabled={pending}>{pending ? t('workspace.noteTarget.creating') : t('workspace.noteTarget.submit')}</Button></DialogFooter>
      </form>
    </Dialog>
  );
}

import type { CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Dialog, DialogBody, DialogFooter, DialogHeader } from '../ui/Dialog';
import type { Activity, Project } from '../../types';

/** Colors offered for a project (the current color stays valid even if it is not among them). */
const PROJECT_COLORS = ['#3b82f6', '#0ea5e9', '#14b8a6', '#22c55e', '#eab308', '#f97316', '#ef4444', '#ec4899', '#8b5cf6', '#64748b'];

interface ProjectSettingsDialogProps {
  project: Project;
  activities: Activity[];
  deleteConfirm: boolean;
  nameDraft: string;
  onNameChange: (name: string) => void;
  colorDraft: string;
  onColorChange: (color: string) => void;
  activityDraft: string[];
  onToggleActivity: (id: string) => void;
  error: string | null;
  pending: boolean;
  onClose: () => void;
  onSave: () => void;
  onToggleArchived: () => void;
  onAskDelete: () => void;
  onCancelDelete: () => void;
  onDelete: () => void;
}

/** "Project" window: name, color, activities, archiving and deletion (with confirmation). */
export default function ProjectSettingsDialog({
  project, activities, deleteConfirm, nameDraft, onNameChange, colorDraft, onColorChange, activityDraft, onToggleActivity,
  error, pending, onClose, onSave, onToggleArchived, onAskDelete, onCancelDelete, onDelete,
}: ProjectSettingsDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog onClose={onClose} size="sm" describedBy="workspace-project-settings-desc">
      {deleteConfirm ? <div className="ui-dialog__form">
        <DialogHeader title={t('workspace.projectSettings.deleteTitle', { name: project.name })} description={<p id="workspace-project-settings-desc">{t('workspace.projectSettings.deleteDesc', { space: t(project.space === 'private' ? 'common.space.private' : 'common.space.work') })}</p>} />
        <DialogBody>{error && <div role="alert" className="ui-dialog__error">{error}</div>}</DialogBody>
        <DialogFooter><Button type="button" variant="secondary" onClick={onCancelDelete} disabled={pending}>{t('workspace.projectSettings.back')}</Button><Button type="button" variant="destructive" onClick={onDelete} disabled={pending}>{pending ? t('workspace.projectSettings.deleting') : t('workspace.projectSettings.deleteProject')}</Button></DialogFooter>
      </div> : <form className="ui-dialog__form" onSubmit={event => { event.preventDefault(); onSave(); }}>
        <DialogHeader title={t('workspace.projectSettings.title')} description={<p id="workspace-project-settings-desc">{t('workspace.projectSettings.desc')}</p>} />
        <DialogBody>
          <div className="ui-dialog__field"><label htmlFor="workspace-project-rename">{t('workspace.projectSettings.name')}</label>
            <input id="workspace-project-rename" className="ui-field" value={nameDraft} onChange={event => onNameChange(event.target.value)} required maxLength={120} /></div>
          <fieldset className="ui-dialog__field workspace-project-colors"><legend>{t('workspace.projectSettings.color')}</legend>
            <div role="radiogroup" aria-label={t('workspace.projectSettings.colorGroup')} className="workspace-project-colors__list">{(PROJECT_COLORS.includes(project.color.toLowerCase()) ? PROJECT_COLORS : [project.color.toLowerCase(), ...PROJECT_COLORS]).map(color => <label key={color} className="workspace-project-colors__swatch" style={{ '--swatch': color } as CSSProperties} title={color}>
              <input type="radio" name="workspace-project-color" value={color} checked={colorDraft.toLowerCase() === color} onChange={() => onColorChange(color)} aria-label={t('workspace.projectSettings.colorOption', { color })} /></label>)}</div></fieldset>
          {project.space && <fieldset className="ui-dialog__field workspace-dialog__choices"><legend>{t('workspace.projectSettings.areas')}</legend>{activities.filter(activity => activity.space === project.space).map(activity => <label key={activity.id}><input type="checkbox" checked={activityDraft.includes(activity.id)} onChange={() => onToggleActivity(activity.id)} />{activity.name}</label>)}
            {!activities.some(activity => activity.space === project.space) && <p>{t('workspace.projectSettings.noAreas')}</p>}</fieldset>}
          {error && <div role="alert" className="ui-dialog__error">{error}</div>}
        </DialogBody>
        <DialogFooter className="workspace-project-settings__footer">
          <span className="workspace-project-settings__secondary">
            <Button type="button" variant="outline" onClick={onToggleArchived} disabled={pending}>{project.archived_at ? t('workspace.projectSettings.unarchive') : t('workspace.projectSettings.archive')}</Button>
            {project.space && <Button type="button" variant="ghost" onClick={onAskDelete} disabled={pending}>{t('workspace.projectSettings.deleteEllipsis')}</Button>}
          </span>
          <Button type="button" variant="secondary" onClick={onClose} disabled={pending}>{t('common.cancel')}</Button><Button type="submit" disabled={pending || !nameDraft.trim()}>{pending ? t('workspace.projectSettings.saving') : t('common.save')}</Button>
        </DialogFooter>
      </form>}
    </Dialog>
  );
}

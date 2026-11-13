import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Dialog, DialogBody, DialogFooter, DialogHeader } from '../ui/Dialog';
import type { ProjectSpace, SpacePreferences } from '../../utils/spaces';
import type { Activity } from '../../types';

interface ProjectCreateDialogProps {
  name: string;
  onNameChange: (name: string) => void;
  space: ProjectSpace;
  onSpaceChange: (space: ProjectSpace) => void;
  selectedActivities: string[];
  onToggleActivity: (id: string) => void;
  activities: Activity[];
  usage: SpacePreferences['usage'];
  error: string | null;
  pending: boolean;
  onClose: () => void;
  onSubmit: () => void;
}

/** "New project" window: name, space and optional activities. */
export default function ProjectCreateDialog({
  name, onNameChange, space, onSpaceChange, selectedActivities, onToggleActivity, activities, usage, error, pending, onClose, onSubmit,
}: ProjectCreateDialogProps) {
  const { t } = useTranslation();
  const spaceActivities = activities.filter(activity => activity.space === space);
  return (
    <Dialog onClose={onClose} size="sm">
      <form className="ui-dialog__form" onSubmit={event => { event.preventDefault(); onSubmit(); }}>
        <DialogHeader title={t('workspace.createProject.title')} />
        <DialogBody>
          <div className="ui-dialog__field"><label htmlFor="workspace-project-name">{t('workspace.createProject.name')}</label>
            <input id="workspace-project-name" className="ui-field" value={name} onChange={event => onNameChange(event.target.value)} required maxLength={120} /></div>
          <div className="ui-dialog__field"><label htmlFor="workspace-project-space">{t('workspace.createProject.space')}</label>
            <select id="workspace-project-space" className="ui-field" value={space} onChange={event => onSpaceChange(event.target.value as ProjectSpace)}>{usage !== 'work' && <option value="private">{t('common.space.private')}</option>}{usage !== 'private' && <option value="work">{t('common.space.work')}</option>}</select></div>
          {spaceActivities.length > 0 && <fieldset className="ui-dialog__field workspace-dialog__choices"><legend>{t('workspace.createProject.areas')}</legend>{spaceActivities.map(activity => <label key={activity.id}><input type="checkbox" checked={selectedActivities.includes(activity.id)} onChange={() => onToggleActivity(activity.id)} />{activity.name}</label>)}</fieldset>}
          {error && <div role="alert" className="ui-dialog__error">{error}</div>}
        </DialogBody>
        <DialogFooter><Button type="button" variant="secondary" onClick={onClose} disabled={pending}>{t('common.cancel')}</Button><Button type="submit" disabled={pending || !name.trim()}>{pending ? t('workspace.createProject.creating') : t('workspace.createProject.submit')}</Button></DialogFooter>
      </form>
    </Dialog>
  );
}

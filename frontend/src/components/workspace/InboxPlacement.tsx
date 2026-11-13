import { useTranslation } from 'react-i18next';
import { Tray } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { SelectField } from '../ui/Controls';
import { Notice } from '../ui/Toast';
import type { Note, Project } from '../../types';

interface InboxPlacementProps {
  projects: Project[];
  inboxProject: Project;
  targetId: string;
  onTargetChange: (id: string) => void;
  /** Cases the note can go into, once a project is chosen. */
  cases: Note[];
  parentId: string;
  onParentChange: (id: string) => void;
  pending: boolean;
  error: string | null;
  onMove: () => void;
}

/** Inbox note banner: choose a project of the space, and if wished a case of it, and file the note there. */
export default function InboxPlacement({ projects, inboxProject, targetId, onTargetChange, cases, parentId, onParentChange, pending, error, onMove }: InboxPlacementProps) {
  const { t } = useTranslation();
  const byId = new Map(cases.map(item => [item.id, item]));
  const pathOf = (item: Note) => {
    const titles: string[] = [];
    const seen = new Set<string>();
    for (let cursor: Note | undefined = item; cursor && !seen.has(cursor.id); cursor = cursor.parent_id ? byId.get(cursor.parent_id) : undefined) {
      seen.add(cursor.id);
      titles.unshift(cursor.title || t('common.untitled'));
    }
    return titles.join(' › ');
  };
  const parents = cases.filter(item => item.project_id === targetId && !item.deleted_at && !item.archived_at)
    .map(item => ({ id: item.id, label: pathOf(item) }))
    .sort((a, b) => a.label.localeCompare(b.label));
  const targets = projects.filter(project => !project.is_inbox && !project.archived_at && project.space === inboxProject.space);
  // Always in view, one compact line under the top bar: filing is never hidden behind a fold,
  // and nothing has to be chosen before writing in the case.
  return (
    <section className="workspace-inbox-placement" aria-labelledby="workspace-inbox-placement-title">
      <p className="workspace-inbox-placement__intro"><Tray size={16} aria-hidden="true" /><strong id="workspace-inbox-placement-title">{t('workspace.inboxPlacement.title')}</strong><span>{t('workspace.inboxPlacement.hint')}</span></p>
      <div className="workspace-inbox-placement__fields">
        <label className="workspace-inbox-placement__field" htmlFor="workspace-move-target">{t('workspace.inboxPlacement.label')}</label>
        <SelectField id="workspace-move-target" className="ui-field--sm" value={targetId} disabled={pending} onChange={event => onTargetChange(event.target.value)}>
          <option value="">{t('workspace.inboxPlacement.choose')}</option>
          {targets.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}
        </SelectField>
        {targetId && parents.length > 0 && <>
          <label className="workspace-inbox-placement__field" htmlFor="workspace-move-parent">{t('workspace.inboxPlacement.parentLabel')}</label>
          <SelectField id="workspace-move-parent" className="ui-field--sm" value={parentId} disabled={pending} onChange={event => onParentChange(event.target.value)}>
            <option value="">{t('workspace.inboxPlacement.topLevel')}</option>
            {parents.map(parent => <option key={parent.id} value={parent.id}>{parent.label}</option>)}
          </SelectField>
        </>}
        <Button type="button" size="sm" disabled={!targetId || pending} onClick={onMove}>{pending ? t('workspace.inboxPlacement.moving') : t('workspace.inboxPlacement.move')}</Button>
      </div>
      {error && <Notice tone="danger" className="workspace-inbox-placement__error">{error}</Notice>}
    </section>
  );
}

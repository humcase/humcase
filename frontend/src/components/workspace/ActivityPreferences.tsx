import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Plus, SlidersHorizontal } from '@phosphor-icons/react';
import type { Activity, ProjectSpace } from '../../types';
import { ACTIVITY_STARTERS } from '../../utils/activityProfiles';
import './activities.css';

interface Props {
  activities: Activity[];
  enabledSpaces: ProjectSpace[];
  onCreate: (name: string, space: ProjectSpace) => Promise<void>;
  onRename: (id: string, name: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}

export function ActivityPreferences({ activities, enabledSpaces, onCreate, onRename, onDelete }: Props) {
  const { t } = useTranslation();
  const [newNames, setNewNames] = useState<Record<ProjectSpace, string>>({ work: '', private: '' });
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run(action: () => Promise<void>, after?: () => void) {
    setBusy(true);
    setError(null);
    try { await action(); after?.(); }
    catch (cause) { setError(t('settings.activities.error', { error: String(cause) })); }
    finally { setBusy(false); }
  }

  return <Card as="section" padding="none" className="settings-card activity-preferences" aria-labelledby="activity-preferences-heading">
    <div className="activity-preferences__heading"><SlidersHorizontal size={20} aria-hidden="true" /><h3 id="activity-preferences-heading">{t('settings.activities.title')}</h3></div>
    <p>{t('settings.activities.intro')}</p>
    {enabledSpaces.map(space => {
      const items = activities.filter(activity => activity.space === space);
      const suggested = ACTIVITY_STARTERS[space].filter(name => !items.some(activity => activity.name.toLocaleLowerCase() === name.toLocaleLowerCase()));
      return <div key={space} className="activity-preferences__group">
        <h4>{space === 'work' ? t('settings.activities.groupWork') : t('settings.activities.groupPrivate')}</h4>
        <div className="activity-preferences__list">
          {items.length === 0 && <p>{t('settings.activities.empty')}</p>}
          {items.map(activity => <div className="activity-preferences__row" key={activity.id}>
            {editingId === activity.id ? <form onSubmit={event => { event.preventDefault(); if (editName.trim()) void run(() => onRename(activity.id, editName.trim()), () => setEditingId(null)); }}>
              <input className="ui-field" aria-label={t('settings.activities.renameAria', { name: activity.name })} value={editName} maxLength={80} onChange={event => setEditName(event.target.value)} />
              <Button size="sm" type="submit" disabled={busy || !editName.trim()}>{t('common.save')}</Button><Button variant="secondary" size="sm" type="button" onClick={() => setEditingId(null)} disabled={busy}>{t('common.cancel')}</Button>
            </form> : <><span><strong>{activity.name}</strong><small>{t('settings.activities.projectCount', { count: activity.project_ids.length })}</small></span>
              <Button variant="secondary" size="sm" type="button" disabled={busy} onClick={() => { setEditingId(activity.id); setEditName(activity.name); }}>{t('settings.activities.rename')}</Button>
              {confirmDeleteId === activity.id ? <span className="activity-preferences__confirm"><small>{t('settings.activities.linksRemoved', { count: activity.project_ids.length })}</small><Button size="sm" variant="destructive" data-tone="soft" type="button" disabled={busy} onClick={() => void run(() => onDelete(activity.id), () => setConfirmDeleteId(null))}>{t('settings.activities.confirm')}</Button><Button variant="secondary" size="sm" type="button" disabled={busy} onClick={() => setConfirmDeleteId(null)}>{t('common.cancel')}</Button></span> : <Button variant="secondary" size="sm" type="button" disabled={busy} onClick={() => setConfirmDeleteId(activity.id)}>{t('common.delete')}</Button>}</>}
          </div>)}
        </div>
        <form className="activity-preferences__new" onSubmit={event => { event.preventDefault(); const name = newNames[space].trim(); if (name) void run(() => onCreate(name, space), () => setNewNames(current => ({ ...current, [space]: '' }))); }}>
          <input className="ui-field" aria-label={t('settings.activities.newAria', { space: t(space === 'work' ? 'common.space.work' : 'common.space.private') })} placeholder={t('settings.activities.namePlaceholder')} value={newNames[space]} maxLength={80} onChange={event => setNewNames(current => ({ ...current, [space]: event.target.value }))} />
          <Button size="sm" type="submit" disabled={busy || !newNames[space].trim()}><Plus size={16} aria-hidden="true" />{t('settings.activities.add')}</Button>
        </form>
        {suggested.length > 0 && <details className="activity-preferences__suggestion-disclosure"><summary>{t('settings.activities.suggestions')}</summary><div className="activity-preferences__suggestions">{suggested.map(name => <Button variant="secondary" size="sm" type="button" key={name} disabled={busy} onClick={() => void run(() => onCreate(name, space))}>+ {name}</Button>)}</div></details>}
      </div>;
    })}
    {error && <p role="alert" className="ui-field-error">{error}</p>}
  </Card>;
}

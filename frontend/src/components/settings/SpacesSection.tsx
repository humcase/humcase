import type { Ref } from 'react';
import { useTranslation } from 'react-i18next';
import type { Activity, ProjectSpace } from '../../types';
import type { SpacePreferences, SpaceUsage } from '../../utils/spaces';
import { ActivityPreferences } from '../workspace/ActivityPreferences';
import { Card } from '../ui/Card';

/** Spaces: Work, Personal or both, the combined view, and the activities of each space. */
export function SpacesSection({ spacePreferences, onSpaceUsage, onCombinedView, activities, onCreateActivity, onRenameActivity, onDeleteActivity, activitiesRef }: {
  spacePreferences: SpacePreferences;
  onSpaceUsage: (usage: SpaceUsage) => void;
  onCombinedView: (enabled: boolean) => void;
  activities: Activity[];
  onCreateActivity: (name: string, space: ProjectSpace) => Promise<void>;
  onRenameActivity: (id: string, name: string) => Promise<void>;
  onDeleteActivity: (id: string) => Promise<void>;
  /** Scrolled into view when Settings open on the activities. */
  activitiesRef: Ref<HTMLDivElement>;
}) {
  const { t } = useTranslation();
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.spaces.heading')}</h3><p>{t('settings.spaces.description')}</p></div>
      <Card as="section" padding="none" className="workspace-space-settings settings-card">
        <p>{t('settings.spaces.keepSpace')}</p>
        <fieldset><legend>{t('settings.spaces.usageLegend')}</legend>
          {([{ value: 'private', label: t('settings.spaces.usage.private') }, { value: 'work', label: t('settings.spaces.usage.work') }, { value: 'both', label: t('settings.spaces.usage.both') }] as const).map(option => <label key={option.value}><input type="radio" name="space-usage" checked={spacePreferences.usage === option.value} onChange={() => onSpaceUsage(option.value)} />{option.label}</label>)}
        </fieldset>
        {spacePreferences.usage === 'both' && <label className="workspace-space-settings__combined"><input type="checkbox" checked={spacePreferences.combined} onChange={event => onCombinedView(event.target.checked)} />{t('settings.spaces.combined')} <small>{t('settings.spaces.combinedHint')}</small></label>}
        <p>{t('settings.spaces.unsorted')}</p>
      </Card>
      <div ref={activitiesRef} className="activity-settings-target"><ActivityPreferences activities={activities} enabledSpaces={spacePreferences.usage === 'both' ? ['work', 'private'] as ProjectSpace[] : [spacePreferences.usage]} onCreate={onCreateActivity} onRename={onRenameActivity} onDelete={onDeleteActivity} /></div>
    </div>
  );
}

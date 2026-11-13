import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { IdentificationCard } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { api } from '../../services/tauriApi';
import { notifyHomeSettingsChanged } from '../../utils/homeSettings';
import type { ProfileSettings, ProjectSpace } from '../../types';

/**
 * Profile and suggestions: whether Humcase proposes lines for the work profile, whether Home
 * shows the cases that wait for a next step, and the way to the profile of each space. Humcase
 * proposes; nothing enters the profile and nothing starts without the person.
 */
export function ProfileSection({ spaces, onOpenProfile }: { spaces: ProjectSpace[]; onOpenProfile?: (space: ProjectSpace) => void }) {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<ProfileSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    api.profileSettings()
      .then(next => { if (active) setSettings(next); })
      .catch(cause => { if (active) setError(String(cause)); });
    return () => { active = false; };
  }, []);
  const save = async (next: ProfileSettings) => {
    setBusy(true);
    setError(null);
    try {
      setSettings(await api.setProfileSettings(next));
      notifyHomeSettingsChanged();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.profile.heading')}</h3><p>{t('settings.profile.description')}</p></div>
      <Card padding="none" className="settings-card">
        <label htmlFor="profile-propose" className="settings-label flex items-center gap-2">
          <input
            id="profile-propose"
            type="checkbox"
            checked={settings?.propose ?? false}
            disabled={settings === null || busy}
            onChange={event => { if (settings) void save({ ...settings, propose: event.target.checked }); }}
            className="accent-primary"
          />
          {t('settings.profile.propose')}
        </label>
        <p className="text-[11px] text-muted-foreground">{t('settings.profile.proposeHint')}</p>
        <label htmlFor="profile-nudges" className="settings-label flex items-center gap-2">
          <input
            id="profile-nudges"
            type="checkbox"
            checked={settings?.nudges ?? false}
            disabled={settings === null || busy}
            onChange={event => { if (settings) void save({ ...settings, nudges: event.target.checked }); }}
            className="accent-primary"
          />
          {t('settings.profile.nudges')}
        </label>
        <p className="text-[11px] text-muted-foreground">{t('settings.profile.nudgesHint')}</p>
        {onOpenProfile && <div className="flex flex-wrap gap-2">
          {spaces.map(space => <Button key={space} type="button" variant="outline" size="sm" onClick={() => onOpenProfile(space)}>
            <IdentificationCard size={16} aria-hidden="true" />{t('settings.profile.open', { space: t(`common.space.${space}`) })}
          </Button>)}
        </div>}
        {error && <p role="alert" className="settings-alert">{t('settings.profile.error', { error })}</p>}
      </Card>
    </div>
  );
}

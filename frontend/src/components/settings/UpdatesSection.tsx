import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Card } from '../ui/Card';
import { api } from '../../services/tauriApi';
import type { UpdateSettings } from '../../types';

/** Updates: the switch of the check at startup, or why this copy does not update itself. */
export function UpdatesSection() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<UpdateSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api.getUpdateSettings()
      .then(value => { if (!cancelled) setSettings(value); })
      .catch(e => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, []);

  const change = async (enabled: boolean) => {
    if (!settings) return;
    setBusy(true);
    setError(null);
    try {
      await api.setUpdateCheck(enabled);
      setSettings({ ...settings, enabled });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.updates.heading')}</h3><p>{t('settings.updates.description')}</p></div>
      <Card padding="none" className="settings-card">
        {settings === null && !error && <p role="status" className="settings-meaning-status">{t('settings.updates.loading')}</p>}
        {settings && !settings.supported && <p className="settings-meaning-status">{t('settings.updates.unsupported')}</p>}
        {settings?.supported && (
          <label htmlFor="update-check" className="settings-label flex items-center gap-2">
            <input
              id="update-check"
              type="checkbox"
              checked={settings.enabled}
              disabled={busy}
              onChange={event => { void change(event.target.checked); }}
              className="accent-primary"
            />
            {t('settings.updates.enable')}
          </label>
        )}
        {error && <p role="alert" className="settings-alert">{t('settings.updates.error', { error })}</p>}
      </Card>
    </div>
  );
}

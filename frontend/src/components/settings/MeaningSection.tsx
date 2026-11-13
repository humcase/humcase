import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Card } from '../ui/Card';
import { api } from '../../services/tauriApi';
import { useMeaningStatus } from '../../hooks/useMeaning';

/** Search by meaning: where the reading of the cases stands, and the switch to turn it off. */
export function MeaningSection() {
  const { t } = useTranslation();
  const [status, refresh] = useMeaningStatus();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const on = status !== null && status.state !== 'off';
  const change = async (enabled: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await api.setMeaningEnabled(enabled);
      refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const line = status === null ? t('settings.meaning.loading')
    : status.state === 'ready' ? t('settings.meaning.ready', { count: status.total })
    : status.state === 'reading' && status.total === 0 ? t('settings.meaning.starting')
    : status.state === 'reading' ? t('settings.meaning.reading', { done: status.done, total: status.total })
    : status.state === 'off' ? t('settings.meaning.off')
    : t('settings.meaning.unavailable');
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.meaning.heading')}</h3><p>{t('settings.meaning.description')}</p></div>
      <Card padding="none" className="settings-card">
        <label htmlFor="meaning-enabled" className="settings-label flex items-center gap-2">
          <input
            id="meaning-enabled"
            type="checkbox"
            checked={on}
            disabled={status === null || busy}
            onChange={event => { void change(event.target.checked); }}
            className="accent-primary"
          />
          {t('settings.meaning.enable')}
        </label>
        <p role="status" className="settings-meaning-status">{line}</p>
        {error && <p role="alert" className="settings-alert">{t('settings.meaning.error', { error })}</p>}
      </Card>
    </div>
  );
}

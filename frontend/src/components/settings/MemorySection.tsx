import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Card } from '../ui/Card';
import { SegmentedControl } from '../ui/SegmentedControl';
import { api } from '../../services/tauriApi';
import { notifyHomeSettingsChanged } from '../../utils/homeSettings';
import type { TidySettings } from '../../types';

const WEEKS = ['2', '4', '8', '0'] as const;

/**
 * Memory ("To tidy"): after how long a resolved case is proposed for the archive, whether
 * Humcase compares decisions (replaced by a newer one, contradicted by a case) and whether it
 * proposes to file close cases together. It proposes; it never tidies alone.
 */
export function MemorySection() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<TidySettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    api.tidySettings()
      .then(next => { if (active) setSettings(next); })
      .catch(cause => { if (active) setError(String(cause)); });
    return () => { active = false; };
  }, []);
  const save = async (next: TidySettings) => {
    setBusy(true);
    setError(null);
    try {
      setSettings(await api.setTidySettings(next));
      notifyHomeSettingsChanged();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.memory.heading')}</h3><p>{t('settings.memory.description')}</p></div>
      <Card padding="none" className="settings-card">
        <span id="memory-weeks-label" className="settings-label">{t('settings.memory.archiveAfter')}</span>
        {settings && <SegmentedControl label={t('settings.memory.archiveAfter')} value={String(settings.archive_weeks)}
          onValueChange={value => { if (!busy) void save({ ...settings, archive_weeks: Number(value) }); }}
          items={WEEKS.map(value => ({ value, label: value === '0' ? t('settings.memory.never') : t('settings.memory.weeks', { count: Number(value) }) }))} />}
        <label htmlFor="memory-supersede" className="settings-label flex items-center gap-2">
          <input
            id="memory-supersede"
            type="checkbox"
            checked={settings?.supersede ?? false}
            disabled={settings === null || busy}
            onChange={event => { if (settings) void save({ ...settings, supersede: event.target.checked }); }}
            className="accent-primary"
          />
          {t('settings.memory.supersede')}
        </label>
        <p className="text-[11px] text-muted-foreground">{t('settings.memory.supersedeHint')}</p>
        <label htmlFor="memory-group" className="settings-label flex items-center gap-2">
          <input
            id="memory-group"
            type="checkbox"
            checked={settings?.group ?? false}
            disabled={settings === null || busy}
            onChange={event => { if (settings) void save({ ...settings, group: event.target.checked }); }}
            className="accent-primary"
          />
          {t('settings.memory.group')}
        </label>
        <p className="text-[11px] text-muted-foreground">{t('settings.memory.groupHint')}</p>
        {error && <p role="alert" className="settings-alert">{t('settings.memory.error', { error })}</p>}
      </Card>
    </div>
  );
}

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { api } from '../../services/tauriApi';
import type { NatureReadingStatus } from '../../types';

/** How often the reading state is asked: often while cases are read, rarely otherwise. */
const READING_POLL_MS = 2000;
const IDLE_POLL_MS = 15000;

/**
 * Natures to check: the switch that lets TypeSafe's review read the cases, and where the reading
 * stands. Off until the person turns it on: the cases leave the computer. The key lives in AI services.
 */
export function NaturesSection({ onOpenAiServices }: { onOpenAiServices?: () => void } = {}) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<NatureReadingStatus | null>(null);
  const [tick, setTick] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refresh = () => setTick(value => value + 1);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ask = () => {
      api.natureReadingStatus()
        .then(next => {
          if (cancelled) return;
          setStatus(next);
          timer = setTimeout(ask, next.state === 'reading' ? READING_POLL_MS : IDLE_POLL_MS);
        })
        .catch(() => { if (!cancelled) timer = setTimeout(ask, IDLE_POLL_MS); });
    };
    ask();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [tick]);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const on = status !== null && status.state !== 'off';
  const line = status === null ? t('settings.natures.loading')
    : status.state === 'off' ? t('settings.natures.off')
    : status.state === 'no_key' ? t('settings.natures.noKey')
    : status.state === 'key_refused' ? t('settings.natures.keyRefused')
    : status.state === 'failed' ? t('settings.natures.failed', { error: status.error ?? '' })
    : status.state === 'ready' ? t('settings.natures.ready', { count: status.total })
    : status.total === 0 ? t('settings.natures.starting')
    : t('settings.natures.reading', { done: status.read, total: status.total });
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.natures.heading')}</h3><p>{t('settings.natures.description')}</p></div>
      <Card padding="none" className="settings-card">
        <label htmlFor="natures-enabled" className="settings-label flex items-center gap-2">
          <input
            id="natures-enabled"
            type="checkbox"
            checked={on}
            disabled={status === null || busy}
            onChange={event => { const enabled = event.target.checked; void run(() => api.setNatureReading(enabled)); }}
            className="accent-primary"
          />
          {t('settings.natures.enable')}
        </label>
        <p className="text-[11px] text-muted-foreground">{t('settings.natures.sent')}</p>
        <p role="status" className="settings-meaning-status">{line}</p>

        {(status?.state === 'no_key' || status?.state === 'key_refused') && onOpenAiServices && (
          <div><Button variant="secondary" type="button" onClick={onOpenAiServices}>{t('settings.natures.openAiServices')}</Button></div>
        )}
        {error && <p role="alert" className="settings-alert">{t('settings.natures.error', { error })}</p>}
      </Card>
    </div>
  );
}

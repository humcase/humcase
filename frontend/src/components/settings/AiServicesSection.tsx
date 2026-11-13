import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import type { AiProviderInfo } from '../../types';
import { SelectField } from '../ui/Controls';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Notice } from '../ui/Toast';
import { api } from '../../services/tauriApi';
import { SETTINGS_FIELD } from './fields';
import type { AiProviders } from './useAiProviders';

/** Describes a provider without ever exposing a key (variable name and default model only). */
function providerDescription(f: AiProviderInfo, t: TFunction): string {
  const parts: string[] = [f.key_env ? t('settings.voice.provider.keyEnv', { name: f.key_env }) : t('settings.voice.provider.noKeyNeeded')];
  if (f.default_model) parts.push(t('settings.voice.provider.model', { model: f.default_model }));
  if (f.url_env) parts.push(t('settings.voice.provider.url', { name: f.url_env }));
  return parts.join(' · ');
}

/** State of a provider's key (never the value). */
function keyLabel(t: TFunction, f?: AiProviderInfo): string {
  if (!f) return '';
  if (!f.key_defined) return f.needs_key ? t('settings.voice.key.none') : t('settings.voice.key.notRequired');
  return f.key_source === 'app'
    ? t('settings.voice.key.app')
    : t('settings.voice.key.env');
}

/**
 * AI services: every service Humcase can call, with its key, in one place. Transcription and
 * clean-up first, then the review of suggestions (TypeSafe). Humcase works without any of them.
 */
export function AiServicesSection({ ai }: { ai: AiProviders }) {
  const { t } = useTranslation();
  const { cleanInfo, sttInfo } = ai;
  return <>
    <div className="settings-panel">
      <Card padding="none" className="settings-card settings-card--ai">
        <div>
          <label className="settings-label block">{t('settings.voice.providersTitle')}</label>
          <p className="text-xs text-muted-foreground mt-1">
            {t('settings.voice.providersHelp')}
          </p>
          <p className="text-xs text-muted-foreground mt-2">
            {t('settings.voice.recommendations')}
          </p>
        </div>

        {ai.providers?.config_alert && (
          <p role="alert" className="settings-alert">
            {t('settings.voice.configAlert', { alert: ai.providers.config_alert })}
          </p>
        )}

        {ai.providersFailed && (
          <Notice tone="danger" className="settings-status">
            {t('settings.voice.providersFailed')}{' '}
            <Button type="button" variant="link" size="sm" onClick={ai.retryProviders}>{t('common.retry')}</Button>
          </Notice>
        )}

        <div className="flex flex-col gap-2">
          <label htmlFor="ai-clean-provider" className="settings-label settings-label--small">
            {t('settings.voice.cleanLabel')}
          </label>
          <p className="text-xs text-muted-foreground">{t('settings.voice.cleanHint')}</p>
          <SelectField
            id="ai-clean-provider"
            value={ai.cleanProvider}
            onChange={e => ai.chooseCleanProvider(e.target.value)}
            className={SETTINGS_FIELD}
          >
            {(ai.providers?.cleanup ?? []).map(p => (
              <option key={p.id} value={p.id}>{p.label}</option>
            ))}
          </SelectField>
          {cleanInfo && <span className="text-[11px] text-muted-foreground">{providerDescription(cleanInfo, t)}</span>}

          <input
            type="text"
            aria-label={t('settings.voice.cleanModelAria')}
            value={ai.cleanModel}
            onChange={e => ai.changeCleanModel(e.target.value)}
            placeholder={cleanInfo?.default_model ? t('settings.voice.modelPlaceholderDefault', { model: cleanInfo.default_model }) : t('settings.voice.modelPlaceholder')}
            className={SETTINGS_FIELD}
          />

          {cleanInfo?.url_editable && (
            <div className="flex flex-col gap-1">
              <label htmlFor="ai-clean-url" className="settings-label settings-label--small">
                {t('settings.voice.urlLabel')}
              </label>
              <div className="flex gap-2 items-center">
                <input
                  id="ai-clean-url"
                  type="text"
                  value={ai.cleanUrlInput}
                  onChange={e => ai.setCleanUrlInput(e.target.value)}
                  placeholder={t('settings.voice.urlPlaceholder')}
                  autoComplete="off"
                  className={SETTINGS_FIELD}
                />
                <Button
                  onClick={() => { if (ai.cleanUrlInput.trim()) void ai.saveUrl(ai.cleanProvider, ai.cleanUrlInput); }}
                  aria-label={t('settings.voice.urlSaveAria')}
                  disabled={ai.savingUrl}
                  className="shrink-0"
                >
                  {t('common.save')}
                </Button>
                <Button variant="secondary"
                  onClick={() => { void ai.saveUrl(ai.cleanProvider, ''); }}
                  disabled={ai.savingUrl || !cleanInfo.url}
                  className="shrink-0"
                >
                  {t('settings.voice.clear')}
                </Button>
              </div>
              <span className="text-[11px] text-muted-foreground">
                {cleanInfo.url ? t('settings.voice.urlCurrent', { url: cleanInfo.url }) : t('settings.voice.urlNone')}
              </span>
            </div>
          )}

          <div className="flex gap-2 items-center">
            <input
              type="password"
              aria-label={t('settings.voice.cleanKeyAria')}
              value={ai.cleanKeyInput}
              onChange={e => ai.setCleanKeyInput(e.target.value)}
              placeholder={t('settings.voice.keyPlaceholder')}
              autoComplete="off"
              className={SETTINGS_FIELD}
            />
            <Button
              onClick={() => { void ai.submitKey('clean'); }}
              aria-label={t('settings.voice.cleanSaveAria')}
              disabled={ai.savingClean || !ai.cleanKeyInput.trim()}
              className="shrink-0"
            >
              {t('common.save')}
            </Button>
            <Button variant="secondary"
              onClick={() => ai.clearKey('clean')}
              aria-label={t('settings.voice.cleanClearAria')}
              disabled={ai.savingClean || cleanInfo?.key_source !== 'app'}
              className="shrink-0"
            >
              {t('settings.voice.clear')}
            </Button>
          </div>
          <span className="text-[11px] text-muted-foreground">{keyLabel(t, cleanInfo)}</span>
          {ai.keyStatus?.section === 'clean' && <Notice className="settings-status">{ai.keyStatus.text}</Notice>}
          <div className="flex gap-2 items-center flex-wrap">
            <Button variant="secondary"
              onClick={() => { void ai.testConnection(ai.cleanProvider); }}
              disabled={ai.testingConnection || !cleanInfo}
              className="shrink-0"
            >
              {ai.testingConnection ? t('settings.voice.testing') : t('settings.voice.test')}
            </Button>
            {cleanInfo && (
              <span className="text-[11px] text-muted-foreground" role="status">
                {ai.connectionStatus ?? (cleanInfo.ready ? t('settings.voice.ready') : t('settings.voice.notReady'))}
              </span>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <label htmlFor="ai-stt-provider" className="settings-label settings-label--small">
            {t('settings.voice.sttLabel')}
          </label>
          <SelectField
            id="ai-stt-provider"
            value={ai.sttProvider}
            onChange={e => ai.chooseSttProvider(e.target.value)}
            className={SETTINGS_FIELD}
          >
            {(ai.providers?.transcription ?? []).map(p => (
              <option key={p.id} value={p.id}>{p.label}</option>
            ))}
          </SelectField>
          {sttInfo && <span className="text-[11px] text-muted-foreground">{providerDescription(sttInfo, t)}</span>}

          <div className="flex gap-2 items-center">
            <input
              type="password"
              aria-label={t('settings.voice.sttKeyAria')}
              value={ai.sttKeyInput}
              onChange={e => ai.setSttKeyInput(e.target.value)}
              placeholder={t('settings.voice.keyPlaceholder')}
              autoComplete="off"
              className={SETTINGS_FIELD}
            />
            <Button
              onClick={() => { void ai.submitKey('stt'); }}
              aria-label={t('settings.voice.sttSaveAria')}
              disabled={ai.savingStt || !ai.sttKeyInput.trim()}
              className="shrink-0"
            >
              {t('common.save')}
            </Button>
            <Button variant="secondary"
              onClick={() => ai.clearKey('stt')}
              aria-label={t('settings.voice.sttClearAria')}
              disabled={ai.savingStt || sttInfo?.key_source !== 'app'}
              className="shrink-0"
            >
              {t('settings.voice.clear')}
            </Button>
          </div>
          <span className="text-[11px] text-muted-foreground">{keyLabel(t, sttInfo)}</span>
          {ai.keyStatus?.section === 'stt' && <Notice className="settings-status">{ai.keyStatus.text}</Notice>}
        </div>
      </Card>
      <ReviewServiceCard ai={ai} />
    </div>
  </>;
}

const REVIEW_PROVIDER = 'typesafe';

/**
 * The review of suggestions: TypeSafe's key. Saving it asks the readers to look again, so a reading
 * waiting for its key starts without reopening Settings. The key is never shown back.
 */
function ReviewServiceCard({ ai }: { ai: AiProviders }) {
  const { t } = useTranslation();
  const provider = ai.providers?.reading?.find(p => p.id === REVIEW_PROVIDER) ?? null;
  const [keyInput, setKeyInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const saveKey = async (key: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.setAiKey(REVIEW_PROVIDER, key);
      setKeyInput('');
      await api.recheckNatureReading();
      ai.retryProviders();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card padding="none" className="settings-card">
      <div>
        <label htmlFor="review-key" className="settings-label block">{t('settings.aiServices.review')}</label>
        <p className="text-xs text-muted-foreground mt-1">{t('settings.aiServices.reviewHint')}</p>
      </div>
      <div className="flex gap-2 items-center">
        <input
          id="review-key"
          type="password"
          value={keyInput}
          onChange={event => setKeyInput(event.target.value)}
          placeholder={t('settings.aiServices.reviewKeyPlaceholder')}
          autoComplete="off"
          className={SETTINGS_FIELD}
        />
        <Button onClick={() => { void saveKey(keyInput.trim()); }} disabled={busy || !keyInput.trim()} className="shrink-0">{t('common.save')}</Button>
        <Button variant="secondary" onClick={() => { void saveKey(''); }} disabled={busy || provider?.key_source !== 'app'} className="shrink-0">{t('settings.voice.clear')}</Button>
      </div>
      <span className="text-[11px] text-muted-foreground">{keyLabel(t, provider ?? undefined)}</span>
      {error && <p role="alert" className="settings-alert">{t('settings.natures.error', { error })}</p>}
    </Card>
  );
}

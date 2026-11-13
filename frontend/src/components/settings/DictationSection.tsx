import { useTranslation } from 'react-i18next';
import { LANGUAGES } from '../../i18n/language';
import { DICTATION_LANGUAGES, type DictationLanguage } from '../../utils/prefs';
import { SelectField } from '../ui/Controls';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { SETTINGS_FIELD } from './fields';
import type { Microphones } from './useMicrophones';
import type { AiProviders } from './useAiProviders';

/**
 * Dictation: the microphone, the language spoken and what happens when a dictation ends. The
 * transcription service and its key live in AI services, reached from here in one click.
 */
export function DictationSection({ microphones, ai, onOpenAiServices }: { microphones: Microphones; ai: AiProviders; onOpenAiServices: () => void }) {
  const { t, i18n } = useTranslation();
  const interfaceLanguage = LANGUAGES.find(l => l.code === i18n.resolvedLanguage)?.name ?? i18n.resolvedLanguage;
  const { sttInfo } = ai;
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.dictation.heading')}</h3><p>{t('settings.dictation.description')}</p></div>
      <Card padding="none" className="settings-card">
        <label htmlFor="settings-microphone" className="settings-label mb-3">
          {t('settings.voice.microphone')}
        </label>
        <SelectField
          id="settings-microphone"
          value={microphones.selectedMic}
          onChange={(e) => microphones.selectMic(e.target.value)}
          className="ui-field w-full"
        >
          {microphones.devices.length === 0 && <option value="">{t('settings.voice.noMicrophone')}</option>}
          {microphones.devices.map(d => (
            <option key={d.deviceId} value={d.deviceId}>
              {d.label || (d.deviceId ? t('settings.voice.unknownMicrophoneId', { id: d.deviceId.substring(0, 5) }) : t('settings.voice.unknownMicrophone'))}
            </option>
          ))}
        </SelectField>
        <div className="flex flex-wrap items-center gap-2 mt-2">
          <Button variant="secondary" type="button" onClick={microphones.requestMicAccess} disabled={microphones.isRequestingMic}>
            {microphones.isRequestingMic ? t('settings.voice.detecting') : t('settings.voice.detect')}
          </Button>
          <span className="text-[11px] text-muted-foreground">
            {t('settings.voice.fullNames')}
          </span>
        </div>
        {microphones.micStatus && (
          <p role="status" className="text-[11px] text-muted-foreground mt-1">{microphones.micStatus}</p>
        )}
        <label htmlFor="settings-dictation-language" className="settings-label settings-label--small mt-4 mb-2 block">
          {t('settings.voice.dictationLanguage')}
        </label>
        <SelectField
          id="settings-dictation-language"
          value={ai.dictationLanguage}
          onChange={event => ai.changeDictationLanguage(event.target.value as DictationLanguage)}
          className={SETTINGS_FIELD}
        >
          <option value="interface">{t('settings.voice.dictationLanguageInterface', { language: interfaceLanguage })}</option>
          {DICTATION_LANGUAGES.map(language => <option key={language.code} value={language.code} lang={language.code}>{language.name}</option>)}
          <option value="multi">{t('settings.voice.dictationLanguageMulti')}</option>
        </SelectField>
        <p className="text-xs text-muted-foreground mt-2">{t('settings.voice.dictationLanguageHint')}</p>
      </Card>

      <Card padding="none" className="settings-card">
        <div className="flex flex-col gap-2">
          <label htmlFor="dictation-auto-structure" className="settings-label">
            <input
              id="dictation-auto-structure"
              type="checkbox"
              checked={ai.structuredDictation}
              onChange={event => ai.changeStructuredDictation(event.target.checked)}
              className="accent-primary"
            />
            {t('settings.voice.autoStructure')}
          </label>
          <p className="text-xs text-muted-foreground">{t('settings.voice.autoStructureHint')}</p>
        </div>
      </Card>

      <Card padding="none" className="settings-card settings-preference-row">
        <div className="settings-preference-copy">
          <h4>{t('settings.dictation.service')}</h4>
          <p role="status">{sttInfo
            ? `${sttInfo.label} · ${sttInfo.ready ? t('settings.dictation.serviceReady') : t('settings.dictation.serviceNotReady')}`
            : t('settings.dictation.serviceNone')}</p>
        </div>
        <Button variant="secondary" type="button" onClick={onOpenAiServices}>{t('settings.dictation.openAiServices')}</Button>
      </Card>
    </div>
  );
}

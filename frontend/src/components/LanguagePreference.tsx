import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { chooseLanguage } from '../i18n';
import { LANGUAGES, systemLanguage, browserLanguages, readLanguagePreference, type LanguagePreferenceValue } from '../i18n/language';
import { Card } from './ui/Card';
import { SelectField } from './ui/Controls';

/** Settings → Appearance: interface language (system language by default). */
export function LanguagePreference() {
  const { t } = useTranslation();
  const [preference, setPreference] = useState<LanguagePreferenceValue>(readLanguagePreference);
  const system = LANGUAGES.find(l => l.code === systemLanguage(browserLanguages()))?.name ?? 'English';
  return (
    <Card padding="none" className="settings-card settings-preference-row">
      <div className="settings-preference-copy"><h4 id="settings-language-label">{t('settings.language.title')}</h4><p>{t('settings.language.description')}</p></div>
      <SelectField
        id="settings-language"
        aria-labelledby="settings-language-label"
        value={preference}
        onChange={event => {
          const nextValue = event.target.value as LanguagePreferenceValue;
          setPreference(nextValue);
          void chooseLanguage(nextValue);
        }}
      >
        <option value="system">{t('settings.language.system', { language: system })}</option>
        {LANGUAGES.map(language => (
          <option key={language.code} value={language.code} lang={language.code}>
            {language.beta ? t('settings.language.beta', { language: language.name }) : language.name}
          </option>
        ))}
      </SelectField>
    </Card>
  );
}

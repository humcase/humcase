import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import en from './locales/en';
import fr from './locales/fr';
import es from './locales/es';
import de from './locales/de';
import ptBR from './locales/pt-BR';
import it from './locales/it';
import {
  LANGUAGE_PREFERENCE_KEY, DEFAULT_LANGUAGE, saveLanguagePreference, browserLanguages, dateLocale,
  readLanguagePreference, resolveLanguage, type LanguageCode, type LanguagePreferenceValue,
} from './language';

/**
 * Interface texts (i18next). One file per language in `locales/`; English is the
 * reference and the text-by-text fallback: a key missing from a language is displayed in English,
 * never empty nor as a technical key.
 */
export const resources = {
  en: { translation: en }, fr: { translation: fr }, es: { translation: es }, de: { translation: de },
  'pt-BR': { translation: ptBR }, it: { translation: it },
} as const;

function applyDocumentLanguage(language: string) {
  if (typeof document !== 'undefined') document.documentElement.lang = language;
}

void i18n.use(initReactI18next).init({
  resources: resources,
  lng: resolveLanguage(readLanguagePreference(), browserLanguages()),
  fallbackLng: DEFAULT_LANGUAGE,
  // React already escapes the displayed text.
  interpolation: { escapeValue: false },
});
applyDocumentLanguage(i18n.language);
i18n.on('languageChanged', applyDocumentLanguage);

/** Changes this window's language and saves it for the other windows and the next launches. */
export function chooseLanguage(preference: LanguagePreferenceValue): Promise<unknown> {
  saveLanguagePreference(preference);
  return i18n.changeLanguage(resolveLanguage(preference, browserLanguages()));
}

// Quick capture is another window: it follows a change made in Settings.
if (typeof window !== 'undefined') {
  window.addEventListener('storage', event => {
    if (event.key === LANGUAGE_PREFERENCE_KEY || event.key === null) {
      void i18n.changeLanguage(resolveLanguage(readLanguagePreference(), browserLanguages()));
    }
  });
}

/** Current language, for `Intl` (dates, numbers, sorting). */
export function currentLanguage(): LanguageCode {
  return (i18n.resolvedLanguage ?? i18n.language ?? DEFAULT_LANGUAGE) as LanguageCode;
}

/** Locale for every date and time shown (see `dateLocale`): one convention across the interface. */
export function currentDateLocale(): string {
  return dateLocale(currentLanguage(), browserLanguages());
}

export default i18n;

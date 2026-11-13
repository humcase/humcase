/**
 * Choice of the interface language (decision of 09/28/2026): the system language if it is
 * available, otherwise English. The user can force one in Settings.
 * Pure functions, with no dependency on i18next: they are testable on their own.
 */

/**
 * Languages for which a texts file exists, with their name in their own language. `beta`: translated
 * from English without a native review yet (docs/TRANSLATION.md), said so in the language menu.
 */
export const LANGUAGES = [
  { code: 'en', name: 'English', beta: false },
  { code: 'fr', name: 'Français', beta: false },
  { code: 'es', name: 'Español', beta: true },
  { code: 'de', name: 'Deutsch', beta: true },
  { code: 'pt-BR', name: 'Português (Brasil)', beta: true },
  { code: 'it', name: 'Italiano', beta: true },
] as const;

export type LanguageCode = (typeof LANGUAGES)[number]['code'];
/** `system`: follow the Windows language. */
export type LanguagePreferenceValue = 'system' | LanguageCode;

export const DEFAULT_LANGUAGE: LanguageCode = 'en';
export const LANGUAGE_PREFERENCE_KEY = 'humcase.language';

const codes: readonly string[] = LANGUAGES.map(l => l.code);
const isCode = (value: string): value is LanguageCode => codes.includes(value);

/**
 * First available language among those of the system, in their order of preference. A regional
 * variant falls back to its language (`fr-CA` → `fr`), and a language to its available variant
 * (`pt` → `pt-BR`).
 */
export function systemLanguage(languages: readonly string[]): LanguageCode {
  for (const brute of languages) {
    const language = brute.trim();
    if (!language) continue;
    const exact = codes.find(code => code.toLowerCase() === language.toLowerCase());
    if (exact && isCode(exact)) return exact;
    const base = language.split('-')[0].toLowerCase();
    const proche = codes.find(code => code.split('-')[0].toLowerCase() === base);
    if (proche && isCode(proche)) return proche;
  }
  return DEFAULT_LANGUAGE;
}

export function resolveLanguage(preference: LanguagePreferenceValue, systemLanguages: readonly string[]): LanguageCode {
  return preference === 'system' ? systemLanguage(systemLanguages) : preference;
}

/** Saved preference; any unknown value (or unavailable storage) is `system`. */
export function readLanguagePreference(): LanguagePreferenceValue {
  try {
    const value = localStorage.getItem(LANGUAGE_PREFERENCE_KEY);
    return value && isCode(value) ? value : 'system';
  } catch {
    return 'system';
  }
}

export function saveLanguagePreference(preference: LanguagePreferenceValue): void {
  try {
    if (preference === 'system') localStorage.removeItem(LANGUAGE_PREFERENCE_KEY);
    else localStorage.setItem(LANGUAGE_PREFERENCE_KEY, preference);
  } catch {
    /* Storage unavailable: the preference only applies to this session. */
  }
}

/** Region of each language's dates when the computer speaks another language: day before month, 24-hour clock. */
const DATE_REGIONS: Record<LanguageCode, string> = { en: 'en-GB', fr: 'fr-FR', es: 'es-ES', de: 'de-DE', 'pt-BR': 'pt-BR', it: 'it-IT' };

/**
 * Locale of dates and times (decision of 10/05/2026): the interface language, with the computer's
 * region when its first language is that same language (`en-US` keeps month/day and the 12-hour
 * clock), otherwise the language's usual region. A French Windows with the English interface thus
 * writes 07/10 and 16:48, like the date fields Windows draws itself.
 */
export function dateLocale(language: LanguageCode, systemLanguages: readonly string[]): string {
  const first = systemLanguages.map(l => l.trim()).find(Boolean);
  if (first && first.includes('-') && first.split('-')[0].toLowerCase() === language.split('-')[0].toLowerCase()) {
    try {
      return new Intl.DateTimeFormat(first).resolvedOptions().locale;
    } catch {
      /* Unknown tag: the usual region below. */
    }
  }
  return DATE_REGIONS[language];
}

export function browserLanguages(): readonly string[] {
  if (typeof navigator === 'undefined') return [];
  return navigator.languages?.length ? navigator.languages : [navigator.language].filter(Boolean);
}

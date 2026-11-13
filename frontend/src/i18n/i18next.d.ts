import 'i18next';
import type fr from './locales/fr';

// Keys checked at compile time: `t('missing.key')` is a TypeScript error. French
// serves as the list of keys during extraction (it is the current language of the code); a test requires
// that English has exactly the same keys.
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    resources: { translation: typeof fr };
  }
}

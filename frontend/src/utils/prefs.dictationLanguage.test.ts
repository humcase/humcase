import { describe, it, expect, beforeEach } from 'vitest';
import { dictationLanguageCode, getDictationLanguage, setDictationLanguage } from './prefs';

describe('dictation language', () => {
  beforeEach(() => localStorage.clear());

  it('follows the interface language until another is chosen', () => {
    expect(getDictationLanguage()).toBe('interface');
    expect(dictationLanguageCode(getDictationLanguage(), 'en')).toBe('en');
    expect(dictationLanguageCode(getDictationLanguage(), 'fr')).toBe('fr');
  });

  it('keeps the chosen language, whatever the interface language', () => {
    setDictationLanguage('pt-BR');
    expect(dictationLanguageCode(getDictationLanguage(), 'fr')).toBe('pt-BR');
    setDictationLanguage('multi');
    expect(dictationLanguageCode(getDictationLanguage(), 'en')).toBe('multi');
    setDictationLanguage('interface');
    expect(localStorage.getItem('dictation_language')).toBeNull();
  });

  it('reads an unknown saved value as the interface language', () => {
    localStorage.setItem('dictation_language', 'klingon');
    expect(getDictationLanguage()).toBe('interface');
  });
});

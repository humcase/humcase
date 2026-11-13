import { afterEach, describe, expect, it } from 'vitest';
import { LANGUAGE_PREFERENCE_KEY, saveLanguagePreference, systemLanguage, readLanguagePreference, resolveLanguage, dateLocale } from './language';

describe('interface language', () => {
  afterEach(() => localStorage.clear());

  it('follows the first available system language', () => {
    expect(systemLanguage(['fr-FR', 'en-US'])).toBe('fr');
    expect(systemLanguage(['ja-JP', 'fr-CA'])).toBe('fr');
    expect(systemLanguage(['en-GB'])).toBe('en');
    expect(systemLanguage(['de-AT'])).toBe('de');
    expect(systemLanguage(['pt-PT'])).toBe('pt-BR');
    expect(systemLanguage(['es-MX', 'en-US'])).toBe('es');
  });

  it('falls back to English when no system language is available', () => {
    expect(systemLanguage(['ja-JP', 'zh-CN'])).toBe('en');
    expect(systemLanguage([])).toBe('en');
  });

  it('respects a language forced in Settings', () => {
    expect(resolveLanguage('en', ['fr-FR'])).toBe('en');
    expect(resolveLanguage('system', ['fr-FR'])).toBe('fr');
  });

  it('saves the preference and ignores an unknown value', () => {
    expect(readLanguagePreference()).toBe('system');
    saveLanguagePreference('fr');
    expect(readLanguagePreference()).toBe('fr');
    saveLanguagePreference('system');
    expect(localStorage.getItem(LANGUAGE_PREFERENCE_KEY)).toBeNull();
    localStorage.setItem(LANGUAGE_PREFERENCE_KEY, 'klingon');
    expect(readLanguagePreference()).toBe('system');
  });
});

describe('date locale', () => {
  it('keeps the region of the computer when it speaks the interface language', () => {
    expect(dateLocale('en', ['en-US', 'en'])).toBe('en-US');
    expect(dateLocale('en', ['en-GB'])).toBe('en-GB');
    expect(dateLocale('fr', ['fr-CA', 'en-US'])).toBe('fr-CA');
    expect(dateLocale('pt-BR', ['pt-BR'])).toBe('pt-BR');
    expect(dateLocale('de', ['de-CH'])).toBe('de-CH');
    expect(dateLocale('it', ['fr-FR'])).toBe('it-IT');
  });

  it('uses the usual region of the language otherwise: day before month, 24-hour clock', () => {
    // English interface on a French Windows: like the date fields Windows draws itself.
    expect(dateLocale('en', ['fr-FR', 'fr', 'en-US', 'en'])).toBe('en-GB');
    expect(dateLocale('fr', ['en-US'])).toBe('fr-FR');
    expect(dateLocale('en', ['en'])).toBe('en-GB');
    expect(dateLocale('en', [])).toBe('en-GB');
  });
});

import { describe, expect, it } from 'vitest';
import { MISSING_KEY_CODE, aiErrorMessage, isMissingKeyError } from './aiErrors';

describe('aiErrors — "missing key" contract with Rust', () => {
  it('keeps the same code as src-tauri/src/providers.rs (MISSING_KEY_CODE)', () => {
    expect(MISSING_KEY_CODE).toBe('ai_key_missing');
  });

  it('recognizes the missing key by the code, whatever the language of the text', () => {
    expect(isMissingKeyError('[ai_key_missing] CEREBRAS_API_KEY manquante')).toBe(true);
    expect(isMissingKeyError('[ai_key_missing] CEREBRAS_API_KEY is missing')).toBe(true);
  });

  it('does not rely on the variable name in the text', () => {
    expect(isMissingKeyError('CEREBRAS_API_KEY manquante')).toBe(false);
    expect(isMissingKeyError('Cerebras HTTP 401 (OPENAI_API_KEY refused)')).toBe(false);
    expect(isMissingKeyError('erreur [ai_key_missing] au milieu')).toBe(false);
  });

  it('removes the code from the displayed text and leaves other messages intact', () => {
    expect(aiErrorMessage('[ai_key_missing] CEREBRAS_API_KEY manquante')).toBe('CEREBRAS_API_KEY manquante');
    expect(aiErrorMessage('Cerebras HTTP 401')).toBe('Cerebras HTTP 401');
  });
});

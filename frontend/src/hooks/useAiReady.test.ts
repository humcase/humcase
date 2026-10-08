import { describe, expect, it } from 'vitest';
import { effectiveCleanProvider } from './useAiReady';

describe('effectiveCleanProvider', () => {
  const list = (ready: string[]) =>
    ['cerebras', 'openai', 'mistral', 'ollama', 'custom'].map(id => ({ id, ready: ready.includes(id) }));

  it('keeps the chosen provider when it is ready', () => {
    expect(effectiveCleanProvider(list(['cerebras', 'mistral']), 'mistral')).toBe('mistral');
  });

  it('uses the AI whose key is set when the chosen one has none', () => {
    expect(effectiveCleanProvider(list(['mistral', 'ollama']), 'cerebras')).toBe('mistral');
  });

  it('never takes the local server by default, its address being always known', () => {
    expect(effectiveCleanProvider(list(['ollama']), 'cerebras')).toBe('cerebras');
    expect(effectiveCleanProvider(list(['ollama']), 'ollama')).toBe('ollama');
  });
});

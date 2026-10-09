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

  it('with nothing chosen, uses the first AI whose key is set, else the default', () => {
    expect(effectiveCleanProvider(list(['cerebras', 'mistral']), '')).toBe('cerebras');
    expect(effectiveCleanProvider(list(['ollama']), '')).toBe('cerebras');
  });

  it('never takes the local server by default, its address being always known', () => {
    expect(effectiveCleanProvider(list(['ollama']), 'cerebras')).toBe('cerebras');
    expect(effectiveCleanProvider(list(['ollama']), 'ollama')).toBe('ollama');
  });
});

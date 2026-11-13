import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import type { AiUsageRecord } from '../../types';
import { api } from '../../services/tauriApi';
import { UsageSection } from './UsageSection';
import { readSource } from '../../test/cssSources';
import en from '../../i18n/locales/en/settings.json';
import fr from '../../i18n/locales/fr/settings.json';

vi.mock('../../services/tauriApi', () => ({ api: { readAiUsage: vi.fn(), openDataDir: vi.fn() } }));

const now = Date.now();
const record = (fields: Partial<AiUsageRecord>): AiUsageRecord => ({
  at_unix_ms: now, operation: 'structure', provider: 'cerebras', model: 'gpt-oss-120b', language: null,
  input_tokens: 700, output_tokens: 500, reasoning_tokens: 400, audio_seconds: null, ok: true, ...fields,
});

beforeEach(() => { vi.resetAllMocks(); });

describe('Settings → Consumption', () => {
  it('shows minutes, calls and tokens, and never a price', async () => {
    // A record as the file holds it: the estimated cost must not reach the screen even if sent.
    const withCost = { ...record({}), estimated_cost_usd: 0.00051 } as AiUsageRecord;
    vi.mocked(api.readAiUsage).mockResolvedValue([
      record({ operation: 'transcription', provider: 'deepgram', model: 'nova-3', language: 'multi', audio_seconds: 300, input_tokens: null, output_tokens: null, reasoning_tokens: null }),
      withCost,
      record({ operation: 'clean', provider: 'custom', model: 'mistral-small-latest', reasoning_tokens: null }),
    ]);
    const { container } = render(<UsageSection />);

    expect(await screen.findByText('Minutes dictées')).toBeTruthy();
    expect(vi.mocked(api.readAiUsage)).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Personnalisé')).toBeTruthy();
    expect(screen.getByText(/nova-3 · plusieurs langues/)).toBeTruthy();
    expect(container.textContent).not.toMatch(/\$|€|USD|0[.,]00051/);

    const clean = screen.getByRole('button', { name: /Nettoyage de dictée/ });
    fireEvent.click(clean);
    expect(clean.getAttribute('aria-expanded')).toBe('true');
    expect(within(clean.closest('li')!).getByText(/raisonnement non indiqué/)).toBeTruthy();
  });

  it('invites to dictate when the period is empty', async () => {
    vi.mocked(api.readAiUsage).mockResolvedValue([]);
    render(<UsageSection />);
    expect(await screen.findByText('Rien sur cette période')).toBeTruthy();
  });
});

describe('Settings → Consumption, every recorded operation', () => {
  // The engine's list (src-tauri `ai_usage::operation`): an operation without a label showed its raw
  // name ("germinate") and the default icon.
  const operations = [...readSource('../../src-tauri/src/ai_usage.rs').matchAll(/pub const [A-Z_]+: &str = "([a-z_]+)";/g)].map(match => match[1])
    .filter(name => name !== 'ai-usage.jsonl');
  const icons = readSource('components/settings/UsageSection.tsx').match(/const OPERATION_ICONS[^{]*\{([^}]*)\}/)![1];

  it('reads the engine list', () => {
    expect(operations).toContain('germinate');
    expect(operations.length).toBeGreaterThanOrEqual(11);
  });

  it.each(operations)('%s has a label in English and French, and an icon', operation => {
    expect((en.usage.operation as Record<string, string>)[operation]).toBeTruthy();
    expect((fr.usage.operation as Record<string, string>)[operation]).toBeTruthy();
    expect(icons).toMatch(new RegExp(`\\b${operation}:`));
  });
});

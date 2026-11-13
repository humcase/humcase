import { describe, it, expect } from 'vitest';
import type { AiUsageRecord } from '../types';
import { formatMinutes, periodDays, periodStart, summarizeUsage } from './aiUsage';

const at = (y: number, m: number, d: number, h = 12) => new Date(y, m - 1, d, h).getTime();
const record = (fields: Partial<AiUsageRecord>): AiUsageRecord => ({
  at_unix_ms: at(2026, 10, 3), operation: 'structure', provider: 'cerebras', model: 'gpt-oss-120b',
  language: null, input_tokens: null, output_tokens: null, reasoning_tokens: null, audio_seconds: null, ok: true, ...fields,
});

describe('consumption periods', () => {
  const now = new Date(2026, 9, 3, 15, 30);

  it('counts whole local days, today included', () => {
    expect(periodDays('7d', now)).toEqual(['2026-09-27', '2026-09-28', '2026-09-29', '2026-09-30', '2026-10-01', '2026-10-02', '2026-10-03']);
    expect(periodDays('30d', now)).toHaveLength(30);
    expect(periodDays('month', now)).toEqual(['2026-10-01', '2026-10-02', '2026-10-03']);
    expect(periodStart('month', now).getTime()).toBe(new Date(2026, 9, 1).getTime());
  });
});

describe('consumption summary', () => {
  const days = periodDays('7d', new Date(2026, 9, 3, 15));

  it('separates dictation audio from language model tokens, per operation and per model', () => {
    const summary = summarizeUsage([
      record({ operation: 'transcription', provider: 'deepgram', model: 'nova-3', language: 'fr', audio_seconds: 90, at_unix_ms: at(2026, 10, 2) }),
      record({ operation: 'transcription', provider: 'deepgram', model: 'nova-3', language: 'multi', audio_seconds: 30 }),
      record({ operation: 'structure', input_tokens: 700, output_tokens: 500, reasoning_tokens: 400 }),
      record({ operation: 'structure', input_tokens: 300, output_tokens: 100, reasoning_tokens: 60 }),
      record({ operation: 'clean', provider: 'custom', model: 'mistral-small-latest', input_tokens: 200, output_tokens: 80 }),
      record({ operation: 'integrate', ok: false }),
      record({ operation: 'structure', input_tokens: 9999, at_unix_ms: at(2026, 9, 20) }),
    ], days);

    expect(summary.sessions).toBe(2);
    expect(summary.audioSeconds).toBe(120);
    expect(summary.calls).toBe(4);
    expect(summary.inputTokens).toBe(1200);
    expect(summary.outputTokens).toBe(680);
    expect(summary.reasoningTokens).toBe(460);
    expect(summary.days.find(d => d.date === '2026-10-02')?.audioSeconds).toBe(90);
    expect(summary.days.find(d => d.date === '2026-10-03')?.audioSeconds).toBe(30);

    expect(summary.operations.map(o => o.operation)).toEqual(['transcription', 'integrate', 'structure', 'clean']);
    const structure = summary.operations.find(o => o.operation === 'structure')!;
    expect(structure).toMatchObject({ calls: 2, inputTokens: 1000, outputTokens: 600, reasoningTokens: 460, reasoningKnown: true });
    expect(summary.operations.find(o => o.operation === 'clean')!.reasoningKnown).toBe(false);
    expect(summary.operations.find(o => o.operation === 'integrate')).toMatchObject({ calls: 1, failed: 1 });

    expect(summary.models.map(m => [m.provider, m.model, m.language])).toEqual([
      ['deepgram', 'nova-3', 'fr'],
      ['deepgram', 'nova-3', 'multi'],
      ['cerebras', 'gpt-oss-120b', null],
      ['custom', 'mistral-small-latest', null],
    ]);
  });

  it('is empty, with every day at zero, without records', () => {
    const summary = summarizeUsage([], days);
    expect(summary.calls + summary.sessions).toBe(0);
    expect(summary.days).toHaveLength(7);
    expect(summary.days.every(d => d.audioSeconds === 0)).toBe(true);
  });

  it('shows minutes without hiding a short dictation', () => {
    expect(formatMinutes(0)).toBe('0');
    expect(formatMinutes(20)).toBe('< 1');
    expect(formatMinutes(150)).toBe('3');
  });
});

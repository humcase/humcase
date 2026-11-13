import type { AiUsageRecord } from '../types';
import { todayLocalIso } from './datetime';

/**
 * Settings → Consumption: what the AI calls and dictations of this computer consumed, as
 * quantities only (founder decision of 03/10: no price in the app, whatever the model). Pure
 * functions over the records of `read_ai_usage`, testable on their own.
 */

export type UsagePeriod = '7d' | '30d' | 'month';

/** Operations in display order; an operation unknown to this version comes after them. */
export const USAGE_OPERATIONS = ['transcription', 'integrate', 'structure', 'clean', 'structure_agent'] as const;

/** First local day of the period, at midnight. */
export function periodStart(period: UsagePeriod, now: Date): Date {
  if (period === 'month') return new Date(now.getFullYear(), now.getMonth(), 1);
  const days = period === '7d' ? 7 : 30;
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() - (days - 1));
}

/** Local days of the period, oldest first, today included. */
export function periodDays(period: UsagePeriod, now: Date): string[] {
  const start = periodStart(period, now);
  const today = todayLocalIso(now);
  const days: string[] = [];
  for (let day = start; ; day = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1)) {
    const iso = todayLocalIso(day);
    days.push(iso);
    if (iso >= today) return days;
  }
}

export interface OperationSummary {
  operation: string;
  /** Calls (a transcription session counts as one). */
  calls: number;
  failed: number;
  audioSeconds: number;
  inputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  /** False when no call of this operation reported its reasoning tokens. */
  reasoningKnown: boolean;
  /** "provider model" pairs used, in order of first use. */
  models: { provider: string; model: string }[];
}

export interface ModelSummary {
  provider: string;
  model: string;
  /** Language of a transcription (`multi`: several); none for a language model. */
  language: string | null;
  calls: number;
  audioSeconds: number;
  inputTokens: number;
  outputTokens: number;
}

export interface UsageSummary {
  /** Dictation sessions and their audio. */
  sessions: number;
  audioSeconds: number;
  /** Language model calls (transcription excluded) and their tokens. */
  calls: number;
  inputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  days: { date: string; audioSeconds: number }[];
  operations: OperationSummary[];
  models: ModelSummary[];
}

const isTranscription = (record: AiUsageRecord) => record.operation === 'transcription';

export function summarizeUsage(records: AiUsageRecord[], days: string[]): UsageSummary {
  const inPeriod = new Set(days);
  const perDay = new Map(days.map(day => [day, 0]));
  const operations = new Map<string, OperationSummary>();
  const models = new Map<string, ModelSummary>();
  const summary: UsageSummary = { sessions: 0, audioSeconds: 0, calls: 0, inputTokens: 0, outputTokens: 0, reasoningTokens: 0, days: [], operations: [], models: [] };

  for (const record of records) {
    const day = todayLocalIso(new Date(record.at_unix_ms));
    if (!inPeriod.has(day)) continue;
    const audio = record.audio_seconds ?? 0;
    const input = record.input_tokens ?? 0;
    const output = record.output_tokens ?? 0;
    const reasoning = record.reasoning_tokens ?? 0;
    if (isTranscription(record)) {
      summary.sessions += 1;
      summary.audioSeconds += audio;
      perDay.set(day, (perDay.get(day) ?? 0) + audio);
    } else {
      summary.calls += 1;
      summary.inputTokens += input;
      summary.outputTokens += output;
      summary.reasoningTokens += reasoning;
    }

    let operation = operations.get(record.operation);
    if (!operation) {
      operation = { operation: record.operation, calls: 0, failed: 0, audioSeconds: 0, inputTokens: 0, outputTokens: 0, reasoningTokens: 0, reasoningKnown: false, models: [] };
      operations.set(record.operation, operation);
    }
    operation.calls += 1;
    if (!record.ok) operation.failed += 1;
    operation.audioSeconds += audio;
    operation.inputTokens += input;
    operation.outputTokens += output;
    operation.reasoningTokens += reasoning;
    if (record.reasoning_tokens != null) operation.reasoningKnown = true;
    if (!operation.models.some(m => m.provider === record.provider && m.model === record.model)) {
      operation.models.push({ provider: record.provider, model: record.model });
    }

    const language = isTranscription(record) ? record.language ?? null : null;
    const key = `${record.provider}\u0000${record.model}\u0000${language ?? ''}`;
    let model = models.get(key);
    if (!model) {
      model = { provider: record.provider, model: record.model, language, calls: 0, audioSeconds: 0, inputTokens: 0, outputTokens: 0 };
      models.set(key, model);
    }
    model.calls += 1;
    model.audioSeconds += audio;
    model.inputTokens += input;
    model.outputTokens += output;
  }

  const order = (operation: string) => {
    const index = (USAGE_OPERATIONS as readonly string[]).indexOf(operation);
    return index === -1 ? USAGE_OPERATIONS.length : index;
  };
  summary.days = days.map(date => ({ date, audioSeconds: perDay.get(date) ?? 0 }));
  summary.operations = [...operations.values()].sort((a, b) => order(a.operation) - order(b.operation) || b.calls - a.calls);
  // Transcription first (by audio), then the language models by number of calls.
  summary.models = [...models.values()].sort((a, b) =>
    (b.audioSeconds > 0 ? 1 : 0) - (a.audioSeconds > 0 ? 1 : 0) || b.audioSeconds - a.audioSeconds || b.calls - a.calls);
  return summary;
}

/** Whole minutes; under a minute of real audio reads "< 1" rather than 0. */
export function formatMinutes(seconds: number): string {
  if (seconds <= 0) return '0';
  if (seconds < 60) return '< 1';
  return String(Math.round(seconds / 60));
}

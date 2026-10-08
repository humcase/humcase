import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { ArrowsMerge, CaretDown, ChartBar, ChatCircleText, ChatsCircle, Eraser, FolderOpen, Gavel, IdentificationCard, ListChecks, Microphone, Plant, Robot, Stack, Swap, TreeStructure } from '@phosphor-icons/react';
import { api } from '../../services/tauriApi';
import { formatMinutes, periodDays, summarizeUsage, type ModelSummary, type OperationSummary, type UsagePeriod } from '../../utils/aiUsage';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { EmptyState } from '../ui/EmptyState';
import { SegmentedControl } from '../ui/SegmentedControl';
import { useAiUsage } from './useAiUsage';
import { currentDateLocale } from '../../i18n';

/** Brand names of the providers; only "custom" is translated. */
const PROVIDER_NAMES: Record<string, string> = {
  cerebras: 'Cerebras', deepgram: 'Deepgram', openai: 'OpenAI', groq: 'Groq', openrouter: 'OpenRouter', mistral: 'Mistral', anthropic: 'Anthropic', ollama: 'Ollama', typesafe: 'TypeSafe',
};

// One per operation of `ai_usage::operation` (src-tauri), checked by the test.
const OPERATION_ICONS: Record<string, typeof Microphone> = {
  transcription: Microphone, integrate: ArrowsMerge, structure: TreeStructure, clean: Eraser, structure_agent: Robot, check_nature: Swap,
  germinate: Plant, check_proposals: ListChecks, check_decisions: Gavel, check_gather: Stack, check_profile: IdentificationCard,
  ask: ChatCircleText,
  reflect: ChatsCircle,
};

function providerName(provider: string, t: TFunction): string {
  return provider === 'custom' ? t('settings.usage.customProvider') : PROVIDER_NAMES[provider] ?? provider;
}

/** Settings → Consumption: dictated minutes, AI calls and tokens, never a price (decision of 03/10). */
export function UsageSection() {
  const { t, i18n } = useTranslation();
  const [period, setPeriod] = useState<UsagePeriod>('30d');
  const [open, setOpen] = useState<string | null>(null);
  const { records, error, now } = useAiUsage(period);
  const language = i18n.resolvedLanguage ?? i18n.language;
  const summary = useMemo(() => (records && now ? summarizeUsage(records, periodDays(period, now)) : null), [records, period, now]);

  const tokens = (value: number) => new Intl.NumberFormat(language, { notation: 'compact', maximumFractionDigits: 1 }).format(value);
  const languageName = (code: string) => {
    if (code === 'multi') return t('settings.usage.severalLanguages');
    try { return new Intl.DisplayNames([language], { type: 'language' }).of(code) ?? code; } catch { return code; }
  };
  const modelLabel = (m: Pick<ModelSummary, 'model'> & { language?: string | null }) => m.language ? `${m.model} · ${languageName(m.language)}` : m.model;
  const operationName = (operation: string) => t(`settings.usage.operation.${operation}`, { defaultValue: operation });

  const operationValue = (o: OperationSummary) => o.operation === 'transcription'
    ? t('settings.usage.minutes', { minutes: formatMinutes(o.audioSeconds) })
    : t('settings.usage.calls', { count: o.calls });
  const operationDetail = (o: OperationSummary) => {
    const models = o.models.map(m => `${providerName(m.provider, t)} ${m.model}`).join(', ');
    const parts = [o.operation === 'transcription'
      ? t('settings.usage.detail.audio', { models, minutes: formatMinutes(o.audioSeconds / o.calls) })
      : t(o.reasoningKnown ? 'settings.usage.detail.tokens' : 'settings.usage.detail.tokensNoReasoning', {
        models,
        input: tokens(Math.round(o.inputTokens / o.calls)),
        output: tokens(Math.round(o.outputTokens / o.calls)),
        reasoning: tokens(Math.round(o.reasoningTokens / o.calls)),
      })];
    if (o.operation === 'transcription') parts.push(t('settings.usage.sessions', { count: o.calls }));
    if (o.failed > 0) parts.push(t('settings.usage.detail.failed', { count: o.failed }));
    return parts.join(' · ');
  };

  const isEmpty = summary !== null && summary.calls + summary.sessions === 0;
  const busiestOperation = summary ? Math.max(1, ...summary.operations.filter(o => o.operation !== 'transcription').map(o => o.calls)) : 1;
  const busiestDay = summary ? Math.max(1, ...summary.days.map(d => d.audioSeconds)) : 1;

  return (
    <div className="settings-panel settings-usage">
      <div className="settings-usage__heading">
        <div className="settings-section-heading"><h3>{t('settings.usage.heading')}</h3><p>{t('settings.usage.description')}</p></div>
        <SegmentedControl size="sm" label={t('settings.usage.periodLabel')} value={period} onValueChange={value => { setPeriod(value); setOpen(null); }}
          items={(['7d', '30d', 'month'] as const).map(value => ({ value, label: t(`settings.usage.period.${value}`) }))} />
      </div>

      {error && <p role="alert" className="settings-alert">{t('settings.usage.failed', { error })}</p>}
      {!error && !summary && <p role="status" className="text-xs text-muted-foreground">{t('settings.usage.loading')}</p>}
      {isEmpty && <Card padding="none" className="settings-card"><EmptyState icon={<ChartBar size={22} />} title={t('settings.usage.empty.title')} description={t('settings.usage.empty.description')} /></Card>}

      {summary && !isEmpty && <>
        <div className="settings-usage__stats">
          <div className="settings-usage__stat"><span>{t('settings.usage.stats.minutes')}</span><strong>{formatMinutes(summary.audioSeconds)}</strong></div>
          <div className="settings-usage__stat"><span>{t('settings.usage.stats.calls')}</span><strong>{summary.calls}</strong></div>
          <div className="settings-usage__stat"><span>{t('settings.usage.stats.input')}</span><strong>{tokens(summary.inputTokens)}</strong></div>
          <div className="settings-usage__stat"><span>{t('settings.usage.stats.output')}</span><strong>{tokens(summary.outputTokens)}</strong>
            {summary.reasoningTokens > 0 && <small>{t('settings.usage.stats.reasoning', { tokens: tokens(summary.reasoningTokens) })}</small>}
          </div>
        </div>
        <p className="settings-usage__hint">{t('settings.usage.tokensHint')}</p>

        <Card padding="none" className="settings-card">
          <h4 className="settings-usage__title">{t('settings.usage.daily')}</h4>
          <div className="settings-usage__days" role="img" aria-label={t('settings.usage.dailyAria')}>
            {summary.days.map(day => (
              <span key={day.date} title={t('settings.usage.dayTitle', { date: new Date(`${day.date}T12:00:00`).toLocaleDateString(currentDateLocale(), { day: 'numeric', month: 'short' }), minutes: formatMinutes(day.audioSeconds) })}
                className="settings-usage__day" data-empty={day.audioSeconds === 0 || undefined}
                style={{ height: `${Math.max(4, (day.audioSeconds / busiestDay) * 100)}%` }} />
            ))}
          </div>
        </Card>

        <Card padding="none" className="settings-card">
          <h4 className="settings-usage__title">{t('settings.usage.byOperation')}</h4>
          <ul className="settings-usage__list">
            {summary.operations.map(o => {
              const Icon = OPERATION_ICONS[o.operation] ?? ChartBar;
              // Transcription is in minutes, alone on its scale: a full bar, or none for 0 min.
              const share = o.operation === 'transcription' ? (o.audioSeconds > 0 ? 100 : 0) : (o.calls / busiestOperation) * 100;
              const expanded = open === o.operation;
              return (
                <li key={o.operation}>
                  <button type="button" className="settings-usage__row" aria-expanded={expanded} onClick={() => setOpen(expanded ? null : o.operation)}>
                    <Icon size={18} aria-hidden="true" />
                    <span className="settings-usage__row-main">
                      <span className="settings-usage__row-head"><span>{operationName(o.operation)}</span><strong>{operationValue(o)}</strong></span>
                      <span className="settings-usage__bar" aria-hidden="true"><span style={{ width: `${Math.max(2, share)}%` }} /></span>
                    </span>
                    <CaretDown size={16} aria-hidden="true" className="settings-usage__caret" />
                  </button>
                  {expanded && <p className="settings-usage__detail">{operationDetail(o)}</p>}
                </li>
              );
            })}
          </ul>
        </Card>

        <Card padding="none" className="settings-card">
          <h4 className="settings-usage__title">{t('settings.usage.byModel')}</h4>
          <ul className="settings-usage__models">
            {summary.models.map(m => (
              <li key={`${m.provider}/${m.model}/${m.language ?? ''}`}>
                <span><strong>{providerName(m.provider, t)}</strong> {modelLabel(m)}</span>
                <span>{m.audioSeconds > 0
                  ? t('settings.usage.modelAudio', { minutes: formatMinutes(m.audioSeconds), sessions: t('settings.usage.sessions', { count: m.calls }) })
                  : t('settings.usage.modelTokens', { calls: t('settings.usage.calls', { count: m.calls }), input: tokens(m.inputTokens), output: tokens(m.outputTokens) })}</span>
              </li>
            ))}
          </ul>
        </Card>
      </>}

      <div className="settings-usage__footer">
        <p>{t('settings.usage.fileHint')}</p>
        <Button variant="secondary" type="button" onClick={() => { void api.openDataDir().catch(() => {}); }}>
          <FolderOpen size={16} aria-hidden="true" />{t('settings.usage.openFolder')}
        </Button>
      </div>
    </div>
  );
}

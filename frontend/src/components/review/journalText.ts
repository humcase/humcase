import type { TFunction } from 'i18next';
import type { CaseEvent, CaseReview } from '../../types';

/** `true` when the person wrote it in the interface. */
export const isPerson = (event: Pick<CaseEvent, 'actor_kind'>) => event.actor_kind === 'human';

const capitalize = (text: string) => text.charAt(0).toLocaleUpperCase() + text.slice(1);

/** Who acted: "you", the agent's name, "the CLI"… (lower case, for the middle of a sentence). */
export function actorLabel(event: Pick<CaseEvent, 'actor_kind' | 'actor_name'>, t: TFunction): string {
  if (event.actor_kind === 'human') return t('notes.activity.you');
  if (event.actor_name) return event.actor_name;
  if (event.actor_kind === 'cli') return t('notes.activity.cli');
  if (event.actor_kind === 'agent') return t('notes.activity.agent');
  return t('notes.activity.unknown');
}

/** Same, at the start of a sentence or alone on a badge. */
export const actorTitle = (event: Pick<CaseEvent, 'actor_kind' | 'actor_name'>, t: TFunction) => capitalize(actorLabel(event, t));

const text = (value: unknown) => (typeof value === 'string' ? value : '');

/** One sentence for a recorded write ("Claude Code changed: content, title"). */
export function describeEvent(event: CaseEvent, t: TFunction): string {
  const options = { actor: actorTitle(event, t), context: isPerson(event) ? 'you' : undefined };
  const detail = event.detail ?? {};
  switch (event.kind) {
    case 'edited':
      return t('notes.activity.edited', { ...options, fields: event.fields.map(field => t(`notes.activity.field.${field}`, { defaultValue: field })).join(', ') });
    case 'status':
      return t('notes.activity.status', {
        ...options,
        from: t(`notes.status.${text(detail.from)}`, { defaultValue: text(detail.from) }),
        to: t(`notes.status.${text(detail.to)}`, { defaultValue: text(detail.to) }),
      });
    case 'report':
      return t(detail.cleared === true ? 'notes.activity.reportCleared' : 'notes.activity.report', options);
    case 'created':
    case 'moved':
    case 'trashed':
    case 'restored':
    case 'archived':
    case 'unarchived':
      return t(`notes.activity.${event.kind}`, options);
    default:
      return t('notes.activity.other', options);
  }
}

/** One sentence for the person's decision on a result. */
export function describeReview(review: CaseReview, t: TFunction): string {
  return review.decision === 'accepted'
    ? t('notes.activity.reviewAccepted', { revision: review.revision })
    : t('notes.activity.reviewRework', { number: review.rework_number ?? 1, reason: review.comment ?? '' });
}

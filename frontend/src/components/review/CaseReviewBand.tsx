import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { CheckCircle, HourglassMedium, Robot } from '@phosphor-icons/react';
import type { CaseJournal, CaseReview, Note } from '../../types';
import { formatDateTime, formatRelative } from '../../utils/datetime';
import { transformerUrlMarkdown } from '../../utils/imageRefs';
import { Badge } from '../ui/Badge';
import { ReviewActions } from './ReviewActions';
import { actorTitle } from './journalText';
import './review.css';

interface CaseReviewBandProps {
  note: Note;
  journal: CaseJournal | null;
  onReviewed: (review: CaseReview) => void;
}

/**
 * Band under the title of a case whose result waits for the person, was sent back for a rework,
 * or was accepted. The declared status and the person's decision stay two distinct facts.
 */
export function CaseReviewBand({ note, journal, onReviewed }: CaseReviewBandProps) {
  const { t } = useTranslation();
  const review = journal?.review;
  if (!review || review.state === 'none') return null;
  const declared = review.declared;
  const last = review.last_review;
  const lastActor = journal.events.find(event => event.actor_kind !== 'human');

  if (review.state === 'rework_requested') {
    return (
      <section className="case-review-band" data-state="rework" aria-label={t('notes.review.reworkNumber', { number: last?.rework_number ?? review.rework_count })}>
        <p className="case-review-band__title"><HourglassMedium size={16} aria-hidden="true" />{t('notes.review.reworkPending', { number: last?.rework_number ?? review.rework_count })}</p>
        {last?.comment && <p className="case-review-band__reason">{t('notes.review.reworkReason', { reason: last.comment })}</p>}
      </section>
    );
  }

  if (review.state === 'accepted') {
    return (
      <section className="case-review-band" data-state="accepted" aria-label={t('notes.review.accepted', { when: '' })}>
        <p className="case-review-band__title"><CheckCircle size={16} aria-hidden="true" />
          <span title={last ? formatDateTime(last.created_at) : undefined}>{t('notes.review.accepted', { when: last ? formatRelative(last.created_at) : '' })}</span>
          {review.rework_count > 0 && <Badge tone="neutral">{t('notes.review.reworkNumber', { number: review.rework_count })}</Badge>}
        </p>
      </section>
    );
  }

  const title = review.state === 'accepted_changed'
    ? t('notes.review.acceptedChanged', { actor: lastActor ? actorTitle(lastActor, t) : t('notes.activity.agent') })
    : t('notes.review.declaredBy', { actor: declared ? actorTitle(declared, t) : t('notes.activity.agent') });
  return (
    <section className="case-review-band" data-state="to-review" aria-label={title}>
      <div className="case-review-band__head">
        <p className="case-review-band__title"><Robot size={16} aria-hidden="true" />{title}</p>
        {declared && <time className="case-review-band__when" dateTime={declared.created_at} title={formatDateTime(declared.created_at)}>{formatRelative(declared.created_at)}</time>}
        {review.rework_count > 0 && <Badge tone="warning">{t('notes.review.reworkHandedBack', { number: review.rework_count })}</Badge>}
      </div>
      {/* The decision is taken on the result: show it here, not only in the folded report below. */}
      {note.resolution_comment?.trim() && (
        <div className="case-review-band__report prose max-w-none" role="group" aria-label={t('notes.followUp.report')} tabIndex={0}>
          <ReactMarkdown remarkPlugins={[remarkGfm]} urlTransform={transformerUrlMarkdown}>{note.resolution_comment}</ReactMarkdown>
        </div>
      )}
      <p className="case-review-band__hint">{t('notes.review.declaredHint')}</p>
      <ReviewActions note={note} onReviewed={onReviewed} />
    </section>
  );
}

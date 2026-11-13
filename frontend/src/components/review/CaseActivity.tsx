import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Clock } from '@phosphor-icons/react';
import type { CaseJournal } from '../../types';
import { formatDateTime, formatRelative } from '../../utils/datetime';
import { Notice } from '../ui/Toast';
import { describeEvent, describeReview, isPerson } from './journalText';
import '../NoteHistory.css';
import './review.css';

interface CaseActivityProps {
  journal: CaseJournal | null;
  failed: boolean;
  /** The kept versions of the case, listed under the journal in the same history. */
  children?: ReactNode;
}

/**
 * The case's history: who did what, the person and the agents side by side, with the decisions on
 * its result, then the versions kept. Only recorded writes are listed: a case older than the
 * journal says so.
 */
export function CaseActivity({ journal, failed, children }: CaseActivityProps) {
  const { t } = useTranslation();
  const heading = <h3 className="note-history-heading"><Clock size={15} aria-hidden="true" /> {t('notes.history.heading')}</h3>;
  if (failed) {
    return <section className="note-history case-activity" aria-label={t('notes.history.label')}>{heading}<Notice tone="danger">{t('notes.activity.error')}</Notice>{children}</section>;
  }
  if (!journal) return null;
  const rows = [
    ...journal.events.map(event => ({ id: event.id, at: event.last_at, text: describeEvent(event, t), agent: !isPerson(event) })),
    ...journal.reviews.map(review => ({ id: review.id, at: review.created_at, text: describeReview(review, t), agent: false })),
  ].sort((a, b) => b.at.localeCompare(a.at));
  const recordedFromStart = journal.events.some(event => event.kind === 'created');

  return (
    <section className="note-history case-activity" aria-label={t('notes.history.label')}>
      {heading}
      {rows.length > 0 && <ul className="note-history-list">
        {rows.map(row => (
          <li key={row.id} className="note-history-item" data-agent={row.agent || undefined}>
            <span className="note-history-date" title={formatDateTime(row.at)}>{formatRelative(row.at)}</span>
            <span className="case-activity__text">{row.text}</span>
          </li>
        ))}
      </ul>}
      {!recordedFromStart && <p className="case-activity__note">{t('notes.activity.notRecordedBefore')}</p>}
      {children}
    </section>
  );
}

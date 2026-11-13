import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowUUpLeft, Check } from '@phosphor-icons/react';
import type { CaseReview, Note } from '../../types';
import { api } from '../../services/tauriApi';
import { Button } from '../ui/Button';
import { Notice } from '../ui/Toast';
import './review.css';

interface ReviewActionsProps {
  /** The case reviewed, at the revision the person saw. */
  note: Pick<Note, 'id' | 'revision' | 'title'>;
  /** Called once the decision is recorded. */
  onReviewed: (review: CaseReview) => void;
  /** "Accept" is the page's primary action (a case's band); on Home, where "New case" already is, it is secondary. */
  emphasis?: 'primary' | 'secondary';
}

/**
 * The person's decision on a declared result: accept the reviewed revision, or ask for a rework
 * with a reason (required). The backend refuses a decision on a revision that changed since.
 */
export function ReviewActions({ note, onReviewed, emphasis = 'primary' }: ReviewActionsProps) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<'idle' | 'rework'>('idle');
  const [reason, setReason] = useState('');
  const [invalid, setInvalid] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reasonId = useId();
  const errorId = useId();

  const decide = async (decision: CaseReview['decision']) => {
    if (decision === 'rework_requested' && !reason.trim()) { setInvalid(true); return; }
    setBusy(true); setError(null);
    try {
      const review = await api.reviewCase(note.id, decision, note.revision, decision === 'rework_requested' ? reason.trim() : undefined);
      setMode('idle'); setReason('');
      onReviewed(review);
    } catch (cause) {
      setError(typeof cause === 'string' && cause ? cause : t('notes.review.error'));
    } finally {
      setBusy(false);
    }
  };

  if (mode === 'rework') {
    return (
      <form className="review-rework" onSubmit={event => { event.preventDefault(); void decide('rework_requested'); }}>
        <label htmlFor={reasonId} className="review-rework__label">{t('notes.review.reasonLabel')}</label>
        <textarea id={reasonId} className="ui-field review-rework__reason" rows={2} value={reason} autoFocus disabled={busy}
          placeholder={t('notes.review.reasonPlaceholder')} aria-invalid={invalid || undefined} aria-describedby={invalid ? errorId : undefined}
          onChange={event => { setReason(event.target.value); setInvalid(false); }} />
        {invalid && <p id={errorId} className="ui-field-error">{t('notes.review.reasonRequired')}</p>}
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="review-actions">
          <Button type="submit" size="sm" disabled={busy}><ArrowUUpLeft size={15} aria-hidden="true" />{t('notes.review.send')}</Button>
          <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => { setMode('idle'); setInvalid(false); setError(null); }}>{t('notes.review.cancel')}</Button>
        </div>
      </form>
    );
  }

  return (
    <div className="review-actions-wrap">
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="review-actions">
        <Button type="button" variant={emphasis === 'primary' ? undefined : 'secondary'} size="sm" disabled={busy} onClick={() => void decide('accepted')}><Check size={15} aria-hidden="true" />{t('notes.review.accept')}</Button>
        <Button type="button" variant="secondary" size="sm" disabled={busy} onClick={() => setMode('rework')}><ArrowUUpLeft size={15} aria-hidden="true" />{t('notes.review.requestRework')}</Button>
      </div>
    </div>
  );
}

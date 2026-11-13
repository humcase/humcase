import { ArrowRight, ArrowSquareOut, Swap } from '@phosphor-icons/react';
import { useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import type { NatureEvidence, NatureProposal, Note } from '../../types';
import { api } from '../../services/tauriApi';
import { TYPE_LABELS } from '../../utils/notePresentation';
import { TypeBadge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Dialog, DialogBody, DialogFooter, DialogHeader } from '../ui/Dialog';
import './natureReview.css';

const two = (value: number) => String(value).padStart(2, '0');

/** The Home card: how many cases may have another nature, and the way to check them. */
export function NatureReviewCard({ count, onStart }: { count: number; onStart: () => void }) {
  const { t } = useTranslation();
  if (count === 0) return null;
  return (
    <Card as="section" padding="sm" className="nature-review-card" aria-labelledby="nature-review-card-heading">
      <Swap size={22} aria-hidden="true" className="nature-review-card__icon" />
      <div className="nature-review-card__text">
        <h2 id="nature-review-card-heading"><span className="nature-review-card__count">{two(count)}</span> {t('workspace.natures.cardTitle', { count })}</h2>
        <p className="workspace-section__hint">{t('workspace.natures.cardHint')}</p>
      </div>
      <Button type="button" onClick={onStart}>{t('workspace.natures.check')}<ArrowRight size={15} aria-hidden="true" /></Button>
    </Card>
  );
}

function EvidenceText({ evidence }: { evidence: NatureEvidence }) {
  const { t } = useTranslation();
  switch (evidence.kind) {
    case 'words': return <>{t('workspace.natures.evidenceWords', { words: evidence.words })}</>;
    case 'task_verb': return <>{t('workspace.natures.evidenceVerb', { word: evidence.word })}</>;
    case 'deadline': return <>{t('workspace.natures.evidenceDeadline')}</>;
    case 'question_mark': return <>{t('workspace.natures.evidenceQuestion')}</>;
  }
}

/**
 * One case at a time: its nature now, the nature proposed and why; the person changes it
 * (Enter), keeps it (G) or opens the case (O). An answer is remembered: a kept nature is not
 * proposed again. Nothing changes without an answer.
 */
export function NatureReviewDialog({ proposals, notes, onNote, onClose }: {
  proposals: NatureProposal[];
  notes: Note[];
  onNote: (note: Note) => void;
  /** `changed`: whether a nature was changed, so the cases are read again. */
  onClose: (changed: boolean) => void;
}) {
  const { t } = useTranslation();
  // The list is fixed when the review opens: answers do not reorder it.
  const [items] = useState(proposals);
  const [index, setIndex] = useState(0);
  const [changed, setChanged] = useState(0);
  const [kept, setKept] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const done = index >= items.length;
  const current = done ? null : items[index];

  const answer = async (change: boolean) => {
    if (!current || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      const applied = await api.answerNature(current.note_id, current.nature, current.proposed, change);
      if (!applied) setNotice(t('workspace.natures.changedMeanwhile', { title: current.title || t('common.untitled') }));
      else if (change) setChanged(count => count + 1);
      else setKept(count => count + 1);
      setIndex(position => position + 1);
    } catch (error) {
      setNotice(t('workspace.natures.answerFailed', { error: String(error) }));
    } finally {
      setBusy(false);
    }
  };
  const open = () => {
    const note = current && notes.find(candidate => candidate.id === current.note_id);
    if (!note) return;
    onClose(changed > 0);
    onNote(note);
  };
  const keys = (event: KeyboardEvent) => {
    if (done || event.altKey || event.ctrlKey || event.metaKey) return;
    // A focused button, field or menu answers its own keys (Enter on Close closes, nothing else).
    if (event.target instanceof Element && event.target.closest('button, a, input, textarea, select, [role="menuitem"], [contenteditable="true"]')) return;
    const key = event.key.toLowerCase();
    if (event.key === 'Enter') { event.preventDefault(); void answer(true); }
    // The letters are the interface language's (keep, open), as the hint below says.
    else if (key === t('workspace.natures.keepKey').toLowerCase()) { event.preventDefault(); void answer(false); }
    else if (key === t('workspace.natures.openKey').toLowerCase()) { event.preventDefault(); open(); }
  };
  const close = () => { if (!busy) onClose(changed > 0); };

  return (
    <Dialog onClose={close} size="md" className="nature-review" describedBy="nature-review-desc">
      <div onKeyDown={keys}>
        <DialogHeader title={done ? t('workspace.natures.doneTitle') : t('workspace.natures.title')} onClose={close} closeLabel={t('common.close')}
          description={<p id="nature-review-desc">{done ? t('workspace.natures.doneHint') : t('workspace.natures.hint')}</p>} />
        <DialogBody>
          {current ? <>
            <div className="nature-review__progress">
              <span>{t('workspace.natures.position', { position: two(index + 1), total: two(items.length) })}</span>
              <div className="nature-review__bar" role="progressbar" aria-valuemin={0} aria-valuemax={items.length} aria-valuenow={index} aria-label={t('workspace.natures.progress')}>
                <div style={{ width: `${Math.round((index / items.length) * 100)}%` }} />
              </div>
            </div>
            <article className="nature-review__case" aria-live="polite">
              <h3>{current.title || t('common.untitled')}</h3>
              <div className="nature-review__change">
                <TypeBadge type={current.nature} size="md" />
                <ArrowRight size={16} aria-label={t('workspace.natures.becomes')} />
                <span className="nature-review__proposed"><TypeBadge type={current.proposed} size="md" /></span>
              </div>
              <p className="nature-review__why">
                {t('workspace.natures.readAs', { nature: TYPE_LABELS[current.proposed] })}
                {current.reasons.map((reason, position) => reason.kind === 'wording'
                  ? <span key={position}> · <EvidenceText evidence={reason.evidence} /></span> : null)}
              </p>
            </article>
            {notice && <p className="nature-review__notice" role="status">{notice}</p>}
            <p className="nature-review__keys">{t('workspace.natures.keys', { keep: t('workspace.natures.keepKey'), open: t('workspace.natures.openKey') })}</p>
          </> : <>
            <div className="nature-review__totals">
              <div><span>{t('workspace.natures.changed')}</span><strong>{two(changed)}</strong></div>
              <div><span>{t('workspace.natures.kept')}</span><strong>{two(kept)}</strong></div>
            </div>
            {notice && <p className="nature-review__notice" role="status">{notice}</p>}
            <p className="workspace-section__hint">{t('workspace.natures.remembered')}</p>
          </>}
        </DialogBody>
        <DialogFooter>
          {current ? <>
            <Button type="button" variant="ghost" onClick={open} disabled={busy}><ArrowSquareOut size={15} aria-hidden="true" />{t('workspace.natures.open')}</Button>
            <Button type="button" variant="secondary" onClick={() => void answer(false)} disabled={busy}>{t('workspace.natures.keep', { nature: TYPE_LABELS[current.nature] })}</Button>
            <Button type="button" onClick={() => void answer(true)} disabled={busy}>{t('workspace.natures.change', { nature: TYPE_LABELS[current.proposed] })}</Button>
          </> : <Button type="button" onClick={close}>{t('common.close')}</Button>}
        </DialogFooter>
      </div>
    </Dialog>
  );
}

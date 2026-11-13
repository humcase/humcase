import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowRight, CheckCircle, HourglassMedium, Robot } from '@phosphor-icons/react';
import type { Brief, CaseEvent, CaseReview, Note, Project } from '../../types';
import { api } from '../../services/tauriApi';
import { formatDateTime, formatRelative, formatShortDate, parseTimestamp } from '../../utils/datetime';
import { notePreview } from '../../utils/notePresentation';
import { Badge, Count } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Dialog, DialogFooter, DialogHeader } from '../ui/Dialog';
import { List, ListRow } from '../ui/List';
import { Notice } from '../ui/Toast';
import { ReviewActions } from '../review/ReviewActions';
import { actorTitle, describeEvent } from '../review/journalText';
import { CaseLocation } from './CaseLocation';
import '../review/review.css';
import { currentDateLocale } from '../../i18n';

interface HomeBriefProps {
  brief: Brief | null;
  failed: boolean;
  /** Visible cases (current space and area), sub-cases included: the brief lists only those. */
  notes: Note[];
  /** Projects of the space, to say where each case is. */
  projects?: Project[];
  onNote: (note: Note) => void;
  onReviewed: (review: CaseReview) => void;
  /** After "Accept all": the brief is read again. */
  onAcceptedAll: () => void;
}

/**
 * "Since your last visit": results declared by agents that wait for the person, writes of
 * agents and of the CLI not seen yet, reworks waiting for an agent. The person's own writes are
 * not listed, and the period is always stated.
 */
export function HomeBrief({ brief, failed, notes, projects = [], onNote, onReviewed, onAcceptedAll }: HomeBriefProps) {
  const { t } = useTranslation();
  const [confirmAll, setConfirmAll] = useState(false);
  const [accepting, setAccepting] = useState(false);
  const [notAccepted, setNotAccepted] = useState(0);
  if (failed) return <Notice tone="danger">{t('workspace.since.loadError')}</Notice>;
  if (!brief) return null;
  const byId = new Map(notes.map(note => [note.id, note]));
  const toReview = brief.to_review.flatMap(item => { const note = byId.get(item.note_id); return note ? [{ item, note }] : []; });
  const reworks = brief.reworks_pending.flatMap(item => { const note = byId.get(item.note_id); return note ? [{ item, note }] : []; });
  const reviewIds = new Set(toReview.map(({ note }) => note.id));
  const projectById = new Map(projects.map(project => [project.id, project]));
  const where = (note: Note) => <CaseLocation note={note} byId={byId} projectById={projectById} />;
  // One row per visible case: its latest write, how many others came before it, and the
  // sub-cases an agent created inside it (sub-cases are not listed on their own).
  const changesByNote = new Map<string, { own: CaseEvent[]; subCases: CaseEvent[] }>();
  const entry = (id: string) => changesByNote.get(id) ?? changesByNote.set(id, { own: [], subCases: [] }).get(id)!;
  for (const event of brief.changes) {
    const parent = typeof event.detail?.parent_id === 'string' ? event.detail.parent_id : null;
    if (event.kind === 'created' && parent && byId.has(parent)) { if (!reviewIds.has(parent)) entry(parent).subCases.push(event); }
    else if (byId.has(event.note_id) && !reviewIds.has(event.note_id)) entry(event.note_id).own.push(event);
  }
  const changes = [...changesByNote.entries()].map(([id, { own, subCases }]) => {
    const latest = [own[0], subCases[0]].filter(Boolean).sort((a, b) => b.last_at.localeCompare(a.last_at))[0];
    const parts = [
      own.length > 0 && <>{describeEvent(own[0], t)}{own.length > 1 && <> · {t('workspace.since.moreChanges', { count: own.length - 1 })}</>}</>,
      subCases.length > 0 && t('workspace.since.subCasesCreated', { actor: actorTitle(subCases[0], t), count: subCases.length }),
    ].filter(Boolean);
    return { note: byId.get(id)!, latest, description: <>{parts.map((part, index) => <span key={index}>{index > 0 && ' · '}{part}</span>)}</> };
  });
  const title = (note: Note) => note.title || t('common.untitled');
  const sinceDate = parseTimestamp(brief.since);
  const sinceTime = sinceDate?.toLocaleTimeString(currentDateLocale(), { hour: '2-digit', minute: '2-digit' }) ?? '';
  const period = brief.since_is_fallback ? t('workspace.since.periodFallback') : t('workspace.since.periodSince', { date: formatShortDate(brief.since), time: sinceTime });
  const nothing = !toReview.length && !changes.length && !reworks.length;

  // "Accept all" (founder decision of 03/10, instead of "Mark all as seen"): each listed result
  // is accepted at the revision on screen, like its own "Accept"; one changed since is refused
  // and stays to review. Then the brief starts again from now.
  const acceptAll = async () => {
    setAccepting(true);
    let refused = 0;
    for (const { note } of toReview) {
      try { await api.reviewCase(note.id, 'accepted', note.revision); } catch { refused += 1; }
    }
    try { await api.markBriefSeen(); } catch { /* The accepted results already left the brief. */ }
    setAccepting(false);
    setConfirmAll(false);
    setNotAccepted(refused);
    onAcceptedAll();
  };

  // Everything seen: a single line, so the rest of the home moves up.
  if (nothing) return (
    <Card as="section" padding="sm" className="home-brief home-brief--quiet" aria-labelledby="home-brief-heading">
      <h2 id="home-brief-heading">{t('workspace.since.title')}</h2>
      <p className="home-brief__nothing">{t('workspace.since.nothing')} <span className="workspace-section__hint">{period}</span></p>
    </Card>
  );

  return (
    <Card as="section" className="home-brief" aria-labelledby="home-brief-heading">
      <header className="home-brief__header">
        <div>
          <h2 id="home-brief-heading">{t('workspace.since.title')}</h2>
          <p className="workspace-section__hint">{period} · {t('workspace.since.scope')}</p>
        </div>
        {toReview.length > 0 && <Button type="button" variant="secondary" size="sm" onClick={() => { setNotAccepted(0); setConfirmAll(true); }}>
          <CheckCircle size={15} aria-hidden="true" />{t('workspace.since.acceptAll')}
        </Button>}
      </header>
      {notAccepted > 0 && <Notice tone="neutral">{t('workspace.since.acceptAllRefused', { count: notAccepted })}</Notice>}
      {confirmAll && <Dialog onClose={() => { if (!accepting) setConfirmAll(false); }} size="sm" describedBy="accept-all-desc">
        <div className="ui-dialog__form">
          <DialogHeader title={t('workspace.since.acceptAllTitle', { count: toReview.length })}
            description={<p id="accept-all-desc">{t('workspace.since.acceptAllBody', { count: toReview.length })}</p>} />
          <DialogFooter>
            <Button type="button" variant="secondary" onClick={() => setConfirmAll(false)} disabled={accepting}>{t('common.cancel')}</Button>
            <Button type="button" onClick={() => void acceptAll()} disabled={accepting}>{accepting ? t('workspace.since.accepting') : t('workspace.since.acceptAllConfirm', { count: toReview.length })}</Button>
          </DialogFooter>
        </div>
      </Dialog>}

      {toReview.length > 0 && <section className="home-brief__group" aria-labelledby="home-brief-review">
        <h3 id="home-brief-review">{t('workspace.since.toReview')} <Count value={toReview.length} /></h3>
        <p className="workspace-section__hint">{t('workspace.since.toReviewHint')}</p>
        <ul className="home-brief__reviews">
          {toReview.map(({ item, note }) => (
            <li key={note.id} className="home-brief__review">
              {where(note)}
              <div className="home-brief__review-head">
                <button type="button" className="home-brief__case" onClick={() => onNote(note)} aria-label={t('workspace.since.openCase', { title: title(note) })}>{title(note)}</button>
                {item.declared && <span className="review-actor"><Robot size={14} aria-hidden="true" />{actorTitle(item.declared, t)} · <time dateTime={item.declared.created_at} title={formatDateTime(item.declared.created_at)}>{formatRelative(item.declared.created_at)}</time></span>}
              </div>
              {item.rework_count > 0 && <Badge tone="warning">{t('notes.review.reworkHandedBack', { number: item.rework_count })}</Badge>}
              {note.resolution_comment?.trim() && <p className="home-brief__report">{notePreview({ ...note, content: note.resolution_comment })}</p>}
              <ReviewActions note={note} onReviewed={onReviewed} emphasis="secondary" />
            </li>
          ))}
        </ul>
      </section>}

      {changes.length > 0 && <section className="home-brief__group" aria-labelledby="home-brief-changes">
        <h3 id="home-brief-changes">{t('workspace.since.changes')} <Count value={changes.length} /></h3>
        <p className="workspace-section__hint">{t('workspace.since.changesHint')}</p>
        <List variant="divided">{changes.slice(0, 8).map(({ note, latest, description }) =>
          <ListRow key={note.id} size="sm" onClick={() => onNote(note)} actionLabel={t('workspace.since.openCase', { title: title(note) })}
            overline={where(note)}
            title={title(note)}
            description={description}
            meta={<span className="review-actor"><Robot size={14} aria-hidden="true" />{actorTitle(latest, t)} · <time dateTime={latest.last_at} title={formatDateTime(latest.last_at)}>{formatRelative(latest.last_at)}</time></span>}
            trailing={<ArrowRight size={15} aria-hidden="true" />} />)}</List>
      </section>}

      {reworks.length > 0 && <section className="home-brief__group" aria-labelledby="home-brief-reworks">
        <h3 id="home-brief-reworks">{t('workspace.since.reworks')} <Count value={reworks.length} /></h3>
        <p className="workspace-section__hint">{t('workspace.since.reworksHint')}</p>
        <List variant="divided">{reworks.map(({ item, note }) =>
          <ListRow key={note.id} size="sm" onClick={() => onNote(note)} actionLabel={t('workspace.since.openCase', { title: title(note) })}
            leading={<HourglassMedium size={16} aria-hidden="true" />}
            overline={where(note)}
            title={title(note)}
            description={item.last_review?.comment ? t('notes.review.reworkReason', { reason: item.last_review.comment }) : undefined}
            meta={<>{t('notes.review.reworkNumber', { number: item.last_review?.rework_number ?? item.rework_count })}{item.last_review && <> · <time dateTime={item.last_review.created_at}>{formatRelative(item.last_review.created_at)}</time></>}</>}
            trailing={<ArrowRight size={15} aria-hidden="true" />} />)}</List>
      </section>}
    </Card>
  );
}

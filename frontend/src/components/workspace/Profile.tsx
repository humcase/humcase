import { ArrowRight, IdentificationCard, PencilSimple, Plant, Signpost } from '@phosphor-icons/react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import type { Note, Nudge, NudgeAnswer, ProfileItem, ProjectSpace } from '../../types';
import { api } from '../../services/tauriApi';
import { useWorkProfile } from '../../hooks/useProfile';
import { caseProjectName } from '../../utils/caseRefText';
import { formatShortDate } from '../../utils/datetime';
import { TYPE_ICONS } from '../../utils/notePresentation';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Count } from '../ui/Badge';
import { EmptyState } from '../ui/EmptyState';
import { SegmentedControl } from '../ui/SegmentedControl';
import { PageHeader } from './PageHeader';
import { readProfile } from '../../utils/profileText';
import './profile.css';

/** What waits for a next step: the label of a nudge's case. */
function nudgeKind(nudge: Nudge, t: TFunction) {
  if (nudge.case.nature === 'decision') return t('workspace.nudges.kindDecision');
  if (nudge.case.nature === 'idea') return t('workspace.nudges.kindIdea');
  return t('workspace.nudges.kindQuestion');
}

/**
 * The Home card "What waits for a next step": at most three open questions, ideas or decisions
 * to take that nobody touched for days, chosen once a day without AI. Sprout opens the case and
 * proposes the steps its text raises (a case too short for it is only opened); Later brings it
 * back in a week; Never stops it.
 */
export function NudgesCard({ nudges, onOpen, onGerminate, onAnswered }: {
  nudges: Nudge[];
  onOpen: (id: string) => void;
  onGerminate: (id: string) => void;
  onAnswered: () => void;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  if (nudges.length === 0) return null;
  const answer = async (nudge: Nudge, value: NudgeAnswer) => {
    setError(null);
    try {
      await api.answerNudge(nudge.id, value);
      onAnswered();
      if (value === 'accepted' && nudge.can_sprout) onGerminate(nudge.case.id);
      else if (value === 'accepted') onOpen(nudge.case.id);
    } catch (cause) {
      setError(t('workspace.nudges.answerFailed', { error: String(cause) }));
    }
  };
  return (
    <Card as="section" padding="none" className="nudges-card" aria-labelledby="nudges-heading">
      <header className="nudges-card__header">
        <Signpost size={20} aria-hidden="true" className="nudges-card__icon" />
        <h2 id="nudges-heading">{t('workspace.nudges.title')}</h2>
        <Count value={nudges.length} />
        <span className="nudges-card__hint">{t('workspace.nudges.hint')}</span>
      </header>
      {error && <div role="alert" className="ui-dialog__error">{error}</div>}
      <ul className="nudges-card__list">{nudges.map(nudge => {
        const Icon = TYPE_ICONS[nudge.case.nature];
        const title = nudge.case.title || t('common.untitled');
        return <li key={nudge.id} className="nudge">
          <div className="nudge__text">
            <div className="nudge__line">
              <span className="profile-tag">{nudgeKind(nudge, t)}</span>
              <button type="button" className="tidy-link" onClick={() => onOpen(nudge.case.id)} title={t('workspace.tidy.open', { title })}>
                {Icon && <Icon size={15} aria-hidden="true" />}<span>{title}</span>
              </button>
            </div>
            <p className="nudge__why">{t('workspace.nudges.idle', { count: nudge.idle_days, project: caseProjectName(nudge.case, t) })}</p>
          </div>
          <div className="nudge__actions">
            <Button type="button" variant="ghost" size="sm" onClick={() => void answer(nudge, 'refused')} aria-label={t('workspace.nudges.neverLabel', { title })}>{t('workspace.nudges.never')}</Button>
            <Button type="button" variant="ghost" size="sm" onClick={() => void answer(nudge, 'skipped')} aria-label={t('workspace.nudges.laterLabel', { title })}>{t('workspace.nudges.later')}</Button>
            {nudge.can_sprout
              ? <Button type="button" variant="secondary" size="sm" onClick={() => void answer(nudge, 'accepted')} aria-label={t('workspace.nudges.sproutLabel', { title })}><Plant size={16} aria-hidden="true" />{t('workspace.nudges.sprout')}</Button>
              : <Button type="button" variant="secondary" size="sm" onClick={() => void answer(nudge, 'accepted')} aria-label={t('workspace.nudges.openLabel', { title })}>{t('workspace.nudges.open')}<ArrowRight size={15} aria-hidden="true" /></Button>}
          </div>
        </li>;
      })}</ul>
    </Card>
  );
}

/** The Home card: lines proposed for the work profile, and the way to review them. */
export function ProfileCard({ count, onOpen }: { count: number; onOpen: () => void }) {
  const { t } = useTranslation();
  if (count === 0) return null;
  return (
    <Card as="section" padding="sm" className="tidy-card profile-card" aria-labelledby="profile-card-heading">
      <IdentificationCard size={22} aria-hidden="true" className="tidy-card__icon" />
      <div className="tidy-card__text">
        <h2 id="profile-card-heading">{t('workspace.profile.title')} <span className="profile-card__count">{t('workspace.profile.cardCount', { count })}</span></h2>
        <p className="workspace-section__hint">{t('workspace.profile.cardText')}</p>
      </div>
      <Button type="button" onClick={onOpen}>{t('workspace.profile.review')}<ArrowRight size={15} aria-hidden="true" /></Button>
    </Card>
  );
}

type Answer = 'yes' | 'no';

/** Where a proposed line comes from, and who read it. */
function origin(item: ProfileItem, t: TFunction) {
  if (item.source === 'rule') return {
    from: item.case ? t('workspace.profile.fromDecision', { title: item.case.title || t('common.untitled'), date: formatShortDate(item.case.date) }) : '',
    by: t('workspace.profile.byJev'),
  };
  if (item.source === 'agent') return { from: item.detail, by: t('workspace.profile.byAgent', { agent: item.proposed_by ?? 'agent' }) };
  if (item.source === 'work') return { from: t('workspace.profile.fromCount', { detail: item.detail }), by: t('workspace.profile.byCount') };
  return { from: t('workspace.profile.fromSprout', { detail: item.detail }), by: t('workspace.profile.byCount') };
}

/**
 * "How you work": the profile of a space (a case of its Inbox the person edits freely; agents
 * read it at the start of a session) and the lines Humcase or an agent proposes for it. Each line
 * is added only when the person says yes; one button applies every answer and Undo takes them
 * back.
 */
export function ProfilePage({ space, spaces, onSpace, notes, onNote, onEdit }: {
  space: ProjectSpace;
  /** Spaces the person uses: a switch when there are two. */
  spaces: ProjectSpace[];
  onSpace: (space: ProjectSpace) => void;
  notes: Note[];
  onNote: (note: Note) => void;
  /** Opens the profile case in the editor (created if needed). */
  onEdit: (space: ProjectSpace) => void;
}) {
  const { t } = useTranslation();
  const { view, failed, reload } = useWorkProfile(space, notes);
  const pending = view?.pending ?? [];
  const [answers, setAnswers] = useState<Record<string, Answer>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<{ ids: string[]; line: string } | null>(null);
  const answer = (item: ProfileItem): Answer => answers[item.id] ?? 'yes';
  const added = pending.filter(item => answer(item) === 'yes').length;
  const blocks = readProfile(view?.profile.content ?? '');
  const hasLines = blocks.some(block => block.kind === 'line');
  const open = (id: string) => { const note = notes.find(candidate => candidate.id === id); if (note) onNote(note); };
  const sectionLabel = (item: ProfileItem) => t(`workspace.profile.section.${item.section}`);

  const apply = async (refuseAll: boolean) => {
    if (pending.length === 0) return;
    const list = pending.map(item => ({ id: item.id, accept: !refuseAll && answer(item) === 'yes' }));
    setBusy(true);
    setError(null);
    try {
      const outcome = await api.answerProfile(list);
      const parts = [
        outcome.added > 0 && t('workspace.profile.doneAdded', { count: outcome.added }),
        outcome.refused > 0 && t('workspace.profile.doneRefused', { count: outcome.refused }),
      ].filter(Boolean);
      setDone({ ids: outcome.ids, line: parts.join(' · ') });
      setAnswers({});
      reload();
    } catch (cause) {
      setError(t('workspace.profile.answerFailed', { error: String(cause) }));
    } finally {
      setBusy(false);
    }
  };
  const undo = async () => {
    if (!done) return;
    setBusy(true);
    setError(null);
    try {
      await api.undoProfile(done.ids);
      setDone(null);
      reload();
    } catch (cause) {
      setError(t('workspace.profile.undoFailed', { error: String(cause) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="workspace-page profile-view" aria-labelledby="profile-view-heading">
      <PageHeader eyebrow={t(`common.space.${space}`)} title={t('workspace.profile.title')} titleId="profile-view-heading" description={t('workspace.profile.intro')}
        action={<div className="profile-view__actions">
          {spaces.length > 1 && <SegmentedControl size="sm" label={t('workspace.profile.spaceLabel')} value={space} onValueChange={value => onSpace(value as ProjectSpace)}
            items={spaces.map(value => ({ value, label: t(`common.space.${value}`) }))} />}
          <Button type="button" variant="secondary" onClick={() => onEdit(space)}><PencilSimple size={16} aria-hidden="true" />{t('workspace.profile.edit')}</Button>
        </div>} />
      {done && <div className="tidy-done" role="status">
        <div><strong>{done.line}</strong><p>{t('workspace.profile.doneHint')}</p></div>
        <Button type="button" variant="secondary" size="sm" disabled={busy} onClick={() => void undo()}>{t('workspace.profile.undo')}</Button>
      </div>}
      {error && <div role="alert" className="ui-dialog__error">{error}</div>}
      {failed && <div role="alert" className="ui-dialog__error">{t('workspace.profile.readFailed')}</div>}

      {view && (hasLines
        ? <Card as="section" padding="none" className="profile-lines" aria-label={t('workspace.profile.contentLabel')}>
          {blocks.map((block, index) => block.kind === 'heading'
            ? <h2 key={index} className="profile-lines__heading">{block.text}</h2>
            : block.kind === 'line'
              ? <p key={index} className="profile-lines__line">{block.text}{block.source && <span className="profile-lines__source"> — {block.source}</span>}</p>
              : <p key={index} className="profile-lines__text">{block.text}</p>)}
        </Card>
        : <EmptyState size="sm" icon={<IdentificationCard />} title={t('workspace.profile.emptyTitle')} description={pending.length > 0 ? t('workspace.profile.emptyDescription') : t('workspace.profile.emptyQuiet')} />)}

      {pending.length > 0 && <section className="tidy-section profile-pending" aria-labelledby="profile-pending-heading">
        <h2 id="profile-pending-heading" className="tidy-section__title">{t('workspace.profile.pendingTitle')}<Count value={pending.length} /></h2>
        <ul className="tidy-pairs">{pending.map(item => {
          const { from, by } = origin(item, t);
          return <li key={item.id} className="tidy-pair">
            <div className="nudge__line"><span className="profile-tag">{sectionLabel(item)}</span><strong className="profile-pending__text">{item.text}</strong></div>
            {from && (item.case && item.source === 'rule'
              ? <p className="tidy-pair__quote profile-pending__from"><button type="button" className="tidy-link profile-pending__case" onClick={() => open(item.case!.id)}>{from}</button></p>
              : <p className="tidy-pair__quote profile-pending__from">{from}</p>)}
            <div className="tidy-pair__answer">
              <span className="tidy-pair__source">{by}</span>
              <SegmentedControl size="sm" label={t('workspace.profile.answerLabel', { text: item.text })} value={answer(item)}
                onValueChange={next => setAnswers(current => ({ ...current, [item.id]: next as Answer }))}
                items={[{ value: 'yes', label: t('workspace.profile.yes') }, { value: 'no', label: t('workspace.profile.no') }]} />
            </div>
          </li>;
        })}</ul>
        <footer className="tidy-footer">
          <p>{t('workspace.profile.footer')}</p>
          <div className="tidy-footer__actions">
            <Button type="button" variant="ghost" disabled={busy} onClick={() => void apply(true)}>{t('workspace.profile.refuseAll')}</Button>
            <Button type="button" disabled={busy} onClick={() => void apply(false)}>{added > 0 ? t('workspace.profile.apply', { count: added }) : t('workspace.profile.applyRefusals')}</Button>
          </div>
        </footer>
      </section>}
    </main>
  );
}

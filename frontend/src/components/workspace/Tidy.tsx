import { Archive, ArrowRight, ArrowsDownUp, Copy, Tray, Warning } from '@phosphor-icons/react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import type { CaseRef, Note, TidyAnswer, TidyItem } from '../../types';
import { api } from '../../services/tauriApi';
import { useTidy } from '../../hooks/useTidy';
import { caseProjectName } from '../../utils/caseRefText';
import { formatShortDate } from '../../utils/datetime';
import { TYPE_ICONS } from '../../utils/notePresentation';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Count } from '../ui/Badge';
import { EmptyState } from '../ui/EmptyState';
import { SegmentedControl } from '../ui/SegmentedControl';
import { PageHeader } from './PageHeader';
import './tidy.css';

const two = (value: number) => String(value).padStart(2, '0');

/** The Home card: what "To tidy" proposes, and the way to review it. Nothing is deleted. */
export function TidyCard({ items, weeks, onStart }: { items: TidyItem[]; weeks: number; onStart: () => void }) {
  const { t } = useTranslation();
  if (items.length === 0) return null;
  const of = (kind: TidyItem['kind']) => items.filter(item => item.kind === kind).length;
  const parts = [
    of('conflict') > 0 && t('workspace.tidy.cardConflicts', { count: of('conflict') }),
    of('supersede') > 0 && t('workspace.tidy.cardDecisions', { count: of('supersede') }),
    of('group') > 0 && t('workspace.tidy.cardGroups', { count: of('group') }),
    of('archive') > 0 && t('workspace.tidy.cardArchives', { count: of('archive'), weeks }),
  ].filter(Boolean).join(t('workspace.tidy.and'));
  return (
    <Card as="section" padding="sm" className="tidy-card" aria-labelledby="tidy-card-heading">
      <Archive size={22} aria-hidden="true" className="tidy-card__icon" />
      <div className="tidy-card__text">
        <h2 id="tidy-card-heading"><span className="tidy-card__count">{two(items.length)}</span> {t('workspace.tidy.title')}</h2>
        <p className="workspace-section__hint">{t('workspace.tidy.cardText', { parts })}</p>
      </div>
      <Button type="button" onClick={onStart}>{t('workspace.tidy.review')}<ArrowRight size={15} aria-hidden="true" /></Button>
    </Card>
  );
}

type Answer = 'yes' | 'no' | null;

/** Who found the pair, and how sure: an agent, Jev, or the words alone. */
function pairSource(item: TidyItem, t: TFunction) {
  if (item.proposed_by) return t('workspace.tidy.byAgent', { agent: item.proposed_by });
  if (item.kind === 'group') {
    if (item.duplicate) return t('workspace.tidy.byWords');
    return item.doubt ? t('workspace.tidy.jevDoubtsGroup') : t('workspace.tidy.byJevGroup');
  }
  if (item.kind === 'conflict') {
    const found = item.passage ? t('workspace.tidy.passagesFound') : '';
    return (item.doubt ? t('workspace.tidy.jevDoubtsConflict') : t('workspace.tidy.byJevConflict')) + found;
  }
  // A decision pair waits only once Jev judged it, or an agent proposed it.
  return item.doubt ? t('workspace.tidy.jevDoubts') : t('workspace.tidy.byJev');
}

function CaseLine({ value, onOpen, struck = false, quote }: { value: CaseRef; onOpen: (id: string) => void; struck?: boolean; quote?: string | null }) {
  const { t } = useTranslation();
  const Icon = TYPE_ICONS[value.nature];
  const title = value.title || t('common.untitled');
  const text = quote || value.excerpt;
  return (
    <div className="tidy-pair__case">
      <div className="tidy-pair__head">
        <span className="tidy-pair__date">{formatShortDate(value.date)}</span>
        <button type="button" className="tidy-link" data-struck={struck || undefined} onClick={() => onOpen(value.id)} title={t('workspace.tidy.open', { title })}>
          {Icon && <Icon size={15} aria-hidden="true" />}<span>{title}</span>
        </button>
      </div>
      {text && <p className="tidy-pair__quote">{t('workspace.tidy.quoted', { text })}</p>}
    </div>
  );
}

function CaseName({ value, onOpen }: { value: CaseRef; onOpen: (id: string) => void }) {
  const { t } = useTranslation();
  const Icon = TYPE_ICONS[value.nature];
  const title = value.title || t('common.untitled');
  return <button type="button" className="tidy-link" onClick={() => onOpen(value.id)} title={t('workspace.tidy.open', { title })}>
    {Icon && <Icon size={15} aria-hidden="true" />}<span>{title}</span>
  </button>;
}

/**
 * "To tidy": cases that may go against a decision (to correct, or not a contradiction),
 * decisions a newer one may replace (yes / no each), close cases to file one inside the other
 * (the direction can be swapped), and cases resolved long ago to archive (checked by default).
 * Humcase proposes, the person decides; one button applies every answer, and Undo takes them
 * back. A refused proposal is not made again.
 */
export function TidyView({ notes, onNote, onChanged }: {
  /** Every case of the space: the proposals shown, and the case opened from a title. */
  notes: Note[];
  onNote: (note: Note) => void;
  /** Answers applied or taken back: the cases and "To tidy" are read again. */
  onChanged: () => void;
}) {
  const { t } = useTranslation();
  const { items, weeks, reload } = useTidy(new Set(notes.map(note => note.id)), notes);
  const conflicts = items.filter(item => item.kind === 'conflict');
  const pairs = items.filter(item => item.kind === 'supersede');
  const groups = items.filter(item => item.kind === 'group');
  const archives = items.filter(item => item.kind === 'archive');
  const [answers, setAnswers] = useState<Record<string, Answer>>({});
  const [swapped, setSwapped] = useState<ReadonlySet<string>>(new Set());
  const [unchecked, setUnchecked] = useState<ReadonlySet<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<{ ids: string[]; line: string } | null>(null);
  // A doubtful pair waits for an answer; a sure one is proposed. A conflict always waits: only
  // the person knows whether the case or the decision is right.
  const answer = (item: TidyItem): Answer => item.id in answers ? answers[item.id]
    : item.kind === 'conflict' || item.doubt ? null : 'yes';
  const setAnswer = (id: string, value: Answer) => setAnswers(current => ({ ...current, [id]: value }));
  const open = (id: string) => { const note = notes.find(candidate => candidate.id === id); if (note) onNote(note); };
  const replaced = pairs.filter(item => answer(item) === 'yes').length;
  const grouped = groups.filter(item => answer(item) === 'yes').length;
  const toFix = conflicts.filter(item => answer(item) === 'yes').length;
  const archived = archives.filter(item => !unchecked.has(item.id)).length;
  const allChecked = archives.length > 0 && archived === archives.length;

  const apply = async (refuseAll: boolean) => {
    const answered = (item: TidyItem) => {
      const value = refuseAll ? 'no' : answer(item);
      return value === null ? [] : [{ id: item.id, accept: value === 'yes', ...(swapped.has(item.id) ? { swap: true } : {}) }];
    };
    const list: TidyAnswer[] = [
      ...conflicts.flatMap(answered),
      ...pairs.flatMap(answered),
      ...groups.flatMap(answered),
      ...archives.map(item => ({ id: item.id, accept: !refuseAll && !unchecked.has(item.id) })),
    ];
    if (list.length === 0) return;
    const fix = refuseAll ? undefined : conflicts.find(item => answer(item) === 'yes');
    setBusy(true);
    setError(null);
    try {
      const outcome = await api.answerTidy(list);
      const parts = [
        outcome.to_fix > 0 && t('workspace.tidy.doneToFix', { count: outcome.to_fix }),
        outcome.superseded > 0 && t('workspace.tidy.doneReplaced', { count: outcome.superseded }),
        outcome.grouped > 0 && t('workspace.tidy.doneGrouped', { count: outcome.grouped }),
        outcome.archived > 0 && t('workspace.tidy.doneArchived', { count: outcome.archived }),
        outcome.refused > 0 && t('workspace.tidy.doneRefused', { count: outcome.refused }),
        outcome.skipped > 0 && t('workspace.tidy.doneSkipped', { count: outcome.skipped }),
      ].filter(Boolean);
      setDone({ ids: list.map(item => item.id), line: parts.length ? parts.join(' · ') : t('workspace.tidy.doneNothing') });
      setAnswers({});
      setSwapped(new Set());
      setUnchecked(new Set());
      reload();
      onChanged();
      // "I correct the case": the case opens, to be corrected now.
      if (fix) open(fix.case.id);
    } catch (cause) {
      setError(t('workspace.tidy.answerFailed', { error: String(cause) }));
    } finally {
      setBusy(false);
    }
  };
  const undo = async () => {
    if (!done) return;
    setBusy(true);
    setError(null);
    try {
      await api.undoTidy(done.ids);
      setDone(null);
      reload();
      onChanged();
    } catch (cause) {
      setError(t('workspace.tidy.undoFailed', { error: String(cause) }));
    } finally {
      setBusy(false);
    }
  };
  const applyParts = [
    toFix > 0 && t('workspace.tidy.applyFix', { count: toFix }),
    replaced > 0 && t('workspace.tidy.applyReplaced', { count: replaced }),
    grouped > 0 && t('workspace.tidy.applyGroup', { count: grouped }),
    archived > 0 && t('workspace.tidy.applyArchive', { count: archived }),
  ].filter(Boolean);
  const applyLabel = applyParts.length ? applyParts.join(' · ') : t('workspace.tidy.applyRefusals');
  const answered = [...conflicts, ...pairs, ...groups].some(item => answer(item) !== null) || archives.length > 0;
  const answerControl = (item: TidyItem, label: string, yes: string, no: string) =>
    <SegmentedControl size="sm" label={t('workspace.tidy.answerLabel', { title: label })} value={answer(item) ?? ''}
      onValueChange={next => setAnswer(item.id, next as Answer)}
      items={[{ value: 'yes', label: yes }, { value: 'no', label: no }]} />;
  const source = (item: TidyItem) =>
    <span className="tidy-pair__source" data-doubt={item.doubt || undefined}>{item.doubt && <strong>{t('workspace.tidy.toCheck')} · </strong>}{pairSource(item, t)}</span>;

  return (
    <main className="workspace-page tidy-view" aria-labelledby="tidy-view-heading">
      <PageHeader eyebrow={t('workspace.sidebar.home')} title={t('workspace.tidy.title')} titleId="tidy-view-heading" count={items.length} description={t('workspace.tidy.intro')} />
      {done && <div className="tidy-done" role="status">
        <div><strong>{done.line}</strong><p>{t('workspace.tidy.doneHint')}</p></div>
        <Button type="button" variant="secondary" size="sm" disabled={busy} onClick={() => void undo()}>{t('workspace.tidy.undo')}</Button>
      </div>}
      {error && <div role="alert" className="ui-dialog__error">{error}</div>}
      {items.length === 0 && !done && <EmptyState icon={<Tray />} title={t('workspace.tidy.emptyTitle')} description={t('workspace.tidy.emptyDescription')} />}

      {conflicts.length > 0 && <section className="tidy-section" aria-labelledby="tidy-conflicts-heading">
        <h2 id="tidy-conflicts-heading" className="tidy-section__title"><Warning size={17} aria-hidden="true" className="tidy-section__warning" />{t('workspace.tidy.conflictsTitle')}<Count value={conflicts.length} /></h2>
        <ul className="tidy-pairs">{conflicts.map(item => <li key={item.id} className="tidy-pair">
          <CaseLine value={item.case} onOpen={open} quote={item.passage} />
          <span className="tidy-pair__by">{t('workspace.tidy.goesAgainst')}</span>
          {item.other && <CaseLine value={item.other} onOpen={open} quote={item.other_passage} />}
          <div className="tidy-pair__answer">
            {source(item)}
            {answerControl(item, item.case.title || t('common.untitled'), t('workspace.tidy.iFix'), t('workspace.tidy.noConflict'))}
          </div>
        </li>)}</ul>
      </section>}

      {pairs.length > 0 && <section className="tidy-section" aria-labelledby="tidy-pairs-heading">
        <h2 id="tidy-pairs-heading" className="tidy-section__title">{t('workspace.tidy.pairsTitle')}<Count value={pairs.length} /></h2>
        <ul className="tidy-pairs">{pairs.map(item => <li key={item.id} className="tidy-pair">
          <CaseLine value={item.case} onOpen={open} struck={answer(item) === 'yes'} />
          <span className="tidy-pair__by">{t('workspace.tidy.replacedBy')}</span>
          {item.other && <CaseLine value={item.other} onOpen={open} />}
          <div className="tidy-pair__answer">
            {source(item)}
            {answerControl(item, item.case.title || t('common.untitled'), t('workspace.tidy.yesReplaced'), t('workspace.tidy.noBothHold'))}
          </div>
        </li>)}</ul>
      </section>}

      {groups.length > 0 && <section className="tidy-section" aria-labelledby="tidy-groups-heading">
        <h2 id="tidy-groups-heading" className="tidy-section__title">{t('workspace.tidy.groupsTitle')}<Count value={groups.length} /></h2>
        <ul className="tidy-pairs">{groups.map(item => {
          if (!item.other) return null;
          const turned = swapped.has(item.id);
          const child = turned ? item.other : item.case;
          const parent = turned ? item.case : item.other;
          return <li key={item.id} className="tidy-pair">
            <p className="tidy-group__line">
              <span>{t('workspace.tidy.fileThe')}</span> <CaseName value={child} onOpen={open} /> <span>{t('workspace.tidy.into')}</span> <CaseName value={parent} onOpen={open} />
              <Button type="button" variant="ghost" size="sm" className="tidy-group__swap" aria-label={t('workspace.tidy.swap')} title={t('workspace.tidy.swap')}
                onClick={() => setSwapped(current => { const next = new Set(current); if (next.has(item.id)) next.delete(item.id); else next.add(item.id); return next; })}>
                <ArrowsDownUp size={16} aria-hidden="true" />
              </Button>
            </p>
            <p className="tidy-group__meta">{caseProjectName(child, t)}</p>
            {item.duplicate && <p className="tidy-group__duplicate"><Copy size={15} aria-hidden="true" />{t('workspace.tidy.duplicateHint')}</p>}
            <div className="tidy-pair__answer">
              {source(item)}
              {answerControl(item, child.title || t('common.untitled'), t('workspace.tidy.yesGroup'), t('workspace.tidy.noKeepApart'))}
            </div>
          </li>;
        })}</ul>
      </section>}

      {archives.length > 0 && <section className="tidy-section" aria-labelledby="tidy-archives-heading">
        <div className="tidy-section__bar">
          <h2 id="tidy-archives-heading" className="tidy-section__title">{t('workspace.tidy.archivesTitle', { count: weeks })}<Count value={archives.length} /></h2>
          <Button type="button" variant="ghost" size="sm" onClick={() => setUnchecked(allChecked ? new Set(archives.map(item => item.id)) : new Set())}>{allChecked ? t('workspace.tidy.uncheckAll') : t('workspace.tidy.checkAll')}</Button>
        </div>
        <ul className="tidy-archives">{archives.map(item => {
          const Icon = TYPE_ICONS[item.case.nature];
          const title = item.case.title || t('common.untitled');
          return <li key={item.id} className="tidy-archive">
            <input type="checkbox" id={`tidy-${item.id}`} checked={!unchecked.has(item.id)} aria-label={t('workspace.tidy.archiveLabel', { title })}
              onChange={() => setUnchecked(current => { const next = new Set(current); if (next.has(item.id)) next.delete(item.id); else next.add(item.id); return next; })} />
            {Icon && <Icon size={16} aria-hidden="true" className="tidy-archive__icon" />}
            <button type="button" className="tidy-link" onClick={() => open(item.case.id)} title={t('workspace.tidy.open', { title })}><span>{title}</span></button>
            <span className="tidy-archive__meta">{caseProjectName(item.case, t)} · {t('workspace.tidy.unchangedSince', { date: formatShortDate(item.case.date) })}</span>
          </li>;
        })}</ul>
      </section>}

      {items.length > 0 && <footer className="tidy-footer">
        <p>{t('workspace.tidy.footerFound')}<br />{t('workspace.tidy.footerSettings')}</p>
        <div className="tidy-footer__actions">
          <Button type="button" variant="ghost" disabled={busy} onClick={() => void apply(true)}>{t('workspace.tidy.refuseAll')}</Button>
          <Button type="button" disabled={busy || !answered} onClick={() => void apply(false)}>{applyLabel}</Button>
        </div>
      </footer>}
    </main>
  );
}

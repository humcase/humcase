import { ClockCounterClockwise, Swap, Warning } from '@phosphor-icons/react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CaseRef, Note, Supersession, TidyItem } from '../../types';
import { api } from '../../services/tauriApi';
import { formatShortDate } from '../../utils/datetime';
import { Button } from '../ui/Button';
import './decisionHistory.css';

/**
 * Wait after a write before looking for the decisions this one replaces: the reader by meaning
 * (a pass every three seconds) has read the new text by then.
 */
const CHECK_AFTER_MS = 8000;

/**
 * A decision's place in the history ("To tidy"): marked as replaced (with the way back), the
 * older decisions it replaces, and the question asked when it is written ("Does it replace the
 * one of 28/09?"). Any case: the decisions in force it may go against, with what each says.
 * Nothing is marked without the person's answer.
 */
export function DecisionHistory({ noteId, isDecision, revision, allNotes, onOpen, onChanged }: {
  noteId: string;
  isDecision: boolean;
  revision: number;
  allNotes: Note[];
  onOpen?: (note: Note) => void;
  /** An answer changed a case: the cases are read again. */
  onChanged: () => void;
}) {
  const { t } = useTranslation();
  const [history, setHistory] = useState<Supersession | null>(null);
  const [conflicts, setConflicts] = useState<{ noteId: string; items: TidyItem[] }>({ noteId, items: [] });
  const [answered, setAnswered] = useState<{ noteId: string; proposalId: string; older: CaseRef } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ noteId: string; text: string } | null>(null);
  const [tick, setTick] = useState(0);
  const seenRef = useRef<{ noteId: string; revision: number } | null>(null);

  useEffect(() => {
    let active = true;
    const seen = seenRef.current;
    // Written while open (a new revision of the same decision): look for what it replaces.
    const written = isDecision && seen?.noteId === noteId && revision > seen.revision;
    seenRef.current = { noteId, revision };
    Promise.resolve(api.caseSupersession?.(noteId))
      .then(next => { if (active) setHistory(next ?? null); })
      .catch(() => { if (active) setHistory(null); });
    Promise.resolve(api.caseConflicts?.(noteId))
      .then(next => { if (active) setConflicts({ noteId, items: next ?? [] }); })
      .catch(() => { if (active) setConflicts({ noteId, items: [] }); });
    if (!written) return () => { active = false; };
    const timer = setTimeout(() => {
      Promise.resolve(api.tidyCheckDecision?.(noteId))
        .then(next => { if (active) setHistory(next ?? null); })
        .catch(() => undefined);
    }, CHECK_AFTER_MS);
    return () => { active = false; clearTimeout(timer); };
  }, [noteId, revision, isDecision, tick]);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      setTick(value => value + 1);
      onChanged();
    } catch (cause) {
      setError({ noteId, text: t('editor.decisionHistory.failed', { error: String(cause) }) });
    } finally {
      setBusy(false);
    }
  };
  const findCase = (id: string) => allNotes.find(candidate => candidate.id === id);
  const titleOf = (value: CaseRef) => value.title || t('common.untitled');
  const opener = (value: CaseRef) => {
    const target = onOpen && findCase(value.id);
    return target && onOpen
      ? <button type="button" className="decision-history__link" onClick={() => onOpen(target)}>{t('editor.decisionHistory.quoted', { text: titleOf(value) })}</button>
      : <span>{t('editor.decisionHistory.quoted', { text: titleOf(value) })}</span>;
  };

  // What belongs to another case (switched meanwhile) is not shown here.
  const mine = answered?.noteId === noteId ? answered : null;
  const failure = error?.noteId === noteId ? error.text : null;
  const against = conflicts.noteId === noteId ? conflicts.items.filter(item => item.case.id === noteId) : [];
  if (!history && against.length === 0) return null;
  const asking = history?.asking.filter(item => item.other?.id === noteId) ?? [];
  const replaces = history?.replaces.filter(value => value.id !== mine?.older.id) ?? [];
  const current = history?.superseded_by ? findCase(history.superseded_by.id) : undefined;
  if (!history?.superseded_by && asking.length === 0 && replaces.length === 0 && against.length === 0 && !mine && !failure) return null;

  return (
    <div className="decision-history">
      {against.map(item => {
        const decision = item.other;
        const target = decision ? findCase(decision.id) : undefined;
        return <section key={item.id} className="decision-history__band" data-kind="conflict" aria-label={t('editor.decisionHistory.conflictLabel')}>
          <Warning size={18} aria-hidden="true" className="decision-history__icon" />
          <div className="decision-history__text">
            <strong>{t('editor.decisionHistory.conflict', { title: decision ? titleOf(decision) : '' })}</strong>
            {item.passage && item.other_passage
              ? <p>{t('editor.decisionHistory.conflictHere', { text: item.passage })} {t('editor.decisionHistory.conflictThere', { text: item.other_passage })}</p>
              : <p>{item.doubt ? t('editor.decisionHistory.askDoubt') : t('editor.decisionHistory.conflictSure')}</p>}
          </div>
          <div className="decision-history__actions">
            <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => void run(async () => { await api.answerTidy([{ id: item.id, accept: false }]); })}>{t('editor.decisionHistory.notAConflict')}</Button>
            {onOpen && target && <Button type="button" variant="secondary" size="sm" onClick={() => onOpen(target)}>{t('editor.decisionHistory.seeDecision')}</Button>}
          </div>
        </section>;
      })}
      {history?.superseded_by && <section className="decision-history__band" data-kind="past" aria-label={t('editor.decisionHistory.pastLabel')}>
        <ClockCounterClockwise size={18} aria-hidden="true" className="decision-history__icon" />
        <div className="decision-history__text">
          <strong>{t('editor.decisionHistory.replacedOn', { date: formatShortDate(history.superseded_at) })}</strong>
          <p>{t('editor.decisionHistory.replacedBy')} {opener(history.superseded_by)}. {t('editor.decisionHistory.agentsRead')}</p>
        </div>
        <div className="decision-history__actions">
          <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => void run(() => api.keepDecisionInForce(noteId))}>{t('editor.decisionHistory.stillHolds')}</Button>
          {onOpen && current && <Button type="button" variant="secondary" size="sm" onClick={() => onOpen(current)}>{t('editor.decisionHistory.seeCurrent')}</Button>}
        </div>
      </section>}
      {asking.map(item => <section key={item.id} className="decision-history__band" data-kind="ask" aria-label={t('editor.decisionHistory.askLabel')}>
        <Swap size={18} aria-hidden="true" className="decision-history__icon" />
        <div className="decision-history__text">
          <strong>{t('editor.decisionHistory.ask', { date: formatShortDate(item.case.date) })}</strong>
          <p>{opener(item.case)} · {item.doubt ? t('editor.decisionHistory.askDoubt') : t('editor.decisionHistory.askSure')}</p>
        </div>
        <div className="decision-history__actions">
          <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => void run(async () => { await api.answerTidy([{ id: item.id, accept: false }]); })}>{t('editor.decisionHistory.no')}</Button>
          <Button type="button" size="sm" disabled={busy} onClick={() => void run(async () => {
            await api.answerTidy([{ id: item.id, accept: true }]);
            setAnswered({ noteId, proposalId: item.id, older: item.case });
          })}>{t('editor.decisionHistory.yes')}</Button>
        </div>
      </section>)}
      {mine && <p className="decision-history__line" role="status">
        <span>{t('editor.decisionHistory.replaces')} {opener(mine.older)} ({formatShortDate(mine.older.date)})</span>
        <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => void run(async () => {
          await api.undoTidy([mine.proposalId]);
          setAnswered(null);
        })}>{t('editor.decisionHistory.undo')}</Button>
      </p>}
      {replaces.map(value => <p key={value.id} className="decision-history__line">
        <span>{t('editor.decisionHistory.replaces')} {opener(value)} ({formatShortDate(value.date)})</span>
      </p>)}
      {failure && <p role="alert" className="decision-history__error">{failure}</p>}
    </div>
  );
}

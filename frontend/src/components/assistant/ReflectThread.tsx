import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { CircleNotch, Microphone, PaperPlaneRight, Plus, Square, X } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { Hint } from '../ui/Hint';
import type { ReflectEngine, ReflectReply, ReflectSummary, ReflectTurn } from '../../types/reflect';
import type { Note } from '../../types';
import { useComposerDictation } from './useComposerDictation';

type Subject = { id: string; title: string };

/**
 * "Think it through": a partner that restates, links what the cases already say (decisions,
 * contradictions, with their passage) and asks one question at a time. At the end it proposes a
 * summary the person corrects, then keeps as a case; nothing is written before that click.
 */
export function ReflectThread({ engine, notes, subject, candidate, onStart, onOpenNote, onKeep, onClose }: {
  engine: ReflectEngine;
  notes: Note[];
  /** The case the conversation is about, if it started from one. */
  subject: Subject | null;
  /** The case open in the workspace, offered as a starting point. */
  candidate: Subject | null;
  onStart: (subject: Subject | null) => void;
  onOpenNote: (note: Note) => void;
  /** Keeps the corrected summary; resolves to the new case's id. */
  onKeep: (subject: Subject | null, summary: ReflectSummary, turns: ReflectTurn[]) => Promise<string>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [turns, setTurns] = useState<ReflectTurn[]>([]);
  const [draft, setDraft] = useState('');
  const [pending, setPending] = useState<'reply' | 'summary' | null>(null);
  const [summary, setSummary] = useState<ReflectSummary | null>(null);
  const [kept, setKept] = useState<string | null>(null);
  // Dictation into the composer: what was heard lands in the draft, to read before sending.
  const dictation = useComposerDictation();
  const nextId = useRef(1);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const startedFor = useRef<string | null>(null);

  const summaryRef = useRef<HTMLDivElement>(null);
  useEffect(() => { endRef.current?.scrollIntoView({ block: 'end' }); }, [turns, pending]);
  // The summary opens at its top: it is read from its title down.
  const hasSummary = summary !== null;
  useEffect(() => { if (hasSummary) summaryRef.current?.scrollIntoView({ block: 'start' }); }, [hasSummary]);
  // The composer grows with a dictated answer, so it can be read before sending.
  useEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight + 2, 160)}px`;
  }, [draft]);

  const replyTo = async (list: ReflectTurn[]) => {
    const id = nextId.current++;
    setPending('reply');
    try {
      const reply = await engine.reply(subject, list);
      setTurns(current => [...current, { id, reply }]);
    } catch (error) {
      setTurns(current => [...current, { id, error: String(error instanceof Error ? error.message : error) }]);
    } finally {
      setPending(null);
      inputRef.current?.focus();
    }
  };

  // Started from a case: the partner opens by restating it and asking a first question.
  useEffect(() => {
    if (!subject || startedFor.current === subject.id) return;
    startedFor.current = subject.id;
    setTurns([]); setSummary(null); setKept(null);
    void replyTo([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [subject?.id]);

  const say = async (text: string) => {
    const said = text.trim();
    if (!said || pending) return;
    const turn = { id: nextId.current++, said };
    const list = [...turns, turn];
    setTurns(list);
    setDraft('');
    await replyTo(list);
  };

  const summarize = async () => {
    setPending('summary');
    try { setSummary(await engine.summarize(subject, turns)); } finally { setPending(null); }
  };

  const submit = (event: FormEvent) => { event.preventDefault(); void say(draft); };
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void say(draft); }
    if (event.key === 'Escape') onClose();
  };
  const toggleMic = async () => {
    if (!dictation.listening) { void dictation.start(); return; }
    const heard = await dictation.stop();
    if (heard) setDraft(current => (current.trim() ? `${current.trim()} ${heard}` : heard));
    inputRef.current?.focus();
  };
  const open = (noteId: string) => { const note = notes.find(n => n.id === noteId); if (note) onOpenNote(note); };
  const spoke = turns.some(turn => turn.said);

  return (
    <>
      <div className="assistant-panel__thread" aria-live="polite">
        {!subject && turns.length === 0 && (
          <div className="assistant-panel__intro">
            <p>{t('workspace.assistant.reflect.intro')}</p>
            {candidate && (
              <div className="assistant-panel__suggestions">
                <button type="button" className="assistant-panel__suggestion" onClick={() => onStart(candidate)}>
                  {t('workspace.assistant.reflect.onCase', { title: candidate.title })}
                </button>
              </div>
            )}
          </div>
        )}
        {subject && (
          <p className="reflect-subject">
            <span>{t('workspace.assistant.reflect.about')}</span>
            <button type="button" onClick={() => open(subject.id)}>{subject.title}</button>
          </p>
        )}

        {turns.map(turn => (
          <section key={turn.id} className="assistant-turn">
            {turn.said && <p className="assistant-turn__question">{turn.said}</p>}
            {turn.error && <p className="assistant-turn__error" role="alert">{t('workspace.assistant.error', { error: turn.error })}</p>}
            {turn.reply && <Reply reply={turn.reply} onOpen={open} last={turn === turns[turns.length - 1]} busy={pending !== null || summary !== null}
              onSummarize={() => void summarize()} onContinue={() => inputRef.current?.focus()} />}
          </section>
        ))}
        {pending && (
          <p className="assistant-turn__pending" role="status">
            <CircleNotch size={15} className="animate-spin" aria-hidden="true" />
            {pending === 'summary' ? t('workspace.assistant.reflect.summarizing') : t('workspace.assistant.reflect.thinking')}
          </p>
        )}

        {summary && (
          <div ref={summaryRef}><SummaryCard summary={summary} onChange={setSummary} kept={kept} onOpen={open}
            onBack={() => { setSummary(null); inputRef.current?.focus(); }}
            onKeep={async () => setKept(await onKeep(subject, summary, turns))} /></div>
        )}
        <div ref={endRef} />
      </div>

      {!summary && (
        <form className="assistant-panel__composer reflect-composer" onSubmit={submit}>
          <textarea ref={inputRef} rows={2} value={draft} onChange={event => setDraft(event.target.value)} onKeyDown={onKeyDown}
            placeholder={dictation.listening ? t('workspace.assistant.reflect.listening') : turns.length ? t('workspace.assistant.reflect.answer') : t('workspace.assistant.reflect.placeholder')}
            aria-label={t('workspace.assistant.reflect.placeholder')} />
          {dictation.listening && dictation.interim && <p className="reflect-composer__interim" aria-live="polite">{dictation.interim}</p>}
          {dictation.error && <p className="assistant-turn__error" role="alert">{t('workspace.assistant.reflect.micError')}</p>}
          <div className="reflect-composer__bar">
            {spoke && !pending && (
              <Button type="button" variant="link" size="sm" className="reflect-composer__end" onClick={() => void summarize()}>
                {t('workspace.assistant.reflect.end')}
              </Button>
            )}
            <Hint label={dictation.listening ? t('workspace.assistant.reflect.micStop') : t('workspace.assistant.reflect.mic')}>
              <Button type="button" variant={dictation.listening ? 'destructive' : 'outline'} size="icon-sm" disabled={dictation.starting} className="reflect-composer__mic" onClick={() => void toggleMic()}
                aria-label={dictation.listening ? t('workspace.assistant.reflect.micStop') : t('workspace.assistant.reflect.mic')} aria-pressed={dictation.listening}>
                {dictation.listening ? <Square size={14} weight="fill" /> : <Microphone size={17} />}
              </Button>
            </Hint>
            <Button type="submit" size="sm" disabled={!draft.trim() || pending !== null}><PaperPlaneRight size={16} aria-hidden="true" />{t('workspace.assistant.reflect.send')}</Button>
          </div>
        </form>
      )}
    </>
  );
}

function Reply({ reply, onOpen, last, busy, onSummarize, onContinue }: {
  reply: ReflectReply;
  onOpen: (noteId: string) => void;
  last: boolean;
  busy: boolean;
  onSummarize: () => void;
  onContinue: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="assistant-answer reflect-reply">
      {reply.restated && <p className="reflect-reply__restated">{reply.restated}</p>}
      {reply.links.length > 0 && (
        <ul className="reflect-links">
          {reply.links.map(link => (
            <li key={`${link.kind}-${link.source.note_id}`} className="reflect-link" data-kind={link.kind}>
              <span className="reflect-link__kind">{t(`workspace.assistant.reflect.link.${link.kind}`)}</span>
              <button type="button" className="assistant-sources__title" onClick={() => onOpen(link.source.note_id)}>{link.source.title}</button>
              {link.source.passages.map(passage => <q key={passage} className="assistant-sources__passage">{passage}</q>)}
            </li>
          ))}
        </ul>
      )}
      {reply.lead && (
        <p className="reflect-reply__lead"><span className="reflect-link__kind">{t('workspace.assistant.reflect.lead')}</span>{reply.lead}</p>
      )}
      {reply.question && <p className="reflect-reply__question">{reply.question}</p>}
      {reply.offer_summary && last && !busy && (
        <div className="assistant-answer__foot">
          <Button size="sm" onClick={onSummarize}>{t('workspace.assistant.reflect.summarize')}</Button>
          <Button variant="ghost" size="sm" onClick={onContinue}>{t('workspace.assistant.reflect.continue')}</Button>
        </div>
      )}
    </div>
  );
}

type ListKey = 'kept' | 'rejected' | 'evolution' | 'questions';

function SummaryCard({ summary, onChange, kept, onOpen, onBack, onKeep }: {
  summary: ReflectSummary;
  onChange: (summary: ReflectSummary) => void;
  kept: string | null;
  onOpen: (noteId: string) => void;
  onBack: () => void;
  onKeep: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const setList = (key: ListKey, items: string[]) => onChange({ ...summary, [key]: items });
  const actionCount = summary.actions.filter(action => action.create && action.title.trim()).length;

  return (
    <section className="reflect-summary" aria-label={t('workspace.assistant.reflect.summaryTitle')}>
      <header>
        <span className="reflect-link__kind">{t('workspace.assistant.reflect.summaryTitle')}</span>
        <p className="reflect-summary__hint">{t('workspace.assistant.reflect.summaryHint')}</p>
      </header>
      <AutoText className="reflect-summary__title" value={summary.title} onChange={title => onChange({ ...summary, title })} label={t('workspace.assistant.reflect.section.title')} />

      <div className="reflect-summary__section">
        <h3>{t('workspace.assistant.reflect.section.direction')}</h3>
        <AutoText value={summary.direction} onChange={direction => onChange({ ...summary, direction })} label={t('workspace.assistant.reflect.section.direction')} />
      </div>
      {(['kept', 'rejected', 'evolution', 'questions'] as const).map(key => (
        <div key={key} className="reflect-summary__section">
          <h3>{t(`workspace.assistant.reflect.section.${key}`)}</h3>
          <ul>
            {summary[key].map((item, index) => (
              <li key={index}>
                <AutoText value={item} onChange={value => setList(key, summary[key].map((v, i) => (i === index ? value : v)))} label={t(`workspace.assistant.reflect.section.${key}`)} />
                <Hint label={t('workspace.assistant.reflect.remove')}>
                  <Button variant="ghost" size="icon-sm" onClick={() => setList(key, summary[key].filter((_, i) => i !== index))} aria-label={t('workspace.assistant.reflect.remove')}><X size={14} /></Button>
                </Hint>
              </li>
            ))}
          </ul>
          <Button variant="link" size="sm" className="reflect-summary__add" onClick={() => setList(key, [...summary[key], ''])}><Plus size={13} aria-hidden="true" />{t('workspace.assistant.reflect.add')}</Button>
        </div>
      ))}

      <div className="reflect-summary__section">
        <h3>{t('workspace.assistant.reflect.section.actions')}</h3>
        <ul>
          {summary.actions.map((action, index) => (
            <li key={index} className="reflect-summary__action">
              <input type="checkbox" checked={action.create} aria-label={t('workspace.assistant.reflect.createAction', { title: action.title })}
                onChange={event => onChange({ ...summary, actions: summary.actions.map((a, i) => (i === index ? { ...a, create: event.target.checked } : a)) })} />
              <AutoText value={action.title} onChange={title => onChange({ ...summary, actions: summary.actions.map((a, i) => (i === index ? { ...a, title } : a)) })} label={t('workspace.assistant.reflect.section.actions')} />
            </li>
          ))}
        </ul>
        <Button variant="link" size="sm" className="reflect-summary__add" onClick={() => onChange({ ...summary, actions: [...summary.actions, { title: '', create: true }] })}><Plus size={13} aria-hidden="true" />{t('workspace.assistant.reflect.add')}</Button>
        {summary.waiting_on.map(item => (
          <p key={item.note_id} className="reflect-summary__waiting">
            {t('workspace.assistant.reflect.waitingOn')} <button type="button" onClick={() => onOpen(item.note_id)}>{item.title}</button>
          </p>
        ))}
      </div>

      <p className="reflect-summary__raw">{t('workspace.assistant.reflect.rawKept')}</p>
      <div className="assistant-answer__foot">
        {kept
          ? <Button variant="link" size="sm" onClick={() => onOpen(kept)}>{t('workspace.assistant.reflect.keptOpen')}</Button>
          : <>
            <Button size="sm" disabled={busy || !summary.title.trim()} onClick={async () => { setBusy(true); try { await onKeep(); } finally { setBusy(false); } }}>
              {actionCount ? t('workspace.assistant.reflect.keepWith', { count: actionCount }) : t('workspace.assistant.reflect.keep')}
            </Button>
            <Button variant="ghost" size="sm" disabled={busy} onClick={onBack}>{t('workspace.assistant.reflect.back')}</Button>
          </>}
      </div>
    </section>
  );
}

/** A line of the summary, edited in place; it grows with its text. */
function AutoText({ value, onChange, label, className }: { value: string; onChange: (value: string) => void; label: string; className?: string }) {
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }, [value]);
  return <textarea ref={ref} rows={1} className={`reflect-autotext ${className ?? ''}`} value={value} aria-label={label} onChange={event => onChange(event.target.value)} />;
}

import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowCounterClockwise, CircleNotch, Copy, PaperPlaneRight, Plus, X } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { Hint } from '../ui/Hint';
import type { AssistantAnswer, AssistantEngine, AssistantTurn } from '../../types/assistant';
import type { Note } from '../../types';
import './assistant.css';

/**
 * "Ask Humcase": a question answered from the cases of the space, each sentence citing its case.
 * The conversation keeps its exchanges for the session only: the cases are the memory, an answer
 * worth keeping becomes a case. Nothing is written without a click.
 */
export function AssistantPanel({ engine, notes, scopeLabel, onOpenNote, onKeep, onCreateSubCase, onConnectAi, onClose, devControls }: {
  engine: AssistantEngine;
  notes: Note[];
  scopeLabel: string;
  onOpenNote: (note: Note) => void;
  /** Keeps the answer as a case; resolves to the new case's id. */
  onKeep: (question: string, answer: AssistantAnswer) => Promise<string>;
  onCreateSubCase: (parentId: string, title: string) => Promise<string>;
  /** Opens Settings on the AI services (shown when no AI is connected). */
  onConnectAi: () => void;
  onClose: () => void;
  /** Development only (the mockup): extra controls under the composer. */
  devControls?: React.ReactNode;
}) {
  const { t } = useTranslation();
  const [turns, setTurns] = useState<AssistantTurn[]>([]);
  const [draft, setDraft] = useState('');
  const [pending, setPending] = useState(false);
  const [done, setDone] = useState<Record<string, string>>({});
  const nextId = useRef(1);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => { inputRef.current?.focus(); }, []);
  useEffect(() => { endRef.current?.scrollIntoView({ block: 'end' }); }, [turns, pending]);

  const ask = async (question: string) => {
    const text = question.trim();
    if (!text || pending) return;
    const id = nextId.current++;
    const previous = turns.filter(turn => turn.answer);
    setTurns(list => [...list, { id, question: text }]);
    setDraft('');
    setPending(true);
    try {
      const answer = await engine(text, previous);
      setTurns(list => list.map(turn => turn.id === id ? { ...turn, answer } : turn));
    } catch (error) {
      setTurns(list => list.map(turn => turn.id === id ? { ...turn, error: String(error instanceof Error ? error.message : error) } : turn));
    } finally {
      setPending(false);
      inputRef.current?.focus();
    }
  };

  const submit = (event: FormEvent) => { event.preventDefault(); void ask(draft); };
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void ask(draft); }
    if (event.key === 'Escape') onClose();
  };
  const restart = () => { setTurns([]); setDone({}); inputRef.current?.focus(); };
  const open = (noteId: string) => { const note = notes.find(n => n.id === noteId); if (note) onOpenNote(note); };
  const mark = (key: string, label: string) => setDone(state => ({ ...state, [key]: label }));

  return (
    <aside className="assistant-panel" aria-label={t('workspace.assistant.title')}>
      <header className="assistant-panel__head">
        <div className="assistant-panel__titles">
          <h2>{t('workspace.assistant.title')}</h2>
          <span className="assistant-panel__scope">{scopeLabel}</span>
        </div>
        <div className="assistant-panel__head-actions">
          {turns.length > 0 && <Hint label={t('workspace.assistant.restart')}><Button variant="ghost" size="icon-sm" onClick={restart} aria-label={t('workspace.assistant.restart')}><ArrowCounterClockwise size={17} /></Button></Hint>}
          <Hint label={t('workspace.assistant.close')}><Button variant="ghost" size="icon-sm" onClick={onClose} aria-label={t('workspace.assistant.close')}><X size={17} /></Button></Hint>
        </div>
      </header>

      <div className="assistant-panel__thread" aria-live="polite">
        {turns.length === 0 && (
          <div className="assistant-panel__intro">
            <p>{t('workspace.assistant.intro')}</p>
            <div className="assistant-panel__suggestions">
              {(['decide', 'stale', 'week'] as const).map(key => (
                <button key={key} type="button" className="assistant-panel__suggestion" onClick={() => void ask(t(`workspace.assistant.suggest.${key}`))}>{t(`workspace.assistant.suggest.${key}`)}</button>
              ))}
            </div>
          </div>
        )}
        {turns.map(turn => (
          <section key={turn.id} className="assistant-turn">
            <p className="assistant-turn__question">{turn.question}</p>
            {turn.error && <p className="assistant-turn__error" role="alert">{t('workspace.assistant.error', { error: turn.error })}</p>}
            {turn.answer && <Answer answer={turn.answer} turnId={turn.id} done={done} onOpen={open}
              onKeep={async () => { const id = await onKeep(turn.question, turn.answer!); mark(`keep-${turn.id}`, id); }}
              onAction={async () => { const action = turn.answer!.action!; const id = await onCreateSubCase(action.parentId, action.title); mark(`action-${turn.id}`, id); }}
              onConnectAi={onConnectAi} />}
          </section>
        ))}
        {pending && <p className="assistant-turn__pending" role="status"><CircleNotch size={15} className="animate-spin" aria-hidden="true" />{t('workspace.assistant.searching')}</p>}
        <div ref={endRef} />
      </div>

      <form className="assistant-panel__composer" onSubmit={submit}>
        <textarea ref={inputRef} rows={2} value={draft} onChange={event => setDraft(event.target.value)} onKeyDown={onKeyDown}
          placeholder={turns.length ? t('workspace.assistant.followUp') : t('workspace.assistant.placeholder')} aria-label={t('workspace.assistant.placeholder')} />
        <Button type="submit" size="sm" disabled={!draft.trim() || pending}><PaperPlaneRight size={16} aria-hidden="true" />{t('workspace.assistant.send')}</Button>
      </form>
      {devControls && <div className="assistant-panel__dev">{devControls}</div>}
    </aside>
  );
}

function Answer({ answer, turnId, done, onOpen, onKeep, onAction, onConnectAi }: {
  answer: AssistantAnswer;
  turnId: number;
  done: Record<string, string>;
  onOpen: (noteId: string) => void;
  onKeep: () => Promise<void>;
  onAction: () => Promise<void>;
  onConnectAi: () => void;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState<string | null>(null);
  const run = async (key: string, task: () => Promise<void>) => { setBusy(key); try { await task(); } finally { setBusy(null); } };
  const plain = answer.parts.map(part => part.text).join('');

  return (
    <div className="assistant-answer">
      {answer.kind === 'no_ai' && <p className="assistant-answer__note">{t('workspace.assistant.noAi')}</p>}
      {answer.kind === 'not_found' && <p className="assistant-answer__note">{t('workspace.assistant.notFound')}</p>}
      {answer.parts.length > 0 && (
        <p className="assistant-answer__text">
          {answer.parts.map((part, index) => (
            <span key={index}>{part.text}{part.cite && answer.sources[part.cite - 1] && (
              <button type="button" className="assistant-cite" onClick={() => onOpen(answer.sources[part.cite! - 1].noteId)}
                aria-label={t('workspace.assistant.openSource', { title: answer.sources[part.cite - 1].title })}>{part.cite}</button>
            )}</span>
          ))}
        </p>
      )}

      {answer.sources.length > 0 && (
        <div className="assistant-sources">
          <h3>{answer.kind === 'no_ai' ? t('workspace.assistant.closest') : t('workspace.assistant.sources')}</h3>
          <ol>
            {answer.sources.map((source, index) => (
              <li key={source.noteId}>
                <span className="assistant-sources__number">{index + 1}</span>
                <div>
                  <button type="button" className="assistant-sources__title" onClick={() => onOpen(source.noteId)}>{source.title}</button>
                  {source.passage && <q className="assistant-sources__passage">{source.passage}</q>}
                </div>
              </li>
            ))}
          </ol>
        </div>
      )}

      {answer.action && (
        <div className="assistant-action">
          <p>{t('workspace.assistant.actionSubCase', { title: answer.action.title, parent: answer.action.parentTitle })}</p>
          {done[`action-${turnId}`]
            ? <Button variant="link" size="sm" onClick={() => onOpen(done[`action-${turnId}`])}>{t('workspace.assistant.actionDone')}</Button>
            : <Button variant="secondary" size="sm" disabled={busy !== null} onClick={() => void run('action', onAction)}><Plus size={15} aria-hidden="true" />{t('workspace.assistant.create')}</Button>}
        </div>
      )}

      <div className="assistant-answer__foot">
        {answer.kind === 'not_found'
          ? (done[`keep-${turnId}`]
            ? <Button variant="link" size="sm" onClick={() => onOpen(done[`keep-${turnId}`])}>{t('workspace.assistant.kept')}</Button>
            : <Button variant="secondary" size="sm" disabled={busy !== null} onClick={() => void run('keep', onKeep)}><Plus size={15} aria-hidden="true" />{t('workspace.assistant.newCaseFromQuestion')}</Button>)
          : answer.kind === 'answer' && (done[`keep-${turnId}`]
            ? <Button variant="link" size="sm" onClick={() => onOpen(done[`keep-${turnId}`])}>{t('workspace.assistant.kept')}</Button>
            : <Button variant="secondary" size="sm" disabled={busy !== null} onClick={() => void run('keep', onKeep)}>{t('workspace.assistant.keep')}</Button>)}
        {answer.kind === 'answer' && <Hint label={t('workspace.assistant.copy')}><Button variant="ghost" size="icon-sm" onClick={() => void navigator.clipboard?.writeText(plain)} aria-label={t('workspace.assistant.copy')}><Copy size={16} /></Button></Hint>}
        {answer.kind === 'no_ai' && <Button variant="link" size="sm" onClick={onConnectAi}>{t('workspace.assistant.connectAi')}</Button>}
      </div>
      {answer.sent && <p className="assistant-answer__sent">{t('workspace.assistant.sent', { service: answer.sent.service, count: answer.sent.excerpts })}</p>}
    </div>
  );
}

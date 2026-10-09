import { Fragment, useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowUp, Check, DotsThree, Microphone, Minus, Square, X } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { Hint, Kbd } from '../ui/Hint';
import { shortcutKeys } from '../ui/shortcutKeys';
import { Menu, MenuItem, MenuLabel, MenuRadioGroup, MenuSeparator } from '../ui/Menu';
import { HumcaseKey } from '../brand/HumcaseKey';
import { api } from '../../services/tauriApi';
import { getAiCleanProvider, getAiModel } from '../../utils/prefs';
import { previousTurnsV2, type AssistantEntry, type AssistantPreviousTurnV2, type AssistantStatus } from '../../types/assistant';
import type { ProjectSpace } from '../../types';
import { AssistantReplyView } from './AssistantReplyView';
import { conversationSources, keptConversationMarkdown } from './keptConversation';
import { useComposerDictation } from './useComposerDictation';
import { useFloatingWindow } from './useFloatingWindow';
import './assistant-window.css';

export type CaseRef = { id: string; title: string };
/** "Think it through" from a case: a new conversation that starts from it (`key` changes each time). */
export type AssistantKickoff = { key: number; case: CaseRef };

const SUGGESTIONS = ['week', 'conflict', 'howto', 'decide'] as const;
const message = (error: unknown) => String(error instanceof Error ? error.message : error);

/**
 * The assistant: one window inside the app that answers from the cases and from Humcase's help,
 * discusses, cites what it read, asks at most one question and proposes actions the person
 * confirms with a click. Minimised (or closed) it becomes a key in the corner; the conversation
 * lives for the session, a conversation worth keeping becomes a case.
 */
export function AssistantWindow({ open, hidden = false, onOpenChange, space, scopeLabel, openCase, kickoff, keepSpace, onOpenCase, onChanged, onConnectAi }: {
  open: boolean;
  /** Mini mode: nothing shown, the conversation kept. */
  hidden?: boolean;
  onOpenChange: (open: boolean) => void;
  /** The space read (undefined: every space). */
  space: ProjectSpace | 'unassigned' | undefined;
  scopeLabel: string;
  /** The case open in the editor, offered as the context. */
  openCase: CaseRef | null;
  kickoff: AssistantKickoff | null;
  /** Where "Keep as a case" files the conversation (its Inbox). */
  keepSpace: ProjectSpace;
  onOpenCase: (id: string) => void;
  onChanged: () => void;
  onConnectAi: () => void;
}) {
  const { t } = useTranslation();
  const frame = useFloatingWindow();
  const [entries, setEntries] = useState<AssistantEntry[]>([]);
  const [draft, setDraft] = useState('');
  const [pending, setPending] = useState(false);
  const [status, setStatus] = useState<AssistantStatus | null>(null);
  const [modelLabel, setModelLabel] = useState<string | null>(null);
  const [statusTick, setStatusTick] = useState(0);
  // The context chip, taken off by its ×: no case is sent until another one is opened.
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [subject, setSubject] = useState<CaseRef | null>(null);
  const [kept, setKept] = useState<string | null>(null);
  const [keepError, setKeepError] = useState<string | null>(null);
  // A reply arrived while minimised: the key shows a light.
  const [unseen, setUnseen] = useState(false);
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) { setWasOpen(open); if (open) setUnseen(false); }
  const dictation = useComposerDictation();
  const nextId = useRef(1);
  // A new conversation drops the replies still on their way.
  const epoch = useRef(0);
  const openRef = useRef(open);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const startedFor = useRef<number | null>(null);

  const context = openCase && openCase.id !== dismissed ? openCase : null;

  useEffect(() => { openRef.current = open; if (open) inputRef.current?.focus(); }, [open]);
  useEffect(() => { endRef.current?.scrollIntoView?.({ block: 'end' }); }, [entries, pending, kept]);
  // The composer grows with a dictated text, so it can be read before sending.
  useEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(Math.max(el.scrollHeight, 40), 160)}px`;
  }, [draft]);

  // The small screen and the AI's name: read when the window opens and after an action.
  useEffect(() => {
    if (!open) return;
    let live = true;
    api.assistantStatus(space, getAiCleanProvider())
      .then(next => { if (live) { setStatus(next); setModelLabel(next.model_label); } })
      .catch(() => { if (live) setStatus(null); });
    return () => { live = false; };
  }, [open, space, statusTick]);

  const ask = async (said: string | null, contextId: string | undefined, previous: AssistantPreviousTurnV2[]) => {
    const run = epoch.current;
    const id = nextId.current++;
    setEntries(list => [...list, { id, said }]);
    setPending(true);
    const provider = getAiCleanProvider();
    try {
      const reply = await api.assistantReply(said ?? '', previous, space, contextId, provider, getAiModel(provider) || undefined);
      if (run !== epoch.current) return;
      setEntries(list => list.map(entry => entry.id === id ? { ...entry, reply } : entry));
      if (reply.model_label) setModelLabel(reply.model_label);
      if (!openRef.current) setUnseen(true);
    } catch (error) {
      if (run !== epoch.current) return;
      setEntries(list => list.map(entry => entry.id === id ? { ...entry, error: message(error) } : entry));
    } finally {
      if (run === epoch.current) { setPending(false); inputRef.current?.focus(); }
    }
  };

  const send = (text: string) => {
    const said = text.trim();
    if (!said || pending) return;
    setDraft('');
    void ask(said, context?.id, previousTurnsV2(entries));
  };

  const restart = () => {
    epoch.current += 1;
    setEntries([]); setPending(false); setKept(null); setKeepError(null); setSubject(null);
    inputRef.current?.focus();
  };

  // "Think it through" on a case: a new conversation, the engine restates the case and asks first.
  useEffect(() => {
    if (!kickoff || startedFor.current === kickoff.key) return;
    startedFor.current = kickoff.key;
    restart();
    setDismissed(null);
    setSubject(kickoff.case);
    void ask(null, kickoff.case.id, []);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kickoff?.key]);

  const changed = () => { onChanged(); setStatusTick(tick => tick + 1); };

  const keep = async () => {
    setKeepError(null);
    const title = entries.find(entry => entry.said)?.said ?? subject?.title ?? t('workspace.assistantWindow.title');
    const content = keptConversationMarkdown(entries, {
      you: t('workspace.assistantWindow.you'),
      assistant: t('workspace.assistantWindow.title'),
      sources: t('workspace.assistantWindow.keptSources'),
      source: (source, passage) => t('workspace.assistantWindow.keptSource', { title: source, passage }),
    });
    try {
      setKept(await api.keepAnswer(keepSpace, title, content, conversationSources(entries)));
      changed();
    } catch (error) { setKeepError(message(error)); }
  };

  const submit = (event: FormEvent) => { event.preventDefault(); send(draft); };
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); send(draft); }
    if (event.key === 'Escape') onOpenChange(false);
  };
  const toggleMic = async () => {
    if (!dictation.listening) { void dictation.start(); return; }
    const heard = await dictation.stop();
    if (heard) setDraft(current => (current.trim() ? `${current.trim()} ${heard}` : heard));
    inputRef.current?.focus();
  };

  if (hidden) return null;
  if (!open) {
    return (
      <Hint label={t('workspace.assistantWindow.reopen')} side="left">
        <button type="button" className="assistant-window-key" onClick={() => onOpenChange(true)} aria-label={t('workspace.assistantWindow.reopen')}>
          <HumcaseKey size={54} />
          {unseen && <i className="assistant-window-key__light" role="status" aria-label={t('workspace.assistantWindow.newReply')} />}
        </button>
      </Hint>
    );
  }

  const replied = entries.some(entry => entry.reply?.kind === 'reply');
  const statusText = pending ? t('workspace.assistantWindow.status.reading')
    : modelLabel ? t('workspace.assistantWindow.status.ready', { model: modelLabel })
    : status ? t('workspace.assistantWindow.status.noAi')
    : t('workspace.assistantWindow.status.readyPlain');
  const { rect, ghost } = frame;

  return (
    <>
      {ghost && <div className="assistant-window-ghost" aria-hidden="true" style={{ left: ghost.x, top: ghost.y, width: ghost.width, height: ghost.height }} />}
      <section className="assistant-window" aria-label={t('workspace.assistantWindow.title')} data-thinking={pending || undefined} data-snapping={frame.snapping || undefined}
        style={{ left: rect.x, top: rect.y, width: rect.width, height: rect.height }}>
        <header className="assistant-window__head" onPointerDown={frame.onDragStart} title={t('workspace.assistantWindow.move')}>
          <span className="assistant-window__grip" aria-hidden="true" />
          <HumcaseKey size={26} />
          <div className="assistant-window__title">
            <strong>{t('workspace.assistantWindow.title')}</strong>
            <small><span className="assistant-window__led" data-off={!pending && !modelLabel && status !== null ? true : undefined} aria-hidden="true" /><span>{statusText}</span></small>
          </div>
          <Menu align="end" trigger={
            <Button variant="ghost" size="icon-sm" className="assistant-window__head-key" aria-label={t('workspace.assistantWindow.options')}><DotsThree size={18} weight="bold" /></Button>
          }>
            <MenuLabel>{t('workspace.assistantWindow.reads')}</MenuLabel>
            <MenuRadioGroup value="scope" onValueChange={() => {}} label={t('workspace.assistantWindow.reads')} options={[{ value: 'scope', label: scopeLabel }]} />
            <MenuSeparator />
            {replied && !kept && <MenuItem onSelect={() => void keep()}>{t('workspace.assistantWindow.keep')}</MenuItem>}
            <MenuItem onSelect={restart}>{t('workspace.assistantWindow.newConversation')}</MenuItem>
          </Menu>
          <Hint label={t('workspace.assistantWindow.minimise')} shortcut="Control+J">
            <Button variant="ghost" size="icon-sm" className="assistant-window__head-key" onClick={() => onOpenChange(false)} aria-label={t('workspace.assistantWindow.minimise')}><Minus size={16} weight="bold" /></Button>
          </Hint>
          <Hint label={t('workspace.assistantWindow.close')}>
            <Button variant="ghost" size="icon-sm" className="assistant-window__head-key" onClick={() => onOpenChange(false)} aria-label={t('workspace.assistantWindow.close')}><X size={16} weight="bold" /></Button>
          </Hint>
        </header>

        <div className="assistant-window__context">
          {context && <>
            <span>{t('workspace.assistantWindow.context')}</span>
            <span className="assistant-window__pill">
              <i aria-hidden="true" />
              <span>{context.title}</span>
              <button type="button" onClick={() => setDismissed(context.id)} aria-label={t('workspace.assistantWindow.removeContext')}><X size={11} weight="bold" /></button>
            </span>
          </>}
          <span className="assistant-window__scope">{scopeLabel}</span>
        </div>

        <div className="assistant-window__thread" aria-live="polite">
          {entries.length === 0 && (
            <div className="assistant-window__empty">
              {status && (
                <div className="assistant-window__screen">
                  <div><span>{scopeLabel}</span><b>{t('workspace.assistantWindow.screen.cases', { count: status.cases })}</b></div>
                  <div><span>{t('workspace.assistantWindow.screen.toDecide')}</span><b>{status.to_decide}</b></div>
                  <div><span>{t('workspace.assistantWindow.screen.toTidy')}</span><b>{status.to_tidy}</b></div>
                </div>
              )}
              <h2>{t('workspace.assistantWindow.heading')}</h2>
              <p>{t('workspace.assistantWindow.intro')}</p>
              <div className="assistant-window__suggestions">
                {SUGGESTIONS.map(key => (
                  <button key={key} type="button" className="assistant-window__suggestion" onClick={() => send(t(`workspace.assistantWindow.suggest.${key}.text`))}>
                    <small>{t(`workspace.assistantWindow.suggest.${key}.tag`)}</small>
                    {t(`workspace.assistantWindow.suggest.${key}.text`)}
                  </button>
                ))}
              </div>
            </div>
          )}
          {entries.map(entry => (
            <Fragment key={entry.id}>
              {entry.said && <p className="assistant-window__said">{entry.said}</p>}
              {entry.error && <p className="assistant-window__error" role="alert">{t('workspace.assistantWindow.error', { error: entry.error })}</p>}
              {entry.reply && <AssistantReplyView reply={entry.reply} onOpenCase={onOpenCase} onConnectAi={onConnectAi} onChanged={changed} />}
            </Fragment>
          ))}
          {pending && <div className="assistant-window__typing" role="status" aria-label={t('workspace.assistantWindow.status.reading')}><i /><i /><i /></div>}
          {kept && (
            <div className="assistant-window__done" role="status">
              <Check size={15} weight="bold" aria-hidden="true" />
              <span>{t('workspace.assistantWindow.kept')}</span>
              <button type="button" onClick={() => onOpenCase(kept)}>{t('workspace.assistantWindow.open')}</button>
            </div>
          )}
          {keepError && <p className="assistant-window__error" role="alert">{t('workspace.assistantWindow.actionError', { error: keepError })}</p>}
          <div ref={endRef} />
        </div>

        <form className="assistant-window__composer" onSubmit={submit}>
          <div className="assistant-window__box">
            <textarea ref={inputRef} rows={1} value={draft} onChange={event => setDraft(event.target.value)} onKeyDown={onKeyDown}
              placeholder={dictation.listening ? t('workspace.assistantWindow.listening') : t('workspace.assistantWindow.placeholder')}
              aria-label={t('workspace.assistantWindow.placeholder')} />
            <Hint label={dictation.listening ? t('workspace.assistantWindow.stopDictation') : t('workspace.assistantWindow.dictate')}>
              <Button type="button" variant={dictation.listening ? 'destructive' : 'outline'} size="icon-sm" disabled={dictation.starting} onClick={() => void toggleMic()}
                aria-label={dictation.listening ? t('workspace.assistantWindow.stopDictation') : t('workspace.assistantWindow.dictate')} aria-pressed={dictation.listening}>
                {dictation.listening ? <Square size={14} weight="fill" /> : <Microphone size={17} />}
              </Button>
            </Hint>
            <Hint label={t('workspace.assistantWindow.send')}>
              <Button type="submit" size="icon-sm" disabled={!draft.trim() || pending} aria-label={t('workspace.assistantWindow.send')}><ArrowUp size={16} weight="bold" /></Button>
            </Hint>
          </div>
          {dictation.listening && dictation.interim && <p className="assistant-window__interim" aria-live="polite">{dictation.interim}</p>}
          {dictation.error && <p className="assistant-window__error" role="alert">{t('workspace.assistantWindow.micError')}</p>}
          <div className="assistant-window__foot">
            <span>{t('workspace.assistantWindow.footer')}</span>
            <span><Kbd>{t('workspace.assistantWindow.keyEnter')}</Kbd> {t('workspace.assistantWindow.keySend')} · <Kbd>{shortcutKeys('Control+J').join(' ')}</Kbd></span>
          </div>
          <span className="assistant-window__resize" aria-hidden="true" title={t('workspace.assistantWindow.resize')} onPointerDown={frame.onResizeStart} />
        </form>
      </section>
    </>
  );
}

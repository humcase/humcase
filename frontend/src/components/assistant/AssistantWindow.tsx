import { Fragment, useEffect, useRef, useState, type FormEvent, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowUp, Check, DotsThree, Microphone, Minus, PushPin, Square, X } from '@phosphor-icons/react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Button } from '../ui/Button';
import { Hint, Kbd } from '../ui/Hint';
import { shortcutKeys } from '../ui/shortcutKeys';
import { Menu, MenuCheckboxItem, MenuItem, MenuLabel, MenuRadioGroup, MenuSeparator } from '../ui/Menu';
import { HumcaseKey } from '../brand/HumcaseKey';
import { api } from '../../services/tauriApi';
import { getAiCleanProvider, getAiModel, getAssistantSeen, setAssistantSeen } from '../../utils/prefs';
import { previousTurnsV2, type AssistantDone, type AssistantEntry, type AssistantPreviousTurnV2, type AssistantReply, type AssistantStatus } from '../../types/assistant';
import type { ProjectSpace } from '../../types';
import { AssistantReplyView } from './AssistantReplyView';
import { conversationSources, keptConversationMarkdown } from './keptConversation';
import { useComposerDictation } from './useComposerDictation';
import { aiErrorMessage, isMissingKeyError } from '../../utils/aiErrors';
import { useFloatingWindow } from './useFloatingWindow';
import './assistant-window.css';

export type CaseRef = { id: string; title: string };
/** A reply, or its error, for one exchange. */
export type EntryLanded = { id: number; reply?: AssistantReply; error?: string };

const SUGGESTIONS = ['week', 'conflict', 'howto', 'decide'] as const;
/** Offered while a case is open (it is the context): what one asks about one case. */
const CASE_SUGGESTIONS = ['open', 'sum', 'split', 'against'] as const;
const STATUS_EVERY_MS = 120_000;
const message = (error: unknown) => String(error instanceof Error ? error.message : error);
// Ids stay unique across the two windows a conversation moves between.
let exchangeSeq = 0;
const newExchangeId = () => Date.now() * 100 + (exchangeSeq++ % 100);

/**
 * The assistant: one window inside the app that answers from the cases and from Humcase's help,
 * discusses, cites what it read, asks at most one question and proposes actions the person
 * confirms with a click. Closed, it is a key in the corner, the way in with Ctrl+J; the
 * conversation lives for the session, a conversation worth keeping becomes a case. Pinned (or when
 * Humcase goes behind), it moves to Mini Humcase, its own window on top of other applications.
 */
export function AssistantWindow({ open, hidden = false, onOpenChange, space, scopeLabel, openCase, keepSpace, onOpenCase, onChanged, onGerminate, onConnectAi,
  variant = 'inside', seed = null, landed = null, onSnapshot, onLanded, onPin, stayOnTop, onStayOnTopChange }: {
  open: boolean;
  /** Mini mode: nothing shown, the conversation kept. */
  hidden?: boolean;
  onOpenChange: (open: boolean) => void;
  /** The space read (undefined: every space). */
  space: ProjectSpace | 'unassigned' | undefined;
  scopeLabel: string;
  /** The case open in the editor, offered as the context. */
  openCase: CaseRef | null;
  /** Where "Keep as a case" files the conversation (its Inbox). */
  keepSpace: ProjectSpace;
  onOpenCase: (id: string) => void;
  onChanged: () => void;
  /** Opens the case with Sprout running (an action the assistant proposed). */
  onGerminate: (id: string) => void;
  onConnectAi: () => void;
  /** `mini`: Mini Humcase, the assistant in its own window on top of other applications. */
  variant?: 'inside' | 'mini';
  /** A conversation handed over by the other window (`key` changes each time). */
  seed?: { key: number; entries: AssistantEntry[] } | null;
  /** A reply for an exchange asked in the other window, landed there. */
  landed?: (EntryLanded & { key: number }) | null;
  /** The conversation as it is now, for handing it over. */
  onSnapshot?: (entries: AssistantEntry[]) => void;
  /** A reply landed here: the other window may show its exchange now. */
  onLanded?: (landed: EntryLanded) => void;
  /** The pin: inside, to Mini Humcase; in Mini Humcase, back into the app. */
  onPin?: () => void;
  /** "Stay on top when Humcase goes behind". */
  stayOnTop?: boolean;
  onStayOnTopChange?: (on: boolean) => void;
}) {
  const { t } = useTranslation();
  const frame = useFloatingWindow();
  const [entries, setEntries] = useState<AssistantEntry[]>([]);
  const [draft, setDraft] = useState('');
  const [status, setStatus] = useState<AssistantStatus | null>(null);
  const [modelLabel, setModelLabel] = useState<string | null>(null);
  const [statusTick, setStatusTick] = useState(0);
  // The context chip, taken off by its ×: no case is sent until another one is opened.
  const [dismissed, setDismissed] = useState<string | null>(null);
  // While a case is the context, its own suggestions come first; "Other questions" shows the rest.
  const [general, setGeneral] = useState(false);
  const [kept, setKept] = useState<string | null>(null);
  const [keepError, setKeepError] = useState<string | null>(null);
  // A reply arrived while minimised: the key shows a light.
  const [unseen, setUnseen] = useState(false);
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) { setWasOpen(open); if (open) setUnseen(false); }
  const dictation = useComposerDictation();
  const epoch = useRef(0);
  const handover = useRef({ onSnapshot, onLanded });
  useEffect(() => { handover.current = { onSnapshot, onLanded }; });
  // An exchange still waiting for its reply (asked here or in the other window).
  const pending = entries.some(entry => !entry.reply && !entry.error);
  const openRef = useRef(open);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const endRef = useRef<HTMLDivElement>(null);

  // The × holds for the case it was clicked on, until another case is opened.
  const [dismissedFor, setDismissedFor] = useState(openCase?.id ?? null);
  if ((openCase?.id ?? null) !== dismissedFor) { setDismissedFor(openCase?.id ?? null); if (dismissed) setDismissed(null); }
  const context = openCase && openCase.id !== dismissed ? openCase : null;

  const keyRef = useRef<HTMLButtonElement>(null);
  const wasShown = useRef(open && !hidden);
  useEffect(() => {
    openRef.current = open;
    const shown = open && !hidden;
    if (shown) inputRef.current?.focus();
    // Closed or handed to the other window while focused inside: focus returns to the key.
    else if (wasShown.current && !hidden && variant === 'inside') keyRef.current?.focus();
    wasShown.current = shown;
  }, [open, hidden, variant]);
  // Closed or hidden while listening: the microphone and the transcription stop, what was heard is
  // dropped (nothing is sent without the person reading it).
  const { listening, cancel: cancelDictation } = dictation;
  useEffect(() => { if (listening && (!open || hidden)) cancelDictation(); }, [listening, open, hidden, cancelDictation]);
  useEffect(() => { endRef.current?.scrollIntoView?.({ block: 'end' }); }, [entries, pending, kept]);
  // The composer grows with a dictated text, so it can be read before sending.
  useEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(Math.max(el.scrollHeight, 40), 160)}px`;
  }, [draft]);

  // The small screen and the AI's name: read when the window opens, after an action, and every
  // two minutes while it is the key (more to decide or a new contradiction lights it).
  const seenKey = space ?? 'all';
  useEffect(() => {
    if (hidden || (!open && variant === 'mini')) return;
    let live = true;
    const read = () => api.assistantStatus(space, getAiCleanProvider())
      .then(next => {
        if (!live) return;
        // Open, the person sees the screen; the first reading is taken as seen too.
        if (openRef.current || !getAssistantSeen(seenKey)) setAssistantSeen(seenKey, { toDecide: next.to_decide, conflicts: next.conflicts });
        setStatus(next); setModelLabel(next.model_label);
      })
      .catch(() => { if (live) setStatus(null); });
    void read();
    const timer = open ? null : setInterval(() => void read(), STATUS_EVERY_MS);
    return () => { live = false; if (timer) clearInterval(timer); };
  }, [open, hidden, variant, space, seenKey, statusTick]);

  // A reply fills its exchange if it is still waiting (a new conversation dropped it otherwise).
  const settle = ({ id, reply, error }: EntryLanded) =>
    setEntries(list => list.map(entry => entry.id === id && !entry.reply && !entry.error ? (reply ? { ...entry, reply } : { ...entry, error }) : entry));

  const ask = async (said: string | null, contextId: string | undefined, previous: AssistantPreviousTurnV2[]) => {
    const run = epoch.current;
    const id = newExchangeId();
    setEntries(list => [...list, { id, said }]);
    const provider = getAiCleanProvider();
    let result: EntryLanded;
    try {
      const reply = await api.assistantReply(said ?? '', previous, space, contextId, provider, getAiModel(provider) || undefined);
      result = { id, reply };
      if (reply.model_label) setModelLabel(reply.model_label);
    } catch (error) {
      result = { id, error: message(error) };
    }
    // A new conversation dropped this exchange: nothing lights, nothing goes to the other window.
    if (run !== epoch.current) return;
    if (result.reply && !openRef.current) setUnseen(true);
    settle(result);
    handover.current.onLanded?.(result);
    inputRef.current?.focus();
  };

  // A conversation handed over by the other window, and replies that landed there.
  const [seenSeed, setSeenSeed] = useState<typeof seed>(null);
  if (seed !== seenSeed) {
    setSeenSeed(seed);
    if (seed) { setEntries(seed.entries); setKept(null); setKeepError(null); }
  }
  const [seenLanded, setSeenLanded] = useState<typeof landed>(null);
  if (landed !== seenLanded) {
    setSeenLanded(landed);
    if (landed) settle(landed);
  }
  useEffect(() => { handover.current.onSnapshot?.(entries); }, [entries]);

  const send = (text: string) => {
    const said = text.trim();
    if (!said || pending) return;
    setDraft('');
    void ask(said, context?.id, previousTurnsV2(entries));
  };

  const restart = () => {
    epoch.current += 1;
    setEntries([]); setKept(null); setKeepError(null); setGeneral(false);
    inputRef.current?.focus();
  };

  const changed = () => { onChanged(); setStatusTick(tick => tick + 1); };
  // An action done (or undone) is recorded in its exchange, which the other window receives too.
  const markDone = (id: number, index: number, next: AssistantDone | null) => setEntries(list => list.map(entry => {
    if (entry.id !== id) return entry;
    const done = { ...entry.done };
    if (next) done[index] = next; else delete done[index];
    return { ...entry, done };
  }));

  const keep = async () => {
    setKeepError(null);
    const title = entries.find(entry => entry.said)?.said ?? t('workspace.assistantWindow.title');
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
  const isMini = variant === 'mini';
  if (!open && isMini) return null;
  if (!open) {
    const seen = status ? getAssistantSeen(seenKey) : null;
    const attention = !!status && !!seen && (status.to_decide > seen.toDecide || status.conflicts > seen.conflicts);
    return (
      <Hint label={t('workspace.assistantWindow.reopen')} side="left">
        <button ref={keyRef} type="button" className="assistant-window-key" onClick={() => onOpenChange(true)} aria-label={t('workspace.assistantWindow.reopen')}>
          <HumcaseKey size={54} />
          {unseen
            ? <i className="assistant-window-key__light" role="status" aria-label={t('workspace.assistantWindow.newReply')} />
            : attention && <i className="assistant-window-key__light" data-attention role="status" aria-label={t('workspace.assistantWindow.newAttention')} />}
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
  const title = t(isMini ? 'workspace.assistantWindow.miniTitle' : 'workspace.assistantWindow.title');
  // Mini Humcase is a window of its own: the system carries it, the app's frame does not.
  const onHeadDown = isMini
    ? (event: ReactPointerEvent) => { if (!(event.target as HTMLElement).closest('button') && event.button === 0) void getCurrentWindow().startDragging(); }
    : frame.onDragStart;

  return (
    <>
      {ghost && !isMini && <div className="assistant-window-ghost" aria-hidden="true" style={{ left: ghost.x, top: ghost.y, width: ghost.width, height: ghost.height }} />}
      <section className="assistant-window" data-variant={variant} aria-label={title} data-thinking={pending || undefined} data-snapping={frame.snapping || undefined}
        style={isMini ? undefined : { left: rect.x, top: rect.y, width: rect.width, height: rect.height }}>
        <header className="assistant-window__head" onPointerDown={onHeadDown} title={t('workspace.assistantWindow.move')}>
          <span className="assistant-window__grip" aria-hidden="true" />
          <HumcaseKey size={26} />
          <div className="assistant-window__title">
            <strong>{title}</strong>
            <small><span className="assistant-window__led" data-off={!pending && !modelLabel && status !== null ? true : undefined} aria-hidden="true" /><span>{statusText}</span></small>
          </div>
          {onPin && (
            <Hint label={t(isMini ? 'workspace.assistantWindow.unpin' : 'workspace.assistantWindow.pin')}>
              <Button variant="ghost" size="icon-sm" className="assistant-window__head-key" data-lit={isMini || undefined} onClick={onPin}
                aria-label={t(isMini ? 'workspace.assistantWindow.unpin' : 'workspace.assistantWindow.pin')} aria-pressed={isMini}><PushPin size={16} weight={isMini ? 'fill' : 'bold'} /></Button>
            </Hint>
          )}
          <Menu align="end" trigger={
            <Button variant="ghost" size="icon-sm" className="assistant-window__head-key" aria-label={t('workspace.assistantWindow.options')}><DotsThree size={18} weight="bold" /></Button>
          }>
            <MenuLabel>{t('workspace.assistantWindow.reads')}</MenuLabel>
            <MenuRadioGroup value="scope" onValueChange={() => {}} label={t('workspace.assistantWindow.reads')} options={[{ value: 'scope', label: scopeLabel }]} />
            {onStayOnTopChange && <>
              <MenuSeparator />
              <MenuLabel>{t('workspace.assistantWindow.windowLabel')}</MenuLabel>
              <MenuCheckboxItem checked={stayOnTop ?? false} onCheckedChange={onStayOnTopChange} detail={t('workspace.assistantWindow.stayOnTopDetail')}>{t('workspace.assistantWindow.stayOnTop')}</MenuCheckboxItem>
            </>}
            <MenuSeparator />
            {replied && !kept && <MenuItem onSelect={() => void keep()}>{t('workspace.assistantWindow.keep')}</MenuItem>}
            <MenuItem onSelect={restart}>{t('workspace.assistantWindow.newConversation')}</MenuItem>
          </Menu>
          {!isMini && <Hint label={t('workspace.assistantWindow.minimise')} shortcut="Control+J">
            <Button variant="ghost" size="icon-sm" className="assistant-window__head-key" onClick={() => onOpenChange(false)} aria-label={t('workspace.assistantWindow.minimise')}><Minus size={16} weight="bold" /></Button>
          </Hint>}
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
                  {([
                    ['toDecide', status.to_decide, t('workspace.assistantWindow.suggest.decide.text'), true],
                    ['conflicts', status.conflicts, t('workspace.assistantWindow.suggest.conflict.text'), true],
                    ['toTidy', status.to_tidy, t('workspace.assistantWindow.screen.askToTidy'), false],
                  ] as const).map(([key, count, question, lit]) => count > 0
                    ? (
                      <Hint key={key} label={t('workspace.assistantWindow.screen.ask')} side="left">
                        <button type="button" data-lit={lit || undefined} onClick={() => send(question)}>
                          <span>{t(`workspace.assistantWindow.screen.${key}`)}</span><b>{count}</b>
                        </button>
                      </Hint>
                    )
                    : <div key={key}><span>{t(`workspace.assistantWindow.screen.${key}`)}</span><b>{count}</b></div>)}
                </div>
              )}
              <h2>{t(context ? 'workspace.assistantWindow.caseHeading' : 'workspace.assistantWindow.heading')}</h2>
              <p>{t('workspace.assistantWindow.intro')}</p>
              <div className="assistant-window__suggestions">
                {context && !general
                  ? CASE_SUGGESTIONS.map(key => (
                    <button key={key} type="button" className="assistant-window__suggestion" data-case onClick={() => send(t(`workspace.assistantWindow.suggestCase.${key}`))}>
                      <small>{t('workspace.assistantWindow.suggestCase.tag')}</small>
                      {t(`workspace.assistantWindow.suggestCase.${key}`)}
                    </button>
                  ))
                  : SUGGESTIONS.map(key => (
                    <button key={key} type="button" className="assistant-window__suggestion" onClick={() => send(t(`workspace.assistantWindow.suggest.${key}.text`))}>
                      <small>{t(`workspace.assistantWindow.suggest.${key}.tag`)}</small>
                      {t(`workspace.assistantWindow.suggest.${key}.text`)}
                    </button>
                  ))}
              </div>
              {context && !general && <button type="button" className="assistant-window__more" onClick={() => setGeneral(true)}>{t('workspace.assistantWindow.otherQuestions')}</button>}
            </div>
          )}
          {entries.map(entry => (
            <Fragment key={entry.id}>
              {entry.said && <p className="assistant-window__said">{entry.said}</p>}
              {entry.error && <p className="assistant-window__error" role="alert">
                {t('workspace.assistantWindow.error', { error: aiErrorMessage(entry.error) })}
                {isMissingKeyError(entry.error) && <>{' '}<Button variant="link" size="sm" onClick={onConnectAi}>{t('workspace.assistantWindow.connectAi')}</Button></>}
              </p>}
              {entry.reply && <AssistantReplyView reply={entry.reply} done={entry.done} onDone={(index, next) => markDone(entry.id, index, next)}
                onOpenCase={onOpenCase} onConnectAi={onConnectAi} onChanged={changed} onGerminate={onGerminate} />}
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
            <span><Kbd>{t('workspace.assistantWindow.keyEnter')}</Kbd> {t('workspace.assistantWindow.keySend')}{!isMini && <> · <Kbd>{shortcutKeys('Control+J').join(' ')}</Kbd></>}</span>
          </div>
          {!isMini && <span className="assistant-window__resize" aria-hidden="true" title={t('workspace.assistantWindow.resize')} onPointerDown={frame.onResizeStart} />}
        </form>
      </section>
    </>
  );
}

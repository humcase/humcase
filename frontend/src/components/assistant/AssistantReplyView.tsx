import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Check } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { api } from '../../services/tauriApi';
import type { AssistantActionV2, AssistantReply } from '../../types/assistant';
import type { TidyOutcome } from '../../types';

const message = (error: unknown) => String(error instanceof Error ? error.message : error);

/** A reply of the assistant: its paragraph with citation numbers, its question, its sources and
 * the actions it proposes. */
export function AssistantReplyView({ reply, onOpenCase, onConnectAi, onChanged }: {
  reply: AssistantReply;
  onOpenCase: (id: string) => void;
  onConnectAi: () => void;
  /** Something was written (an action or its undo): the workspace reloads. */
  onChanged: () => void;
}) {
  const { t } = useTranslation();
  const caseOf = (cite: number | null) => {
    const source = cite ? reply.sources[cite - 1] : undefined;
    return source && !source.help && source.note_id ? source : null;
  };

  return (
    <div className="assistant-window__reply">
      {reply.kind === 'no_ai' && (
        <p className="assistant-window__note">
          {t('workspace.assistantWindow.noAi')}{' '}
          <Button variant="link" size="sm" onClick={onConnectAi}>{t('workspace.assistantWindow.connectAi')}</Button>
        </p>
      )}
      {reply.parts.length > 0 && (
        <p className="assistant-window__text">
          {reply.parts.map((part, index) => {
            const source = caseOf(part.cite);
            return (
              <span key={index}>{part.text}{part.cite !== null && reply.sources[part.cite - 1] && (source
                ? <button type="button" className="assistant-window__cite" onClick={() => onOpenCase(source.note_id!)}
                  aria-label={t('workspace.assistantWindow.openSource', { title: source.title })}>{part.cite}</button>
                : <span className="assistant-window__cite">{part.cite}</span>)}</span>
            );
          })}
        </p>
      )}
      {reply.question && <p className="assistant-window__question">{reply.question}</p>}
      {reply.sources.length > 0 && (
        <details className="assistant-window__sources">
          <summary>{t('workspace.assistantWindow.sources', { n: reply.sources.length })}</summary>
          <ol>
            {reply.sources.map((source, index) => (
              <li key={`${source.note_id ?? source.title}-${index}`}>
                {source.help || !source.note_id
                  ? <span className="assistant-window__source-help">{t('workspace.assistantWindow.helpSource', { title: source.title })}</span>
                  : <button type="button" className="assistant-window__source" onClick={() => onOpenCase(source.note_id!)}>{source.title}</button>}
                {source.passages.map(passage => <q key={passage} className="assistant-window__passage">{passage}</q>)}
              </li>
            ))}
          </ol>
        </details>
      )}
      {reply.actions.map((action, index) => <ActionCard key={index} action={action} onOpenCase={onOpenCase} onChanged={onChanged} />)}
    </div>
  );
}

type Done = { text: string; undo: () => Promise<void>; openId?: string };

/** A proposed action: nothing happens before the click; once done, a line says what changed and
 * offers to undo it. */
function ActionCard({ action, onOpenCase, onChanged }: { action: AssistantActionV2; onOpenCase: (id: string) => void; onChanged: () => void }) {
  const { t } = useTranslation();
  const [done, setDone] = useState<Done | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (task: () => Promise<Done>) => {
    setBusy(true); setError(null);
    try { setDone(await task()); onChanged(); } catch (err) { setError(message(err)); } finally { setBusy(false); }
  };
  const undo = async () => {
    if (!done) return;
    setBusy(true); setError(null);
    try { await done.undo(); setDone(null); onChanged(); } catch (err) { setError(message(err)); } finally { setBusy(false); }
  };
  const failed = error && <p className="assistant-window__error" role="alert">{t('workspace.assistantWindow.actionError', { error })}</p>;

  if (done) {
    return (
      <>
        <div className="assistant-window__done" role="status">
          <Check size={15} weight="bold" aria-hidden="true" />
          <span>{done.text}</span>
          {done.openId && <button type="button" onClick={() => onOpenCase(done.openId!)}>{t('workspace.assistantWindow.open')}</button>}
          <button type="button" disabled={busy} onClick={() => void undo()}>{t('workspace.assistantWindow.undo')}</button>
        </div>
        {failed}
      </>
    );
  }

  const title = action.case_title;
  let kind: 'proposed' | 'conflict' | 'supersede' | 'archive' | 'group' | 'move' = 'proposed';
  let body: ReactNode = null;
  let buttons: ReactNode = null;

  if (action.kind === 'create_sub_case') {
    body = t('workspace.assistantWindow.action.subCase', { title: action.title, parent: title });
    buttons = <Button size="sm" disabled={busy} onClick={() => void run(async () => {
      const id = await api.createSubCase(action.case_id, action.title, 'todo');
      return { text: t('workspace.assistantWindow.action.subCaseDone', { title: action.title, parent: title }), openId: id, undo: () => api.deleteNote(id) };
    })}>{t('workspace.assistantWindow.action.createSubCase')}</Button>;
  } else if (action.kind === 'archive') {
    kind = 'archive';
    body = t('workspace.assistantWindow.action.archiveText', { title });
    buttons = <Button size="sm" disabled={busy} onClick={() => void run(async () => {
      await api.setNoteArchived(action.case_id, true);
      return { text: t('workspace.assistantWindow.action.archiveDone', { title }), undo: () => api.setNoteArchived(action.case_id, false) };
    })}>{t('workspace.assistantWindow.action.archive')}</Button>;
  } else if (action.kind === 'move') {
    kind = 'move';
    body = t('workspace.assistantWindow.action.moveText', { title, project: action.project_name });
    buttons = <Button size="sm" disabled={busy} onClick={() => void run(async () => {
      await api.moveNoteToProject(action.case_id, action.project_id);
      return { text: t('workspace.assistantWindow.action.moveDone', { title, project: action.project_name }), openId: action.case_id, undo: () => api.moveNoteToProject(action.case_id, action.from_project_id) };
    })}>{t('workspace.assistantWindow.action.move', { project: action.project_name })}</Button>;
  } else {
    // An item of "To tidy": answered like on its page, taken back with its Undo.
    const other = action.other_title ?? '';
    const answer = (accept: boolean, text: string, openId?: string) => run(async () => {
      const outcome: TidyOutcome = await api.answerTidy([{ id: action.tidy_id, accept }]);
      return {
        text: outcome.skipped > 0 ? t('workspace.tidy.doneSkipped', { count: outcome.skipped }) : text,
        openId,
        undo: () => api.undoTidy([action.tidy_id]),
      };
    });
    const refused = t('workspace.assistantWindow.action.refusedDone');
    kind = action.tidy_kind;
    if (action.tidy_kind === 'supersede' || action.tidy_kind === 'conflict') {
      const supersede = action.tidy_kind === 'supersede';
      body = (
        <div className="assistant-window__versus">
          <div><b>{title}</b>{action.passage && <q>{action.passage}</q>}</div>
          <div data-newer><b>{other}</b>{action.other_passage && <q>{action.other_passage}</q>}</div>
        </div>
      );
      buttons = <>
        <Button size="sm" disabled={busy} onClick={() => void (supersede
          ? answer(true, t('workspace.assistantWindow.action.replacedDone', { title, other }))
          : answer(true, t('workspace.assistantWindow.action.fixDone', { title }), action.case_id))}>
          {supersede ? t('workspace.assistantWindow.action.markReplaced') : t('workspace.assistantWindow.action.iFix')}
        </Button>
        <Button size="sm" variant="secondary" disabled={busy} onClick={() => void answer(false, refused)}>
          {supersede ? t('workspace.assistantWindow.action.bothHold') : t('workspace.assistantWindow.action.noConflict')}
        </Button>
        {action.other_id && <Button size="sm" variant="ghost" onClick={() => { onOpenCase(action.other_id!); onOpenCase(action.case_id); }}>{t('workspace.assistantWindow.action.openBoth')}</Button>}
      </>;
    } else if (action.tidy_kind === 'archive') {
      body = t('workspace.assistantWindow.action.archiveText', { title });
      buttons = <>
        <Button size="sm" disabled={busy} onClick={() => void answer(true, t('workspace.assistantWindow.action.archiveDone', { title }))}>{t('workspace.assistantWindow.action.archive')}</Button>
        <Button size="sm" variant="secondary" disabled={busy} onClick={() => void answer(false, refused)}>{t('workspace.assistantWindow.action.keep')}</Button>
      </>;
    } else {
      body = t('workspace.assistantWindow.action.groupText', { title, other });
      buttons = <>
        <Button size="sm" disabled={busy} onClick={() => void answer(true, t('workspace.assistantWindow.action.groupDone', { title, other }))}>{t('workspace.assistantWindow.action.fileTogether')}</Button>
        <Button size="sm" variant="secondary" disabled={busy} onClick={() => void answer(false, refused)}>{t('workspace.assistantWindow.action.keepApart')}</Button>
      </>;
    }
  }

  return (
    <div className="assistant-window__card" data-kind={kind}>
      <span className="assistant-window__card-kind">{t(`workspace.assistantWindow.action.kind.${kind}`)}</span>
      <div className="assistant-window__card-body">{body}</div>
      <div className="assistant-window__card-actions">{buttons}</div>
      {failed}
    </div>
  );
}

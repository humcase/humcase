import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import type { PendingWrite } from '../../utils/pendingWrites';

interface PendingWritesPanelProps {
  writes: PendingWrite[];
  /** True if the escrow is persistent (local storage available), false = memory only. */
  persistentWrites: boolean;
  recoveryInProgress: boolean;
  recoveryProblems: Record<string, string>;
  onRetry: () => void;
  onIgnore: () => void;
}

/** Recovery panel for failed writes (kept drafts and raw dictations). */
export function PendingWritesPanel({ writes, persistentWrites, recoveryInProgress, recoveryProblems, onRetry, onIgnore }: PendingWritesPanelProps) {
  const { t } = useTranslation();
  return (
        <div className="note-editor-pending mx-6 mt-3 p-3 text-xs">
          <div className="flex items-center justify-between gap-3 mb-2">
            <strong>
              {t(persistentWrites ? 'editor.pending.heading' : 'editor.pending.headingMemory', { count: writes.length })}
            </strong>
            <div className="flex gap-2 shrink-0">
              <Button
                variant="secondary"
                size="sm"
                onClick={onRetry}
                disabled={recoveryInProgress}
              >
                {recoveryInProgress ? t('editor.pending.checking') : t('common.retry')}
              </Button>
              <Button
                onClick={onIgnore}
                disabled={recoveryInProgress}
                variant="ghost"
                size="sm"
              >
                {t('editor.pending.ignore')}
              </Button>
            </div>
          </div>
          <ul className="list-disc list-inside space-y-0.5">
            {writes.map(e => (
              <li key={e.id}>
                {t('editor.pending.entry', { kind: e.kind === 'brut' ? t('editor.pending.kindRaw') : e.kind === 'task-draft' ? t('editor.pending.kindTaskDraft') : t('editor.pending.kindDraft'), id: e.noteId.slice(0, 8) })}
                {recoveryProblems[e.id] && <p role="alert" className="mt-1">{recoveryProblems[e.id]}</p>}
                <details className="mt-1">
                  <summary className="cursor-pointer">{t('editor.pending.show')}</summary>
                  <pre className="mt-1 p-2 max-h-48 overflow-auto whitespace-pre-wrap break-words select-text note-editor-pending__text">
                    {e.kind === 'brut'
                      ? e.texte
                      : e.kind === 'task-draft'
                        ? [e.titre, e.contenu].filter(Boolean).join('\n\n')
                        : Object.entries(e.payload).map(([field, value]) => t('editor.pending.field', { field: field, value: value ?? '' })).join('\n\n')}
                  </pre>
                </details>
              </li>
            ))}
          </ul>
        </div>
  );
}

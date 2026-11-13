import { useTranslation } from 'react-i18next';
import { Archive, Copy } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { DeleteConfirmation } from './DeleteConfirmation';

interface NoteActionsProps {
  isMiniMode?: boolean;
  archived: boolean;
  onCopy: () => void;
  onToggleArchive: () => void;
  isConfirmingDelete: boolean;
  onAskDelete: () => void;
  onConfirmDelete: () => void;
  onCancelDelete: () => void;
  subCases?: number;
}

/** Note actions: copy as Markdown, archive, delete. */
export function NoteActions({ isMiniMode, archived, onCopy, onToggleArchive, isConfirmingDelete, onAskDelete, onConfirmDelete, onCancelDelete, subCases }: NoteActionsProps) {
  const { t } = useTranslation();
  return (
        <div className={`note-editor-actions flex gap-2 items-center shrink-0 ${isMiniMode ? 'mr-12' : ''}`}>
            <Button
              variant="ghost"
              size="sm"
              onClick={onCopy}
              className="note-editor-action"
              title={t('editor.actions.copyTitle')}
            >
              <Copy aria-hidden="true" /><span>{t('common.copy')}</span>
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={onToggleArchive}
              className="note-editor-action"
              title={archived ? t('editor.actions.unarchiveTitle') : t('editor.actions.archiveTitle')}
            >
              <Archive aria-hidden="true" /><span>{archived ? t('editor.actions.unarchive') : t('editor.actions.archive')}</span>
            </Button>
          <DeleteConfirmation isConfirming={isConfirmingDelete} onAskDelete={onAskDelete} onConfirm={onConfirmDelete} onCancel={onCancelDelete} subCases={subCases} />

        </div>
  );
}

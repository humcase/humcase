import { useTranslation } from 'react-i18next';
import { Trash as Trash2 } from '@phosphor-icons/react';
import { Button } from '../ui/Button';

interface DeleteConfirmationProps {
  isConfirming: boolean;
  /** Requests confirmation (the parent remembers the identifier of the targeted note). */
  onAskDelete: () => void;
  onConfirm: () => void;
  onCancel: () => void;
  /** Live sub-cases that go to the trash with the case. */
  subCases?: number;
}

/** "Delete" button and its inline confirmation (move to the trash). */
export function DeleteConfirmation({ isConfirming, onAskDelete, onConfirm, onCancel, subCases = 0 }: DeleteConfirmationProps) {
  const { t } = useTranslation();
  return isConfirming ? (
            <div className="note-editor-confirm-delete" role="group" aria-label={t('editor.delete.confirmGroup')}>
              <span title={t('editor.delete.trashHint')}>{subCases > 0 ? t('editor.delete.trashWithSubCases', { count: subCases }) : t('editor.delete.trashQuestion')}</span>
              <Button variant="destructive" size="sm" onClick={onConfirm}>{t('editor.delete.confirm')}</Button>
              <Button variant="ghost" size="sm" onClick={onCancel}>{t('editor.delete.no')}</Button>
            </div>
          ) : (
            <Button
              variant="ghost"
              size="sm"
              data-tone="danger"
              onClick={onAskDelete}
              className="note-editor-action"
              aria-label={t('editor.delete.aria')}
              title={t('editor.delete.button')}
            >
              <Trash2 aria-hidden="true" /><span>{t('editor.delete.button')}</span>
            </Button>
          );
}

import { useTranslation } from 'react-i18next';
import type { AppUpdate } from '../../hooks/useAppUpdate';
import { Button } from '../ui/Button';
import { Toast } from '../ui/Toast';
import './update-notice.css';

/** The message of a newer Humcase, at the bottom of the window until the person answers. */
export function UpdateNotice({ update, onInstall, onDismiss }: {
  update: AppUpdate;
  onInstall: () => void;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  if (update.phase === 'idle') return null;
  if (update.phase === 'failed') {
    return (
      <Toast tone="danger" className="update-notice" action={<span className="update-notice__actions">
        <Button variant="link" size="sm" onClick={onInstall}>{t('common.retry')}</Button>
        <Button variant="link" size="sm" onClick={onDismiss}>{t('common.close')}</Button>
      </span>}>
        {t('workspace.updates.failed', { error: update.error })}
      </Toast>
    );
  }
  if (update.phase === 'downloading') {
    return (
      <Toast className="update-notice">
        {update.percent === null ? t('workspace.updates.downloadingNoSize') : t('workspace.updates.downloading', { percent: update.percent })}
      </Toast>
    );
  }
  if (update.phase === 'installing') {
    return <Toast className="update-notice">{t('workspace.updates.installing')}</Toast>;
  }
  return (
    <Toast className="update-notice" action={<span className="update-notice__actions">
      <Button variant="link" size="sm" onClick={onInstall}>{t('workspace.updates.install')}</Button>
      <Button variant="link" size="sm" onClick={onDismiss}>{t('workspace.updates.later')}</Button>
    </span>}>
      {t('workspace.updates.available', { version: update.version })}
    </Toast>
  );
}

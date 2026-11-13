import { Trans, useTranslation } from 'react-i18next';
import { FolderOpen, Database as DatabaseBackup } from '@phosphor-icons/react';
import { currentDateLocale } from '../../i18n';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Notice } from '../ui/Toast';
import { SETTINGS_FIELD } from './fields';
import type { ArchiveRestore } from './useArchiveRestore';

/** Data: where it lives, a backup archive, and the restore of an archive after its preview. */
export function DataSection({ data }: { data: ArchiveRestore }) {
  const { t } = useTranslation();
  const { archivePreview, isBusy, isRestoring } = data;
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.data.heading')}</h3><p>{t('settings.data.description')}</p></div>
      <Card padding="none" className="settings-card settings-card--data">
        <label className="settings-label block">{t('settings.data.localTitle')}</label>
        <p className="text-xs text-muted-foreground">
          <Trans
            i18nKey="settings.data.location"
            values={{ database: '%USERPROFILE%\\.humcase\\humcase.db', images: 'images\\' }}
            components={{ mono: <span className="font-mono" /> }}
          />
        </p>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={data.openDataDir} disabled={isBusy} className="flex-1">
            <FolderOpen className="w-4 h-4" /> {t('settings.data.openFolder')}
          </Button>
          <Button variant="secondary" onClick={data.backup} disabled={isBusy} className="flex-1">
            <DatabaseBackup className="w-4 h-4" /> {t('settings.data.backup')}
          </Button>
        </div>
        <div className="flex flex-col gap-2 border-t border-border pt-3">
          <label htmlFor="backup-archive-path" className="settings-label settings-label--small">
            {t('settings.data.restoreTitle')}
          </label>
          <input
            id="backup-archive-path"
            type="text"
            value={data.archivePath}
            onChange={e => data.changeArchivePath(e.target.value)}
            disabled={isRestoring}
            placeholder={t('settings.data.archivePlaceholder')}
            className={SETTINGS_FIELD}
          />
          <Button variant="secondary"
            onClick={data.inspectArchive}
            disabled={isBusy || data.isInspectingArchive || isRestoring || !data.archivePath.trim()}
            className="self-start"
          >
            {data.isInspectingArchive ? t('settings.data.inspecting') : t('settings.data.inspect')}
          </Button>
          {data.isInspectingArchive && <p role="status" className="text-[11px] text-muted-foreground">{t('settings.data.verifying')}</p>}
          {archivePreview && (
            <div className="settings-preview">
              <p className="font-semibold text-primary">{t('settings.data.archiveValid')}</p>
              <p>{t('settings.data.archiveCreated', { date: new Date(archivePreview.created_at).toLocaleString(currentDateLocale()) })}</p>
              <p>
                {t('settings.data.archiveProjects', { count: archivePreview.project_count })} · {t('settings.data.archiveNotes', { count: archivePreview.note_count })} ·{' '}
                {t('settings.data.archiveImages', { count: archivePreview.image_count })}
              </p>
              <p>{t('settings.data.archiveSchema', { version: archivePreview.database_schema_version })}</p>
            </div>
          )}
          {archivePreview && !data.restoreConfirmationOpen && (
            <Button variant="secondary" onClick={data.askRestore} disabled={isBusy || isRestoring} className="self-start">
              {t('settings.data.restore')}
            </Button>
          )}
          {data.restoreConfirmationOpen && archivePreview && (
            <div className="settings-alert settings-alert--block">
              <p className="text-xs">
                {t('settings.data.restoreWarning')}
              </p>
              <label className="flex items-start gap-2 text-xs">
                <input
                  type="checkbox"
                  checked={data.restoreAcknowledged}
                  onChange={e => data.setRestoreAcknowledged(e.target.checked)}
                  disabled={isRestoring}
                  className="mt-0.5"
                />
                <span>{t('settings.data.restoreAck')}</span>
              </label>
              <div className="flex flex-wrap gap-2">
                <Button
                  onClick={data.confirmRestore}
                  disabled={!data.restoreAcknowledged || isBusy || isRestoring}
                  variant="destructive" data-tone="soft"
                >
                  {isRestoring ? t('settings.data.restoring') : t('settings.data.confirmReplace')}
                </Button>
                <Button variant="secondary" onClick={data.cancelRestore} disabled={isRestoring}>
                  {t('common.cancel')}
                </Button>
              </div>
            </div>
          )}
          {data.restoreStatus && (
            <Notice className="settings-status">{data.restoreStatus}</Notice>
          )}
        </div>
        {data.dataStatus && (
          <Notice className="settings-status">{data.dataStatus}</Notice>
        )}
        <p className="text-[11px] text-muted-foreground">
          {t('settings.data.archiveContents')}
        </p>
      </Card>
    </div>
  );
}

import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type BackupArchivePreview } from '../../services/tauriApi';
import type { BackupPreparation } from '../../utils/backupBarrier';
import { prepareQuickCaptureForRestore } from '../../utils/quickCaptureBarrier';
import { todayLocalIso } from '../../utils/datetime';

/**
 * The data folder, backups and restore (safeguard 5). A restore only starts from the archive
 * just previewed, once the person has acknowledged the replacement, and after the editor and
 * quick capture have saved what they hold; `restoringRef` keeps Settings open meanwhile.
 */
export function useArchiveRestore(prepareBackup: () => Promise<BackupPreparation>) {
  const { t } = useTranslation();
  const [dataStatus, setDataStatus] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [archivePath, setArchivePath] = useState('');
  const [archivePreview, setArchivePreview] = useState<BackupArchivePreview | null>(null);
  const [isInspectingArchive, setIsInspectingArchive] = useState(false);
  const [restoreConfirmationOpen, setRestoreConfirmationOpen] = useState(false);
  const [restoreAcknowledged, setRestoreAcknowledged] = useState(false);
  const [restoreStatus, setRestoreStatus] = useState<string | null>(null);
  const [isRestoring, setIsRestoring] = useState(false);
  const archivePathRef = useRef('');
  const inspectionRequestRef = useRef(0);
  const restoringRef = useRef(false);

  const openDataDir = async () => {
    setIsBusy(true);
    try {
      const path = await api.openDataDir();
      setDataStatus(t('settings.data.status.opened', { path }));
    } catch (e) {
      setDataStatus(t('settings.data.status.openFailed', { error: String(e) }));
    } finally {
      setIsBusy(false);
    }
  };

  const backup = async () => {
    setIsBusy(true);
    try {
      const preparation = await prepareBackup();
      if (!preparation.ready) {
        setDataStatus(t('settings.data.status.backupBlocked', { reason: preparation.reason }));
        return;
      }
      const now = new Date();
      const stamp = `${todayLocalIso(now)}-${String(now.getHours()).padStart(2, '0')}${String(now.getMinutes()).padStart(2, '0')}`;
      const path = await api.createBackupArchive(`humcase-${stamp}.humcase.zip`);
      setDataStatus(t('settings.data.status.backupCreated', { path }));
      archivePathRef.current = path;
      inspectionRequestRef.current += 1;
      setArchivePath(path);
      setArchivePreview(null);
      setRestoreConfirmationOpen(false);
      setRestoreAcknowledged(false);
      setRestoreStatus(null);
    } catch (e) {
      setDataStatus(t('settings.data.status.backupFailed', { error: String(e) }));
    } finally {
      setIsBusy(false);
    }
  };

  const changeArchivePath = (path: string) => {
    archivePathRef.current = path;
    inspectionRequestRef.current += 1;
    setArchivePath(path);
    setArchivePreview(null);
    setIsInspectingArchive(false);
    setRestoreConfirmationOpen(false);
    setRestoreAcknowledged(false);
    setRestoreStatus(null);
  };

  const inspectArchive = async () => {
    const path = archivePath.trim();
    if (!path) {
      setRestoreStatus(t('settings.data.status.pathMissing'));
      return;
    }

    const requestId = ++inspectionRequestRef.current;
    setIsInspectingArchive(true);
    setArchivePreview(null);
    setRestoreConfirmationOpen(false);
    setRestoreAcknowledged(false);
    setRestoreStatus(null);
    try {
      const preview = await api.inspectBackupArchive(path);
      if (inspectionRequestRef.current === requestId && archivePathRef.current.trim() === path) {
        setArchivePreview(preview);
      }
    } catch (e) {
      if (inspectionRequestRef.current === requestId && archivePathRef.current.trim() === path) {
        setRestoreStatus(t('settings.data.status.inspectFailed', { error: String(e) }));
      }
    } finally {
      if (inspectionRequestRef.current === requestId) setIsInspectingArchive(false);
    }
  };

  const askRestore = () => {
    setRestoreStatus(null);
    setRestoreAcknowledged(false);
    setRestoreConfirmationOpen(true);
  };

  const cancelRestore = () => {
    setRestoreConfirmationOpen(false);
    setRestoreAcknowledged(false);
  };

  const confirmRestore = async () => {
    if (!archivePreview || !restoreAcknowledged || isRestoring || isBusy) return;
    const path = archivePath.trim();
    const expectedSha256 = archivePreview.archive_sha256;
    if (!path || !expectedSha256) return;

    restoringRef.current = true;
    setIsRestoring(true);
    setIsBusy(true);
    setRestoreStatus(null);
    try {
      const preparation = await prepareBackup();
      if (!preparation.ready) {
        setRestoreStatus(t('settings.data.status.restoreBlocked', { reason: preparation.reason }));
        return;
      }

      const quickCapture = prepareQuickCaptureForRestore();
      if (!quickCapture.ready) {
        setRestoreStatus(t('settings.data.status.restoreBlocked', { reason: quickCapture.reason }));
        return;
      }

      try {
        await api.restoreBackupArchive(path, expectedSha256);
        setRestoreConfirmationOpen(false);
        setRestoreAcknowledged(false);
        setRestoreStatus(t('settings.data.status.restoreDone'));
      } catch (e) {
        // The Rust command always returns a complete, localized message ready to display
        // (simple failure framed by "Restore failed", or detailed blocked state):
        // we do not interpret it, we only remove the optional "Error: " prefix.
        const raw = String(e);
        setRestoreStatus(raw.startsWith('Error: ') ? raw.slice('Error: '.length) : raw);
      }
    } catch (e) {
      setRestoreStatus(t('settings.data.status.editorPrepFailed', { error: String(e) }));
    } finally {
      restoringRef.current = false;
      setIsRestoring(false);
      setIsBusy(false);
    }
  };

  return {
    dataStatus, isBusy, openDataDir, backup,
    archivePath, changeArchivePath, archivePreview, isInspectingArchive, inspectArchive,
    restoreConfirmationOpen, askRestore, cancelRestore, restoreAcknowledged, setRestoreAcknowledged,
    confirmRestore, isRestoring, restoreStatus, restoringRef,
  };
}

export type ArchiveRestore = ReturnType<typeof useArchiveRestore>;

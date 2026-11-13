import { Trans, useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Dialog, DialogBody, DialogFooter, DialogHeader } from '../ui/Dialog';

interface BriefDialogProps {
  kind: 'handoff' | 'standup';
  text: string;
  copyError: string | null;
  onClose: () => void;
  onCopy: () => void;
}

/** Text modal for the Handoff and Daily Standup briefs (read-only, copy to the clipboard). */
export default function BriefDialog({ kind, text, copyError, onClose, onCopy }: BriefDialogProps) {
  const { t } = useTranslation();
  const showHandoff = kind === 'handoff';
  return (
    <Dialog onClose={onClose} size="lg" className="workspace-text-modal" describedBy={showHandoff ? 'texte-modal-desc' : undefined}>
      <DialogHeader title={showHandoff ? t('workspace.briefDialog.handoffTitle') : t('workspace.briefDialog.standupTitle')} onClose={onClose}
        description={showHandoff ? <p id="texte-modal-desc"><Trans i18nKey="workspace.briefDialog.handoffDesc" components={{ code: <span className="font-mono" /> }} /></p> : undefined} />
      <DialogBody>
        <textarea value={text} readOnly aria-label={showHandoff ? t('workspace.briefDialog.handoffText') : t('workspace.briefDialog.standupText')} />
      </DialogBody>
      <DialogFooter note={copyError ? <span className="ui-dialog__error">{copyError}</span> : undefined}>
          <Button onClick={onCopy}>
            {t('workspace.briefDialog.copyClose')}
          </Button>
      </DialogFooter>
    </Dialog>
  );
}

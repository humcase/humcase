import { useTranslation } from 'react-i18next';
import { CheckCircle as CheckCircle2, CircleNotch as Loader2 } from '@phosphor-icons/react';
import { STRUCTURE_FRAMES, type StructureFrame } from '../../utils/prefs';
import TiptapEditor from '../TiptapEditor';
import { Button } from '../ui/Button';
import { Dialog, DialogFooter, DialogHeader } from '../ui/Dialog';

export interface StructureProposal {
  noteId: string;
  source: string;
  content: string;
}

interface StructureDialogProps {
  onClose: () => void;
  structureBusy: boolean;
  structureProposal: StructureProposal | null;
  structureError: string | null;
  structuringContextLabel?: string;
  structureFrame: StructureFrame;
  onFrameChange: (frame: StructureFrame) => void;
  /** Current content of the note: used to detect that it changed since the request. */
  content: string;
  onProposalChange: (value: string) => void;
  onApply: () => void;
}

/** Structure proposal window: source, editable proposal, application. */
export function StructureDialog({
  onClose,
  structureBusy,
  structureProposal,
  structureError,
  structuringContextLabel,
  structureFrame,
  onFrameChange,
  content,
  onProposalChange,
  onApply,
}: StructureDialogProps) {
  const { t } = useTranslation();
  return (
        <Dialog onClose={onClose} size="xl" className="note-structure-dialog" describedBy="structure-dialog-desc">
            <DialogHeader title={t('editor.structure.title')} onClose={onClose} closeLabel={t('editor.structure.closeLabel')} closeDisabled={structureBusy && !!structureProposal}
              description={<><p id="structure-dialog-desc">{t('editor.structure.privacy')}</p>
                {structuringContextLabel && <p className="note-structure-context">{t('editor.structure.context', { label: structuringContextLabel })}</p>}</>} />
            <div className="note-structure-frame">
              <label htmlFor="structure-frame">{t('editor.structure.frame')}</label>
              <select id="structure-frame" className="ui-field ui-field--sm" value={structureFrame} disabled={structureBusy}
                onChange={event => onFrameChange(event.target.value as StructureFrame)}>
                {STRUCTURE_FRAMES.map(frame => <option key={frame.id} value={frame.id}>{t(`editor.structure.frames.${frame.id}`)}</option>)}
              </select>
            </div>
            {structureError && <p role="alert" className="mx-4 sm:mx-5 mt-4 p-3 rounded border text-sm" style={{ color: 'var(--workspace-danger)', backgroundColor: 'var(--workspace-danger-soft)', borderColor: 'var(--workspace-danger-border)' }}>{structureError}</p>}
            {structureProposal && content !== structureProposal.source && <p role="alert" className="mx-4 sm:mx-5 mt-4 p-3 rounded border text-sm" style={{ color: 'var(--workspace-warning-ink)', backgroundColor: 'var(--workspace-paper)', borderColor: 'var(--workspace-paper-border)' }}>{t('editor.structure.changedSinceRequest')}</p>}
            {structureBusy && !structureProposal && <p className="p-6 text-sm text-muted-foreground flex items-center gap-2"><Loader2 className="w-4 h-4 animate-spin" /> {t('editor.structure.preparing')}</p>}
            {structureProposal && (
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 p-4 sm:p-5 min-h-0 overflow-y-auto">
                <section className="min-w-0 flex flex-col">
                  <h3 className="text-sm font-medium text-foreground mb-2">{t('editor.structure.source')}</h3>
                  <pre className="note-structure-source bg-card border border-border rounded-lg p-3 whitespace-pre-wrap break-words text-sm text-muted-foreground overflow-auto min-h-32 max-h-[50vh] flex-1 select-text">{structureProposal.source}</pre>
                </section>
                <section className="min-w-0 flex flex-col">
                  <h3 className="text-sm font-medium text-foreground mb-2">{t('editor.structure.proposal')}</h3>
                  <div className="note-structure-proposal bg-card border border-border rounded-lg p-3 min-h-64 max-h-[50vh] overflow-hidden flex-1">
                    <TiptapEditor
                      key={`structure-${structureProposal.noteId}`}
                      value={structureProposal.content}
                      noteId={`structure-${structureProposal.noteId}`}
                      onChange={onProposalChange}
                    />
                  </div>
                </section>
              </div>
            )}
            <DialogFooter className="note-structure-footer">
              <Button variant="secondary" onClick={onClose} disabled={structureBusy && !!structureProposal}>{t('common.cancel')}</Button>
              {structureProposal && (
                <Button onClick={onApply} disabled={structureBusy || !structureProposal.content.trim() || content !== structureProposal.source}>
                  {structureBusy ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <CheckCircle2 className="w-4 h-4 mr-2" />}
                  {t('editor.structure.apply')}
                </Button>
              )}
            </DialogFooter>
        </Dialog>
  );
}

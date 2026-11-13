import { useCallback, useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import type { Note } from '../../types';
import { api } from '../../services/tauriApi';
import { getAiCleanProvider, getAiModel, getStructureFrame, setStructureFrame, type StructureFrame } from '../../utils/prefs';
import { trackEditorWrite } from '../../utils/backupBarrier';
import { aiErrorMessage } from '../../utils/aiErrors';
import { readPendingWrites } from '../../utils/pendingWrites';
import type { EditableState } from './editableState';
import type { StructureProposal } from './StructureDialog';
import type { ShowToast } from './useEditorToast';

/**
 * Structure with AI: a proposal made from the text actually saved, shown for review, then applied
 * through the backend, which compares the content in the transaction that archives the original.
 * Any input or case change in between refuses the proposal rather than overwrite anything.
 */
export function useStructure({
  note, voiceBusy, currentNoteIdRef, localStateRef, baselineRef, saveChainRef, flushDraft, restoreVersion, setContent, showToast, onNoteUpdated,
}: {
  note: Note | null;
  /** A dictation is starting, running or being processed. */
  voiceBusy: () => boolean;
  currentNoteIdRef: MutableRefObject<string | null>;
  localStateRef: MutableRefObject<EditableState>;
  baselineRef: MutableRefObject<EditableState>;
  saveChainRef: MutableRefObject<Promise<void>>;
  flushDraft: () => Promise<void>;
  restoreVersion: (noteId: string, expected: string | null, version: string) => Promise<void>;
  setContent: Dispatch<SetStateAction<string>>;
  showToast: ShowToast;
  onNoteUpdated: () => void;
}) {
  const { t } = useTranslation();
  const [structureOpen, setStructureOpen] = useState(false);
  const [structureBusy, setStructureBusy] = useState(false);
  const [structureFrame, setStructureFrameState] = useState<StructureFrame>(() => getStructureFrame());
  const [structureError, setStructureError] = useState<string | null>(null);
  const [structureProposal, setStructureProposal] = useState<StructureProposal | null>(null);
  const structureRequestRef = useRef(0);
  const structureApplyingRef = useRef(false);

  // Another case is displayed: a request in flight is forgotten and the dialog closes.
  const shownNoteId = note?.id ?? null;
  /* eslint-disable react/set-state-in-effect -- resets the dialog when the case changes. */
  useEffect(() => {
    structureRequestRef.current += 1;
    setStructureOpen(false);
    setStructureBusy(false);
    setStructureError(null);
    setStructureProposal(null);
  }, [shownNoteId]);
  /* eslint-enable react/set-state-in-effect */

  const closeStructure = useCallback(() => {
    if (structureApplyingRef.current) return;
    structureRequestRef.current += 1;
    setStructureOpen(false);
    setStructureBusy(false);
    setStructureError(null);
    setStructureProposal(null);
  }, []);

  const requestStructure = async (frame: StructureFrame = structureFrame, relaunch = false) => {
    if (!note || (structureOpen && !relaunch) || voiceBusy()) return;
    const noteId = note.id;
    const requestId = ++structureRequestRef.current;
    setStructureOpen(true);
    setStructureBusy(true);
    setStructureError(null);
    setStructureProposal(null);
    try {
      // Wait for all previous writes, then compare the text actually in the database.
      // A flush failure is escrowed; in that case, no local data is sent to the AI.
      await flushDraft();
      if (requestId !== structureRequestRef.current || currentNoteIdRef.current !== noteId) return;
      if (readPendingWrites().some(e => e.noteId === noteId && e.kind !== 'task-draft')) {
        throw new Error(t('editor.structure.errorSaveFirst'));
      }
      const saved = await api.getNote(noteId);
      if (requestId !== structureRequestRef.current || currentNoteIdRef.current !== noteId) return;
      const source = localStateRef.current.content;
      if (saved.content !== source) {
        throw new Error(t('editor.structure.errorChangedWhileSaving'));
      }
      if (!source.trim()) throw new Error(t('editor.structure.errorWriteFirst'));
      const provider = getAiCleanProvider();
      const proposed = await api.structureNote(noteId, source, provider, getAiModel(provider) || undefined, frame);
      if (requestId !== structureRequestRef.current || currentNoteIdRef.current !== noteId) return;
      if (!proposed.trim()) throw new Error(t('editor.structure.errorEmptyProposal'));
      setStructureProposal({ noteId, source, content: proposed });
    } catch (e) {
      if (requestId === structureRequestRef.current && currentNoteIdRef.current === noteId) {
        setStructureError(aiErrorMessage(e));
      }
    } finally {
      if (requestId === structureRequestRef.current) setStructureBusy(false);
    }
  };

  const applyStructure = async () => {
    const proposal = structureProposal;
    if (!proposal || structureApplyingRef.current || !proposal.content.trim()) return;
    structureApplyingRef.current = true;
    setStructureBusy(true);
    setStructureError(null);
    const execute = async () => {
      if (currentNoteIdRef.current !== proposal.noteId ||
          localStateRef.current.id !== proposal.noteId ||
          localStateRef.current.content !== proposal.source) {
        throw new Error(t('editor.structure.errorChangedBeforeApply'));
      }
      // The backend compares the content in the transaction that archives the original and writes the
      // result. A concurrent MCP modification therefore triggers a refusal without a partial write.
      const saved = await api.applyStructuredNote(proposal.noteId, proposal.source, proposal.content);
      if (currentNoteIdRef.current === proposal.noteId) {
        baselineRef.current.content = saved.content;
        // Network processing may take long: a local input arrived after sending remains a
        // draft to save against the new baseline, never replaced by the response.
        if (localStateRef.current.content === proposal.source) {
          localStateRef.current.content = saved.content;
          setContent(saved.content);
        }
        setStructureProposal(null);
        setStructureOpen(false);
        const structure = saved.content;
        showToast(t('editor.structure.done'), {
          duration: 8000,
          action: { label: t('common.cancel'), run: () => { void restoreVersion(proposal.noteId, structure, proposal.source); } },
        });
      }
      onNoteUpdated();
    };
    saveChainRef.current = trackEditorWrite(saveChainRef.current.then(execute, execute));
    try {
      await saveChainRef.current;
    } catch (e) {
      if (currentNoteIdRef.current === proposal.noteId) setStructureError(aiErrorMessage(e));
    } finally {
      structureApplyingRef.current = false;
      setStructureBusy(false);
    }
  };

  /** A frame chosen in the dialog is remembered and asks for a new proposal. */
  const changeFrame = (frame: StructureFrame) => {
    setStructureFrameState(frame);
    setStructureFrame(frame);
    void requestStructure(frame, true);
  };

  const editProposal = (value: string) => setStructureProposal(previous => previous ? { ...previous, content: value } : null);

  return {
    structureOpen, structureBusy, structureFrame, structureError, structureProposal,
    closeStructure, requestStructure, applyStructure, changeFrame, editProposal,
  };
}

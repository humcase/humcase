import { useCallback, useState, type MutableRefObject } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../services/tauriApi';
import { todayLocalIso } from '../utils/datetime';

interface UseBriefsOptions {
  activeProjectId: string | null;
  visibleProjectIds: Set<string>;
  spaceEpochRef: MutableRefObject<number>;
  reportAction: (message: string, tone?: 'danger' | 'success') => void;
}

/** Handoff and Daily Standup briefs: generation, text modal and copy to the clipboard. */
export function useBriefs({ activeProjectId, visibleProjectIds, spaceEpochRef, reportAction }: UseBriefsOptions) {
  const { t } = useTranslation();
  // Brief or daily update generation in progress: button busy, no double launch.
  const [briefPending, setBriefPending] = useState<'handoff' | 'standup' | null>(null);
  const [showStandup, setShowStandup] = useState(false);
  const [showHandoff, setShowHandoff] = useState(false);
  const [handoffText, setHandoffText] = useState('');
  const [standupText, setStandupText] = useState('');
  // Copy failure: the "Copy & Close" button must not announce an unverified success.
  const [copyError, setCopyError] = useState<string | null>(null);

  // Handoff/Standup modals: Escape, focus containment and return to the triggering element.
  const closeTextModal = useCallback(() => {
    setShowStandup(false);
    setShowHandoff(false);
    setCopyError(null);
  }, []);

  const openHandoff = async () => {
    if (!activeProjectId || !visibleProjectIds.has(activeProjectId) || briefPending) return;
    const epoch = spaceEpochRef.current;
    setBriefPending('handoff');
    try {
      const brief = await api.getHandoff(activeProjectId);
      if (spaceEpochRef.current !== epoch) return;
      setHandoffText(brief);
      setCopyError(null);
      setShowHandoff(true);
    } catch (e) {
      console.error(e);
      reportAction(t('workspace.brief.handoffError'));
    } finally { setBriefPending(null); }
  };
  const openStandup = async () => {
    if (!activeProjectId || !visibleProjectIds.has(activeProjectId) || briefPending) return;
    const epoch = spaceEpochRef.current;
    setBriefPending('standup');
    try {
      const standupNotes = await api.getDailyStandup(activeProjectId);
      if (spaceEpochRef.current !== epoch) return;
      const done = standupNotes.filter(n => n.status === 'resolved');
      const doing = standupNotes.filter(n => n.status === 'in_progress');
      const todo = standupNotes.filter(n => n.status === 'open');
      let md = `${t('workspace.brief.standupTitle', { date: todayLocalIso() })}\n\n`;
      if (done.length) md += `${t('workspace.brief.standupDone')}\n${done.map(n => `- ${n.title}`).join('\n')}\n\n`;
      if (doing.length) md += `${t('workspace.brief.standupDoing')}\n${doing.map(n => `- ${n.title}`).join('\n')}\n\n`;
      if (todo.length) md += `${t('workspace.brief.standupTodo')}\n${todo.map(n => `- ${n.title}`).join('\n')}\n\n`;
      setStandupText(md);
      setCopyError(null);
      setShowStandup(true);
    } catch (e) {
      console.error(e);
      reportAction(t('workspace.brief.standupError'));
    } finally { setBriefPending(null); }
  };

  // Closing is only announced after a write actually confirmed.
  const copyAndClose = async () => {
    try {
      await navigator.clipboard.writeText(showHandoff ? handoffText : standupText);
      closeTextModal();
    } catch (e) {
      console.error(e);
      setCopyError(t('workspace.brief.copyError'));
    }
  };

  return {
    briefPending, showStandup, setShowStandup, showHandoff, setShowHandoff, handoffText, standupText, copyError,
    closeTextModal, openHandoff, openStandup, copyAndClose,
  };
}

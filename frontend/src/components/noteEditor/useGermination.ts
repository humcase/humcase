import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Note, NoteType, Proposal, ProposalAnswer, ProposalOutcome } from '../../types';
import { api } from '../../services/tauriApi';
import { getAiCleanProvider, getAiModel } from '../../utils/prefs';
import { aiErrorMessage } from '../../utils/aiErrors';
import type { PassageMark } from './passageMarks';

/** What the person chose for a proposal before answering. */
export interface ProposalDraft {
  keep: boolean;
  nature: NoteType;
  title: string;
  /** For a proposal a near case may cover: place that case here, create anyway, or create nothing. */
  nearChoice: 'place' | 'create' | 'skip';
}

export type GerminationStage = 'idle' | 'reading' | 'open' | 'closed' | 'done';

function draftOf(proposal: Proposal): ProposalDraft {
  const near = proposal.near;
  return {
    // A proposal Jev doubts waits unchecked: the person decides to keep it.
    keep: !proposal.doubt,
    nature: proposal.nature,
    title: proposal.title,
    nearChoice: !near ? 'create' : near.place === 'movable' ? 'place' : 'skip',
  };
}

/** The answer sent for a proposal and its draft. */
export function answerOf(proposal: Proposal, draft: ProposalDraft): ProposalAnswer {
  if (!draft.keep) return { id: proposal.id, choice: 'refuse' };
  if (proposal.near && draft.nearChoice !== 'create') return { id: proposal.id, choice: draft.nearChoice };
  const title = draft.title.trim();
  return {
    id: proposal.id,
    choice: 'create',
    ...(draft.nature !== proposal.nature ? { nature: draft.nature } : {}),
    ...(title && title !== proposal.title ? { title } : {}),
  };
}

/**
 * Faire germer in a case: asks the AI for the next steps, keeps the proposals waiting (Humcase's
 * and the agents'), the person's choices on each, and their answer. Reloads the waiting proposals
 * when the case changes or its revision advances (an agent proposed).
 */
export function useGermination({ note, flushDraft, onNoteUpdated }: {
  note: Note | null;
  /** Saves the text being typed: Humcase reads the case as saved. */
  flushDraft: () => Promise<void>;
  onNoteUpdated: () => void;
}) {
  const [stage, setStage] = useState<GerminationStage>('idle');
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [drafts, setDrafts] = useState<Record<string, ProposalDraft>>({});
  const [dropped, setDropped] = useState(0);
  const [checked, setChecked] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ result: ProposalOutcome; ids: string[] } | null>(null);
  const [lit, setLit] = useState<{ passage: string; held: boolean } | null>(null);
  const [reveal, setReveal] = useState<{ passage: string; at: number } | null>(null);
  const noteId = note?.id ?? null;
  const shownRef = useRef(noteId);
  useEffect(() => { shownRef.current = noteId; }, [noteId]);

  const adopt = useCallback((waiting: Proposal[]) => {
    setProposals(waiting);
    setDrafts(previous => {
      const next: Record<string, ProposalDraft> = {};
      for (const proposal of waiting) next[proposal.id] = previous[proposal.id] ?? draftOf(proposal);
      return next;
    });
  }, []);

  /* eslint-disable react/set-state-in-effect -- resets the panel when another case is shown. */
  useEffect(() => {
    setStage('idle');
    setProposals([]);
    setDrafts({});
    setDropped(0);
    setChecked(false);
    setError(null);
    setOutcome(null);
    setLit(null);
  }, [noteId]);
  /* eslint-enable react/set-state-in-effect */

  // Proposals waiting in the case: shown as a band to resume, never opened by surprise.
  const revision = note?.revision;
  useEffect(() => {
    if (!noteId) return;
    let cancelled = false;
    Promise.resolve()
      .then(() => api.listProposals(noteId))
      .then(waiting => {
        if (cancelled || shownRef.current !== noteId) return;
        adopt(waiting);
        setStage(current => {
          if (waiting.length === 0) return current === 'open' || current === 'closed' ? 'idle' : current;
          return current === 'idle' ? 'closed' : current;
        });
      })
      .catch(() => { /* The panel stays as it is; the next change asks again. */ });
    return () => { cancelled = true; };
  }, [noteId, revision, adopt]);

  const germinate = useCallback(async () => {
    if (!noteId || busy) return;
    if (stage === 'closed' && proposals.length > 0) { setStage('open'); return; }
    setBusy(true);
    setError(null);
    setOutcome(null);
    setStage('reading');
    try {
      await flushDraft();
      const provider = getAiCleanProvider();
      const result = await api.germinateCase(noteId, provider, getAiModel(provider) || undefined);
      if (shownRef.current !== noteId) return;
      setDropped(result.dropped);
      setChecked(result.checked);
      const waiting = await api.listProposals(noteId);
      if (shownRef.current !== noteId) return;
      // A new germination starts from fresh choices.
      setDrafts({});
      adopt(waiting);
      setStage('open');
    } catch (cause) {
      if (shownRef.current !== noteId) return;
      setError(aiErrorMessage(cause));
      setStage(proposals.length > 0 ? 'open' : 'idle');
    } finally {
      setBusy(false);
    }
  }, [noteId, busy, stage, proposals.length, flushDraft, adopt]);

  const update = useCallback((id: string, change: Partial<ProposalDraft>) => {
    setDrafts(previous => previous[id] ? { ...previous, [id]: { ...previous[id], ...change } } : previous);
  }, []);

  const refuseAll = useCallback(() => {
    setDrafts(previous => Object.fromEntries(Object.entries(previous).map(([id, draft]) => [id, { ...draft, keep: false }])));
  }, []);

  const answers = useMemo(
    () => proposals.filter(p => drafts[p.id]).map(p => answerOf(p, drafts[p.id])),
    [proposals, drafts],
  );

  const submit = useCallback(async () => {
    if (!noteId || busy || answers.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.answerProposals(noteId, answers);
      if (shownRef.current !== noteId) return;
      setOutcome({ result, ids: answers.map(a => a.id) });
      setLit(null);
      setStage('done');
      adopt(await api.listProposals(noteId));
      onNoteUpdated();
    } catch (cause) {
      if (shownRef.current === noteId) setError(String(cause));
    } finally {
      setBusy(false);
    }
  }, [noteId, busy, answers, adopt, onNoteUpdated]);

  const undo = useCallback(async () => {
    if (!noteId || !outcome || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.undoProposals(noteId, outcome.ids);
      if (shownRef.current !== noteId) return;
      setOutcome(null);
      adopt(await api.listProposals(noteId));
      setStage('open');
      onNoteUpdated();
    } catch (cause) {
      if (shownRef.current === noteId) setError(String(cause));
    } finally {
      setBusy(false);
    }
  }, [noteId, outcome, busy, adopt, onNoteUpdated]);

  const close = useCallback(() => {
    setLit(null);
    setStage(proposals.length > 0 ? 'closed' : 'idle');
  }, [proposals.length]);

  const hover = useCallback((passage: string | null) => {
    setLit(current => (current?.held ? current : passage ? { passage, held: false } : null));
  }, []);

  const showPassage = useCallback((passage: string) => {
    setLit(current => (current?.held && current.passage === passage ? null : { passage, held: true }));
    setReveal({ passage, at: Date.now() });
  }, []);

  // The passages of the proposals kept are marked with their nature while the panel is open.
  const marks = useMemo<PassageMark[]>(() => {
    if (stage !== 'open') return [];
    const seen = new Set<string>();
    const result: PassageMark[] = [];
    for (const proposal of proposals) {
      const draft = drafts[proposal.id];
      const isLit = lit?.passage === proposal.passage;
      if (seen.has(proposal.passage) || (!draft?.keep && !isLit)) continue;
      seen.add(proposal.passage);
      result.push({ passage: proposal.passage, nature: draft?.nature ?? proposal.nature, lit: isLit });
    }
    return result;
  }, [stage, proposals, drafts, lit]);

  return {
    stage, proposals, drafts, dropped, checked, error, busy, outcome: outcome?.result ?? null, answers, marks, reveal,
    litPassage: lit?.passage ?? null,
    germinate, update, refuseAll, submit, undo, close, hover, showPassage,
    reopen: () => setStage('open'),
  };
}

export type Germinating = ReturnType<typeof useGermination>;

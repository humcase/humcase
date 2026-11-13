import { useCallback, useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import i18n from '../../i18n';
import type { Note, NoteType, ProjectSpace } from '../../types';
import { api } from '../../services/tauriApi';
import { dictationLanguageCode, getAiCleanProvider, getAiModel, getAiSttProvider, getDictationAutoStructure, getDictationLanguage, type StructureFrame } from '../../utils/prefs';
import { deriveTitle, isDefaultTitle } from '../../utils/title';
import { beginVoiceProcessing, trackEditorWrite } from '../../utils/backupBarrier';
import { isMissingKeyError } from '../../utils/aiErrors';
import { isCleanProviderReady } from '../../hooks/useAiReady';
import { addPendingWrite } from '../../utils/pendingWrites';
import { unpersistedRemainder, type EditableState } from './editableState';
import { persistRaw } from './usePendingWrites';
import type { ShowToast } from './useEditorToast';

type SaveNote = (newContent?: string, newTitle?: string, newType?: NoteType, newPriority?: string, newTags?: string[], newDue?: string) => Promise<void> | undefined;

/**
 * Voice dictation in the editor (safeguard 3): the raw text is written before any AI call and
 * appended, never replaced; a session interrupted by an unmount or a window close keeps what was
 * received, without writing it twice; the stop waits for the last audio fragment. The editor
 * owns the shared refs (displayed case, local draft, write chain), read here at the time a
 * response arrives, never as captured when the dictation started.
 */
export function useDictation({
  note, noteSpace, structureOpen, structureFrame, mountedRef, currentNoteIdRef, localStateRef, baselineRef, saveChainRef,
  flushDraft, handleSave, restoreVersion, setContent, setTitle, showToast, refreshPendingWrites, onNoteUpdated,
}: {
  note: Note | null;
  noteSpace: ProjectSpace | null;
  structureOpen: boolean;
  structureFrame: StructureFrame;
  /** False once the editor has left: after an `await`, nothing more is opened or shown. */
  mountedRef: MutableRefObject<boolean>;
  currentNoteIdRef: MutableRefObject<string | null>;
  localStateRef: MutableRefObject<EditableState>;
  baselineRef: MutableRefObject<EditableState>;
  /** The editor's write chain: every write of the case goes behind the previous one. */
  saveChainRef: MutableRefObject<Promise<void>>;
  flushDraft: () => Promise<void>;
  handleSave: SaveNote;
  restoreVersion: (noteId: string, expected: string | null, version: string) => Promise<void>;
  setContent: Dispatch<SetStateAction<string>>;
  setTitle: Dispatch<SetStateAction<string>>;
  showToast: ShowToast;
  refreshPendingWrites: () => void;
  onNoteUpdated: () => void;
}) {
  const { t } = useTranslation();
  // Dictation session in progress: allows persisting the accumulated text if the editor is unmounted.
  // `alreadyPersisted` remembers the prefix already written (window close) to never append it
  // twice to the raw text when the dictation then ends normally.
  const sessionRef = useRef<{ noteId: string; transcript: string; alreadyPersisted: string; space: ProjectSpace | null; projectId: string | null } | null>(null);
  // Voice Assistant States
  const [isRecording, setIsRecording] = useState(false);
  const [isProcessingVoice, setIsProcessingVoice] = useState(false);
  // Note targeted by the dictation session in progress: the overlay must not show on another note.
  const [voiceTargetNoteId, setVoiceTargetNoteId] = useState<string | null>(null);

  // Real-time states
  const [liveTranscript, setLiveTranscript] = useState('');
  const [interimTranscript, setInterimTranscript] = useState('');

  const mediaRecorder = useRef<MediaRecorder | null>(null);
  // Listeners for the Rust proxy events (transcription://message, transcription://closed).
  const unlistenRef = useRef<UnlistenFn[]>([]);
  const streamRef = useRef<MediaStream | null>(null);
  // Chain of audio fragment sends: awaited before closing the session, so that the
  // last fragment (produced asynchronously by `stop()`) is actually transmitted.
  const audioSendsRef = useRef<Promise<void>>(Promise.resolve());
  const stopDictationRef = useRef<Promise<void> | null>(null);
  const finalizeDictationRef = useRef<(() => void) | null>(null);
  // Start lock: blocks a double click before `isRecording` becomes `true`.
  const startingRef = useRef(false);

  // A transcript belongs to the case it was dictated on: another case starts without one.
  const shownNoteId = note?.id ?? null;
  /* eslint-disable react/set-state-in-effect -- resets the display when the case changes. */
  useEffect(() => {
    setLiveTranscript('');
    setInterimTranscript('');
  }, [shownNoteId]);
  /* eslint-enable react/set-state-in-effect */

  /** Persists an interrupted raw dictation; on failure, keeps it in the escrow. */
  const flushRaw = useCallback((noteId: string, text: string, space?: ProjectSpace | null, projectId?: string | null): Promise<void> => {
    const execute = async () => {
      try {
        await persistRaw(noteId, text);
      } catch (e) {
        console.error("Failed to save the raw dictation", e);
        addPendingWrite({ kind: 'brut', noteId, texte: text, space: space ?? null, projectId: projectId ?? null });
        if (mountedRef.current) refreshPendingWrites();
      }
    };
    saveChainRef.current = trackEditorWrite(saveChainRef.current.then(execute, execute));
    return saveChainRef.current;
  }, [mountedRef, refreshPendingWrites, saveChainRef]);

  /**
   * Persists the **not yet written** part of the dictation in progress.
   *
   * A window close hides Humcase without stopping the dictation: the received text is saved
   * and remembered in `alreadyPersisted`. When the dictation then ends normally, only the rest
   * is appended to the raw text, otherwise the already written prefix would be duplicated.
   */
  const persistRemainderForSession = useCallback((): void => {
    const session = sessionRef.current;
    if (!session) return;
    const text = session.transcript.trim();
    if (text.length <= session.alreadyPersisted.length) return;
    const remainder = unpersistedRemainder(text, session.alreadyPersisted);
    session.alreadyPersisted = text;
    if (remainder) void flushRaw(session.noteId, remainder, session.space, session.projectId);
  }, [flushRaw]);

  /**
   * Smart dictation (0.9.80): a single AI call integrates the dictation into the whole note,
   * structures it and corrects transcription errors. Applied through the same path as Structure
   * (version check in the database, replaced version in the history, Undo). Returns `false`
   * without writing anything if the note is no longer displayed or was modified during the call: the caller
   * then falls back to simply appending the cleaned dictation. The raw text is already saved.
   */
  const integrateDictationText = async (noteId: string, dictation: string, provider: string, model?: string): Promise<boolean> => {
    await flushDraft();
    if (currentNoteIdRef.current !== noteId || localStateRef.current.id !== noteId) return false;
    const before = localStateRef.current.content;
    const integrated = await api.integrateDictation(noteId, before, dictation, provider, model, structureFrame);
    const result = { content: null as string | null };
    const execute = async () => {
      if (currentNoteIdRef.current !== noteId || localStateRef.current.id !== noteId) return;
      // Input during the AI call: we do not replace what the user just wrote.
      if (localStateRef.current.content !== before) return;
      const saved = await api.applyStructuredNote(noteId, before, integrated);
      baselineRef.current.content = saved.content;
      localStateRef.current.content = saved.content;
      setContent(saved.content);
      result.content = saved.content;
    };
    saveChainRef.current = trackEditorWrite(saveChainRef.current.then(execute, execute));
    await saveChainRef.current;
    const savedContent = result.content;
    if (savedContent === null) return false;
    const autoTitle = isDefaultTitle(localStateRef.current.title) ? deriveTitle(savedContent) : null;
    if (autoTitle) {
      setTitle(autoTitle);
      await handleSave(undefined, autoTitle);
    }
    onNoteUpdated();
    showToast(t('editor.structure.dictationDone'), {
      duration: 8000,
      action: { label: t('common.cancel'), run: () => { void restoreVersion(noteId, savedContent, before); } },
    });
    return true;
  };

  const processWithLLM = async (fullTranscript: string, targetNoteId: string, alreadyPersistedPrefix = '', space: ProjectSpace | null = null, projectId: string | null = null) => {
    const endVoiceProcessing = beginVoiceProcessing();
    if (mountedRef.current) setIsProcessingVoice(true);

    // Only rewrite the part not already persisted: a window close may have saved a
    // prefix of the dictation (which then continues); appending it again would duplicate that text.
    const toPersist = unpersistedRemainder(fullTranscript, alreadyPersistedPrefix);

    // 1) Persist the raw dictation BEFORE any cleanup: an LLM failure (missing key, HTTP
    //    error, empty response) must never cause the transcription to be lost.
    try {
      // An unmount or a close may have put the prefix in the write chain. Waiting for
      // its end keeps the raw text order when the last Deepgram message arrives afterwards.
      await saveChainRef.current;
      if (toPersist) await persistRaw(targetNoteId, toPersist);
      if (mountedRef.current) onNoteUpdated();
    } catch (e) {
      console.error("Failed to save the raw dictation", e);
      if (mountedRef.current) setIsProcessingVoice(false);
      // Never erase unsaved text: it joins the shared recoverable escrow
      // (the one for unmount failures), and is only removed once actually recovered. A
      // later successful dictation can therefore no longer erase it.
      if (toPersist) {
        addPendingWrite({ kind: 'brut', noteId: targetNoteId, texte: toPersist, space: space, projectId });
        if (mountedRef.current) refreshPendingWrites();
      }
      if (mountedRef.current) showToast(t('editor.messages.dictationNotSaved'), { duration: 6000 });
      endVoiceProcessing();
      return;
    }

    // 2) Optional LLM cleanup: its failure must not destroy anything.
    try {
      // The chosen provider's key is read on the Rust side: the webview never sees it.
      const cleanProvider = getAiCleanProvider();
      const cleanModel = getAiModel(cleanProvider) || undefined;
      // No AI connected: the dictated text is appended as is (the raw dictation is already saved).
      const aiConnected = (await isCleanProviderReady()) !== false;
      if (aiConnected && getDictationAutoStructure() && mountedRef.current && currentNoteIdRef.current === targetNoteId) {
        try {
          if (await integrateDictationText(targetNoteId, fullTranscript, cleanProvider, cleanModel)) return;
        } catch (e) {
          // Integration refused (safeguard), failed or concurrent: fall back to simple appending.
          console.warn('Smart dictation not applied', e);
        }
      }
      const finalMarkdown = aiConnected
        ? await api.cleanTranscript(fullTranscript, cleanProvider, getAiModel(cleanProvider) || undefined, targetNoteId)
        : fullTranscript;

      // The raw dictation is already saved: we only append the cleaned text.
      if (mountedRef.current && currentNoteIdRef.current === targetNoteId) {
        // We start from the **local draft** (not the content reread from the database): an input not yet
        // saved during AI processing must not be replaced. `handleSave` persists this
        // merged content and only writes the modified fields.
        const draft = localStateRef.current.content;
        const newContent = draft ? `${draft}\n\n${finalMarkdown}` : finalMarkdown;
        setContent(newContent);
        // Automatic title: only if the user has not yet entered one.
        const autoTitle = isDefaultTitle(localStateRef.current.title)
          ? deriveTitle(finalMarkdown)
          : null;
        if (autoTitle) setTitle(autoTitle);
        await handleSave(newContent, autoTitle ?? undefined);
      } else {
        // The targeted note is no longer displayed: **atomic** append in the database (no get-modify-write),
        // to never overwrite a concurrent write (MCP agent) on the same note. The title
        // is only applied on the Rust side if it is still empty or default.
        await api.appendNoteContent(targetNoteId, finalMarkdown, deriveTitle(finalMarkdown) ?? undefined);
        if (mountedRef.current) onNoteUpdated();
      }
      if (!aiConnected && mountedRef.current) showToast(t('editor.messages.dictationNoAi'), { duration: 6000 });
    } catch (e) {
      console.error(e);
      if (!mountedRef.current) return;
      // The message is translated: we test the stable code returned by Rust, never the text.
      if (isMissingKeyError(e)) {
        showToast(t('editor.messages.dictationNoKey'), { duration: 6000 });
      } else {
        showToast(t('editor.messages.dictationCleanFailed'), { duration: 6000 });
      }
    } finally {
      endVoiceProcessing();
      if (mountedRef.current) {
        setIsProcessingVoice(false);
        setLiveTranscript('');
        setInterimTranscript('');
      }
    }
  };

  const reportStopFailure = useCallback((err: unknown) => {
    console.error('Transcription stop', err);
    finalizeDictationRef.current?.();
    // `i18n.t` and not the hook's `t`: this function is called from the unmount effect, whose
    // dependencies must not change (otherwise the cleanup would run on every render).
    if (mountedRef.current) showToast(i18n.t('editor.messages.dictationInterrupted'));
  }, [mountedRef, showToast]);

  // Dictation stop: `stop()` produces a **last** `dataavailable` asynchronously. We
  // wait for the `stop` event (emitted after this fragment) then for the sends in progress before closing the
  // Rust session, otherwise the last word may be lost.
  const stopRecording = useCallback(async () => {
    if (stopDictationRef.current) return stopDictationRef.current;
    const stopFn = (async () => {
      const recorder = mediaRecorder.current;
      if (recorder && recorder.state === 'recording') {
        await new Promise<void>(resolve => {
          recorder.addEventListener('stop', () => resolve(), { once: true });
          recorder.stop();
        });
      }
      await audioSendsRef.current;
      audioSendsRef.current = Promise.resolve();
      // `stop_transcription` only closes the Rust proxy's input; `transcription://closed`
      // will arrive after the last transcriptions emitted by Deepgram.
      await api.stopTranscription();
    })();
    stopDictationRef.current = stopFn;
    return stopFn;
  }, []);

  const startRecording = async () => {
    if (!note) return;
    // Immediate lock: two quick clicks must not start two sessions (the button stays
    // active until the connection and the microphone permission are obtained).
    if (isRecording || isProcessingVoice || startingRef.current || structureOpen) return;
    startingRef.current = true;
    stopDictationRef.current = null;

    const targetNoteId = note.id; // Locks the ID at the start of the dictation
    const dictationSpace = noteSpace; // Space frozen with the ID: used if the failure goes to the escrow.
    const dictationProjectId = note.project_id || null; // Project frozen with the ID, for the space repair.
    setVoiceTargetNoteId(targetNoteId);

    let localFullTranscript = '';

    const release = () => {
      unlistenRef.current.forEach(stop => stop());
      unlistenRef.current = [];
      streamRef.current?.getTracks().forEach(t => t.stop());
      streamRef.current = null;
    };

    /** Remembers the accumulated text without losing the already persisted part (`alreadyPersisted`). */
    const sessionUpdate = (transcript: string) => {
      const previousValue = sessionRef.current?.noteId === targetNoteId ? sessionRef.current : null;
      sessionRef.current = {
        noteId: targetNoteId,
        transcript,
        alreadyPersisted: previousValue?.alreadyPersisted ?? '',
        // Space of the dictated note, frozen at start: an escrow failure keeps the right space.
        space: previousValue?.space ?? dictationSpace,
        projectId: previousValue?.projectId ?? dictationProjectId,
      };
    };

    let finishedFlag = false;
    const finalize = () => {
      if (finishedFlag) return;
      finishedFlag = true;
      finalizeDictationRef.current = null;
      if (mountedRef.current) {
        setIsRecording(false);
        setVoiceTargetNoteId(null);
      }
      startingRef.current = false;
      // The session ends: we remove it, but keep the already persisted prefix (a
      // window close may have written the start of the dictation) so as not to duplicate it.
      const session = sessionRef.current;
      sessionRef.current = null;
      release();
      if (localFullTranscript.trim().length > 0) {
        const already = session?.noteId === targetNoteId ? session.alreadyPersisted : '';
        void processWithLLM(localFullTranscript.trim(), targetNoteId, already, session?.space ?? dictationSpace, session?.projectId ?? dictationProjectId);
      } else {
        if (mountedRef.current) {
          setLiveTranscript('');
          setInterimTranscript('');
        }
      }
    };
    finalizeDictationRef.current = finalize;

    try {
      // The transcription provider's key is read on the Rust side: the webview never sees it.
      await api.startTranscription(
        getAiSttProvider(),
        dictationLanguageCode(getDictationLanguage(), i18n.resolvedLanguage ?? i18n.language),
      );

      // The editor was unmounted during the connection: close the opened session, start nothing.
      if (!mountedRef.current) {
        void api.stopTranscription().catch(() => {});
        startingRef.current = false;
        finalizeDictationRef.current = null;
        return;
      }

      // Listen for transcriptions and the close, BEFORE starting the microphone.
      unlistenRef.current.push(
        await listen<string>('transcription://message', event => {
          try {
            const received = JSON.parse(event.payload);
            const transcript = received.channel?.alternatives[0]?.transcript;
            if (!transcript) return;
            if (received.is_final) {
              localFullTranscript += transcript + ' ';
              // Accumulated text remembered: if the editor is unmounted before the end (view change),
              // it will be persisted instead of disappearing with the component.
              sessionUpdate(localFullTranscript);

              // Magic voice command "Note terminée" (French; matches the dictated phrase).
              const stopRegex = /note(s)? termin[é|e]e?(s)?/i;
              if (stopRegex.test(localFullTranscript)) {
                localFullTranscript = localFullTranscript.replace(stopRegex, '').trim();
                sessionUpdate(localFullTranscript);
                if (mountedRef.current) {
                  setLiveTranscript(localFullTranscript);
                  setInterimTranscript('');
                }
                // Clean stop (last fragment included): see `stopRecording`.
                void stopRecording().catch(reportStopFailure);
                return;
              }

              if (mountedRef.current) {
                setLiveTranscript(localFullTranscript);
                setInterimTranscript('');
              }
            } else {
              if (mountedRef.current) setInterimTranscript(transcript);
            }
          } catch (err) {
            console.error('Message Deepgram illisible', err);
          }
        }),
        await listen('transcription://closed', finalize)
      );

      if (!mountedRef.current) {
        release();
        void api.stopTranscription().catch(() => {});
        startingRef.current = false;
        finalizeDictationRef.current = null;
        return;
      }

      const preferredMic = localStorage.getItem('preferred_mic');
      const audioConstraints = preferredMic ? { deviceId: { exact: preferredMic } } : true;
      const stream = await navigator.mediaDevices.getUserMedia({ audio: audioConstraints });

      // Unmount during the permission request: release the obtained microphone, start nothing.
      if (!mountedRef.current) {
        stream.getTracks().forEach(t => t.stop());
        release();
        void api.stopTranscription().catch(() => {});
        startingRef.current = false;
        finalizeDictationRef.current = null;
        return;
      }
      streamRef.current = stream;

      setIsRecording(true);
      setLiveTranscript('');
      setInterimTranscript('');

      const recorder = new MediaRecorder(stream);
      mediaRecorder.current = recorder;
      recorder.ondataavailable = e => {
        if (e.data.size === 0) return;
        // Fragments are sent in order; the chain can be awaited before closing.
        audioSendsRef.current = audioSendsRef.current.then(async () => {
          try {
            // Send the `Uint8Array` directly (Tauri serializes it as an array): no more
            // useless `Array.from` every 250 ms.
            const bytes = new Uint8Array(await e.data.arrayBuffer());
            await api.sendAudioChunk(bytes);
          } catch (err) {
            console.error("Envoi d'un fragment audio impossible", err);
          }
        });
      };
      recorder.start(250);
    } catch (err) {
      console.error(err);
      startingRef.current = false;
      finalizeDictationRef.current = null;
      release();
      if (mountedRef.current) setVoiceTargetNoteId(null);
      // Close an already opened Rust session: otherwise it would stay active and block the next one.
      void api.stopTranscription().catch(() => {});
      if (mountedRef.current) {
        showToast(t('editor.messages.micError'));
        setInterimTranscript(t('editor.dictation.micError'));
        setTimeout(() => { if (mountedRef.current) setInterimTranscript(''); }, 4000);
      }
    }
  };

  /** A dictation is starting, running or being processed: no structuring, no other case. */
  const isBusy = () => startingRef.current || isRecording || isProcessingVoice;

  /** Something of a dictation is still alive (session, microphone): no backup now. */
  const isActive = useCallback(() => startingRef.current || isRecording || isProcessingVoice ||
    sessionRef.current !== null || mediaRecorder.current?.state === 'recording', [isRecording, isProcessingVoice]);

  /** The editor leaves: stop the dictation cleanly, or release what a failed start left open. */
  const releaseOnUnmount = useCallback(() => {
    if (finalizeDictationRef.current) {
      // Keep the listeners until `closed`: the last message follows CloseStream on the Rust side.
      // `stopRecording` awaits stop(MediaRecorder), all audio sends and stop_transcription.
      void stopRecording().catch(reportStopFailure);
    } else {
      streamRef.current?.getTracks().forEach(track => track.stop());
      streamRef.current = null;
      unlistenRef.current.forEach(stop => stop());
      unlistenRef.current = [];
    }
  }, [stopRecording, reportStopFailure]);

  return {
    isRecording, isProcessingVoice, voiceTargetNoteId, liveTranscript, interimTranscript,
    startRecording, stopRecording, reportStopFailure, persistRemainderForSession, isBusy, isActive, releaseOnUnmount,
  };
}

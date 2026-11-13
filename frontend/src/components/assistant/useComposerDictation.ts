import { useCallback, useEffect, useRef, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import i18n from '../../i18n';
import { api } from '../../services/tauriApi';
import { dictationLanguageCode, getAiSttProvider, getDictationLanguage } from '../../utils/prefs';

/** How long the last words may take to arrive after the microphone stops. */
const CLOSE_WAIT_MS = 4000;

/**
 * Dictation into a composer: what is heard comes back as text, to read and correct before
 * sending. Nothing is written in a case, so there is no raw dictation to keep and no cleanup by
 * AI: the person sees the words as they were transcribed.
 */
export function useComposerDictation() {
  const [listening, setListening] = useState(false);
  const [starting, setStarting] = useState(false);
  const [interim, setInterim] = useState('');
  const [error, setError] = useState<string | null>(null);
  const finals = useRef('');
  const unlisten = useRef<UnlistenFn[]>([]);
  const stream = useRef<MediaStream | null>(null);
  const recorder = useRef<MediaRecorder | null>(null);
  const sends = useRef<Promise<void>>(Promise.resolve());
  const closed = useRef<(() => void) | null>(null);

  const release = useCallback(() => {
    unlisten.current.forEach(stop => stop());
    unlisten.current = [];
    stream.current?.getTracks().forEach(track => track.stop());
    stream.current = null;
    recorder.current = null;
  }, []);

  const start = useCallback(async () => {
    if (listening || starting) return;
    setStarting(true);
    setError(null);
    finals.current = '';
    setInterim('');
    try {
      // The transcription key stays on the Rust side.
      await api.startTranscription(getAiSttProvider(), dictationLanguageCode(getDictationLanguage(), i18n.resolvedLanguage ?? i18n.language));
      unlisten.current.push(
        await listen<string>('transcription://message', event => {
          try {
            const received = JSON.parse(event.payload);
            const transcript: string | undefined = received.channel?.alternatives?.[0]?.transcript;
            if (!transcript) return;
            if (received.is_final) { finals.current += `${transcript} `; setInterim(''); } else setInterim(transcript);
          } catch { /* an unreadable message is skipped */ }
        }),
        await listen('transcription://closed', () => { closed.current?.(); }),
      );
      const preferred = localStorage.getItem('preferred_mic');
      const media = await navigator.mediaDevices.getUserMedia({ audio: preferred ? { deviceId: { exact: preferred } } : true });
      stream.current = media;
      const rec = new MediaRecorder(media);
      recorder.current = rec;
      rec.ondataavailable = event => {
        if (event.data.size === 0) return;
        sends.current = sends.current.then(async () => {
          try { await api.sendAudioChunk(new Uint8Array(await event.data.arrayBuffer())); } catch { /* the next fragment may pass */ }
        });
      };
      rec.start(250);
      setListening(true);
    } catch (err) {
      release();
      void api.stopTranscription().catch(() => {});
      setError(String(err instanceof Error ? err.message : err));
    } finally {
      setStarting(false);
    }
  }, [listening, starting, release]);

  /** Stops the microphone and resolves to everything heard, the last words included. */
  const stop = useCallback(async (): Promise<string> => {
    const rec = recorder.current;
    if (rec && rec.state === 'recording') {
      await new Promise<void>(resolve => { rec.addEventListener('stop', () => resolve(), { once: true }); rec.stop(); });
    }
    await sends.current;
    sends.current = Promise.resolve();
    const done = new Promise<void>(resolve => {
      const timer = setTimeout(resolve, CLOSE_WAIT_MS);
      closed.current = () => { clearTimeout(timer); resolve(); };
    });
    try { await api.stopTranscription(); } catch { /* already closed */ }
    await done;
    closed.current = null;
    release();
    setListening(false);
    setInterim('');
    return finals.current.trim();
  }, [release]);

  /** Stops at once and drops what was heard: the window closed or went to the other one. */
  const cancel = useCallback(() => {
    const rec = recorder.current;
    if (rec && rec.state === 'recording') rec.stop();
    void api.stopTranscription().catch(() => {});
    sends.current = Promise.resolve();
    closed.current = null;
    release();
    finals.current = '';
    setListening(false);
    setInterim('');
  }, [release]);

  // The panel unmounts while listening: the microphone and the session are released.
  useEffect(() => () => {
    if (recorder.current?.state === 'recording') recorder.current.stop();
    unlisten.current.forEach(off => off());
    stream.current?.getTracks().forEach(track => track.stop());
    if (recorder.current) void api.stopTranscription().catch(() => {});
  }, []);

  return { listening, starting, interim, error, start, stop, cancel };
}

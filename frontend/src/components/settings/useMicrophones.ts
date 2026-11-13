import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

/**
 * The microphones offered for dictation and the one chosen (`preferred_mic`). Enumeration alone
 * needs no permission (P2-08): without one already granted, names stay generic, and the
 * microphone is only requested by an explicit action ("Detect microphones").
 */
export function useMicrophones() {
  const { t } = useTranslation();
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const [selectedMic, setSelectedMic] = useState<string>(localStorage.getItem('preferred_mic') || '');
  const [micStatus, setMicStatus] = useState<string | null>(null);
  const [isRequestingMic, setIsRequestingMic] = useState(false);
  const micStreamRef = useRef<MediaStream | null>(null);
  const micDisposedRef = useRef(false);

  const releaseMicStream = () => {
    micStreamRef.current?.getTracks().forEach(track => track.stop());
    micStreamRef.current = null;
  };

  // Enumeration only, without React state: state is only updated on the response (`applyAudioDevices`).
  const readAudioInputs = async () =>
    (await navigator.mediaDevices.enumerateDevices()).filter(d => d.kind === 'audioinput');

  const applyAudioDevices = (audioInputs: MediaDeviceInfo[]) => {
    if (micDisposedRef.current) return;
    setDevices(audioInputs);
    setSelectedMic(previous => {
      if (previous || audioInputs.length === 0) return previous;
      const defaultId = audioInputs[0].deviceId;
      if (defaultId) localStorage.setItem('preferred_mic', defaultId);
      return defaultId;
    });
  };

  const reportUnavailableDevices = (err: unknown) => {
    console.error('Audio devices unavailable: names will stay generic', err);
  };

  const loadAudioDevices = () =>
    readAudioInputs().then(applyAudioDevices, reportUnavailableDevices);

  /** Explicit action: requests the microphone (full labels), then releases the stream immediately. */
  const requestMicAccess = async () => {
    if (!navigator.mediaDevices?.getUserMedia) {
      setMicStatus(t('settings.voice.audioUnavailable'));
      return;
    }
    setIsRequestingMic(true);
    setMicStatus(null);
    try {
      micStreamRef.current = await navigator.mediaDevices.getUserMedia({ audio: true });
      if (micDisposedRef.current) return;
      await loadAudioDevices();
    } catch (err) {
      console.error("Microphone access denied: device names will stay generic", err);
      if (!micDisposedRef.current) {
        setMicStatus(t('settings.voice.micDenied'));
      }
    } finally {
      releaseMicStream();
      if (!micDisposedRef.current) setIsRequestingMic(false);
    }
  };

  useEffect(() => {
    micDisposedRef.current = false;
    void loadAudioDevices();
    return () => {
      micDisposedRef.current = true;
      releaseMicStream();
    };
  }, []);

  const selectMic = (id: string) => {
    setSelectedMic(id);
    localStorage.setItem('preferred_mic', id);
  };

  return { devices, selectedMic, micStatus, isRequestingMic, requestMicAccess, selectMic };
}

export type Microphones = ReturnType<typeof useMicrophones>;

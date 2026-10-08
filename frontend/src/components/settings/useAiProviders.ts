import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../../services/tauriApi';
import type { AiProvidersInfo } from '../../types';
import { getAiCleanProvider, getAiModel, hasAiCleanChoice, getAiSttProvider, getDictationAutoStructure, getDictationLanguage, setAiCleanProvider, setAiModel, setAiSttProvider, setDictationAutoStructure, setDictationLanguage, type DictationLanguage } from '../../utils/prefs';
import { effectiveCleanProvider, notifyAiConfigChanged } from '../../hooks/useAiReady';
import { aiErrorMessage } from '../../utils/aiErrors';

/**
 * The AI providers chosen for cleanup and transcription, their keys and addresses. Keys are
 * written on the Rust side and never read back: the screen only knows whether one is set.
 */
export function useAiProviders() {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<AiProvidersInfo | null>(null);
  const [cleanProvider, setCleanProvider] = useState<string>(() => getAiCleanProvider());
  const [structuredDictation, setStructuredDictation] = useState(() => getDictationAutoStructure());
  const [dictationLanguage, setDictationLanguageState] = useState<DictationLanguage>(() => getDictationLanguage());
  const [sttProvider, setSttProvider] = useState<string>(() => getAiSttProvider());
  const [cleanModel, setCleanModel] = useState<string>(() => getAiModel(getAiCleanProvider()));
  const [cleanKeyInput, setCleanKeyInput] = useState('');
  const [sttKeyInput, setSttKeyInput] = useState('');
  const [keyStatus, setKeyStatus] = useState<string | null>(null);
  // Server address (local or custom provider) and connection test, for the cleanup provider.
  const [cleanUrlInput, setCleanUrlInput] = useState('');
  const [savingUrl, setSavingUrl] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState<string | null>(null);
  const [testingConnection, setTestingConnection] = useState(false);
  // Input generation per section ("clean" / "stt"): each save captures a
  // ticket, and any provider change invalidates responses still in flight. A stale
  // response must neither clear another provider's field nor show a misleading status.
  const keyGenerationRef = useRef({ clean: 0, stt: 0 });
  // m1: a key action (save/clear) in flight for a section disables the buttons of
  // THAT section: avoids crossing "Save" and "Clear" during the 5 s of lock
  // contention on the Rust side.
  const [savingClean, setSavingClean] = useState(false);
  const [savingStt, setSavingStt] = useState(false);

  // List of providers (metadata only) to populate the selectors. A failure is announced
  // with a retry: empty lists without a message do not say why nothing is offered.
  const [providersFailed, setProvidersFailed] = useState(false);
  const [providersAttempt, setProvidersAttempt] = useState(0);
  useEffect(() => {
    let disposed = false;
    api
      .listAiProviders()
      .then(list => {
        if (disposed) return;
        setProviders(list);
        setProvidersFailed(false);
        // Nothing chosen yet: show the AI the engine uses, the first one whose key is set.
        if (!hasAiCleanChoice()) {
          const used = effectiveCleanProvider(list.cleanup, getAiCleanProvider());
          setCleanProvider(used);
          setCleanModel(getAiModel(used));
        }
      })
      .catch(e => {
        console.error('AI providers unavailable', e);
        if (!disposed) setProvidersFailed(true);
      });
    return () => {
      disposed = true;
    };
  }, [providersAttempt]);

  const cleanInfo = providers?.cleanup.find(p => p.id === cleanProvider);
  const sttInfo = providers?.transcription.find(p => p.id === sttProvider);

  const saveKey = async (section: 'clean' | 'stt', provider: string, value: string): Promise<boolean> => {
    const ticket = ++keyGenerationRef.current[section];
    const setSaving = section === 'clean' ? setSavingClean : setSavingStt;
    setSaving(true);
    try {
      // 1. Key write: ONLY this failure is a "save failure".
      try {
        await api.setAiKey(provider, value);
      } catch (e) {
        // A stale error message would designate the wrong provider.
        if (keyGenerationRef.current[section] === ticket) {
          setKeyStatus(section === 'clean'
            ? t('settings.voice.keyStatus.clean.saveFailed', { error: String(e) })
            : t('settings.voice.keyStatus.stt.saveFailed', { error: String(e) }));
        }
        return false;
      }
      // 2. List refresh: a failure does not call the save into question (the key
      //    is saved) and must not prevent clearing the field.
      let refreshed = true;
      try {
        setProviders(await api.listAiProviders());
      } catch (e) {
        refreshed = false;
        console.error('Providers list not refreshed', e);
      }
      if (keyGenerationRef.current[section] !== ticket) {
        // Stale generation: the user left this flow (provider change). We stay
        // deliberately silent and do not touch the field, at the risk of erasing the text already
        // typed for the other provider.
        return false;
      }
      notifyAiConfigChanged();
      const saved = value.trim() !== '';
      if (section === 'clean') {
        setKeyStatus(refreshed
          ? t(saved ? 'settings.voice.keyStatus.clean.saved' : 'settings.voice.keyStatus.clean.cleared')
          : t(saved ? 'settings.voice.keyStatus.clean.savedNoRefresh' : 'settings.voice.keyStatus.clean.clearedNoRefresh'));
      } else {
        setKeyStatus(refreshed
          ? t(saved ? 'settings.voice.keyStatus.stt.saved' : 'settings.voice.keyStatus.stt.cleared')
          : t(saved ? 'settings.voice.keyStatus.stt.savedNoRefresh' : 'settings.voice.keyStatus.stt.clearedNoRefresh'));
      }
      return true;
    } finally {
      setSaving(false);
    }
  };

  const saveUrl = async (provider: string, value: string) => {
    setSavingUrl(true);
    setConnectionStatus(null);
    try {
      const saved = await api.setAiUrl(provider, value);
      setCleanUrlInput('');
      setKeyStatus(t(saved ? 'settings.voice.urlSaved' : 'settings.voice.urlCleared'));
      try {
        setProviders(await api.listAiProviders());
      } catch (e) {
        console.error('Providers list not refreshed', e);
      }
      notifyAiConfigChanged();
    } catch (e) {
      setKeyStatus(t('settings.voice.urlFailed', { error: String(e) }));
    } finally {
      setSavingUrl(false);
    }
  };

  const testConnection = async (provider: string) => {
    setTestingConnection(true);
    setConnectionStatus(null);
    try {
      const models = await api.testAiConnection(provider);
      setConnectionStatus(t('settings.voice.testOk', { count: models.length }));
    } catch (e) {
      setConnectionStatus(t('settings.voice.testFailed', { error: aiErrorMessage(e) }));
    } finally {
      setTestingConnection(false);
    }
  };

  const chooseCleanProvider = (id: string) => {
    // A provider change invalidates any key response still in flight
    // for this section.
    keyGenerationRef.current.clean += 1;
    setCleanProvider(id);
    setAiCleanProvider(id);
    setCleanModel(getAiModel(id));
    // The key field must not stay filled from one provider to the next.
    setCleanKeyInput('');
    setCleanUrlInput('');
    setConnectionStatus(null);
    setKeyStatus(null);
  };

  const chooseSttProvider = (id: string) => {
    keyGenerationRef.current.stt += 1;
    setSttProvider(id);
    setAiSttProvider(id);
    setSttKeyInput('');
    setKeyStatus(null);
  };

  const changeCleanModel = (model: string) => {
    setCleanModel(model);
    setAiModel(cleanProvider, model);
  };

  const changeDictationLanguage = (language: DictationLanguage) => {
    setDictationLanguageState(language);
    setDictationLanguage(language);
  };

  const changeStructuredDictation = (enabled: boolean) => {
    setStructuredDictation(enabled);
    setDictationAutoStructure(enabled);
  };

  /** Saves the typed key; the field is emptied only if it still holds what was saved. */
  const submitKey = async (section: 'clean' | 'stt') => {
    const [input, provider, setInput] = section === 'clean'
      ? [cleanKeyInput, cleanProvider, setCleanKeyInput] as const
      : [sttKeyInput, sttProvider, setSttKeyInput] as const;
    // An empty field does not clear: the "Clear" button is there for that.
    if (!input.trim()) return;
    const submittedValue = input;
    if (await saveKey(section, provider, submittedValue)) {
      // Value-bound clearing: if the user modified the field during the wait
      // (same provider), the existing new input is kept.
      setInput(prev => (prev === submittedValue ? '' : prev));
    }
  };

  const clearKey = (section: 'clean' | 'stt') => {
    if (section === 'clean') {
      setCleanKeyInput('');
      void saveKey('clean', cleanProvider, '');
    } else {
      setSttKeyInput('');
      void saveKey('stt', sttProvider, '');
    }
  };

  return {
    providers, providersFailed, retryProviders: () => setProvidersAttempt(attempt => attempt + 1),
    structuredDictation, changeStructuredDictation,
    dictationLanguage, changeDictationLanguage,
    cleanProvider, cleanInfo, chooseCleanProvider, cleanModel, changeCleanModel,
    cleanUrlInput, setCleanUrlInput, savingUrl, saveUrl,
    cleanKeyInput, setCleanKeyInput, savingClean,
    sttProvider, sttInfo, chooseSttProvider, sttKeyInput, setSttKeyInput, savingStt,
    submitKey, clearKey, keyStatus,
    testConnection, testingConnection, connectionStatus,
  };
}

export type AiProviders = ReturnType<typeof useAiProviders>;

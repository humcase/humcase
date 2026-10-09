import { useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import { DEFAULT_AI_CLEAN_PROVIDER, PREFS_CHANGED_EVENT, getAiCleanProvider } from '../utils/prefs';

/** Emitted in this window when an AI key or server address is saved in Settings. */
export const AI_CONFIG_CHANGED_EVENT = 'humcase:ai-config-changed';
/** Asks the workspace to open Settings (from the editor's "Connect an AI" link). */
export const OPEN_SETTINGS_EVENT = 'humcase:open-settings';

export function notifyAiConfigChanged() {
  window.dispatchEvent(new CustomEvent(AI_CONFIG_CHANGED_EVENT));
}

export function requestOpenSettings() {
  window.dispatchEvent(new CustomEvent(OPEN_SETTINGS_EVENT));
}

/**
 * The provider the engine calls (`pick_provider` on the Rust side): the chosen one when it is
 * ready, else the first other ready one, never the local server by default (its address is
 * always known), else the chosen one.
 */
export function effectiveCleanProvider(cleanup: { id: string; ready: boolean }[], chosen: string): string {
  if (cleanup.some(p => p.id === chosen && p.ready)) return chosen;
  return cleanup.find(p => p.ready && p.id !== 'ollama')?.id ?? (chosen || DEFAULT_AI_CLEAN_PROVIDER);
}

/**
 * Whether the cleanup/structuring provider chosen in Settings is connected (address known,
 * key present when required). `null` when unknown (list not loaded or failed): callers then
 * keep the previous behavior instead of blocking.
 */
export async function isCleanProviderReady(): Promise<boolean | null> {
  try {
    const info = await api.listAiProviders();
    const id = effectiveCleanProvider(info.cleanup, getAiCleanProvider());
    const provider = info.cleanup.find(p => p.id === id);
    return provider ? provider.ready : null;
  } catch {
    return null;
  }
}

/** Reactive `isCleanProviderReady`: refreshed when preferences, AI settings or focus change. */
export function useAiReady(): boolean | null {
  const [ready, setReady] = useState<boolean | null>(null);

  useEffect(() => {
    let active = true;
    const refresh = () => {
      void isCleanProviderReady().then(value => { if (active) setReady(value); });
    };
    refresh();
    window.addEventListener(PREFS_CHANGED_EVENT, refresh);
    window.addEventListener(AI_CONFIG_CHANGED_EVENT, refresh);
    window.addEventListener('focus', refresh);
    return () => {
      active = false;
      window.removeEventListener(PREFS_CHANGED_EVENT, refresh);
      window.removeEventListener(AI_CONFIG_CHANGED_EVENT, refresh);
      window.removeEventListener('focus', refresh);
    };
  }, []);

  return ready;
}

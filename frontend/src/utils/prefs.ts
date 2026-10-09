import { useEffect, useState } from 'react';
import i18n from '../i18n';

/**
 * Local preferences persisted in `localStorage`.
 *
 * The application's two windows (main and Quick Capture) share the same origin,
 * so the same preferences. A `humcase:prefs-changed` event is emitted on every write
 * so that already mounted components (editor) react immediately, without a restart.
 */
export const PREFS_CHANGED_EVENT = 'humcase:prefs-changed';

const KEYS = {
  editorFontScale: 'editor_font_scale',
  lastProjectId: 'last_project_id',
  miniMode: 'mini_mode',
  miniSize: 'mini_size',
  aiCleanProvider: 'ai_clean_provider',
  aiSttProvider: 'ai_stt_provider',
  structureFrame: 'structure_frame',
  dictationAutoStructure: 'dictation_auto_structure',
  dictationLanguage: 'dictation_language',
  welcomeHidden: 'welcome_hidden',
  exampleProjectId: 'example_project_id',
} as const;

export const MIN_FONT_SCALE = 0.8;
export const MAX_FONT_SCALE = 1.4;
export const DEFAULT_MINI_SIZE = { width: 400, height: 500 };

/** Default AI providers (identifiers of the Rust registry `providers.rs`). */
export const DEFAULT_AI_CLEAN_PROVIDER = 'cerebras';
export const DEFAULT_AI_STT_PROVIDER = 'deepgram';

function notifyPrefsChanged() {
  window.dispatchEvent(new CustomEvent(PREFS_CHANGED_EVENT));
}

/** Clamps the font size between 80 % and 140 % (invalid values → 100 %). */
export function normalizeFontScale(value: number): number {
  if (!Number.isFinite(value)) return 1;
  return Math.min(MAX_FONT_SCALE, Math.max(MIN_FONT_SCALE, Math.round(value * 100) / 100));
}

export function getEditorFontScale(): number {
  return normalizeFontScale(Number(localStorage.getItem(KEYS.editorFontScale) ?? '1'));
}

export function setEditorFontScale(scale: number): number {
  const value = normalizeFontScale(scale);
  localStorage.setItem(KEYS.editorFontScale, String(value));
  notifyPrefsChanged();
  return value;
}

export function getLastProjectId(): string | null {
  // Hardening: a refused read (storage disabled, private mode) must not make
  // Quick Capture's `setProjectId` updater throw, which runs outside the `try` of `loadProjects`.
  try {
    return localStorage.getItem(KEYS.lastProjectId);
  } catch {
    return null;
  }
}

export function setLastProjectId(id: string | null) {
  // Cosmetic preference: storage that refuses the write (quota, private mode) must not
  // make a Quick Capture effect throw (and thus unmount the tree); at worst we lose the
  // "last used project".
  try {
    if (id) {
      localStorage.setItem(KEYS.lastProjectId, id);
    } else {
      localStorage.removeItem(KEYS.lastProjectId);
    }
  } catch {
    // persistence unavailable: deliberately ignored
  }
}

/** Home's first steps were hidden by the person. A refused storage shows them again: harmless. */
export function getWelcomeHidden(): boolean {
  try { return localStorage.getItem(KEYS.welcomeHidden) === 'true'; } catch { return false; }
}

export function setWelcomeHidden(hidden: boolean) {
  try { localStorage.setItem(KEYS.welcomeHidden, hidden ? 'true' : 'false'); } catch { /* persistence unavailable */ }
}

/** Project the first steps' "filled example" created, so it can be removed (archived) again. */
export function getExampleProjectId(): string | null {
  try { return localStorage.getItem(KEYS.exampleProjectId); } catch { return null; }
}

export function setExampleProjectId(id: string | null) {
  try {
    if (id) localStorage.setItem(KEYS.exampleProjectId, id);
    else localStorage.removeItem(KEYS.exampleProjectId);
  } catch { /* persistence unavailable */ }
}

export function getMiniMode(): boolean {
  return localStorage.getItem(KEYS.miniMode) === 'true';
}

export function setMiniMode(enabled: boolean) {
  localStorage.setItem(KEYS.miniMode, enabled ? 'true' : 'false');
}

export function getMiniSize(): { width: number; height: number } {
  const raw = localStorage.getItem(KEYS.miniSize);
  if (raw) {
    try {
      const parsed = JSON.parse(raw);
      if (typeof parsed?.width === 'number' && typeof parsed?.height === 'number') {
        return { width: parsed.width, height: parsed.height };
      }
    } catch {
      // corrupted value: fall back to the default size
    }
  }
  return { ...DEFAULT_MINI_SIZE };
}

export function setMiniSize(width: number, height: number) {
  localStorage.setItem(KEYS.miniSize, JSON.stringify({ width: Math.round(width), height: Math.round(height) }));
}

/** **Cleanup** provider (LLM) chosen by the user. */
/** Identifiers of the structuring templates (expected by the Rust command `structure_note`). */
const TEMPLATE_IDS = ['auto', 'reflection', 'dev', 'marketing', 'research'] as const;
export type StructureFrame = typeof TEMPLATE_IDS[number];

/** Label of a template in the current language (resolved on every read, never frozen at import). */
export function templateLabel(id: StructureFrame): string {
  switch (id) {
    case 'auto': return i18n.t('settings.frames.auto');
    case 'reflection': return i18n.t('settings.frames.reflection');
    case 'dev': return i18n.t('settings.frames.dev');
    case 'marketing': return i18n.t('settings.frames.marketing');
    case 'research': return i18n.t('settings.frames.research');
  }
}

/** Structuring templates; `label` follows the current language on every read. */
export const STRUCTURE_FRAMES: ReadonlyArray<{ readonly id: StructureFrame; readonly label: string }> =
  TEMPLATE_IDS.map(id => ({ id, get label() { return templateLabel(id); } }));

export function getStructureFrame(): StructureFrame {
  const stored = localStorage.getItem(KEYS.structureFrame);
  return STRUCTURE_FRAMES.some(frame => frame.id === stored) ? stored as StructureFrame : 'auto';
}

export function setStructureFrame(frame: StructureFrame) {
  localStorage.setItem(KEYS.structureFrame, frame);
}

/** Smart dictation: integrate and structure the note at the end of dictation (enabled by default). */
export function getDictationAutoStructure(): boolean {
  return localStorage.getItem(KEYS.dictationAutoStructure) !== 'false';
}

export function setDictationAutoStructure(active: boolean) {
  localStorage.setItem(KEYS.dictationAutoStructure, active ? 'true' : 'false');
}

/**
 * Languages a dictation can be spoken in, named in their own language. The transcription
 * provider needs the language: a dictation sent with the wrong one comes back garbled or empty.
 */
export const DICTATION_LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'fr', name: 'Français' },
  { code: 'es', name: 'Español' },
  { code: 'de', name: 'Deutsch' },
  { code: 'it', name: 'Italiano' },
  { code: 'pt-BR', name: 'Português (Brasil)' },
] as const;

/** `interface`: the interface language; `multi`: several languages in one dictation. */
export type DictationLanguage = 'interface' | 'multi' | (typeof DICTATION_LANGUAGES)[number]['code'];

const dictationLanguageValues: readonly string[] = ['interface', 'multi', ...DICTATION_LANGUAGES.map(l => l.code)];

/** Saved preference; any unknown value is `interface`. */
export function getDictationLanguage(): DictationLanguage {
  const value = localStorage.getItem(KEYS.dictationLanguage);
  return value && dictationLanguageValues.includes(value) ? value as DictationLanguage : 'interface';
}

export function setDictationLanguage(language: DictationLanguage) {
  if (language === 'interface') localStorage.removeItem(KEYS.dictationLanguage);
  else localStorage.setItem(KEYS.dictationLanguage, language);
  notifyPrefsChanged();
}

/** Language code sent to the transcription: the preference, or the interface language. */
export function dictationLanguageCode(preference: DictationLanguage, interfaceLanguage: string): string {
  return preference === 'interface' ? interfaceLanguage : preference;
}

/** The AI chosen in Settings, or `''` when none is: the engine then uses the first one whose key is set. */
export function getAiCleanProvider(): string {
  return localStorage.getItem(KEYS.aiCleanProvider) || '';
}

/** Whether the person chose a cleanup provider in Settings (otherwise the first ready one is used). */
export function hasAiCleanChoice(): boolean {
  return Boolean(localStorage.getItem(KEYS.aiCleanProvider));
}

export function setAiCleanProvider(id: string) {
  localStorage.setItem(KEYS.aiCleanProvider, id);
  notifyPrefsChanged();
}

/** **Transcription** provider (voice) chosen by the user. */
export function getAiSttProvider(): string {
  return localStorage.getItem(KEYS.aiSttProvider) || DEFAULT_AI_STT_PROVIDER;
}

export function setAiSttProvider(id: string) {
  localStorage.setItem(KEYS.aiSttProvider, id);
  notifyPrefsChanged();
}

/** Model chosen for a cleanup provider (empty = registry default model). */
export function getAiModel(providerId: string): string {
  return localStorage.getItem(`ai_model_${providerId}`) || '';
}

export function setAiModel(providerId: string, model: string) {
  const value = model.trim();
  if (value) {
    localStorage.setItem(`ai_model_${providerId}`, value);
  } else {
    localStorage.removeItem(`ai_model_${providerId}`);
  }
  notifyPrefsChanged();
}

/** Editor font size, reactive to changes made in the Settings modal. */
export function useEditorFontScale(): number {  const [scale, setScale] = useState<number>(() => getEditorFontScale());

  useEffect(() => {
    const refresh = () => setScale(getEditorFontScale());
    window.addEventListener(PREFS_CHANGED_EVENT, refresh);
    return () => window.removeEventListener(PREFS_CHANGED_EVENT, refresh);
  }, []);

  return scale;
}
